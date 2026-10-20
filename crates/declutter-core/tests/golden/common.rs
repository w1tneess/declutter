//! Shared golden-test plumbing: builds a fixture's tree on disk with exact times and attributes, injects a fake
//! platform (drive type, Downloads folder, Recycle Bin, data dir, clock), and compares results with the oracle.

use std::fs::{self, FileTimes, OpenOptions};
use std::io::Write;
use std::os::windows::fs::FileTimesExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use declutter_core::platform::{DriveKind, KnownFolder, Platform};
use declutter_core::Ctx;
use serde_json::{json, Value};
use windows::core::HSTRING;
use windows::Win32::Storage::FileSystem::{
    GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_OFFLINE, FILE_FLAGS_AND_ATTRIBUTES,
};

/// The fake platform fixtures run against.
pub struct TestPlatform {
    pub now: f64,
    pub downloads: Option<PathBuf>,
    /// `GetDriveTypeW` value: 3 = fixed (the default); anything else is refused for removals.
    pub drive_type: u32,
    pub data_dir: PathBuf,
    pub legacy_dir: PathBuf,
    pub bin: PathBuf,
    pub home: PathBuf,
}

impl Platform for TestPlatform {
    fn now(&self) -> f64 { self.now }
    fn home(&self) -> PathBuf { self.home.clone() }
    fn known_folder(&self, which: KnownFolder) -> Option<PathBuf> {
        if which == KnownFolder::Downloads { self.downloads.clone() } else { None }
    }
    fn drive_kind(&self, _p: &Path) -> DriveKind {
        if self.drive_type == 3 { DriveKind::Fixed } else { DriveKind::Removable }
    }
    fn recycle(&self, path: &Path) -> std::io::Result<()> {
        fs::create_dir_all(&self.bin)?;
        let name = path.file_name().map(|n| n.to_owned()).unwrap_or_default();
        let mut dst = self.bin.join(&name);
        let mut i = 0;
        while dst.exists() {
            i += 1;
            dst = self.bin.join(format!("{} ({i})", name.to_string_lossy()));
        }
        fs::rename(path, dst)
    }
    fn data_dir(&self) -> PathBuf { self.data_dir.clone() }
    fn legacy_data_dir(&self) -> PathBuf { self.legacy_dir.clone() }
}

/// One fixture's sandbox. Dropped (and deleted) at the end of the test.
pub struct Env {
    pub root: PathBuf,
    pub ctx: Ctx,
    pub platform: Arc<TestPlatform>,
    _tmp: tempfile::TempDir,
}

/// `%TEMP%` lives under AppData, which `check_root` refuses by design, so roots go in `<repo>/.golden_tmp`.
fn scratch() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.golden_tmp");
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// Strip the `\\?\` verbatim prefix `canonicalize` adds on Windows.
fn plain(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy().into_owned();
    PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s).to_owned())
}

impl Env {
    pub fn new(fx: &Value) -> Env {
        let tmp = tempfile::Builder::new().prefix("run-").tempdir_in(plain(scratch())).unwrap();
        let base = plain(tmp.path().canonicalize().unwrap());
        let root = base.join("work");
        let params = &fx["params"];
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
        materialize(&fx["tree"], &root, now);
        for extra in fx["outside"].as_array().into_iter().flatten() {
            materialize(&json!({ "files": [extra] }), &root, now);
        }
        let platform = Arc::new(TestPlatform {
            now,
            downloads: params["downloads"].as_str().map(|d| root.join(d)),
            drive_type: params["drive_type"].as_u64().unwrap_or(3) as u32,
            data_dir: base.join("logs"),
            legacy_dir: base.join("oldlogs"),
            bin: base.join("bin"),
            home: std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default(),
        });
        let ctx = Ctx::new(platform.clone());
        Env { root, ctx, platform, _tmp: tmp }
    }

    /// Absolute path for a fixture-relative one (`..` allowed, normalised without touching the disk).
    pub fn path(&self, rel: &str) -> PathBuf {
        let mut out = self.root.clone();
        for c in Path::new(rel).components() {
            match c {
                Component::ParentDir => { out.pop(); }
                Component::CurDir => {}
                other => out.push(other),
            }
        }
        out
    }

    /// Path relative to the root with `/` separators, as the fixtures spell them.
    pub fn rel(&self, p: &Path) -> String {
        let mut up = 0;
        let mut a = self.root.components().peekable();
        let mut b = p.components().peekable();
        while let (Some(x), Some(y)) = (a.peek(), b.peek()) {
            if x.as_os_str().eq_ignore_ascii_case(y.as_os_str()) { a.next(); b.next(); } else { break; }
        }
        up += a.count();
        let mut parts: Vec<String> = vec!["..".into(); up];
        parts.extend(b.map(|c| c.as_os_str().to_string_lossy().into_owned()));
        if parts.is_empty() { ".".into() } else { parts.join("/") }
    }

    /// Messages mention absolute paths; fixtures spell the work root `<root>`.
    pub fn norm(&self, s: &str) -> String { s.replace(&*self.root.to_string_lossy(), "<root>") }

    pub fn snapshot(&self) -> Value { snapshot(&self.root) }

    /// Mirror of `_act` in tools/gen_golden.py: mutate the tree between steps of a scenario.
    pub fn act(&self, action: &Value) {
        let path = self.path(action["path"].as_str().unwrap());
        match action["do"].as_str().unwrap() {
            "delete" => fs::remove_file(&path).unwrap(),
            "write" => {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, content_of(action)).unwrap();
            }
            "truncate" => {
                let n = action["size"].as_u64().unwrap() as usize;
                let b = fs::read(&path).unwrap();
                fs::write(&path, &b[..n.min(b.len())]).unwrap();
            }
            "mkdir" => fs::create_dir_all(&path).unwrap(),
            other => panic!("unknown action {other}"),
        }
    }
}

pub fn content_of(f: &Value) -> Vec<u8> {
    if let Some(segs) = f.get("segments").and_then(Value::as_array) {
        let mut out = Vec::new();
        for s in segs {
            let c = s[0].as_str().unwrap().as_bytes()[0];
            out.extend(std::iter::repeat_n(c, s[1].as_u64().unwrap() as usize));
        }
        return out;
    }
    if let Some(t) = f.get("text").and_then(Value::as_str) { return t.as_bytes().to_vec(); }
    let fill = f.get("fill").and_then(Value::as_str).map_or(b'a', |s| s.as_bytes()[0]);
    vec![fill; f.get("size").and_then(Value::as_u64).unwrap_or(10) as usize]
}

fn systime(secs: f64) -> SystemTime { UNIX_EPOCH + Duration::from_secs_f64(secs.max(0.0)) }

fn set_times(p: &Path, mtime: f64, created: f64) {
    let f = OpenOptions::new().write(true).open(p).unwrap();
    f.set_times(
        FileTimes::new().set_accessed(systime(mtime)).set_modified(systime(mtime)).set_created(systime(created)),
    )
    .unwrap();
}

fn add_attrs(p: &Path, attrs: &[&str]) {
    let h = HSTRING::from(p.as_os_str());
    unsafe {
        let mut cur = GetFileAttributesW(&h);
        for a in attrs {
            cur |= match *a {
                "hidden" => FILE_ATTRIBUTE_HIDDEN.0,
                "offline" => FILE_ATTRIBUTE_OFFLINE.0,
                other => panic!("unknown attribute {other}"),
            };
        }
        SetFileAttributesW(&h, FILE_FLAGS_AND_ATTRIBUTES(cur)).unwrap();
    }
}

/// Default file age when a fixture gives none: 10 days, clear of every threshold (5 min, 3 d, 7 d).
const OLD: f64 = 10.0 * 86400.0;

pub fn materialize(tree: &Value, root: &Path, now: f64) {
    fs::create_dir_all(root).unwrap();
    for d in tree["dirs"].as_array().into_iter().flatten() {
        fs::create_dir_all(root.join(d.as_str().unwrap())).unwrap();
    }
    let files: Vec<&Value> = tree["files"].as_array().into_iter().flatten().collect();
    for f in &files {
        let p = root.join(f["path"].as_str().unwrap());
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        if let Some(entries) = f.get("zip").and_then(Value::as_object) {
            let mut z = zip::ZipWriter::new(fs::File::create(&p).unwrap());
            for (name, text) in entries {
                z.start_file(name.as_str(), zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(text.as_str().unwrap().as_bytes()).unwrap();
            }
            z.finish().unwrap();
        } else if f.get("corrupt").and_then(Value::as_bool) == Some(true) {
            fs::write(&p, b"this is not a zip archive").unwrap();
        } else {
            fs::write(&p, content_of(f)).unwrap();
        }
    }
    for f in &files {
        let p = root.join(f["path"].as_str().unwrap());
        let age = f["age_s"].as_f64().unwrap_or(OLD);
        set_times(&p, now - age, now - f["created_age_s"].as_f64().unwrap_or(age));
    }
    for f in &files {
        if let Some(a) = f["attrs"].as_array() {
            let names: Vec<&str> = a.iter().filter_map(Value::as_str).collect();
            add_attrs(&root.join(f["path"].as_str().unwrap()), &names);
        }
    }
    for (d, attrs) in tree["dir_attrs"].as_object().into_iter().flatten() {
        let names: Vec<&str> = attrs.as_array().unwrap().iter().filter_map(Value::as_str).collect();
        add_attrs(&root.join(d), &names);
    }
}

/// `{"files": [...], "dirs": [...]}` with relative `/` paths, sorted: the same shape as gen_golden.py's `snapshot`.
pub fn snapshot(root: &Path) -> Value {
    fn walk(base: &Path, dir: &Path, files: &mut Vec<String>, dirs: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let rel = e.path().strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
            if e.file_type().unwrap().is_dir() {
                dirs.push(rel);
                walk(base, &e.path(), files, dirs);
            } else {
                files.push(rel);
            }
        }
    }
    let (mut files, mut dirs) = (vec![], vec![]);
    walk(root, root, &mut files, &mut dirs);
    files.sort();
    dirs.sort();
    json!({ "files": files, "dirs": dirs })
}

/// Pretty failure message with both sides.
pub fn compare(name: &str, expected: &Value, actual: &Value) -> Result<(), String> {
    if expected == actual { return Ok(()); }
    Err(format!(
        "{name}: result differs from the Python oracle\n--- expected\n{}\n--- actual\n{}",
        serde_json::to_string_pretty(expected).unwrap(),
        serde_json::to_string_pretty(actual).unwrap()
    ))
}

/// Compare lists whose order Python leaves unspecified for ties: same members, and `key` never increases.
pub fn compare_unordered_desc(name: &str, expected: &[String], actual: &[String], key: fn(&str) -> usize)
    -> Result<(), String> {
    let (mut e, mut a) = (expected.to_vec(), actual.to_vec());
    e.sort();
    a.sort();
    if e != a { return Err(format!("{name}: members differ\nexpected {expected:?}\nactual   {actual:?}")); }
    if actual.windows(2).any(|w| key(&w[0]) < key(&w[1])) {
        return Err(format!("{name}: not ordered longest first: {actual:?}"));
    }
    Ok(())
}
