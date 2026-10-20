//! CSV export of any preview (new in 1.0). Owner: core-C. Excel-friendly (UTF-8 BOM, CRLF), formula-injection safe
//! (a cell starting with = + - @ gets a leading apostrophe), no secrets, never writes inside the scanned folder
//! unless the user chose that path.

use std::path::Path;

use crate::model::{DupGroup, FileRef, Leftover, Move, Redownload, SimilarGroup};

pub fn moves_csv(moves: &[Move]) -> String { todo!("core-C") }
pub fn duplicates_csv(groups: &[DupGroup]) -> String { todo!("core-C") }
pub fn large_old_csv(files: &[FileRef]) -> String { todo!("core-C") }
pub fn leftovers_csv(items: &[Leftover]) -> String { todo!("core-C") }
pub fn redownloads_csv(pairs: &[Redownload]) -> String { todo!("core-C") }
pub fn similar_csv(groups: &[SimilarGroup]) -> String { todo!("core-C") }
pub fn empty_dirs_csv(dirs: &[String]) -> String { todo!("core-C") }

pub fn write_csv(path: &Path, csv: &str) -> Result<(), String> { todo!("core-C") }
