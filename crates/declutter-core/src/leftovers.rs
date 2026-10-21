//! Likely leftovers. Owner: core-B. See CONTRACT "Leftovers". Port of find_leftovers / _is_extracted.
//! Never "safe", never preselected. The Downloads folder comes from `Platform::known_folder`.

use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};

use crate::ctx::Ctx;
use crate::dupes::{ext, path_key, DEAD_EXTS};
use crate::model::Leftover;
use crate::platform::KnownFolder;
use crate::walk::Entry;

const DAY: f64 = 86400.0;
const INSTALLER_EXTS: [&str; 3] = ["exe", "msi", "iso"];

/// Reasons (exact strings): "Unpacked .zip", "Old installer", "Unfinished download". Size descending, ties by path.
/// A cancel returns nothing, as in the oracle.
pub fn find_leftovers(ctx: &Ctx, entries: &[Entry]) -> Vec<Leftover> {
    let now = ctx.now();
    // Like the oracle's known_folder: ~/Downloads when the shell cannot say.
    let downloads = ctx
        .platform
        .known_folder(KnownFolder::Downloads)
        .unwrap_or_else(|| ctx.platform.home().join("Downloads"));
    // "Inside Downloads" is a case-insensitive prefix test on the folder plus a separator.
    let dl = format!("{}\\", path_key(&downloads).trim_end_matches('\\'));
    let mut out = Vec::new();
    for e in entries {
        if ctx.cancelled() {
            return Vec::new();
        }
        let (x, age) = (ext(&e.path), now - e.mtime);
        let reason = if DEAD_EXTS.contains(&x.as_str()) && age > 3.0 * DAY {
            "Unfinished download"
        } else if INSTALLER_EXTS.contains(&x.as_str())
            && age > 7.0 * DAY
            && path_key(&e.path).starts_with(&dl)
        {
            "Old installer"
        } else if x == "zip" && is_extracted(&e.path) {
            "Unpacked .zip"
        } else {
            continue;
        };
        out.push(Leftover {
            path: e.path.to_string_lossy().into_owned(),
            size: e.size,
            mtime: e.mtime,
            reason: reason.to_owned(),
        });
    }
    out.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    out
}

/// True if the folder named like the zip, right next to it, holds every file of the archive at its stored size.
/// A name match alone proves nothing, and the zip's own folder is never searched.
fn is_extracted(zip_path: &Path) -> bool {
    let (Some(parent), Some(stem)) = (zip_path.parent(), zip_path.file_stem()) else {
        return false;
    };
    let folder = parent.join(stem);
    let Some(archive) = File::open(zip_path)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
    else {
        return false;
    };
    let mut files = 0;
    for i in 0..archive.len() {
        let Ok(entry) = archive.by_index_data(i) else {
            return false;
        };
        if entry.is_dir() {
            continue;
        }
        let Some(target) = entry.name().ok().and_then(|n| enclosed(&folder, &n)) else {
            return false;
        };
        // A link could point anywhere, so it never counts as the extracted file.
        if !fs::symlink_metadata(&target).is_ok_and(|m| m.is_file() && m.len() == entry.size()) {
            return false;
        }
        files += 1;
    }
    files > 0
}

/// `folder` joined with a zip entry name, or None if the name is absolute, empty, or climbs out of the folder.
fn enclosed(folder: &Path, name: &str) -> Option<PathBuf> {
    let mut out = folder.to_path_buf();
    let mut depth = 0usize;
    for c in Path::new(name).components() {
        match c {
            Component::Normal(part) => {
                out.push(part);
                depth += 1;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                depth = depth.checked_sub(1)?;
                out.pop();
            }
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    (depth > 0).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dupes::tests::{ctx, entries};

    #[test]
    fn entry_names_must_stay_inside_the_folder() {
        let f = Path::new(r"C:\x\good");
        assert_eq!(enclosed(f, "sub/../a.txt"), Some(f.join("a.txt")));
        assert_eq!(enclosed(f, r"sub\b.txt"), Some(f.join("sub").join("b.txt")));
        for bad in [
            "../a.txt",
            "sub/../../a.txt",
            "/abs.txt",
            r"C:\abs.txt",
            r"..\a.txt",
            "",
            ".",
        ] {
            assert_eq!(enclosed(f, bad), None, "{bad}");
        }
    }

    #[test]
    fn old_installers_count_only_inside_downloads() {
        // no known folder in the test platform, so Downloads falls back to <home>\Downloads
        let dir = tempfile::tempdir().unwrap();
        let old = ctx(dir.path()).now() - 8.0 * DAY;
        let es = entries(
            dir.path(),
            &[
                ("Downloads/a.exe", "x", old),
                ("Downloads2/b.exe", "x", old),
            ],
        );
        let got = find_leftovers(&ctx(dir.path()), &es);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].reason, "Old installer");
        assert!(got[0].path.ends_with("a.exe"));
    }
}
