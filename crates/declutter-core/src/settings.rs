//! Persistent user data. Owner: core-C. `<data_dir>/settings.json` must stay readable from v0.2.2 files (read
//! `src/declutter/theme.py` for the existing keys and clamping, `recent_hours_setting` in core.py) and tolerate
//! junk (fall back to defaults field by field). Writes are atomic and merge, never clobber unknown keys.
//! Also the local "space freed" tally (`<data_dir>/stats.json`) and the preset store (`presets.rs` uses it).

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde_json::{json, Map, Value};

use crate::ctx::Ctx;
use crate::model::{Settings, SpaceFreed};

const SETTINGS_FILE: &str = "settings.json";
const STATS_FILE: &str = "stats.json";
const RECENT_HOURS_DEFAULT: u32 = 24;
const RECENT_HOURS_MAX: u32 = 72;
const MIN_MB_DEFAULT: f64 = 100.0;
const MIN_MB_MAX: f64 = 1_048_576.0; // 1 TiB; anything larger can only be a typo
const OLDER_DAYS_MAX: u32 = 36_500;
/// v0.2.2 stores the theme with these spellings and falls back to "System" for anything else.
const THEMES: [&str; 3] = ["System", "Light", "Dark"];

static STORE_LOCK: Mutex<()> = Mutex::new(());

/// Serialises read-modify-write of the JSON stores so two commands cannot lose each other's update.
pub(crate) fn lock() -> MutexGuard<'static, ()> {
    STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Write to a temp file beside `path`, flush it to disk, then rename over the old file, so a crash or a full
/// disk leaves either the old or the new content, never half of it.
pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    // The handle is closed at the end of the closure, before the rename (Windows refuses to rename open files).
    let res = File::create(&tmp)
        .and_then(|mut f| f.write_all(data).and_then(|()| f.sync_all()))
        .and_then(|()| fs::rename(&tmp, path));
    if res.is_err() {
        let _ = fs::remove_file(&tmp); // best effort; the original file is untouched either way
    }
    res
}

/// The settings object on disk; a missing new file falls back to the pre-0.2 `%APPDATA%` copy (v0.2.2 migrated
/// it the same way), anything unreadable to an empty object.
fn read_obj(ctx: &Ctx) -> Map<String, Value> {
    let new = ctx.platform.data_dir().join(SETTINGS_FILE);
    let path = if new.exists() {
        new
    } else {
        ctx.platform.legacy_data_dir().join(SETTINGS_FILE)
    };
    match fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

/// A number or a numeric string (v0.2.2 saved `min_mb` and `older_days` as text). Booleans, NaN and infinity
/// count as junk, as in `recent_hours_setting`.
fn number(v: Option<&Value>) -> Option<f64> {
    let f = match v? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().parse().ok()?,
        _ => return None,
    };
    f.is_finite().then_some(f)
}

fn text(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
}

fn whole(v: Option<&Value>, max: u32, default: u32) -> u32 {
    // `int(float(v))` truncates toward zero, then clamps.
    number(v).map_or(default, |f| f.trunc().clamp(0.0, f64::from(max)) as u32)
}

fn parse(obj: &Map<String, Value>) -> Settings {
    let pins = match obj.get("pins") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    };
    clean(Settings {
        theme: text(obj.get("theme")).unwrap_or_default(),
        recent_hours: whole(
            obj.get("recent_hours"),
            RECENT_HOURS_MAX,
            RECENT_HOURS_DEFAULT,
        ),
        min_mb: number(obj.get("min_mb")).unwrap_or(MIN_MB_DEFAULT),
        older_days: whole(obj.get("older_days"), OLDER_DAYS_MAX, 0),
        last_folder: text(obj.get("last_folder")),
        remember_last_folder: obj.get("reopen").and_then(Value::as_bool).unwrap_or(false),
        pins,
        active_preset: text(obj.get("active_preset")),
        reminder_enabled: obj
            .get("reminder_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn pin_key(p: &str) -> String {
    p.trim_end_matches(['\\', '/']).to_lowercase()
}

fn valid_pin(p: &str) -> bool {
    Path::new(p).is_absolute() && !p.chars().any(char::is_control)
}

fn file_theme(theme: &str) -> &'static str {
    THEMES
        .into_iter()
        .find(|t| t.eq_ignore_ascii_case(theme.trim()))
        .unwrap_or(THEMES[0])
}

/// Clamp every field into range so neither junk on disk nor a bad value from the UI is ever stored or used.
fn clean(mut s: Settings) -> Settings {
    s.theme = file_theme(&s.theme).to_lowercase();
    s.recent_hours = s.recent_hours.min(RECENT_HOURS_MAX);
    s.min_mb = if s.min_mb.is_finite() {
        s.min_mb.clamp(0.0, MIN_MB_MAX)
    } else {
        MIN_MB_DEFAULT
    };
    s.older_days = s.older_days.min(OLDER_DAYS_MAX);
    s.last_folder = s.last_folder.filter(|f| !f.trim().is_empty());
    s.active_preset = s.active_preset.filter(|p| !p.trim().is_empty());
    let mut seen = HashSet::new();
    s.pins = s
        .pins
        .iter()
        .map(|p| p.trim())
        .filter(|p| valid_pin(p) && seen.insert(pin_key(p)))
        .map(str::to_owned)
        .collect();
    s
}

pub fn load_settings(ctx: &Ctx) -> Settings {
    parse(&read_obj(ctx))
}

/// Merge into the file under the v0.2.2 key names; keys this version does not know are kept.
fn store(ctx: &Ctx, settings: &Settings) -> Result<Settings, String> {
    let s = clean(settings.clone());
    let mut obj = read_obj(ctx);
    for (k, v) in [
        ("theme", json!(file_theme(&s.theme))),
        ("recent_hours", json!(s.recent_hours)),
        ("min_mb", json!(s.min_mb)),
        ("older_days", json!(s.older_days)),
        ("last_folder", json!(s.last_folder)),
        ("reopen", json!(s.remember_last_folder)),
        ("pins", json!(s.pins)),
        ("active_preset", json!(s.active_preset)),
        ("reminder_enabled", json!(s.reminder_enabled)),
    ] {
        obj.insert(k.to_owned(), v);
    }
    let data =
        serde_json::to_vec(&obj).map_err(|e| format!("Could not save your settings: {e}"))?;
    write_atomic(&ctx.platform.data_dir().join(SETTINGS_FILE), &data)
        .map_err(|e| format!("Could not save your settings: {e}"))?;
    Ok(s)
}

pub fn save_settings(ctx: &Ctx, settings: &Settings) -> Result<(), String> {
    let _guard = lock();
    store(ctx, settings).map(|_| ())
}

pub fn add_pin(ctx: &Ctx, path: &str) -> Result<Settings, String> {
    let path = path.trim();
    if !valid_pin(path) {
        return Err(
            "Pick a full path to a file or folder, like C:\\Users\\you\\Documents\\Taxes.".into(),
        );
    }
    let _guard = lock();
    let mut s = load_settings(ctx);
    if !s.pins.iter().any(|p| pin_key(p) == pin_key(path)) {
        s.pins.push(path.to_owned());
    }
    store(ctx, &s)
}

/// Removing a path that is not pinned is not an error: the list ends up as asked.
pub fn remove_pin(ctx: &Ctx, path: &str) -> Result<Settings, String> {
    let key = pin_key(path.trim());
    let _guard = lock();
    let mut s = load_settings(ctx);
    s.pins.retain(|p| pin_key(p) != key);
    store(ctx, &s)
}

fn zero() -> SpaceFreed {
    SpaceFreed {
        bytes: 0,
        files: 0,
        since: None,
    }
}

fn read_freed(ctx: &Ctx) -> SpaceFreed {
    fs::read(ctx.platform.data_dir().join(STATS_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(zero)
}

fn write_freed(ctx: &Ctx, s: &SpaceFreed) -> Result<(), String> {
    let data = serde_json::to_vec(s)
        .map_err(|e| format!("Could not update the space-freed total: {e}"))?;
    write_atomic(&ctx.platform.data_dir().join(STATS_FILE), &data)
        .map_err(|e| format!("Could not update the space-freed total: {e}"))
}

pub fn space_freed(ctx: &Ctx) -> SpaceFreed {
    read_freed(ctx)
}

/// Add only what was actually sent to the Recycle Bin.
pub fn record_freed(ctx: &Ctx, bytes: u64, files: u64) -> Result<SpaceFreed, String> {
    let _guard = lock();
    let mut s = read_freed(ctx);
    if bytes == 0 && files == 0 {
        return Ok(s);
    }
    s.bytes = s.bytes.saturating_add(bytes);
    s.files = s.files.saturating_add(files);
    if s.since.is_none() {
        s.since = Some(crate::export::local_time(ctx.now()));
    }
    write_freed(ctx, &s)?;
    Ok(s)
}

pub fn reset_freed(ctx: &Ctx) -> Result<SpaceFreed, String> {
    let _guard = lock();
    write_freed(ctx, &zero())?;
    Ok(zero())
}

/// A platform rooted in a temp folder, shared by core-C's unit tests.
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use crate::ctx::Ctx;
    use crate::platform::{DriveKind, KnownFolder, Platform};

    pub(crate) const NOW: f64 = 1_700_000_000.0;

    struct Fake(PathBuf);

    impl Platform for Fake {
        fn now(&self) -> f64 {
            NOW
        }
        fn home(&self) -> PathBuf {
            self.0.join("home")
        }
        fn known_folder(&self, _: KnownFolder) -> Option<PathBuf> {
            None
        }
        fn drive_kind(&self, _: &Path) -> DriveKind {
            DriveKind::Other
        }
        fn recycle(&self, _: &Path) -> std::io::Result<()> {
            Err(std::io::Error::other("unit tests never recycle"))
        }
        fn data_dir(&self) -> PathBuf {
            self.0.join("data")
        }
        fn legacy_data_dir(&self) -> PathBuf {
            self.0.join("legacy")
        }
    }

    pub(crate) fn ctx(dir: &Path) -> Ctx {
        Ctx::new(Arc::new(Fake(dir.to_path_buf())))
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{ctx, NOW};
    use super::*;

    fn write(dir: &Path, sub: &str, name: &str, text: &str) {
        fs::create_dir_all(dir.join(sub)).unwrap();
        fs::write(dir.join(sub).join(name), text).unwrap();
    }

    fn raw(dir: &Path) -> Value {
        serde_json::from_slice(&fs::read(dir.join("data").join(SETTINGS_FILE)).unwrap()).unwrap()
    }

    #[test]
    fn missing_or_garbage_file_gives_defaults() {
        let t = tempfile::tempdir().unwrap();
        let c = ctx(t.path());
        let s = load_settings(&c);
        assert_eq!(s.theme, "system");
        assert_eq!((s.recent_hours, s.min_mb, s.older_days), (24, 100.0, 0));
        assert!(s.pins.is_empty() && s.last_folder.is_none() && !s.remember_last_folder);
        for junk in ["not json", "[1, 2]", "null", "\"text\""] {
            write(t.path(), "data", SETTINGS_FILE, junk);
            assert_eq!(load_settings(&c), s, "{junk}");
        }
    }

    #[test]
    fn reads_a_v022_file() {
        let t = tempfile::tempdir().unwrap();
        write(
            t.path(),
            "data",
            SETTINGS_FILE,
            r#"{"theme": "Dark", "pins": ["C:\\Keep", 5, "C:\\keep\\", "relative\\x"], "recent_hours": "12",
               "last_folder": "C:\\Users\\a\\Downloads", "reopen": true, "min_mb": "250", "older_days": "30"}"#,
        );
        let s = load_settings(&ctx(t.path()));
        assert_eq!(s.theme, "dark");
        assert_eq!(s.pins, vec!["C:\\Keep"]);
        assert_eq!((s.recent_hours, s.min_mb, s.older_days), (12, 250.0, 30));
        assert_eq!(s.last_folder.as_deref(), Some("C:\\Users\\a\\Downloads"));
        assert!(s.remember_last_folder);
    }

    #[test]
    fn recent_hours_matches_the_oracle() {
        // Python: recent_hours_setting(v) = clamp(int(float(v)), 0, 72); bool or junk -> 24.
        for (v, want) in [
            (json!(5), 5),
            (json!(5.9), 5),
            (json!("7.5"), 7),
            (json!(" 3 "), 3),
            (json!(-4), 0),
            (json!(-0.5), 0),
            (json!(100), 72),
            (json!("1e9"), 72),
            (json!(true), 24),
            (json!("nan"), 24),
            (json!("inf"), 24),
            (json!("abc"), 24),
            (json!(null), 24),
            (json!([1]), 24),
        ] {
            let mut m = Map::new();
            m.insert("recent_hours".into(), v.clone());
            assert_eq!(parse(&m).recent_hours, want, "{v}");
        }
    }

    #[test]
    fn junk_falls_back_per_field() {
        let mut m = Map::new();
        m.insert("theme".into(), json!("neon"));
        m.insert("min_mb".into(), json!("lots"));
        m.insert("older_days".into(), json!(-3));
        m.insert("recent_hours".into(), json!(6));
        m.insert("reopen".into(), json!("yes"));
        m.insert("pins".into(), json!("C:\\x"));
        let s = parse(&m);
        assert_eq!(
            (s.theme.as_str(), s.min_mb, s.older_days, s.recent_hours),
            ("system", 100.0, 0, 6)
        );
        assert!(!s.remember_last_folder && s.pins.is_empty());
        m.insert("min_mb".into(), json!(1e12));
        m.insert("older_days".into(), json!(1e12));
        let s = parse(&m);
        assert_eq!((s.min_mb, s.older_days), (MIN_MB_MAX, OLDER_DAYS_MAX));
    }

    #[test]
    fn save_round_trips_clamps_and_keeps_unknown_keys() {
        let t = tempfile::tempdir().unwrap();
        let c = ctx(t.path());
        write(
            t.path(),
            "data",
            SETTINGS_FILE,
            r#"{"future_key": {"a": 1}, "theme": "Light"}"#,
        );
        let mut s = load_settings(&c);
        assert_eq!(s.theme, "light");
        s.recent_hours = 500;
        s.min_mb = f64::NAN;
        s.older_days = 40;
        s.active_preset = Some("work".into());
        s.reminder_enabled = true;
        s.pins = vec!["C:\\A".into(), "c:\\a\\".into(), "nope".into()];
        save_settings(&c, &s).unwrap();
        let back = load_settings(&c);
        assert_eq!(
            (back.recent_hours, back.min_mb, back.older_days),
            (72, 100.0, 40)
        );
        assert_eq!(back.pins, vec!["C:\\A"]);
        assert_eq!(back.active_preset.as_deref(), Some("work"));
        assert!(back.reminder_enabled);
        let v = raw(t.path());
        assert_eq!(v["future_key"], json!({"a": 1}));
        assert_eq!(
            v["theme"],
            json!("Light"),
            "v0.2.2 only understands capitalised themes"
        );
        assert!(!t.path().join("data").join("settings.json.tmp").exists());
    }

    #[test]
    fn legacy_file_is_read_until_a_new_one_exists() {
        let t = tempfile::tempdir().unwrap();
        let c = ctx(t.path());
        write(
            t.path(),
            "legacy",
            SETTINGS_FILE,
            r#"{"recent_hours": 8, "old": 1}"#,
        );
        assert_eq!(load_settings(&c).recent_hours, 8);
        let mut s = load_settings(&c);
        s.older_days = 9;
        save_settings(&c, &s).unwrap();
        let v = raw(t.path());
        assert_eq!(
            (v["recent_hours"].clone(), v["old"].clone()),
            (json!(8), json!(1))
        );
    }

    #[test]
    fn pins_validate_and_dedupe_case_insensitively() {
        let t = tempfile::tempdir().unwrap();
        let c = ctx(t.path());
        assert!(add_pin(&c, "relative\\folder").is_err());
        assert!(add_pin(&c, "C:folder").is_err());
        assert!(add_pin(&c, "").is_err());
        assert!(add_pin(&c, "C:\\a\nb").is_err());
        add_pin(&c, " C:\\Users\\a\\Taxes ").unwrap();
        let s = add_pin(&c, "c:\\users\\A\\taxes\\").unwrap();
        assert_eq!(s.pins, vec!["C:\\Users\\a\\Taxes"]);
        add_pin(&c, "\\\\server\\share\\x").unwrap();
        let s = remove_pin(&c, "C:\\USERS\\a\\taxes").unwrap();
        assert_eq!(s.pins, vec!["\\\\server\\share\\x"]);
        assert_eq!(remove_pin(&c, "C:\\never").unwrap().pins, s.pins);
        assert_eq!(load_settings(&c).pins, s.pins);
    }

    #[test]
    fn space_freed_tally() {
        let t = tempfile::tempdir().unwrap();
        let c = ctx(t.path());
        assert_eq!(space_freed(&c), zero());
        assert_eq!(
            record_freed(&c, 0, 0).unwrap(),
            zero(),
            "nothing recycled, nothing recorded"
        );
        assert!(!t.path().join("data").join(STATS_FILE).exists());
        let s = record_freed(&c, 1000, 2).unwrap();
        let since = s.since.clone().expect("since is set on the first record");
        assert_eq!(since, crate::export::local_time(NOW));
        let s = record_freed(&c, u64::MAX, 1).unwrap();
        assert_eq!(
            (s.bytes, s.files, s.since.as_deref()),
            (u64::MAX, 3, Some(since.as_str()))
        );
        assert_eq!(space_freed(&c), s);
        assert_eq!(reset_freed(&c).unwrap(), zero());
        assert_eq!(space_freed(&c), zero());
        write(
            t.path(),
            "data",
            STATS_FILE,
            r#"{"bytes": -5, "files": "x"}"#,
        );
        assert_eq!(space_freed(&c), zero());
    }

    #[test]
    fn atomic_write_replaces_existing_file() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("deep").join("f.json");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert!(!t.path().join("deep").join("f.json.tmp").exists());
    }
}
