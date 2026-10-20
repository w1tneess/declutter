//! Removals. Owner: core-A. See CONTRACT "Removals". Nothing is ever deleted permanently: everything goes to the
//! Recycle Bin through `Platform::recycle`. `dupes::trash_duplicates` and `redownloads::trash_redownloads`
//! (core-B) call `to_recycle_bin`.

use std::path::{Path, PathBuf};

use crate::ctx::Ctx;
use crate::model::ActionResult;

/// Send paths to the Recycle Bin. With `root`, anything outside it (or the root itself) is refused. Refusals are
/// collected, the batch continues; a missing path is silently skipped. Messages are part of the contract.
pub fn to_recycle_bin(ctx: &Ctx, paths: &[PathBuf], root: Option<&Path>) -> ActionResult { todo!("core-A") }

/// Recycle empty folders, re-checking each is still empty (junk files excepted) right before.
pub fn trash_empty_dirs(ctx: &Ctx, paths: &[PathBuf], root: &Path) -> ActionResult { todo!("core-A") }
