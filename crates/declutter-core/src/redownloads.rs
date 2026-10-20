//! Possible re-downloads. Owner: core-B. See CONTRACT "Re-downloads". Port of find_redownloads / trash_redownloads.

use std::path::{Path, PathBuf};

use crate::ctx::Ctx;
use crate::model::{ActionResult, Redownload};
use crate::walk::Entry;

/// `exclude` holds paths already listed as identical-content duplicates. Sorted by copy path (Windows path string).
pub fn find_redownloads(entries: &[Entry], exclude: &[PathBuf]) -> Vec<Redownload> { todo!("core-B") }

/// Recycle a copy only if its original exists now and is not itself being removed (`removing`).
pub fn trash_redownloads(ctx: &Ctx, pairs: &[Redownload], root: &Path, removing: &[PathBuf]) -> ActionResult {
    todo!("core-B")
}
