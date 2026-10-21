//! Sort by type. Owner: core-A. See CONTRACT "Sort". Port of plan_sort / apply_moves and the CATEGORIES tables.

use std::collections::HashSet;
use std::os::windows::fs::MetadataExt;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::ctx::Ctx;
use crate::model::{ActionResult, Move, SortPlan};
use crate::paths::{
    ext_lower, is_inside, last_touched, leave_alone, require_root, through_link, unique_path,
    DEAD_EXTS, SKIP_SORT_EXTS,
};
use crate::platform::{filetime_to_unix, move_no_replace};
use crate::undo;
use crate::walk::Entry;

/// Decides where a loose top-level file goes: path components relative to the chosen folder (e.g.
/// `["Images", "Screenshots"]`), or None to leave the file where it is. Sort presets (core-C) supply their own.
pub type Classifier<'a> = &'a dyn Fn(&Entry) -> Option<Vec<String>>;

const CATEGORIES: &[(&str, &[&str])] = &[
    (
        "Images",
        &[
            ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp", ".svg", ".heic", ".tiff", ".ico",
        ],
    ),
    (
        "Documents",
        &[
            ".pdf", ".doc", ".docx", ".txt", ".rtf", ".odt", ".md", ".ppt", ".pptx", ".xls",
            ".xlsx", ".csv",
        ],
    ),
    (
        "Videos",
        &[".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm"],
    ),
    ("Audio", &[".mp3", ".wav", ".flac", ".aac", ".ogg", ".m4a"]),
    ("Archives", &[".zip", ".rar", ".7z", ".tar", ".gz", ".bz2"]),
    (
        "Installers",
        &[".exe", ".msi", ".msix", ".apk", ".dmg", ".iso"],
    ),
    (
        "Code",
        &[
            ".py", ".js", ".ts", ".html", ".css", ".json", ".java", ".c", ".cpp", ".cs", ".ipynb",
            ".sql",
        ],
    ),
];

/// One level under Images, matched case-insensitively at the start of the file name without its extension.
static SUB_BUCKETS: LazyLock<[(&str, Regex); 2]> = LazyLock::new(|| {
    // The patterns are constants, so building them cannot fail at run time.
    [
        (
            "Screenshots",
            Regex::new(r"(?i)^(?:screenshot|snip|capture)").unwrap(),
        ),
        (
            "Chat",
            Regex::new(r"(?i)^(?:IMG-\d{8}-WA\d+|WhatsApp Image)").unwrap(),
        ),
    ]
});

/// Shortcuts, config files and dead downloads are never moved, whatever the classifier says.
fn stays(ext: &str) -> bool {
    SKIP_SORT_EXTS.contains(&ext) || DEAD_EXTS.contains(&ext)
}

/// The built-in classifier: category by extension, Images sub-buckets, `.lnk .url .ini` and dead downloads stay.
pub fn builtin_classifier(entry: &Entry) -> Option<Vec<String>> {
    let name = entry.path.file_name()?.to_string_lossy();
    let ext = ext_lower(&name);
    if stays(&ext) {
        return None;
    }
    let cat = CATEGORIES
        .iter()
        .find(|(_, exts)| exts.contains(&ext.as_str()))
        .map_or("Other", |(c, _)| *c);
    let mut parts = vec![cat.to_string()];
    if cat == "Images" {
        let stem = entry.path.file_stem().unwrap_or_default().to_string_lossy();
        if let Some((bucket, _)) = SUB_BUCKETS.iter().find(|(_, rx)| rx.is_match(&stem)) {
            parts.push(bucket.to_string());
        }
    }
    Some(parts)
}

/// Loose files directly in `root` only, planned with the built-in categories.
pub fn plan_sort(
    ctx: &Ctx,
    root: &Path,
    entries: &[Entry],
    recent_hours: u32,
) -> Result<SortPlan, String> {
    plan_sort_with(ctx, root, entries, recent_hours, &builtin_classifier)
}

/// Same, with a custom classifier (presets). Collision, recency and ordering rules are identical.
pub fn plan_sort_with(
    ctx: &Ctx,
    root: &Path,
    entries: &[Entry],
    recent_hours: u32,
    classify: Classifier,
) -> Result<SortPlan, String> {
    let root = require_root(ctx, root)?;
    let cutoff = ctx.now() - f64::from(recent_hours) * 3600.0;
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    // v0.2.2 sorts by the absolute path string; OsStr order is the same code-point order.
    sorted.sort_by(|a, b| a.path.as_os_str().cmp(b.path.as_os_str()));
    let mut plan = SortPlan {
        moves: Vec::new(),
        recent_left: 0,
    };
    let mut taken = HashSet::new(); // destinations already planned in this batch, lower-cased
    for e in sorted {
        let (Some(parent), Some(name)) = (e.path.parent(), e.path.file_name()) else {
            continue;
        };
        if !parent.as_os_str().eq_ignore_ascii_case(root.as_os_str())
            || stays(&ext_lower(&name.to_string_lossy()))
        {
            continue;
        }
        let Some(parts) = classify(e) else { continue };
        if recent_hours > 0 && last_touched(e) > cutoff {
            plan.recent_left += 1;
            continue;
        }
        let mut dst = root.clone();
        dst.extend(&parts);
        dst.push(name);
        let dst = unique_path(&dst, &taken);
        taken.insert(dst.to_string_lossy().to_lowercase());
        plan.moves.push(Move {
            src: dto_path(&e.path),
            dst: dto_path(&dst),
            size: e.size,
            category: parts.join("/"),
        });
    }
    Ok(plan)
}

/// Re-checks every file right before moving it, never overwrites, logs only what moved (see `undo`).
pub fn apply_moves(
    ctx: &Ctx,
    moves: &[Move],
    root: Option<&Path>,
    recent_hours: u32,
) -> ActionResult {
    let base = match root.map(|r| require_root(ctx, r)).transpose() {
        Ok(b) => b,
        Err(reason) => {
            return ActionResult {
                count: 0,
                errors: vec![reason],
            }
        }
    };
    let mut done = Vec::new();
    let mut errors = Vec::new();
    for m in moves {
        let (src, dst) = (Path::new(&m.src), Path::new(&m.dst));
        let name = src.file_name().unwrap_or_default().to_string_lossy();
        if let Some(b) = &base {
            if !(is_inside(src, b) && is_inside(dst, b)) {
                errors.push(format!("{name}: outside the chosen folder, skipped"));
                continue;
            }
            if [src, dst]
                .iter()
                .any(|p| p.parent().is_some_and(|d| through_link(d, b)))
            {
                errors.push(format!("{name}: inside a linked folder, skipped"));
                continue;
            }
        }
        // The scan may be stale: re-check it is still a plain file that may be moved.
        let md = match std::fs::symlink_metadata(src) {
            Ok(md) => md,
            Err(e) => {
                errors.push(format!("{name}: {e}"));
                continue;
            }
        };
        if !md.is_file() {
            errors.push(format!("{name}: is no longer a regular file, skipped"));
            continue;
        }
        let now = ctx.now();
        let mtime = filetime_to_unix(md.last_write_time());
        if leave_alone(&name, mtime, md.file_attributes(), now) {
            errors.push(format!(
                "{name}: still downloading, just changed or online-only, skipped"
            ));
            continue;
        }
        let touched = mtime.max(filetime_to_unix(md.creation_time()));
        if recent_hours > 0 && touched > now - f64::from(recent_hours) * 3600.0 {
            errors.push(format!(
                "{name}: touched in the last {recent_hours} hours, skipped"
            ));
            continue;
        }
        let moved = dst
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                let fin = unique_path(dst, &HashSet::new());
                move_no_replace(src, &fin).map(|()| fin)
            });
        match moved {
            Ok(fin) => done.push(Move {
                dst: dto_path(&fin),
                ..m.clone()
            }),
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    if !done.is_empty() {
        if let Err(e) = undo::append_log(ctx, &done) {
            errors.push(format!("The undo log could not be saved ({e}), so this sort cannot be undone from the app."));
        }
    }
    ActionResult {
        count: done.len() as u32,
        errors,
    }
}

/// Convenience for tests and the shell: `Move` DTOs from paths.
pub fn dto_path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> Entry {
        Entry {
            path: Path::new(r"C:\r").join(name),
            size: 1,
            mtime: 0.0,
            created: 0.0,
            attrs: 0,
        }
    }

    #[test]
    fn builtin_categories_and_buckets() {
        let c = |n: &str| builtin_classifier(&entry(n));
        assert_eq!(c("a.JPG"), Some(vec!["Images".to_string()]));
        assert_eq!(
            c("Screenshot 1.png"),
            Some(vec!["Images".to_string(), "Screenshots".to_string()])
        );
        assert_eq!(
            c("IMG-20240101-WA0001.jpg"),
            Some(vec!["Images".to_string(), "Chat".to_string()])
        );
        assert_eq!(c("IMG-2024-WA1.jpg"), Some(vec!["Images".to_string()]));
        assert_eq!(c("Screenshot.pdf"), Some(vec!["Documents".to_string()]));
        assert_eq!(c("noext"), Some(vec!["Other".to_string()]));
        assert_eq!(c("go.LNK"), None);
        assert_eq!(c("x.crdownload"), None);
    }
}
