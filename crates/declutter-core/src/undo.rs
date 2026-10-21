//! "Undo last sort" and its log. Owner: core-A. See CONTRACT "Undo".
//! The log is `<data_dir>/undo.json`, a JSON list of `{"time", "moves": [{"src", "dst"}]}` batches, newest last,
//! capped at 20, written atomically. The schema and location must stay compatible with v0.2.2.

use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};

use crate::ctx::Ctx;
use crate::model::{ActionResult, Move, UndoInfo};
use crate::paths::{has_protected_part, unique_path};
use crate::platform::{local_time_string, move_no_replace, parse_local_time};
use crate::settings::write_atomic;

const LOG_NAME: &str = "undo.json";
const MAX_BATCHES: usize = 20;

fn log_path(ctx: &Ctx) -> PathBuf {
    ctx.platform.data_dir().join(LOG_NAME)
}

/// Move a data file from its pre-0.2 `%APPDATA%` home once; if that fails it just stays there.
pub fn migrate(old: &Path, new: &Path) {
    if old != new && old.is_file() && !new.exists() {
        if let Some(dir) = new.parent() {
            let _ = std::fs::create_dir_all(dir).and_then(|()| std::fs::rename(old, new));
        }
    }
}

/// The batches, oldest first. A missing, unreadable or non-list log is empty; entries are kept as they are,
/// because the log is a plain file a user (or malware) could edit and every use re-validates it.
fn read_log(ctx: &Ctx) -> Vec<Value> {
    migrate(
        &ctx.platform.legacy_data_dir().join(LOG_NAME),
        &log_path(ctx),
    );
    let text = std::fs::read_to_string(log_path(ctx)).unwrap_or_default();
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Array(batches)) => batches,
        _ => Vec::new(),
    }
}

/// Atomic, so a crash never leaves a half-written log.
fn write_log(ctx: &Ctx, batches: &[Value]) -> std::io::Result<()> {
    write_atomic(
        &log_path(ctx),
        serde_json::to_string_pretty(batches)?.as_bytes(),
    )
}

/// Append one batch of completed moves (called by `sort::apply_moves`).
pub fn append_log(ctx: &Ctx, done: &[Move]) -> Result<(), String> {
    let mut batches = read_log(ctx);
    batches.push(json!({
        "time": local_time_string(ctx.now()).unwrap_or_default(),
        "moves": done.iter().map(|m| json!({"src": m.src, "dst": m.dst})).collect::<Vec<_>>(),
    }));
    let start = batches.len().saturating_sub(MAX_BATCHES);
    write_log(ctx, &batches[start..]).map_err(|e| e.to_string())
}

fn moves_of(batch: Option<&Value>) -> &[Value] {
    batch
        .and_then(|b| b.get("moves"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

pub fn peek_undo(ctx: &Ctx) -> Option<UndoInfo> {
    let batches = read_log(ctx);
    let last = batches.last();
    let moves = moves_of(last);
    let first = Path::new(moves.first()?.get("src")?.as_str()?);
    let age = last
        .and_then(|b| b.get("time"))
        .and_then(Value::as_str)
        .and_then(parse_local_time)
        .map(|t| (ctx.now() - t).max(0.0));
    Some(UndoInfo {
        folder: first
            .parent()
            .unwrap_or(first)
            .to_string_lossy()
            .into_owned(),
        count: moves.len() as u32,
        age_seconds: age,
    })
}

/// Absolute, and no protected folder anywhere past the drive.
fn log_path_ok(p: &Path) -> bool {
    p.is_absolute() && !has_protected_part(p)
}

/// Reverse the newest batch. Never trusts the log blindly: absolute, non-protected paths only.
pub fn undo_last(ctx: &Ctx) -> ActionResult {
    let mut batches = read_log(ctx);
    let Some(batch) = batches.pop() else {
        return ActionResult {
            count: 0,
            errors: vec!["Nothing to undo.".into()],
        };
    };
    let (mut restored, mut errors) = (0, Vec::new());
    for mv in moves_of(Some(&batch)).iter().rev() {
        let (Some(src), Some(dst)) = (
            mv.get("src").and_then(Value::as_str),
            mv.get("dst").and_then(Value::as_str),
        ) else {
            errors.push("Skipped a damaged log entry.".to_string());
            continue;
        };
        let (src, dst) = (Path::new(src), Path::new(dst));
        let name = dst.file_name().unwrap_or_default().to_string_lossy();
        let parent = dst.parent().unwrap_or(dst);
        if !(log_path_ok(src) && log_path_ok(dst)) {
            errors.push(format!("{name}: location not allowed, skipped"));
            continue;
        }
        let is_plain_file = std::fs::symlink_metadata(dst).is_ok_and(|md| md.is_file());
        if !is_plain_file {
            errors.push(format!("{name}: no longer at {}", parent.display()));
            continue;
        }
        let moved = src
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| move_no_replace(dst, &unique_path(src, &Default::default())));
        if let Err(e) = moved {
            errors.push(format!("{name}: {e}"));
            continue;
        }
        restored += 1;
        // Remove the now-empty category folder, and Images too after Images/Screenshots or Images/Chat.
        for d in parent.ancestors().take(bucket_depth(dst)) {
            if std::fs::remove_dir(d).is_err() {
                break;
            }
        }
    }
    if let Err(e) = write_log(ctx, &batches) {
        errors.push(format!("The undo log could not be updated ({e})."));
    }
    ActionResult {
        count: restored,
        errors,
    }
}

/// 2 for `Images/Screenshots/x` and `Images/Chat/x`, else 1 (v0.2.2's `_bucket`).
fn bucket_depth(dst: &Path) -> usize {
    let mut up = dst.components().rev().skip(1).map(Component::as_os_str);
    match (up.next(), up.next()) {
        (Some(sub), Some(images))
            if images == "Images" && (sub == "Screenshots" || sub == "Chat") =>
        {
            2
        }
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::platform::{DriveKind, KnownFolder, Platform};

    struct Fake {
        dir: PathBuf,
        now: f64,
    }

    impl Platform for Fake {
        fn now(&self) -> f64 {
            self.now
        }
        fn home(&self) -> PathBuf {
            self.dir.clone()
        }
        fn known_folder(&self, _: KnownFolder) -> Option<PathBuf> {
            None
        }
        fn drive_kind(&self, _: &Path) -> DriveKind {
            DriveKind::Fixed
        }
        fn recycle(&self, _: &Path) -> std::io::Result<()> {
            Err(std::io::Error::other("not in tests"))
        }
        fn data_dir(&self) -> PathBuf {
            self.dir.join("local")
        }
        fn legacy_data_dir(&self) -> PathBuf {
            self.dir.join("roaming")
        }
    }

    fn ctx(dir: &Path) -> Ctx {
        Ctx::new(Arc::new(Fake {
            dir: dir.to_path_buf(),
            now: 1_760_000_000.0,
        }))
    }

    fn mv(src: &str, dst: &str) -> Move {
        Move {
            src: src.into(),
            dst: dst.into(),
            size: 0,
            category: String::new(),
        }
    }

    #[test]
    fn log_keeps_the_newest_twenty_batches() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ctx(tmp.path());
        for i in 0..25 {
            append_log(
                &c,
                &[mv(
                    &format!(r"C:\r\f{i}.txt"),
                    &format!(r"C:\r\Documents\f{i}.txt"),
                )],
            )
            .unwrap();
        }
        let log = read_log(&c);
        assert_eq!(log.len(), 20);
        assert_eq!(log[0]["moves"][0]["src"], r"C:\r\f5.txt");
        assert_eq!(moves_of(log.last()).len(), 1);
        let info = peek_undo(&c).unwrap();
        assert_eq!(info.folder, r"C:\r");
        assert_eq!(info.age_seconds, Some(0.0));
    }

    #[test]
    fn legacy_log_is_migrated_once() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ctx(tmp.path());
        std::fs::create_dir_all(tmp.path().join("roaming")).unwrap();
        let old = r#"[{"time": "2025-01-01 10:00:00", "moves": [{"src": "C:\\r\\a.txt", "dst": "C:\\r\\Documents\\a.txt"}]}]"#;
        std::fs::write(tmp.path().join("roaming").join(LOG_NAME), old).unwrap();
        assert_eq!(read_log(&c).len(), 1);
        assert!(!tmp.path().join("roaming").join(LOG_NAME).exists());
        assert!(tmp.path().join("local").join(LOG_NAME).exists());
    }

    #[test]
    fn damaged_logs_never_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ctx(tmp.path());
        let local = tmp.path().join("local");
        std::fs::create_dir_all(&local).unwrap();
        for bad in [
            "",
            "not json",
            "{}",
            "[1]",
            r#"[{"moves": 5}]"#,
            r#"[{"time": 3, "moves": [{"src": 1}]}]"#,
        ] {
            std::fs::write(local.join(LOG_NAME), bad).unwrap();
            assert!(peek_undo(&c).is_none(), "{bad}");
            let _ = undo_last(&c);
        }
        std::fs::write(local.join(LOG_NAME), r#"[{"moves": ["x", {"src": "a"}]}]"#).unwrap();
        let r = undo_last(&c);
        assert_eq!(r.count, 0);
        assert_eq!(
            r.errors,
            [
                "Skipped a damaged log entry.",
                "Skipped a damaged log entry."
            ]
        );
        assert!(read_log(&c).is_empty());
    }

    #[test]
    fn relative_and_protected_paths_are_refused() {
        assert!(!log_path_ok(Path::new(r"a\b.txt")));
        assert!(!log_path_ok(Path::new(r"C:\Users\x\AppData\b.txt")));
        assert!(log_path_ok(Path::new(r"C:\Users\x\Downloads\b.txt")));
    }

    #[test]
    fn bucket_depth_matches_v022() {
        assert_eq!(bucket_depth(Path::new(r"C:\r\Images\Screenshots\a.png")), 2);
        assert_eq!(bucket_depth(Path::new(r"C:\r\Images\Chat\a.png")), 2);
        assert_eq!(bucket_depth(Path::new(r"C:\r\Images\a.png")), 1);
        assert_eq!(bucket_depth(Path::new(r"C:\r\Docs\Screenshots\a.png")), 1);
    }
}
