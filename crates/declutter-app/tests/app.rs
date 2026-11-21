//! End-to-end tests of the app layer on a temp tree: a fake platform plus a fake `Engine` standing in for the core
//! analyses, so event order, determinism, cancel and the "only what the scan offered" rule are tested on their own.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};

use declutter_app::{App, Engine};
use declutter_core::model::{
    Breakdown, DupGroup, FileRef, Leftover, Move, Phase, Progress, Redownload, ScanEvent,
    ScanOptions, Settings, SimilarFile, SimilarGroup, SortPlan,
};
use declutter_core::platform::{DriveKind, KnownFolder, Platform};
use declutter_core::walk::{Entry, Scan};
use declutter_core::Ctx;

/// Records every Recycle Bin call; never touches a real bin.
struct FakePlatform {
    data: PathBuf,
    recycled: Mutex<Vec<PathBuf>>,
}

impl Platform for FakePlatform {
    fn now(&self) -> f64 {
        1_700_000_000.0
    }
    fn home(&self) -> PathBuf {
        self.data.join("home")
    }
    fn known_folder(&self, _: KnownFolder) -> Option<PathBuf> {
        None
    }
    fn drive_kind(&self, _: &Path) -> DriveKind {
        DriveKind::Fixed
    }
    fn recycle(&self, path: &Path) -> std::io::Result<()> {
        self.recycled.lock().unwrap().push(path.to_path_buf());
        // Like the real bin, the item leaves the folder (always a temp tree here).
        if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
    }
    fn data_dir(&self) -> PathBuf {
        self.data.clone()
    }
    fn legacy_data_dir(&self) -> PathBuf {
        self.data.join("legacy")
    }
}

/// Stands in for the core: every file is "large and old", `dup*` files are one duplicate group, `pic*` files one
/// similar group, each top-level file moves to `Other`.
#[derive(Default)]
struct FakeEngine {
    /// Phase name at which the fake cancels the scan, as a user pressing Cancel mid-phase would.
    cancel_in: Option<&'static str>,
    /// The walk waits for a cancel (bounded), to test `cancel_scan` on a running scan.
    hold_walk: bool,
    fail_sort: bool,
    exclude_seen: Mutex<Vec<PathBuf>>,
    /// The saved Never touch list.
    pins: Mutex<Vec<String>>,
}

impl FakeEngine {
    fn phase(&self, ctx: &Ctx, name: &str) {
        if self.cancel_in == Some(name) {
            ctx.cancel();
        }
    }
}

fn dto(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn named<'a>(entries: &'a [Entry], prefix: &str) -> Vec<&'a Entry> {
    entries
        .iter()
        .filter(|e| {
            e.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(prefix)
        })
        .collect()
}

fn progress(done: u64, total: Option<u64>) -> Progress {
    Progress {
        phase: Phase::Walking,
        done,
        total,
        bytes: 0,
        eta_secs: None,
        label: "Reading the folder".into(),
    }
}

impl Engine for FakeEngine {
    fn require_root(&self, _: &Ctx, path: &Path) -> Result<PathBuf, String> {
        if path.is_dir() {
            Ok(path.to_path_buf())
        } else {
            Err("That folder does not exist.".into())
        }
    }
    fn settings(&self, _: &Ctx) -> Settings {
        Settings {
            theme: "system".into(),
            recent_hours: 24,
            min_mb: 0.0,
            older_days: 0,
            last_folder: None,
            remember_last_folder: true,
            pins: self.pins.lock().unwrap().clone(),
            active_preset: None,
        }
    }
    fn walk(&self, ctx: &Ctx, root: &Path, _: &[PathBuf]) -> Scan {
        for (done, total) in [(1, None), (2, None), (3, Some(3))] {
            ctx.report(progress(done, total));
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.hold_walk && !ctx.cancelled() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.phase(ctx, "walk");
        let mut entries: Vec<Entry> = fs::read_dir(root)
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| Entry {
                path: e.path(),
                size: e.metadata().unwrap().len(),
                mtime: 0.0,
                created: 0.0,
                attrs: 0,
            })
            .collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Scan {
            entries,
            skipped: vec![root.join("project")],
        }
    }
    fn plan_sort(
        &self,
        ctx: &Ctx,
        root: &Path,
        entries: &[Entry],
        _: u32,
        _: Option<&str>,
    ) -> Result<SortPlan, String> {
        self.phase(ctx, "sort");
        if self.fail_sort {
            return Err("The folder changed while it was being read.".into());
        }
        let moves = entries
            .iter()
            .map(|e| Move {
                src: dto(&e.path),
                dst: dto(&root.join("Other").join(e.path.file_name().unwrap())),
                size: e.size,
                category: "Other".into(),
            })
            .collect();
        Ok(SortPlan {
            moves,
            recent_left: 0,
        })
    }
    fn large_old(&self, _: &Ctx, entries: &[Entry], _: f64, _: u32) -> Vec<FileRef> {
        entries
            .iter()
            .map(|e| FileRef {
                path: dto(&e.path),
                size: e.size,
                mtime: e.mtime,
            })
            .collect()
    }
    fn leftovers(&self, _: &Ctx, _: &[Entry]) -> Vec<Leftover> {
        vec![]
    }
    fn breakdown(&self, _: &Path, _: &[Entry]) -> Breakdown {
        Breakdown {
            folders: vec![],
            types: vec![],
        }
    }
    fn empty_dirs(&self, _: &Ctx, root: &Path, _: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
        Ok(vec![root.join("empty")])
    }
    fn duplicates(&self, ctx: &Ctx, entries: &[Entry]) -> Vec<DupGroup> {
        self.phase(ctx, "duplicates");
        let files: Vec<FileRef> = named(entries, "dup")
            .iter()
            .map(|e| FileRef {
                path: dto(&e.path),
                size: e.size,
                mtime: 0.0,
            })
            .collect();
        if files.len() < 2 {
            vec![]
        } else {
            vec![DupGroup {
                size: files[0].size,
                files,
            }]
        }
    }
    fn redownloads(&self, _: &[Entry], exclude: &[PathBuf]) -> Vec<Redownload> {
        *self.exclude_seen.lock().unwrap() = exclude.to_vec();
        vec![]
    }
    fn similar(&self, _: &Ctx, entries: &[Entry]) -> Vec<SimilarGroup> {
        let files: Vec<SimilarFile> = named(entries, "pic")
            .iter()
            .map(|e| SimilarFile {
                path: dto(&e.path),
                size: e.size,
                mtime: 0.0,
                width: 8,
                height: 8,
                distance: 0,
            })
            .collect();
        if files.len() < 2 {
            vec![]
        } else {
            vec![SimilarGroup { files }]
        }
    }
}

struct Env {
    app: Arc<App>,
    platform: Arc<FakePlatform>,
    engine: Arc<FakeEngine>,
    root: PathBuf,
    _tmp: tempfile::TempDir,
}

fn env(engine: FakeEngine) -> Env {
    // `%TEMP%` lives under AppData, which the core refuses by design, so trees go under the target dir.
    let tmp = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let root = tmp.path().join("work");
    fs::create_dir_all(&root).unwrap();
    for (name, body) in [
        ("a.txt", "a"),
        ("dup1.bin", "same"),
        ("dup2.bin", "same"),
        ("pic1.png", "p"),
        ("pic2.png", "q"),
    ] {
        fs::write(root.join(name), body).unwrap();
    }
    let platform = Arc::new(FakePlatform {
        data: tmp.path().join("data"),
        recycled: Mutex::default(),
    });
    let engine = Arc::new(engine);
    let app = App::new(platform.clone(), engine.clone());
    Env {
        app,
        platform,
        engine,
        root,
        _tmp: tmp,
    }
}

fn options(similar_images: bool) -> ScanOptions {
    ScanOptions {
        min_mb: 0.0,
        older_days: 0,
        recent_hours: 24,
        similar_images,
        ignore: vec![],
        preset_id: None,
    }
}

/// Starts a scan through the real threaded entry point and collects events until it ends.
fn run(e: &Env, path: &Path, opts: ScanOptions, cancel_after_start: bool) -> Vec<ScanEvent> {
    let (tx, rx) = mpsc::channel();
    let id = e.app.start_scan(
        dto(path),
        opts,
        Arc::new(move |ev: ScanEvent| {
            let _ = tx.send(ev);
        }),
    );
    if cancel_after_start {
        e.app.cancel_scan(id);
    }
    let mut events = Vec::new();
    loop {
        let ev = rx
            .recv_timeout(Duration::from_secs(20))
            .expect("the scan must end with Done or Error");
        let end = matches!(ev, ScanEvent::Done { .. } | ScanEvent::Error { .. });
        events.push(ev);
        if end {
            return events;
        }
    }
}

fn kinds(events: &[ScanEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|ev| match ev {
            ScanEvent::Progress { .. } => "progress",
            ScanEvent::Skipped { .. } => "skipped",
            ScanEvent::Sort { .. } => "sort",
            ScanEvent::LargeOld { .. } => "largeOld",
            ScanEvent::Leftovers { .. } => "leftovers",
            ScanEvent::Breakdown { .. } => "breakdown",
            ScanEvent::EmptyDirs { .. } => "emptyDirs",
            ScanEvent::Duplicates { .. } => "duplicates",
            ScanEvent::Redownloads { .. } => "redownloads",
            ScanEvent::Similar { .. } => "similar",
            ScanEvent::Done {
                cancelled: false, ..
            } => "done",
            ScanEvent::Done {
                cancelled: true, ..
            } => "cancelled",
            ScanEvent::Error { .. } => "error",
        })
        .collect()
}

/// Event kinds without the progress ticks, whose count depends on timing.
fn phases(events: &[ScanEvent]) -> Vec<&'static str> {
    kinds(events)
        .into_iter()
        .filter(|k| *k != "progress")
        .collect()
}

const ALL_PHASES: [&str; 9] = [
    "skipped",
    "sort",
    "largeOld",
    "leftovers",
    "breakdown",
    "emptyDirs",
    "duplicates",
    "redownloads",
    "similar",
];

#[test]
fn phases_arrive_in_the_documented_order_with_throttled_progress() {
    let e = env(FakeEngine::default());
    let events = run(&e, &e.root, options(true), false);
    let mut want = ALL_PHASES.to_vec();
    want.push("done");
    assert_eq!(phases(&events), want);
    // Progress comes first, and the throttle always lets the final (done == total) report through.
    let walk: Vec<&Progress> = events
        .iter()
        .take_while(|ev| matches!(ev, ScanEvent::Progress { .. }))
        .map(|ev| match ev {
            ScanEvent::Progress { progress } => progress,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(walk.first().map(|p| p.done), Some(1));
    assert_eq!(walk.last().map(|p| p.total), Some(Some(3)));
}

#[test]
fn similar_runs_only_when_asked() {
    let e = env(FakeEngine::default());
    let events = run(&e, &e.root, options(false), false);
    assert_eq!(phases(&events).last(), Some(&"done"));
    assert!(!phases(&events).contains(&"similar"));
}

#[test]
fn the_same_tree_gives_the_same_events() {
    let e = env(FakeEngine::default());
    // Progress timing is not deterministic; the results are.
    let strip = |evs: Vec<ScanEvent>| -> Vec<ScanEvent> {
        evs.into_iter()
            .filter(|ev| !matches!(ev, ScanEvent::Progress { .. }))
            .map(|ev| match ev {
                ScanEvent::Done { cancelled, .. } => ScanEvent::Done {
                    cancelled,
                    took_ms: 0,
                },
                other => other,
            })
            .collect()
    };
    assert_eq!(
        strip(run(&e, &e.root, options(true), false)),
        strip(run(&e, &e.root, options(true), false))
    );
}

#[test]
fn redownloads_exclude_every_exact_duplicate() {
    let e = env(FakeEngine::default());
    run(&e, &e.root, options(false), false);
    let seen = e.engine.exclude_seen.lock().unwrap().clone();
    assert_eq!(seen, vec![e.root.join("dup1.bin"), e.root.join("dup2.bin")]);
}

#[test]
fn a_cancel_mid_phase_drops_that_phase_and_ends_cancelled() {
    let e = env(FakeEngine {
        cancel_in: Some("duplicates"),
        ..FakeEngine::default()
    });
    let events = run(&e, &e.root, options(true), false);
    let mut want = ALL_PHASES[..6].to_vec();
    want.push("cancelled");
    assert_eq!(phases(&events), want);
}

#[test]
fn a_cancel_during_the_walk_sends_no_results() {
    let e = env(FakeEngine {
        cancel_in: Some("walk"),
        ..FakeEngine::default()
    });
    assert_eq!(
        phases(&run(&e, &e.root, options(true), false)),
        ["cancelled"]
    );
}

#[test]
fn cancel_scan_stops_a_running_scan() {
    let e = env(FakeEngine {
        hold_walk: true,
        ..FakeEngine::default()
    });
    let events = run(&e, &e.root, options(true), true);
    assert_eq!(phases(&events), ["cancelled"]);
}

#[test]
fn a_refused_folder_is_one_error() {
    let e = env(FakeEngine::default());
    let events = run(&e, &e.root.join("missing"), options(true), false);
    assert_eq!(
        events,
        vec![ScanEvent::Error {
            message: "That folder does not exist.".into()
        }]
    );
    let unc = run(&e, Path::new(r"\\server\share"), options(true), false);
    assert_eq!(kinds(&unc), ["error"]);
}

#[test]
fn a_failing_phase_keeps_what_was_sent_and_ends_with_its_error() {
    let e = env(FakeEngine {
        fail_sort: true,
        ..FakeEngine::default()
    });
    let events = run(&e, &e.root, options(true), false);
    assert_eq!(phases(&events), ["skipped", "error"]);
}

#[test]
fn bad_options_are_refused() {
    let e = env(FakeEngine::default());
    let opts = ScanOptions {
        min_mb: f64::NAN,
        ..options(false)
    };
    assert_eq!(kinds(&run(&e, &e.root, opts, false)), ["error"]);
}

// ---------- actions never reach outside what the scan offered ----------

fn nothing_recycled(e: &Env) {
    assert!(e.platform.recycled.lock().unwrap().is_empty());
}

#[test]
fn actions_on_a_folder_never_scanned_are_refused() {
    let e = env(FakeEngine::default());
    let root = dto(&e.root);
    let file = dto(&e.root.join("a.txt"));
    assert!(e
        .app
        .trash_paths(&root, std::slice::from_ref(&file))
        .is_err());
    assert!(e
        .app
        .trash_empty_dirs(&root, std::slice::from_ref(&file))
        .is_err());
    let mv = Move {
        src: file.clone(),
        dst: dto(&e.root.join("Other/a.txt")),
        size: 1,
        category: "Other".into(),
    };
    assert!(e.app.apply_sort(&root, &[mv], 24).is_err());
    assert!(e.root.join("a.txt").exists());
    nothing_recycled(&e);
}

#[test]
fn paths_outside_the_scan_are_refused_one_by_one() {
    let e = env(FakeEngine::default());
    run(&e, &e.root, options(true), false);
    let outside = e.root.parent().unwrap().join("elsewhere.txt");
    fs::write(&outside, "x").unwrap();
    let root = dto(&e.root);
    let r = e
        .app
        .trash_paths(&root, &[dto(&outside), dto(&e.root.join("../work/a.txt"))])
        .unwrap();
    assert_eq!(r.count, 0);
    assert_eq!(r.errors.len(), 2, "{:?}", r.errors);
    assert!(r.errors[0].contains("not in the last scan"));
    let r = e.app.trash_empty_dirs(&root, &[dto(&e.root)]).unwrap();
    assert_eq!((r.count, r.errors.len()), (0, 1));
    assert!(outside.exists());
    nothing_recycled(&e);
}

#[test]
fn a_forged_sort_destination_is_refused() {
    let e = env(FakeEngine::default());
    run(&e, &e.root, options(false), false);
    let outside = e.root.parent().unwrap().join("stolen.txt");
    let forged = Move {
        src: dto(&e.root.join("a.txt")),
        dst: dto(&outside),
        size: 1,
        category: "Other".into(),
    };
    let r = e.app.apply_sort(&dto(&e.root), &[forged], 24).unwrap();
    assert_eq!((r.count, r.errors.len()), (0, 1));
    assert!(e.root.join("a.txt").exists() && !outside.exists());
}

#[test]
fn duplicate_and_redownload_removals_must_match_the_scan() {
    let e = env(FakeEngine::default());
    run(&e, &e.root, options(false), false);
    let root = dto(&e.root);
    // A path no stored group holds.
    let fake = FileRef {
        path: dto(&e.root.join("a.txt")),
        size: 1,
        mtime: 0.0,
    };
    let r = e
        .app
        .trash_duplicates(&root, &[vec![fake.path.clone()]])
        .unwrap();
    assert_eq!((r.count, r.errors.len()), (0, 1));
    let pair = Redownload {
        original: fake.clone(),
        copy: FileRef {
            path: dto(&e.root.join("dup1.bin")),
            ..fake
        },
    };
    let r = e.app.trash_redownloads(&root, &[pair], &[]).unwrap();
    assert_eq!((r.count, r.errors.len()), (0, 1));
    nothing_recycled(&e);
}

#[test]
fn similar_removals_keep_one_picture_per_group() {
    let e = env(FakeEngine::default());
    let events = run(&e, &e.root, options(true), false);
    let group = events
        .iter()
        .find_map(|ev| match ev {
            ScanEvent::Similar { groups } => groups.first().cloned(),
            _ => None,
        })
        .unwrap();
    let all: Vec<String> = group.files.iter().map(|f| f.path.clone()).collect();
    let root = dto(&e.root);
    let r = e
        .app
        .trash_similar(&root, std::slice::from_ref(&all))
        .unwrap();
    assert_eq!((r.count, r.errors.len()), (0, 1));
    assert!(r.errors[0].contains("keep at least one"));
    // The kept picture vanished since the scan: nothing goes.
    fs::remove_file(&all[0]).unwrap();
    let r = e.app.trash_similar(&root, &[vec![all[1].clone()]]).unwrap();
    assert_eq!((r.count, r.errors.len()), (0, 1));
    assert!(r.errors[0].contains("no longer there"));
    nothing_recycled(&e);
}

/// Older than the fake clock, so the core's "just changed" rule (SPEC rule 8) does not hold the file back.
fn make_old(p: &Path) {
    fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(1_699_990_000))
        .unwrap();
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
fn a_sort_never_moves_through_a_linked_folder() {
    let e = env(FakeEngine::default());
    make_old(&e.root.join("a.txt"));
    run(&e, &e.root, options(false), false);
    let outside = e.root.parent().unwrap().join("outside");
    fs::create_dir(&outside).unwrap();
    // The planned destination folder `Other` turns out to be a link to a folder outside the chosen one.
    if !link_dir(&e.root.join("Other"), &outside) {
        eprintln!("skipped: this system refused to create a junction or a symlink");
        return;
    }
    let mv = Move {
        src: dto(&e.root.join("a.txt")),
        dst: dto(&e.root.join("Other").join("a.txt")),
        size: 1,
        category: "Other".into(),
    };
    let r = e.app.apply_sort(&dto(&e.root), &[mv], 0).unwrap();
    assert_eq!((r.count, r.errors.len()), (0, 1), "{:?}", r.errors);
    assert!(r.errors[0].contains("linked folder"));
    assert!(e.root.join("a.txt").exists());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
}

#[test]
fn a_pin_made_through_a_link_still_protects_the_file() {
    let e = env(FakeEngine::default());
    make_old(&e.root.join("a.txt"));
    let alias = e.root.parent().unwrap().join("alias");
    if !link_dir(&alias, &e.root) {
        eprintln!("skipped: this system refused to create a junction or a symlink");
        return;
    }
    // The same file, pinned under the link's spelling; the scan sees the real folder.
    *e.engine.pins.lock().unwrap() = vec![dto(&alias.join("a.txt"))];
    run(&e, &e.root, options(false), false);
    let r = e
        .app
        .trash_paths(&dto(&e.root), &[dto(&e.root.join("a.txt"))])
        .unwrap();
    assert_eq!((r.count, r.errors.len()), (0, 1), "{:?}", r.errors);
    assert!(r.errors[0].contains("Never touch"));
    nothing_recycled(&e);
}

#[test]
fn removals_split_across_entries_still_keep_one_per_group() {
    let e = env(FakeEngine::default());
    for name in ["dup1.bin", "dup2.bin", "pic1.png", "pic2.png"] {
        make_old(&e.root.join(name));
    }
    run(&e, &e.root, options(true), false);
    let root = dto(&e.root);
    let one = |p: &str| vec![dto(&e.root.join(p))];
    let r = e
        .app
        .trash_duplicates(&root, &[one("dup1.bin"), one("dup2.bin")])
        .unwrap();
    assert_eq!((r.count, r.errors.len()), (0, 1), "{:?}", r.errors);
    assert!(r.errors[0].contains("Keep at least one"));
    let r = e
        .app
        .trash_similar(&root, &[one("pic1.png"), one("pic2.png")])
        .unwrap();
    assert_eq!((r.count, r.errors.len()), (0, 1), "{:?}", r.errors);
    assert!(r.errors[0].contains("keep at least one"));
    nothing_recycled(&e);
}

#[test]
fn export_only_writes_csv_files_on_local_drives() {
    let e = env(FakeEngine::default());
    let payload = declutter_core::model::ExportPayload::EmptyDirs { dirs: vec![] };
    assert!(e
        .app
        .export_csv(&dto(&e.root.join("out.exe")), &payload)
        .is_err());
    assert!(e
        .app
        .export_csv(r"\\server\share\out.csv", &payload)
        .is_err());
    assert!(!e.root.join("out.exe").exists());
}

#[test]
fn remind_with_no_saved_folder_says_nothing() {
    let e = env(FakeEngine::default());
    assert_eq!(e.app.remind(), Ok(None));
}

#[test]
fn an_offered_file_goes_to_the_bin_and_is_tallied() {
    let e = env(FakeEngine::default());
    make_old(&e.root.join("a.txt"));
    run(&e, &e.root, options(false), false);
    let r = e
        .app
        .trash_paths(&dto(&e.root), &[dto(&e.root.join("a.txt"))])
        .unwrap();
    assert_eq!(r.count, 1, "{:?}", r.errors);
    assert_eq!(
        *e.platform.recycled.lock().unwrap(),
        vec![e.root.join("a.txt")]
    );
    let freed = e.app.space_freed();
    assert_eq!((freed.bytes, freed.files), (1, 1));
}
