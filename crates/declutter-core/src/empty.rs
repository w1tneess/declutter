//! Empty folders. Owner: core-A. See CONTRACT "Empty folders". Port of find_empty_dirs.
//! Ideally derived from the walk rather than a second pass; behaviour must match the fixtures either way.

use std::path::{Path, PathBuf};

use crate::ctx::Ctx;

/// Folders with no files at any depth, root excluded, deepest (longest path) first.
pub fn find_empty_dirs(ctx: &Ctx, root: &Path, ignore: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    todo!("core-A")
}
