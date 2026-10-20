//! Scan-only reminder (new in 1.0). Owner: core-C. A per-user Windows Task Scheduler task (no admin) that runs
//! `declutter --remind`; that mode scans the saved folder and shows one notification ("212 files waiting"). It must
//! NEVER move, delete or otherwise change a file, and is off by default. Enabling, disabling and status must be
//! idempotent and reversible in one click. Build the `schtasks` argument list in a pure function so it can be unit
//! tested without touching the machine; quote the exe path safely (spaces) and never interpolate user text.

use std::path::Path;

use crate::ctx::Ctx;
use crate::model::ReminderStatus;

pub const TASK_NAME: &str = "Declutter reminder";

/// Arguments for `schtasks /Create ...` (weekly by default). Pure, for tests.
pub fn create_args(exe: &Path, schedule: &str) -> Result<Vec<String>, String> { todo!("core-C") }
pub fn enable(ctx: &Ctx, exe: &Path, schedule: &str) -> Result<ReminderStatus, String> { todo!("core-C") }
pub fn disable(ctx: &Ctx) -> Result<ReminderStatus, String> { todo!("core-C") }
pub fn status(ctx: &Ctx) -> ReminderStatus { todo!("core-C") }

/// The text of the reminder notification for `n` files waiting (0 = say nothing).
pub fn message(files_waiting: u32) -> Option<String> { todo!("core-C") }
