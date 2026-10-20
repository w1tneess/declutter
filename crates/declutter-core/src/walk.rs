//! The single directory walk every analysis reuses. Owner: core-A. See CONTRACT "Walk".
//! Port of walk_files / scan_entries. One pass, stats from the directory listing (no extra syscalls per file).

use std::path::{Path, PathBuf};

use crate::ctx::Ctx;

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
pub fn walk_files(ctx: &Ctx, root: &Path, ignore: &[PathBuf]) -> Scan { todo!("core-A") }

/// `require_root` + `walk_files`.
pub fn scan_entries(ctx: &Ctx, root: &Path, ignore: &[PathBuf]) -> Result<Scan, String> { todo!("core-A") }
