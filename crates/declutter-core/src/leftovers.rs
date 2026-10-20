//! Likely leftovers. Owner: core-B. See CONTRACT "Leftovers". Port of find_leftovers / _is_extracted.
//! Never "safe", never preselected. The Downloads folder comes from `Platform::known_folder`.

use crate::ctx::Ctx;
use crate::model::Leftover;
use crate::walk::Entry;

/// Reasons (exact strings): "Unpacked .zip", "Old installer", "Unfinished download". Size descending.
pub fn find_leftovers(ctx: &Ctx, entries: &[Entry]) -> Vec<Leftover> { todo!("core-B") }
