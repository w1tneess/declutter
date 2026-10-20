//! Duplicate finder. Owner: core-B. See CONTRACT "Duplicates". Port of find_duplicates / check_dup_selection /
//! trash_duplicates. size -> first+last 64 KiB -> full SHA-256 (1 MiB chunks), parallel, cancellable in every
//! stage, with the persisted hash cache (`hashcache`). Report `Phase::Comparing` / `Phase::Hashing` progress with
//! `done`, `total`, `bytes` and an ETA so the UI bar is determinate.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::ctx::Ctx;
use crate::model::{ActionResult, DupGroup};
use crate::walk::Entry;

/// Groups sorted by size x (copies - 1) descending; within a group oldest-modified first, ties by path. A cancel
/// returns no groups. Never reads online-only files.
pub fn find_duplicates(ctx: &Ctx, entries: &[Entry], min_size: u64) -> Vec<DupGroup> { todo!("core-B") }

/// `Err` with the user-visible message if any group would lose every copy.
pub fn check_dup_selection(groups: &[DupGroup], selected: &HashSet<String>) -> Result<(), String> {
    todo!("core-B")
}

/// Recycle the chosen copies, but only where an intact kept copy of the same size exists on disk right now.
/// Optionally byte-compares the copy against that keeper before removing it.
pub fn trash_duplicates(ctx: &Ctx, removals: &[(DupGroup, Vec<PathBuf>)], root: &Path) -> ActionResult {
    todo!("core-B")
}
