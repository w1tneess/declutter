//! Possible re-downloads. Owner: core-B. See CONTRACT "Re-downloads". Port of find_redownloads / trash_redownloads.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::ctx::Ctx;
use crate::dupes::{ext, file_ref, name, path_key, SKIP_EXTS};
use crate::model::{ActionResult, Redownload};
use crate::trash::to_recycle_bin;
use crate::walk::Entry;

/// `name (N)` or `name - Copy[ (N)]`; the lazy base makes `a (1) (2)` a copy of `a (1)`, as Python's fullmatch does.
static COPY_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(.+?)(?: \(\d+\)| - Copy(?: \(\d+\))?)$").expect("constant pattern")
});

fn stem(p: &Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// (folder, extension, stem), case-insensitive.
fn sibling_key(p: &Path, stem: &str) -> (String, String, String) {
    (
        p.parent().map(path_key).unwrap_or_default(),
        ext(p),
        stem.to_lowercase(),
    )
}

/// `exclude` holds paths already listed as identical-content duplicates. Sorted by copy path (Windows path string).
pub fn find_redownloads(entries: &[Entry], exclude: &[PathBuf]) -> Vec<Redownload> {
    let skip: HashSet<String> = exclude.iter().map(|p| path_key(p)).collect();
    let by_name: HashMap<_, &Entry> = entries
        .iter()
        .map(|e| (sibling_key(&e.path, &stem(&e.path)), e))
        .collect();
    let mut out = Vec::new();
    for e in entries {
        let stem = stem(&e.path);
        let Some(base) = COPY_NAME.captures(&stem).and_then(|c| c.get(1)) else {
            continue;
        };
        if e.size == 0
            || skip.contains(&path_key(&e.path))
            || SKIP_EXTS.contains(&ext(&e.path).as_str())
        {
            continue;
        }
        let Some(orig) = by_name.get(&sibling_key(&e.path, base.as_str())) else {
            continue;
        };
        if orig.size > 0 && (e.size.abs_diff(orig.size) as f64) <= 0.02 * orig.size as f64 {
            out.push(Redownload {
                original: file_ref(orig),
                copy: file_ref(e),
            });
        }
    }
    out.sort_by(|a, b| a.copy.path.cmp(&b.copy.path));
    out
}

/// Recycle a copy only if its original exists now and is not itself being removed (`removing`).
pub fn trash_redownloads(
    ctx: &Ctx,
    pairs: &[Redownload],
    root: &Path,
    removing: &[PathBuf],
) -> ActionResult {
    let gone: HashSet<String> = pairs
        .iter()
        .map(|r| path_key(Path::new(&r.copy.path)))
        .chain(removing.iter().map(|p| path_key(p)))
        .collect();
    let mut out = ActionResult {
        count: 0,
        errors: Vec::new(),
    };
    for r in pairs {
        let (orig, copy) = (Path::new(&r.original.path), PathBuf::from(&r.copy.path));
        // At least one copy stays: the original must be a plain file right now and not be sent away by this job.
        if gone.contains(&path_key(orig)) || !fs::symlink_metadata(orig).is_ok_and(|m| m.is_file())
        {
            out.errors.push(format!(
                "{}: its original {} is not staying, skipped",
                name(&copy),
                name(orig)
            ));
            continue;
        }
        let res = to_recycle_bin(ctx, &[copy], Some(root));
        out.count += res.count;
        out.errors.extend(res.errors);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dupes::tests::entries;

    #[test]
    fn exclude_matches_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        let es = entries(
            dir.path(),
            &[
                ("a.bin", "xxxx", 1.0),
                ("a (1).bin", "xxxx", 1.0),
                ("b - copy.bin", "xxxx", 1.0),
                ("B.BIN", "xxxx", 1.0),
            ],
        );
        let got = find_redownloads(&es, &[dir.path().join("A (1).BIN")]);
        assert_eq!(got.len(), 1);
        assert_eq!(name(Path::new(&got[0].copy.path)), "b - copy.bin");
        assert_eq!(name(Path::new(&got[0].original.path)), "B.BIN");
    }
}
