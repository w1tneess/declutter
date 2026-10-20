//! Folder-size and file-type breakdown for Large & Old (new in 1.0). Owner: core-C. Computed from the walk's
//! entries in one pass; top 10 folders (by total bytes, direct and nested) and top 10 types (the sort categories).

use std::path::Path;

use crate::model::Breakdown;
use crate::walk::Entry;

pub fn breakdown(root: &Path, entries: &[Entry], top: usize) -> Breakdown { todo!("core-C") }
