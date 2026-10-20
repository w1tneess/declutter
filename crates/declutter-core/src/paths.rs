//! Folder safety rules and path helpers. Owner: core-A. See CONTRACT "Roots" and "Walk".
//! Port of check_root, require_root, is_project, is_app_folder, _leave_alone, last_touched, is_pinned,
//! unique_path and the constants (PROTECTED_DIRS, PROJECT_MARKERS, PARTIAL_EXTS, DEAD_EXTS, ...) in core.py.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::ctx::Ctx;
use crate::model::FolderCheck;
use crate::walk::Entry;

/// Windows attribute bits used by the rules.
pub const ATTR_HIDDEN: u32 = 0x2;
/// OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS: an online-only cloud placeholder.
pub const ATTR_CLOUD_ONLY: u32 = 0x1000 | 0x40000 | 0x400000;

/// Refuses drive roots, the home folder and its parents, protected names, project folders. User-visible reasons
/// are part of the contract (fixtures `check-root-refusals`).
pub fn check_root(ctx: &Ctx, path: &Path) -> FolderCheck { todo!("core-A") }

/// The resolved root, or the plain-language reason as the error.
pub fn require_root(ctx: &Ctx, path: &Path) -> Result<PathBuf, String> { todo!("core-A") }

/// True if a folder holding these (lower-cased) names is a code project.
pub fn is_project(lower_names: &HashSet<String>) -> bool { todo!("core-A") }

/// True if a folder looks like a portable program: an .exe next to a .dll.
pub fn is_app_folder(lower_names: &HashSet<String>) -> bool { todo!("core-A") }

/// Active download, changed in the last 5 minutes, or an online-only placeholder.
pub fn leave_alone(name: &str, mtime: f64, attrs: u32, now: f64) -> bool { todo!("core-A") }

/// Later of modified and created time.
pub fn last_touched(entry: &Entry) -> f64 { todo!("core-A") }

/// True if `path` is a pinned ("never touch") file or folder, or lies inside one. Case-insensitive.
pub fn is_pinned(path: &Path, pins: &[PathBuf]) -> bool { todo!("core-A") }

/// First free name: `a.txt`, `a (1).txt`, ... `taken` holds lower-cased paths planned in the same batch.
pub fn unique_path(dst: &Path, taken: &HashSet<String>) -> PathBuf { todo!("core-A") }
