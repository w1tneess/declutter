//! Empty folders. Owner: core-A. See CONTRACT "Empty folders". Port of find_empty_dirs.
//! A second, directory-only pass: the walk drops folders it does not enter, which this needs to see.

use std::collections::HashSet;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::ctx::Ctx;
use crate::paths::{
    is_app_folder, is_project, pin_key, require_root, ATTR_HIDDEN, FOLDER_JUNK, PROTECTED_DIRS,
};

/// Deeper than this is treated as content: no real tree gets here, and it bounds the recursion.
const MAX_DEPTH: usize = 1000;

/// Folders with no files at any depth, root excluded, deepest (longest path) first.
pub fn find_empty_dirs(ctx: &Ctx, root: &Path, ignore: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let root = require_root(ctx, root)?;
    let pins: HashSet<String> = ignore.iter().map(|p| pin_key(p)).collect();
    let mut empty = Vec::new();
    visit(ctx, &root, &root, &pins, 0, &mut empty);
    // Python leaves equal-length ties in listing order; break them by path so the output is stable.
    empty.sort_by_cached_key(|p| {
        (
            std::cmp::Reverse(p.to_string_lossy().chars().count()),
            p.clone(),
        )
    });
    Ok(empty)
}

/// True if `dir` holds no files at any depth. Hidden, protected, linked, pinned, project and app folders (and
/// anything unreadable) count as content.
fn visit(
    ctx: &Ctx,
    dir: &Path,
    root: &Path,
    pins: &HashSet<String>,
    depth: usize,
    empty: &mut Vec<PathBuf>,
) -> bool {
    if ctx.cancelled() || depth > MAX_DEPTH {
        return false;
    }
    let Ok(listing) = std::fs::read_dir(dir) else {
        return false;
    };
    let entries: Vec<_> = listing.flatten().collect();
    let names: HashSet<String> = entries
        .iter()
        .map(|e| e.file_name().to_string_lossy().to_lowercase())
        .collect();
    if is_project(&names) || (dir != root && is_app_folder(&names)) {
        return false;
    }
    let mut is_empty = true;
    for e in entries {
        let name = e.file_name();
        let name = name.to_string_lossy();
        let low = name.to_lowercase();
        let plain_dir = match (e.file_type(), e.metadata()) {
            (Ok(ft), Ok(md)) => {
                if md.is_file() && FOLDER_JUNK.contains(&low.as_str()) {
                    continue;
                }
                md.is_dir()
                    && !ft.is_symlink()
                    && !name.starts_with('.')
                    && md.file_attributes() & ATTR_HIDDEN == 0
                    && !PROTECTED_DIRS.contains(&low.as_str())
                    && (pins.is_empty() || !pins.contains(&pin_key(&e.path())))
            }
            _ => false,
        };
        if !plain_dir || !visit(ctx, &e.path(), root, pins, depth + 1, empty) {
            is_empty = false;
        }
    }
    if is_empty && dir != root {
        empty.push(dir.to_path_buf());
    }
    is_empty
}
