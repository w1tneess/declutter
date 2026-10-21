//! The single directory walk every analysis reuses. Owner: core-A. See CONTRACT "Walk".
//! Port of walk_files / scan_entries. One pass, stats from the directory listing (no extra syscalls per file).

use std::collections::HashSet;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::ctx::Ctx;
use crate::model::{Phase, Progress};
use crate::paths::{
    is_app_folder, is_project, leave_alone, pin_key, require_root, ATTR_HIDDEN, PROTECTED_DIRS,
};
use crate::platform::filetime_to_unix;

/// A regular file that survived the walk rules. Times are Unix seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub size: u64,
    pub mtime: f64,
    /// Creation time; `max(mtime, created)` is "last touched".
    pub created: f64,
    /// Windows file attributes.
    pub attrs: u32,
}

#[derive(Debug, Default)]
pub struct Scan {
    pub entries: Vec<Entry>,
    /// Project and portable-app folders that were not entered.
    pub skipped: Vec<PathBuf>,
}

/// Walk `root` (already validated with `paths::require_root`). `ignore` holds pinned files and folders.
/// Honours `ctx.cancelled()` once per directory and reports `Phase::Walking` progress through `ctx`.
pub fn walk_files(ctx: &Ctx, root: &Path, ignore: &[PathBuf]) -> Scan {
    let pins: HashSet<String> = ignore.iter().map(|p| pin_key(p)).collect();
    let now = ctx.now();
    let mut scan = Scan::default();
    let mut stack = vec![root.to_path_buf()];
    let (mut bytes, mut last_report) = (0u64, Instant::now());
    while let Some(here) = stack.pop() {
        if ctx.cancelled() {
            break;
        }
        let Ok(listing) = std::fs::read_dir(&here) else {
            continue;
        };
        let entries: Vec<_> = listing.flatten().collect();
        let names: HashSet<String> = entries
            .iter()
            .map(|e| e.file_name().to_string_lossy().to_lowercase())
            .collect();
        if is_project(&names) || (here != root && is_app_folder(&names)) {
            scan.skipped.push(here);
            continue;
        }
        for e in entries {
            // On Windows both come from the directory listing itself, and neither follows links. `is_symlink`
            // covers symlinks and junctions (the oracle's 0xA000000C and 0xA0000003) and other name surrogates.
            let (Ok(ft), Ok(md)) = (e.file_type(), e.metadata()) else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            let name = e.file_name();
            let name = name.to_string_lossy();
            let attrs = md.file_attributes();
            if name.starts_with('.')
                || attrs & ATTR_HIDDEN != 0
                || (!pins.is_empty() && pins.contains(&pin_key(&e.path())))
            {
                continue;
            }
            if md.is_dir() {
                if !PROTECTED_DIRS.contains(&name.to_lowercase().as_str()) {
                    stack.push(e.path());
                }
            } else if md.is_file() {
                let mtime = filetime_to_unix(md.last_write_time());
                if !leave_alone(&name, mtime, attrs, now) {
                    bytes += md.len();
                    scan.entries.push(Entry {
                        path: e.path(),
                        size: md.len(),
                        mtime,
                        created: filetime_to_unix(md.creation_time()),
                        attrs,
                    });
                }
            }
        }
        if last_report.elapsed() >= Duration::from_millis(50) {
            last_report = Instant::now();
            let n = scan.entries.len() as u64;
            ctx.report(Progress {
                phase: Phase::Walking,
                done: n,
                total: None,
                bytes,
                eta_secs: None,
                label: format!("Looking through {n} files"),
            });
        }
    }
    // The stack order depends on the listing; sorting keeps every analysis' output stable.
    scan.entries
        .sort_by(|a, b| a.path.as_os_str().cmp(b.path.as_os_str()));
    scan.skipped.sort();
    scan
}

/// `require_root` + `walk_files`.
pub fn scan_entries(ctx: &Ctx, root: &Path, ignore: &[PathBuf]) -> Result<Scan, String> {
    let root = require_root(ctx, root)?;
    Ok(walk_files(ctx, &root, ignore))
}
