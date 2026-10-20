//! Sort by type. Owner: core-A. See CONTRACT "Sort". Port of plan_sort / apply_moves and the CATEGORIES tables.

use std::path::Path;

use crate::ctx::Ctx;
use crate::model::{ActionResult, Move, SortPlan};
use crate::walk::Entry;

/// Decides where a loose top-level file goes: path components relative to the chosen folder (e.g.
/// `["Images", "Screenshots"]`), or None to leave the file where it is. Sort presets (core-C) supply their own.
pub type Classifier<'a> = &'a dyn Fn(&Entry) -> Option<Vec<String>>;

/// The built-in classifier: category by extension, Images sub-buckets, `.lnk .url .ini` and dead downloads stay.
pub fn builtin_classifier(entry: &Entry) -> Option<Vec<String>> { todo!("core-A") }

/// Loose files directly in `root` only, planned with the built-in categories.
pub fn plan_sort(ctx: &Ctx, root: &Path, entries: &[Entry], recent_hours: u32) -> Result<SortPlan, String> {
    plan_sort_with(ctx, root, entries, recent_hours, &builtin_classifier)
}

/// Same, with a custom classifier (presets). Collision, recency and ordering rules are identical.
pub fn plan_sort_with(ctx: &Ctx, root: &Path, entries: &[Entry], recent_hours: u32,
                      classify: Classifier) -> Result<SortPlan, String> { todo!("core-A") }

/// Re-checks every file right before moving it, never overwrites, logs only what moved (see `undo`).
pub fn apply_moves(ctx: &Ctx, moves: &[Move], root: Option<&Path>, recent_hours: u32) -> ActionResult {
    todo!("core-A")
}

/// Convenience for tests and the shell: `Move` DTOs from paths.
pub fn dto_path(p: &Path) -> String { p.to_string_lossy().into_owned() }
