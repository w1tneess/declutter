//! "Undo last sort" and its log. Owner: core-A. See CONTRACT "Undo".
//! The log is `<data_dir>/undo.json`, a JSON list of `{"time", "moves": [{"src", "dst"}]}` batches, newest last,
//! capped at 20, written atomically. The schema and location must stay compatible with v0.2.2.

use crate::ctx::Ctx;
use crate::model::{ActionResult, Move, UndoInfo};

/// Append one batch of completed moves (called by `sort::apply_moves`).
pub fn append_log(ctx: &Ctx, done: &[Move]) -> Result<(), String> { todo!("core-A") }

pub fn can_undo(ctx: &Ctx) -> bool { todo!("core-A") }

/// Number of moves in the batch `undo_last` would reverse.
pub fn undo_count(ctx: &Ctx) -> u32 { todo!("core-A") }

pub fn peek_undo(ctx: &Ctx) -> Option<UndoInfo> { todo!("core-A") }

/// Reverse the newest batch. Never trusts the log blindly: absolute, non-protected paths only.
pub fn undo_last(ctx: &Ctx) -> ActionResult { todo!("core-A") }
