//! Large and old files. Owner: core-B. See CONTRACT "Large and old". Port of find_large_old.

use crate::ctx::Ctx;
use crate::model::FileRef;
use crate::walk::Entry;

/// size >= min_mb MiB AND older than `older_days`; 0 disables a filter; dead downloads excluded; size descending.
pub fn find_large_old(ctx: &Ctx, entries: &[Entry], min_mb: f64, older_days: u32) -> Vec<FileRef> {
    todo!("core-B")
}
