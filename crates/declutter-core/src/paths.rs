//! Folder safety rules and path helpers. Owner: core-A. See CONTRACT "Roots" and "Walk".
//! Port of check_root, require_root, is_project, is_app_folder, _leave_alone, last_touched, is_pinned,
//! unique_path and the constants (PROTECTED_DIRS, PROJECT_MARKERS, PARTIAL_EXTS, DEAD_EXTS, ...) in core.py.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf, Prefix};

use crate::ctx::Ctx;
use crate::model::FolderCheck;
use crate::walk::Entry;

/// Windows attribute bits used by the rules.
pub const ATTR_HIDDEN: u32 = 0x2;
/// OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS: an online-only cloud placeholder.
pub const ATTR_CLOUD_ONLY: u32 = 0x1000 | 0x40000 | 0x400000;

/// Folder names (lower case) that are never entered, and refused anywhere in a chosen root.
pub const PROTECTED_DIRS: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "$recycle.bin",
    "system volume information",
    ".git",
    "node_modules",
    "appdata",
];
/// Names (lower case) that make the folder holding them a code project; any `*.sln` counts too.
pub const PROJECT_MARKERS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    ".venv",
    "venv",
    "package.json",
    "pyproject.toml",
    "cargo.toml",
];
/// Downloads in progress.
pub const PARTIAL_EXTS: &[&str] = &[
    ".crdownload",
    ".part",
    ".tmp",
    ".download",
    ".opdownload",
    ".!qb",
];
/// Partial downloads that count as dead (not active) once untouched for `DEAD_SECS`.
pub const DEAD_EXTS: &[&str] = &[".crdownload", ".part"];
pub const DEAD_SECS: f64 = 3.0 * 86400.0;
/// Shortcuts and config files: never moved or listed as copies.
pub const SKIP_SORT_EXTS: &[&str] = &[".lnk", ".url", ".ini"];
/// Files changed this recently are left alone.
pub const RECENT_SECS: f64 = 5.0 * 60.0;
/// Explorer's own files; a folder holding only these is empty.
pub const FOLDER_JUNK: &[&str] = &["desktop.ini", "thumbs.db"];

const PROJECT_REASON: &str =
    "This is a project folder (it has .git, package.json, pyproject.toml, a .sln file or similar).";

/// Refuses drive roots, the home folder and its parents, protected names, project folders. User-visible reasons
/// are part of the contract (fixtures `check-root-refusals`).
pub fn check_root(ctx: &Ctx, path: &Path) -> FolderCheck {
    match require_root(ctx, path) {
        Ok(_) => FolderCheck {
            ok: true,
            reason: String::new(),
        },
        Err(reason) => FolderCheck { ok: false, reason },
    }
}

/// The resolved root, or the plain-language reason as the error.
pub fn require_root(ctx: &Ctx, path: &Path) -> Result<PathBuf, String> {
    // Python's resolve() is lenient about missing paths; either way a missing path is "Not a folder."
    let p = resolve(path).ok_or("Not a folder.")?;
    if !p.is_dir() {
        return Err("Not a folder.".into());
    }
    if p.parent().is_none() {
        return Err("Drive roots are not allowed. Pick a specific folder.".into());
    }
    let home = ctx.platform.home();
    let home = resolve(&home).unwrap_or(home);
    let (pl, hl) = (lower(&p), lower(&home));
    if pl == hl {
        return Err("Pick a subfolder (e.g. Downloads), not your whole home folder.".into());
    }
    if hl.starts_with(&format!("{}\\", pl.trim_end_matches('\\'))) {
        return Err("That folder contains your whole profile. Pick a subfolder.".into());
    }
    if has_protected_part(&p) {
        return Err("System or protected folder.".into());
    }
    let names: HashSet<String> = std::fs::read_dir(&p)
        .and_then(|rd| {
            rd.map(|e| e.map(|e| e.file_name().to_string_lossy().to_lowercase()))
                .collect()
        })
        .map_err(|e| e.to_string())?;
    if is_project(&names) {
        return Err(PROJECT_REASON.into());
    }
    Ok(p)
}

/// The real, absolute path with links resolved, without the `\\?\` prefix `canonicalize` adds (paths shown to the
/// user and written to the undo log must look like v0.2.2's).
pub fn resolve(path: &Path) -> Option<PathBuf> {
    let c = std::fs::canonicalize(path).ok()?;
    let mut parts = c.components();
    let plain: PathBuf = match parts.next()? {
        Component::Prefix(p) => match p.kind() {
            Prefix::VerbatimDisk(l) => format!("{}:", l as char).into(),
            Prefix::VerbatimUNC(server, share) => {
                let mut s = OsString::from(r"\\");
                s.push(server);
                s.push(r"\");
                s.push(share);
                s.into()
            }
            _ => return Some(c),
        },
        _ => return Some(c),
    };
    let mut out = plain;
    out.push(r"\");
    for part in parts.filter(|c| matches!(c, Component::Normal(_))) {
        out.push(part);
    }
    Some(out)
}

fn lower(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

/// Any folder name (past the drive) in the protected list, compared case-insensitively.
pub fn has_protected_part(p: &Path) -> bool {
    p.components().any(|c| match c {
        Component::Normal(n) => {
            PROTECTED_DIRS.contains(&n.to_string_lossy().to_lowercase().as_str())
        }
        _ => false,
    })
}

/// True if a folder holding these (lower-cased) names is a code project.
pub fn is_project(lower_names: &HashSet<String>) -> bool {
    lower_names
        .iter()
        .any(|n| PROJECT_MARKERS.contains(&n.as_str()) || n.ends_with(".sln"))
}

/// True if a folder looks like a portable program: an .exe next to a .dll.
pub fn is_app_folder(lower_names: &HashSet<String>) -> bool {
    lower_names.iter().any(|n| n.ends_with(".exe"))
        && lower_names.iter().any(|n| n.ends_with(".dll"))
}

/// Lower-cased extension with its dot, as Python's `os.path.splitext` sees it: `"a.TXT"` -> `".txt"`,
/// `".bashrc"` -> `""`.
pub fn ext_lower(name: &str) -> String {
    let trimmed = name.trim_start_matches('.');
    trimmed
        .rfind('.')
        .map_or_else(String::new, |i| trimmed[i..].to_lowercase())
}

/// Active download, changed in the last 5 minutes, or an online-only placeholder.
pub fn leave_alone(name: &str, mtime: f64, attrs: u32, now: f64) -> bool {
    let ext = ext_lower(name);
    let ext = ext.as_str();
    let active =
        PARTIAL_EXTS.contains(&ext) && !(DEAD_EXTS.contains(&ext) && now - mtime > DEAD_SECS);
    active || now - mtime < RECENT_SECS || attrs & ATTR_CLOUD_ONLY != 0
}

/// Later of modified and created time.
pub fn last_touched(entry: &Entry) -> f64 {
    entry.mtime.max(entry.created)
}

/// Case-insensitive absolute form used to compare pins (Python's `normcase(abspath(p))`).
pub fn pin_key(p: &Path) -> String {
    lower(&std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()))
}

/// True if `path` is a pinned ("never touch") file or folder, or lies inside one. Case-insensitive.
pub fn is_pinned(path: &Path, pins: &[PathBuf]) -> bool {
    let p = pin_key(path);
    pins.iter().any(|k| {
        let k = pin_key(k);
        p == k || p.starts_with(&format!("{}\\", k.trim_end_matches(['\\', '/'])))
    })
}

/// True if `p` is `root` or lies inside it. Case-insensitive, like Python's `relative_to` on Windows paths.
pub fn is_inside(p: &Path, root: &Path) -> bool {
    let p = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let mut pc = p.components();
    root.components().all(|r| {
        pc.next()
            .is_some_and(|c| c.as_os_str().eq_ignore_ascii_case(r.as_os_str()))
    })
}

/// True if an existing folder from `root` down to `dir` is a link, so a path that reads as inside `root` could lead
/// outside it (e.g. a `Documents` junction on the Desktop). `is_symlink` covers symlinks and junctions, as in the
/// walk; the bare reparse-point attribute is not used because OneDrive folders carry it too.
pub fn through_link(dir: &Path, root: &Path) -> bool {
    dir.ancestors()
        .take_while(|a| is_inside(a, root))
        .any(|a| std::fs::symlink_metadata(a).is_ok_and(|m| m.file_type().is_symlink()))
}

/// First free name: `a.txt`, `a (1).txt`, ... `taken` holds lower-cased paths planned in the same batch.
pub fn unique_path(dst: &Path, taken: &HashSet<String>) -> PathBuf {
    // Anything already there counts, even a broken link, so a move can never land on it.
    let free = |p: &Path| std::fs::symlink_metadata(p).is_err() && !taken.contains(&lower(p));
    let stem = dst.file_stem().unwrap_or_default();
    let mut cand = dst.to_path_buf();
    let mut i = 0u64;
    while !free(&cand) {
        i += 1;
        let mut name = stem.to_os_string();
        name.push(format!(" ({i})"));
        if let Some(ext) = dst.extension() {
            name.push(".");
            name.push(ext);
        }
        cand = dst.with_file_name(name);
    }
    cand
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn project_and_app_folders() {
        assert!(is_project(&set(&["readme.md", "cargo.toml"])));
        assert!(is_project(&set(&["my app.sln"])));
        assert!(!is_project(&set(&["readme.md", "git"])));
        assert!(is_app_folder(&set(&["tool.exe", "lib.dll"])));
        assert!(!is_app_folder(&set(&["tool.exe", "readme.txt"])));
    }

    #[test]
    fn extensions_follow_splitext() {
        assert_eq!(ext_lower("a.TXT"), ".txt");
        assert_eq!(ext_lower("a.tar.GZ"), ".gz");
        assert_eq!(ext_lower(".bashrc"), "");
        assert_eq!(ext_lower("noext"), "");
        assert_eq!(ext_lower("z.!qB"), ".!qb");
    }

    #[test]
    fn leave_alone_rules() {
        let now = 1_000_000.0;
        assert!(leave_alone("a.txt", now - 60.0, 0, now));
        assert!(!leave_alone("a.txt", now - 400.0, 0, now));
        assert!(leave_alone("a.tmp", now - 86400.0 * 10.0, 0, now));
        assert!(leave_alone("a.part", now - 86400.0 * 2.0, 0, now));
        assert!(!leave_alone("a.PART", now - 86400.0 * 4.0, 0, now));
        assert!(leave_alone("a.docx", now - 86400.0, 0x400000, now));
    }

    #[test]
    fn pins_match_case_insensitively_and_inside() {
        let pins = [
            PathBuf::from(r"C:\Data\Keep"),
            PathBuf::from(r"C:\Data\file.txt"),
        ];
        assert!(is_pinned(Path::new(r"c:\data\keep"), &pins));
        assert!(is_pinned(Path::new(r"C:\DATA\Keep\sub\x.txt"), &pins));
        assert!(is_pinned(Path::new(r"C:\Data\FILE.TXT"), &pins));
        assert!(!is_pinned(Path::new(r"C:\Data\Keeper\x.txt"), &pins));
        assert!(!is_pinned(Path::new(r"C:\Data\x.txt"), &[]));
    }

    #[test]
    fn inside_is_case_insensitive_and_includes_root() {
        let root = Path::new(r"C:\Data\Work");
        assert!(is_inside(Path::new(r"c:\data\work\a.txt"), root));
        assert!(is_inside(root, root));
        assert!(!is_inside(Path::new(r"C:\Data\Workshop\a.txt"), root));
        assert!(!is_inside(Path::new(r"C:\Data\Work\..\a.txt"), root));
    }

    #[test]
    fn protected_parts_skip_the_drive() {
        assert!(has_protected_part(Path::new(r"C:\Users\x\AppData\Local")));
        assert!(has_protected_part(Path::new(r"D:\Program Files (x86)\x")));
        assert!(!has_protected_part(Path::new(r"D:\Projects\windows-notes")));
    }

    /// A directory junction (no privilege needed), else a symlink; false if the OS allows neither.
    fn link_dir(link: &Path, target: &Path) -> bool {
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|o| o.status.success())
            || std::os::windows::fs::symlink_dir(target, link).is_ok()
    }

    #[test]
    fn links_between_the_root_and_a_folder_are_found() {
        let dir = tempfile::tempdir().unwrap();
        let (root, outside) = (dir.path().join("root"), dir.path().join("outside"));
        std::fs::create_dir_all(root.join("real")).unwrap();
        std::fs::create_dir(&outside).unwrap();
        assert!(!through_link(&root.join("real").join("new"), &root));
        if !link_dir(&root.join("Docs"), &outside) {
            eprintln!("skipped: this system refused to create a junction or a symlink");
            return;
        }
        assert!(through_link(&root.join("Docs"), &root));
        assert!(through_link(&root.join("Docs").join("sub"), &root));
        assert!(!through_link(&root.join("real"), &root));
    }

    #[test]
    fn unique_path_skips_existing_and_planned_names() {
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("a.tar.gz");
        assert_eq!(unique_path(&dst, &HashSet::new()), dst);
        std::fs::write(&dst, b"x").unwrap();
        let taken = set(&[lower(&dir.path().join("a.tar (1).gz")).as_str()]);
        assert_eq!(unique_path(&dst, &taken), dir.path().join("a.tar (2).gz"));
        let noext = dir.path().join("noext");
        std::fs::write(&noext, b"x").unwrap();
        assert_eq!(
            unique_path(&noext, &HashSet::new()),
            dir.path().join("noext (1)")
        );
    }
}
