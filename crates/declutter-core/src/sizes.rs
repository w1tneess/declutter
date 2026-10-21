//! Folder-size and file-type breakdown for Large & Old (new in 1.0). Owner: core-C. Computed from the walk's
//! entries in one pass; top 10 folders (by total bytes, direct and nested) and top 10 types (the sort categories).

use std::collections::HashMap;
use std::path::Path;

use crate::model::{Breakdown, SizeEntry};
use crate::walk::Entry;

// shortcut: mirrors CATEGORIES in core.py because sort.rs (core-A) has no public table yet; share one at integration.
pub(crate) const CATEGORIES: [(&str, &[&str]); 7] = [
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

/// Lower-cased extension with the dot, as the category tables spell it.
pub(crate) fn dot_ext(path: &Path) -> Option<String> {
    Some(format!(".{}", path.extension()?.to_str()?.to_lowercase()))
}

fn category(path: &Path) -> &'static str {
    dot_ext(path)
        .and_then(|ext| {
            CATEGORIES
                .iter()
                .find(|(_, exts)| exts.contains(&ext.as_str()))
        })
        .map_or("Other", |(cat, _)| cat)
}

fn top_rows(totals: HashMap<String, (u64, u64)>, top: usize) -> Vec<SizeEntry> {
    let mut rows: Vec<SizeEntry> = totals
        .into_iter()
        .map(|(name, (bytes, files))| SizeEntry { name, bytes, files })
        .collect();
    rows.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    rows.truncate(top);
    rows
}

/// A file counts toward every folder between it and `root`; `root` itself is not a row (it would always be first).
/// Entries outside `root` are ignored.
pub fn breakdown(root: &Path, entries: &[Entry], top: usize) -> Breakdown {
    let mut folders: HashMap<&Path, (u64, u64)> = HashMap::new();
    let mut types: HashMap<String, (u64, u64)> = HashMap::new();
    for e in entries
        .iter()
        .filter(|e| e.path.starts_with(root) && e.path != root)
    {
        let t = types.entry(category(&e.path).to_owned()).or_default();
        *t = (t.0.saturating_add(e.size), t.1 + 1);
        for dir in e.path.ancestors().skip(1).take_while(|d| *d != root) {
            let f = folders.entry(dir).or_default();
            *f = (f.0.saturating_add(e.size), f.1 + 1);
        }
    }
    let folders = folders
        .into_iter()
        .map(|(p, v)| (p.to_string_lossy().into_owned(), v))
        .collect();
    Breakdown {
        folders: top_rows(folders, top),
        types: top_rows(types, top),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn e(path: &str, size: u64) -> Entry {
        Entry {
            path: PathBuf::from(path),
            size,
            mtime: 0.0,
            created: 0.0,
            attrs: 0,
        }
    }

    fn row(name: &str, bytes: u64, files: u64) -> SizeEntry {
        SizeEntry {
            name: name.into(),
            bytes,
            files,
        }
    }

    #[test]
    fn folders_accumulate_nested_bytes_and_skip_the_root() {
        let entries = [
            e(r"C:\r\a.JPG", 10),
            e(r"C:\r\x\b.pdf", 20),
            e(r"C:\r\x\y\c.mp4", 30),
            e(r"C:\r\z\d.bin", 5),
            e(r"C:\r\z\noext", 1),
            e(r"C:\other\e.txt", 99),
            e(r"C:\rr\f.txt", 99),
        ];
        let b = breakdown(Path::new(r"C:\r"), &entries, 10);
        assert_eq!(
            b.folders,
            [
                row(r"C:\r\x", 50, 2),
                row(r"C:\r\x\y", 30, 1),
                row(r"C:\r\z", 6, 2)
            ]
        );
        assert_eq!(
            b.types,
            [
                row("Videos", 30, 1),
                row("Documents", 20, 1),
                row("Images", 10, 1),
                row("Other", 6, 2)
            ]
        );
    }

    #[test]
    fn ties_break_by_name_and_top_truncates() {
        let entries = [
            e(r"C:\r\b\1.zip", 7),
            e(r"C:\r\a\2.rar", 7),
            e(r"C:\r\c\3.iso", 9),
        ];
        let b = breakdown(Path::new(r"C:\r"), &entries, 2);
        assert_eq!(b.folders, [row(r"C:\r\c", 9, 1), row(r"C:\r\a", 7, 1)]);
        assert_eq!(b.types, [row("Archives", 14, 2), row("Installers", 9, 1)]);
        assert_eq!(
            breakdown(Path::new(r"C:\r"), &entries, 0),
            Breakdown {
                folders: vec![],
                types: vec![]
            }
        );
        assert_eq!(
            breakdown(Path::new(r"C:\r"), &[], 10),
            Breakdown {
                folders: vec![],
                types: vec![]
            }
        );
    }

    #[test]
    fn sizes_saturate_instead_of_overflowing() {
        let entries = [e(r"C:\r\x\a.mp3", u64::MAX), e(r"C:\r\x\b.mp3", 5)];
        let b = breakdown(Path::new(r"C:\r"), &entries, 10);
        assert_eq!(b.folders, [row(r"C:\r\x", u64::MAX, 2)]);
        assert_eq!(b.types, [row("Audio", u64::MAX, 2)]);
    }

    #[test]
    fn category_tables_do_not_overlap() {
        let mut seen = std::collections::HashSet::new();
        for (_, exts) in CATEGORIES {
            for ext in exts {
                assert!(
                    ext.starts_with('.') && *ext == ext.to_lowercase() && seen.insert(ext),
                    "{ext}"
                );
            }
        }
    }
}
