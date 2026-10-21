//! Sort presets (new in 1.0). Owner: core-C. Built-ins "Downloads", "Photos", "Work" plus user-defined ones stored
//! in `<data_dir>/presets.json`. A preset is a `Classifier` for `sort::plan_sort_with`, so preview, collision,
//! recency, top-level-only and undo behaviour are identical to the built-in sort. Validate on save: folder names
//! are relative, at most two levels, no `..`, no reserved Windows names, no protected folder names; extensions
//! are lower case with the dot; an extension may appear in only one rule.

use std::collections::HashSet;
use std::fs;

use crate::ctx::Ctx;
use crate::model::{Preset, PresetRule};
use crate::settings;
use crate::sizes::{dot_ext, CATEGORIES};
use crate::walk::Entry;

const PRESETS_FILE: &str = "presets.json";
const BUILTIN_PREFIX: &str = "builtin-";
const MAX_ID: usize = 64;
const MAX_NAME: usize = 60;
const MAX_RULES: usize = 40;
const MAX_EXTS: usize = 100;
const MAX_EXT_LEN: usize = 16;
const MAX_COMPONENT: usize = 100;
// shortcut: mirrors core.py SKIP_SORT_EXTS + DEAD_EXTS (never moved by the built-in sort); share sort.rs's at integration.
const SKIP_EXTS: [&str; 5] = [".lnk", ".url", ".ini", ".crdownload", ".part"];
// Mirrors core.py PROTECTED_DIRS: the walk never enters these, so files moved there would vanish from view.
const PROTECTED: [&str; 9] = [
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
const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

fn rule(extensions: &[&str], folder: &str) -> PresetRule {
    PresetRule {
        extensions: extensions.iter().map(|e| (*e).to_owned()).collect(),
        folder: folder.to_owned(),
    }
}

fn builtin(id: &str, name: &str, rules: Vec<PresetRule>) -> Preset {
    Preset {
        id: format!("{BUILTIN_PREFIX}{id}"),
        name: name.to_owned(),
        builtin: true,
        rules,
    }
}

/// Unlike the default sort, a preset leaves files no rule mentions where they are.
pub fn builtin_presets() -> Vec<Preset> {
    // Downloads: the default categories without Code, which in Downloads is mostly loose helper scripts.
    let downloads = CATEGORIES
        .iter()
        .filter(|(cat, _)| *cat != "Code")
        .map(|(cat, exts)| rule(exts, cat))
        .collect();
    vec![
        builtin("downloads", "Downloads", downloads),
        builtin(
            "photos",
            "Photos",
            vec![
                rule(
                    &[
                        ".jpg", ".jpeg", ".png", ".heic", ".webp", ".gif", ".bmp", ".tiff",
                    ],
                    "Photos",
                ),
                rule(
                    &[
                        ".cr2", ".cr3", ".nef", ".arw", ".dng", ".orf", ".rw2", ".raf",
                    ],
                    "Photos/RAW",
                ),
                rule(&[".mp4", ".mov", ".m4v", ".avi", ".mkv", ".3gp"], "Videos"),
            ],
        ),
        builtin(
            "work",
            "Work",
            vec![
                rule(
                    &[".doc", ".docx", ".odt", ".rtf", ".txt", ".md", ".pdf"],
                    "Documents",
                ),
                rule(&[".xls", ".xlsx", ".ods", ".csv"], "Spreadsheets"),
                rule(&[".ppt", ".pptx", ".odp"], "Presentations"),
                rule(
                    &[".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp"],
                    "Images",
                ),
                rule(&[".zip", ".7z", ".rar"], "Archives"),
            ],
        ),
    ]
}

fn valid_component(c: &str) -> bool {
    let lower = c.to_lowercase();
    // Windows reserves CON, NUL, ... with any extension too ("con.txt").
    let stem = lower.split('.').next().unwrap_or_default();
    !c.is_empty()
        && c.chars().count() <= MAX_COMPONENT
        && !c.starts_with(['.', ' '])
        && !c.ends_with(['.', ' '])
        && !c
            .chars()
            .any(|ch| ch.is_control() || r#"<>:"/\|?*"#.contains(ch))
        && !RESERVED.contains(&stem)
        && !PROTECTED.contains(&lower.as_str())
}

/// One or two plain folder names below the chosen folder, or None.
fn folder_parts(folder: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = folder.trim().split(['/', '\\']).collect();
    (parts.len() <= 2 && parts.iter().all(|p| valid_component(p)))
        .then(|| parts.into_iter().map(str::to_owned).collect())
}

fn check_ext(e: &str) -> Result<(), String> {
    let plain = e.len() <= MAX_EXT_LEN
        && e.strip_prefix('.').is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        });
    if !plain {
        return Err(format!(
            "\"{e}\" is not a file type like .pdf (lower case, starting with a dot)."
        ));
    }
    if SKIP_EXTS.contains(&e) {
        return Err(format!(
            "{e} files are never moved, so they can't be part of a preset."
        ));
    }
    Ok(())
}

/// The preset as it will be stored (trimmed name, folders spelled `A/B`), or the plain-language problem.
fn validate(p: &Preset) -> Result<Preset, String> {
    let id = p.id.trim();
    let id_ok = id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if id.is_empty() || id.len() > MAX_ID || !id_ok {
        return Err("A preset id may use only a-z, 0-9 and dashes.".into());
    }
    let name = p.name.trim();
    if name.is_empty() {
        return Err("Give the preset a name.".into());
    }
    if name.chars().count() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(format!(
            "Keep the preset name to {MAX_NAME} characters or fewer."
        ));
    }
    if p.rules.is_empty() || p.rules.len() > MAX_RULES {
        return Err(format!("A preset needs between 1 and {MAX_RULES} rules."));
    }
    let mut seen = HashSet::new();
    let mut rules = Vec::with_capacity(p.rules.len());
    for r in &p.rules {
        let Some(parts) = folder_parts(&r.folder) else {
            return Err(format!(
                "\"{}\" can't be used as a folder. Use one or two plain folder names, like Photos/2024.",
                r.folder
            ));
        };
        if r.extensions.is_empty() || r.extensions.len() > MAX_EXTS {
            return Err(format!(
                "Each rule needs between 1 and {MAX_EXTS} file types."
            ));
        }
        for e in &r.extensions {
            check_ext(e)?;
            if !seen.insert(e.as_str()) {
                return Err(format!(
                    "{e} is in more than one rule. Each file type can go to one folder only."
                ));
            }
        }
        rules.push(PresetRule {
            extensions: r.extensions.clone(),
            folder: parts.join("/"),
        });
    }
    Ok(Preset {
        id: id.to_owned(),
        name: name.to_owned(),
        builtin: p.builtin,
        rules,
    })
}

/// User presets on disk. A damaged file or entry is dropped, never an error: sorting must keep working.
fn load_user(ctx: &Ctx) -> Vec<Preset> {
    let all: Vec<Preset> = fs::read(ctx.platform.data_dir().join(PRESETS_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut ids = HashSet::new();
    all.iter()
        .filter_map(|p| validate(p).ok())
        .filter(|p| !p.id.starts_with(BUILTIN_PREFIX) && ids.insert(p.id.clone()))
        .map(|p| Preset {
            builtin: false,
            ..p
        })
        .collect()
}

fn store(ctx: &Ctx, user: Vec<Preset>) -> Result<Vec<Preset>, String> {
    let data = serde_json::to_vec(&user).map_err(|e| format!("Could not save the preset: {e}"))?;
    settings::write_atomic(&ctx.platform.data_dir().join(PRESETS_FILE), &data)
        .map_err(|e| format!("Could not save the preset: {e}"))?;
    Ok(builtin_presets().into_iter().chain(user).collect())
}

pub fn list_presets(ctx: &Ctx) -> Vec<Preset> {
    builtin_presets()
        .into_iter()
        .chain(load_user(ctx))
        .collect()
}

/// Saving under an existing user preset's id replaces it (that is how an edit is saved).
pub fn save_preset(ctx: &Ctx, preset: &Preset) -> Result<Vec<Preset>, String> {
    let p = Preset {
        builtin: false,
        ..validate(preset)?
    };
    if preset.builtin || p.id.starts_with(BUILTIN_PREFIX) {
        return Err(
            "Built-in presets can't be changed. Save a copy with a new name instead.".into(),
        );
    }
    let _guard = settings::lock();
    let mut user = load_user(ctx);
    match user.iter_mut().find(|u| u.id == p.id) {
        Some(existing) => *existing = p,
        None => user.push(p),
    }
    store(ctx, user)
}

/// Deleting a preset that is not there is not an error.
pub fn delete_preset(ctx: &Ctx, id: &str) -> Result<Vec<Preset>, String> {
    if id.starts_with(BUILTIN_PREFIX) {
        return Err("Built-in presets can't be deleted.".into());
    }
    let _guard = settings::lock();
    let mut user = load_user(ctx);
    user.retain(|p| p.id != id);
    store(ctx, user)
}

/// Where a file goes under `preset`; None leaves it in place (same exclusions as the built-in classifier).
pub fn classify(preset: &Preset, entry: &Entry) -> Option<Vec<String>> {
    let ext = dot_ext(&entry.path)?;
    if SKIP_EXTS.contains(&ext.as_str()) {
        return None;
    }
    // The folder is re-checked so a preset that skipped validation can never aim outside the chosen folder.
    preset
        .rules
        .iter()
        .find(|r| r.extensions.contains(&ext))
        .and_then(|r| folder_parts(&r.folder))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::settings::test_support::ctx;

    fn entry(name: &str) -> Entry {
        Entry {
            path: PathBuf::from(r"C:\d").join(name),
            size: 1,
            mtime: 0.0,
            created: 0.0,
            attrs: 0,
        }
    }

    fn user(id: &str, rules: Vec<PresetRule>) -> Preset {
        Preset {
            id: id.into(),
            name: format!("Mine {id}"),
            builtin: false,
            rules,
        }
    }

    fn numbered(i: usize, folder: &str) -> PresetRule {
        PresetRule {
            extensions: vec![format!(".e{i}")],
            folder: folder.into(),
        }
    }

    fn ids(list: &[Preset]) -> Vec<&str> {
        list.iter().map(|p| p.id.as_str()).collect()
    }

    #[test]
    fn builtins_are_valid_and_distinct() {
        let b = builtin_presets();
        assert_eq!(
            b.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["Downloads", "Photos", "Work"]
        );
        for p in &b {
            assert!(p.builtin);
            assert_eq!(validate(p).as_ref(), Ok(p), "{}", p.name);
        }
    }

    #[test]
    fn classify_follows_rules_and_builtin_exclusions() {
        let b = builtin_presets();
        let (downloads, photos) = (&b[0], &b[1]);
        assert_eq!(
            classify(downloads, &entry("Report.PDF")),
            Some(vec!["Documents".to_owned()])
        );
        assert_eq!(
            classify(downloads, &entry("setup.exe")),
            Some(vec!["Installers".to_owned()])
        );
        assert_eq!(
            classify(downloads, &entry("script.py")),
            None,
            "unmatched files stay"
        );
        assert_eq!(
            classify(photos, &entry("IMG_1.CR2")),
            Some(vec!["Photos".to_owned(), "RAW".to_owned()])
        );
        for skipped in [
            "a.lnk",
            "b.url",
            "desktop.ini",
            "c.crdownload",
            "d.part",
            "noext",
            ".hidden",
        ] {
            assert_eq!(classify(downloads, &entry(skipped)), None, "{skipped}");
        }
        let rogue = user("rogue", vec![rule(&[".txt"], "..\\..\\Windows")]);
        assert_eq!(classify(&rogue, &entry("a.txt")), None);
    }

    #[test]
    fn save_list_replace_delete_round_trip() {
        let t = tempfile::tempdir().unwrap();
        let c = ctx(t.path());
        assert_eq!(
            ids(&list_presets(&c)),
            ["builtin-downloads", "builtin-photos", "builtin-work"]
        );
        let mut p = user("music", vec![rule(&[".mp3", ".flac"], " Media \\ Music ")]);
        assert!(
            save_preset(&c, &p).is_err(),
            "inner spaces around a folder name are refused"
        );
        p.rules = vec![rule(&[".mp3", ".flac"], " Media\\Music ")];
        p.name = "  Music  ".into();
        let list = save_preset(&c, &p).unwrap();
        assert_eq!(
            ids(&list),
            [
                "builtin-downloads",
                "builtin-photos",
                "builtin-work",
                "music"
            ]
        );
        assert_eq!(
            (list[3].name.as_str(), list[3].rules[0].folder.as_str()),
            ("Music", "Media/Music")
        );
        p.rules = vec![rule(&[".wav"], "Sounds")];
        save_preset(&c, &p).unwrap();
        let list = list_presets(&c);
        assert_eq!(list.len(), 4, "same id replaces");
        assert_eq!(list[3].rules[0].extensions, [".wav"]);
        assert_eq!(delete_preset(&c, "music").unwrap().len(), 3);
        assert_eq!(
            delete_preset(&c, "music").unwrap().len(),
            3,
            "deleting twice is fine"
        );
        assert!(delete_preset(&c, "builtin-work").is_err());
    }

    #[test]
    fn save_rejects_bad_presets() {
        let t = tempfile::tempdir().unwrap();
        let c = ctx(t.path());
        let ok = rule(&[".txt"], "Text");
        let bad: Vec<Preset> = vec![
            Preset {
                name: "  ".into(),
                ..user("a", vec![ok.clone()])
            },
            Preset {
                name: "x".repeat(61),
                ..user("a", vec![ok.clone()])
            },
            user("", vec![ok.clone()]),
            user("Has Caps", vec![ok.clone()]),
            user("builtin-work", vec![ok.clone()]),
            Preset {
                builtin: true,
                ..user("a", vec![ok.clone()])
            },
            user("a", vec![]),
            user("a", (0..41).map(|i| numbered(i, "F")).collect()),
            user("a", vec![rule(&[], "Text")]),
            user(
                "a",
                vec![PresetRule {
                    extensions: (0..101).map(|i| format!(".e{i}")).collect(),
                    folder: "F".into(),
                }],
            ),
            user("a", vec![rule(&[".PDF"], "Docs")]),
            user("a", vec![rule(&["pdf"], "Docs")]),
            user("a", vec![rule(&[".tar.gz"], "Docs")]),
            user("a", vec![rule(&["."], "Docs")]),
            user("a", vec![rule(&[".lnk"], "Links")]),
            user("a", vec![rule(&[".txt"], "A"), rule(&[".md", ".txt"], "B")]),
        ];
        for p in &bad {
            assert!(save_preset(&c, p).is_err(), "{p:?}");
        }
        for folder in [
            "",
            " ",
            "..",
            ".",
            "a/../b",
            "a/b/c",
            "C:\\x",
            "\\x",
            "a//b",
            "con",
            "LPT1",
            "Con.txt",
            "Windows",
            "Program Files",
            "x/AppData",
            "node_modules",
            ".git",
            ".hidden",
            "trail.",
            "a<b",
            "a?b",
            "a|b",
        ] {
            assert!(
                save_preset(&c, &user("f", vec![rule(&[".txt"], folder)])).is_err(),
                "folder {folder:?}"
            );
        }
        assert_eq!(list_presets(&c).len(), 3, "nothing bad was stored");
        let at_limit = user(
            "max",
            (0..40).map(|i| numbered(i, &format!("F{i}/Sub"))).collect(),
        );
        assert!(save_preset(&c, &at_limit).is_ok());
    }

    #[test]
    fn damaged_presets_file_falls_back_to_builtins() {
        let t = tempfile::tempdir().unwrap();
        let c = ctx(t.path());
        let file = t.path().join("data").join(PRESETS_FILE);
        fs::create_dir_all(t.path().join("data")).unwrap();
        for junk in ["{not json", "{}", "[1, 2]", "null"] {
            fs::write(&file, junk).unwrap();
            assert_eq!(list_presets(&c), builtin_presets(), "{junk}");
        }
        let good = user("good", vec![rule(&[".txt"], "Text")]);
        let evil = user("evil", vec![rule(&[".txt"], "../../Windows")]);
        let clash = Preset {
            builtin: true,
            ..user("builtin-photos", vec![rule(&[".txt"], "T")])
        };
        let dup = user("good", vec![rule(&[".md"], "Other")]);
        fs::write(
            &file,
            serde_json::to_vec(&[&good, &evil, &clash, &dup]).unwrap(),
        )
        .unwrap();
        let list = list_presets(&c);
        assert_eq!(
            ids(&list),
            [
                "builtin-downloads",
                "builtin-photos",
                "builtin-work",
                "good"
            ]
        );
        assert_eq!(list[3], good);
    }
}
