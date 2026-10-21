//! Removals. Owner: core-A. See CONTRACT "Removals". Nothing is ever deleted permanently: everything goes to the
//! Recycle Bin through `Platform::recycle`. `dupes::trash_duplicates` and `redownloads::trash_redownloads`
//! (core-B) call `to_recycle_bin`.

use std::collections::HashSet;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::ctx::Ctx;
use crate::model::ActionResult;
use crate::paths::{is_inside, is_project, leave_alone, require_root, FOLDER_JUNK};
use crate::platform::{filetime_to_unix, DriveKind};

/// Send paths to the Recycle Bin. With `root`, anything outside it (or the root itself) is refused. Refusals are
/// collected, the batch continues; a missing path is silently skipped. Messages are part of the contract.
pub fn to_recycle_bin(ctx: &Ctx, paths: &[PathBuf], root: Option<&Path>) -> ActionResult {
    let base = match root.map(|r| require_root(ctx, r)).transpose() {
        Ok(b) => b,
        Err(reason) => {
            return ActionResult {
                count: 0,
                errors: vec![reason],
            }
        }
    };
    let (mut count, mut errors) = (0, Vec::new());
    for p in paths {
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        let abs = std::path::absolute(p).unwrap_or_else(|_| p.clone());
        if let Some(b) = &base {
            if !is_inside(&abs, b) || abs.as_os_str().eq_ignore_ascii_case(b.as_os_str()) {
                errors.push(format!("{name}: outside the chosen folder, skipped"));
                continue;
            }
        }
        // Checked right before acting: the scan may be stale. Links (symlinks and junctions) are never followed.
        let Ok(md) = std::fs::symlink_metadata(&abs) else {
            continue;
        };
        if md.file_type().is_symlink() {
            errors.push(format!("{name}: is a link, skipped"));
        } else if ctx.platform.drive_kind(&abs) != DriveKind::Fixed {
            errors.push(format!(
                "{name}: on a removable or network drive, which may have no Recycle Bin, skipped"
            ));
        } else if md.is_file()
            && leave_alone(
                &name,
                filetime_to_unix(md.last_write_time()),
                md.file_attributes(),
                ctx.now(),
            )
        {
            errors.push(format!(
                "{name}: still downloading, just changed or online-only, skipped"
            ));
        } else {
            match ctx.platform.recycle(&abs) {
                Ok(()) => count += 1,
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
    }
    ActionResult { count, errors }
}

/// Recycle empty folders, re-checking each is still empty (junk files excepted) right before.
pub fn trash_empty_dirs(ctx: &Ctx, paths: &[PathBuf], root: &Path) -> ActionResult {
    let (mut count, mut errors) = (0, Vec::new());
    for p in paths {
        if std::fs::symlink_metadata(p).is_err() {
            continue; // a parent was already removed
        }
        if !still_empty(p) {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            errors.push(format!(
                "{name}: now contains files or is a project folder, skipped"
            ));
            continue;
        }
        let r = to_recycle_bin(ctx, std::slice::from_ref(p), Some(root));
        count += r.count;
        errors.extend(r.errors);
    }
    ActionResult { count, errors }
}

/// No file other than desktop.ini / Thumbs.db at any depth, and no project folder. Stricter than v0.2.2's
/// os.walk on purpose: anything unreadable or linked counts as content, so it is never sent away unseen.
fn still_empty(dir: &Path) -> bool {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(listing) = std::fs::read_dir(&d) else {
            return false;
        };
        let mut names = HashSet::new();
        for e in listing {
            let Ok(e) = e else { return false };
            let Ok(ft) = e.file_type() else { return false };
            let low = e.file_name().to_string_lossy().to_lowercase();
            if ft.is_dir() && !ft.is_symlink() {
                stack.push(e.path());
            } else if !(ft.is_file() && FOLDER_JUNK.contains(&low.as_str())) {
                return false;
            }
            names.insert(low);
        }
        if is_project(&names) {
            return false;
        }
    }
    true
}
