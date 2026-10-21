//! Declutter app layer: everything the Tauri shell does, minus Tauri, so it is testable in CI with a fake platform.
//! Owner: shell. The command and event contract is docs/API.md.
//!
//! The UI is never trusted with a result. Each action takes only items the shell itself found in its last scan of
//! that folder (its own copy, not the UI's), drops pinned paths, and then the core re-checks the disk right before
//! acting (CONTRACT "Removals"). Nothing here deletes: removals go through the core to the Recycle Bin.

mod scan;

use std::collections::{HashMap, HashSet};
use std::os::windows::process::CommandExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use declutter_core::model::{
    ActionResult, AppInfo, BinInfo, DupGroup, DupRemoval, ExportPayload, FolderCheck, Move, Phase,
    Preset, QuickFolders, Redownload, ReminderStatus, ScanEvent, ScanOptions, Settings,
    SimilarGroup, SimilarRemoval, SpaceFreed, UndoInfo,
};
use declutter_core::platform::{KnownFolder, Platform};
use declutter_core::sort::dto_path;
use declutter_core::{
    dupes, export, paths, presets, redownloads, reminder, settings, sort, trash, undo, Ctx,
};

pub use scan::{Core, Engine};

/// Where scan events go: the Tauri channel in the app, a recorder in tests.
pub type Emit = Arc<dyn Fn(ScanEvent) + Send + Sync>;

/// At most one progress event per interval and phase, so a fast walk cannot flood the IPC channel.
const PROGRESS_EVERY: Duration = Duration::from_millis(50);

/// What the last scan of one folder offered. Actions only touch items found here.
#[derive(Default)]
struct Memo {
    scan_id: u64,
    moves: Vec<Move>,
    /// Large & Old and Leftovers, lower-cased.
    files: HashSet<String>,
    empty: HashSet<String>,
    dups: Vec<DupGroup>,
    redownloads: Vec<Redownload>,
    similar: Vec<SimilarGroup>,
}

impl Memo {
    fn record(&mut self, ev: &ScanEvent) {
        match ev {
            ScanEvent::Sort { plan } => self.moves = plan.moves.clone(),
            ScanEvent::LargeOld { files } => self.files.extend(files.iter().map(|f| key(&f.path))),
            ScanEvent::Leftovers { items } => self.files.extend(items.iter().map(|f| key(&f.path))),
            ScanEvent::EmptyDirs { dirs } => self.empty = dirs.iter().map(key).collect(),
            ScanEvent::Duplicates { groups } => self.dups = groups.clone(),
            ScanEvent::Redownloads { pairs } => self.redownloads = pairs.clone(),
            ScanEvent::Similar { groups } => self.similar = groups.clone(),
            _ => {}
        }
    }
}

pub struct App {
    platform: Arc<dyn Platform>,
    engine: Arc<dyn Engine>,
    next_id: AtomicU64,
    /// Running scans by id, for `cancel_scan`.
    scans: Mutex<HashMap<u64, Ctx>>,
    /// Last scan of each folder, keyed by the lower-cased validated root.
    memos: Mutex<HashMap<String, Memo>>,
    /// Held by every action, so overlapping calls cannot interleave their writes to the undo log or the tally.
    acting: Mutex<()>,
}

impl App {
    pub fn new(platform: Arc<dyn Platform>, engine: Arc<dyn Engine>) -> Arc<Self> {
        Arc::new(App {
            platform,
            engine,
            next_id: AtomicU64::new(1),
            scans: Mutex::default(),
            memos: Mutex::default(),
            acting: Mutex::default(),
        })
    }

    fn ctx(&self) -> Ctx {
        Ctx::new(self.platform.clone())
    }

    pub fn app_info(&self) -> AppInfo {
        AppInfo {
            version: env!("CARGO_PKG_VERSION").into(),
            data_dir: dto_path(&self.platform.data_dir()),
        }
    }

    pub fn quick_folders(&self) -> QuickFolders {
        // v0.2.2's fallback when the shell cannot say: the folder of that name in the home folder.
        let get = |which, name| {
            dto_path(
                &self
                    .platform
                    .known_folder(which)
                    .unwrap_or_else(|| self.platform.home().join(name)),
            )
        };
        QuickFolders {
            downloads: get(KnownFolder::Downloads, "Downloads"),
            desktop: get(KnownFolder::Desktop, "Desktop"),
            documents: get(KnownFolder::Documents, "Documents"),
        }
    }

    pub fn check_folder(&self, path: &str) -> FolderCheck {
        match local_path(path) {
            Ok(p) => paths::check_root(&self.ctx(), &p),
            Err(reason) => FolderCheck { ok: false, reason },
        }
    }

    // ---------- scans ----------

    /// Starts a scan on a worker thread and returns its id at once. Events arrive through `emit` and end with
    /// exactly one `Done` or `Error`.
    pub fn start_scan(self: &Arc<Self>, path: String, options: ScanOptions, emit: Emit) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let last = Mutex::new(None::<(Instant, Phase)>);
        let sink = emit.clone();
        let ctx = self.ctx().with_progress(move |p| {
            let mut last = lock(&last);
            let due = match *last {
                None => true,
                Some((at, phase)) => {
                    phase != p.phase || at.elapsed() >= PROGRESS_EVERY || p.total == Some(p.done)
                }
            };
            if due {
                *last = Some((Instant::now(), p.phase));
                sink(ScanEvent::Progress { progress: p });
            }
        });
        lock(&self.scans).insert(id, ctx.clone());
        let app = self.clone();
        std::thread::spawn(move || {
            let started = Instant::now();
            // A bug in an analysis must end the scan with a message, not leave the UI waiting forever.
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                app.scan(id, &ctx, &path, &options, &emit)
            }))
            .unwrap_or_else(|_| {
                Err("Something went wrong while scanning. Please try again.".into())
            });
            let took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            match outcome {
                Ok(cancelled) => emit(ScanEvent::Done { cancelled, took_ms }),
                Err(_) if ctx.cancelled() => emit(ScanEvent::Done {
                    cancelled: true,
                    took_ms,
                }),
                Err(message) => emit(ScanEvent::Error { message }),
            }
            lock(&app.scans).remove(&id);
        });
        id
    }

    /// Partial results already sent stay valid; the scan ends with `Done { cancelled: true }`.
    pub fn cancel_scan(&self, id: u64) {
        if let Some(ctx) = lock(&self.scans).get(&id) {
            ctx.cancel();
        }
    }

    fn scan(
        &self,
        id: u64,
        ctx: &Ctx,
        path: &str,
        opts: &ScanOptions,
        emit: &Emit,
    ) -> Result<bool, String> {
        if !(opts.min_mb.is_finite() && opts.min_mb >= 0.0) {
            return Err("The minimum size must be a number, zero or more.".into());
        }
        let root = self.engine.require_root(ctx, &local_path(path)?)?;
        let opts = ScanOptions {
            recent_hours: opts.recent_hours.min(72),
            ..opts.clone()
        };
        let mut ignore = pin_paths(&opts.ignore);
        // Saved pins count even when the UI's copy of the list is stale.
        ignore.extend(self.pins(ctx));
        let k = key(dto_path(&root));
        {
            // An older scan that reaches this point late must not wipe a newer scan's results.
            let mut memos = lock(&self.memos);
            let memo = memos.entry(k.clone()).or_default();
            if memo.scan_id < id {
                *memo = Memo {
                    scan_id: id,
                    ..Memo::default()
                };
            }
        }
        let record = |ev: ScanEvent| {
            if let Some(m) = lock(&self.memos).get_mut(&k).filter(|m| m.scan_id == id) {
                m.record(&ev);
            }
            emit(ev);
        };
        scan::pipeline(self.engine.as_ref(), ctx, &root, &opts, &ignore, &record)
    }

    // ---------- actions ----------

    /// The validated root of a folder scanned in this session, and `f`'s answer about what that scan offered.
    fn scanned<T>(
        &self,
        ctx: &Ctx,
        root: &str,
        f: impl FnOnce(&Memo) -> T,
    ) -> Result<(PathBuf, T), String> {
        let root = self.engine.require_root(ctx, &local_path(root)?)?;
        let memos = lock(&self.memos);
        let memo = memos
            .get(&key(dto_path(&root)))
            .ok_or("Scan this folder first, then try again.")?;
        let answer = f(memo);
        Ok((root, answer))
    }

    fn pins(&self, ctx: &Ctx) -> Vec<PathBuf> {
        pin_paths(&self.engine.settings(ctx).pins)
    }

    /// Runs a Recycle Bin action and adds exactly what left the disk to the "space freed" tally.
    fn recycle(
        &self,
        ctx: &Ctx,
        paths: &[PathBuf],
        mut errors: Vec<String>,
        act: impl FnOnce(&[PathBuf]) -> ActionResult,
    ) -> ActionResult {
        if paths.is_empty() {
            return ActionResult { count: 0, errors };
        }
        let before: Vec<(&PathBuf, u64)> = paths
            .iter()
            .filter_map(|p| {
                p.symlink_metadata()
                    .ok()
                    .map(|m| (p, if m.is_file() { m.len() } else { 0 }))
            })
            .collect();
        let mut result = act(paths);
        let gone: Vec<u64> = before
            .iter()
            .filter(|(p, _)| p.symlink_metadata().is_err())
            .map(|(_, n)| *n)
            .collect();
        if !gone.is_empty() {
            // The tally is a nicety: failing to save it must not report a finished removal as an error.
            let _ = settings::record_freed(ctx, gone.iter().sum(), gone.len() as u64);
        }
        errors.append(&mut result.errors);
        result.errors = errors;
        result
    }

    pub fn apply_sort(
        &self,
        root: &str,
        moves: &[Move],
        recent_hours: u32,
    ) -> Result<ActionResult, String> {
        let _one = lock(&self.acting);
        let ctx = self.ctx();
        let mut errors = Vec::new();
        let (root, planned) = self.scanned(&ctx, root, |m| {
            let known: HashMap<String, &Move> =
                m.moves.iter().map(|mv| (key(&mv.src), mv)).collect();
            let mut ok = Vec::new();
            for mv in moves {
                // Only a move the shell planned itself, destination included: a forged `dst` is refused.
                match known
                    .get(&key(&mv.src))
                    .filter(|k| key(&k.dst) == key(&mv.dst))
                {
                    Some(k) => ok.push((*k).clone()),
                    None => errors.push(not_scanned(&mv.src)),
                }
            }
            ok
        })?;
        let pins = self.pins(&ctx);
        let planned: Vec<Move> = planned
            .into_iter()
            .filter(|m| allowed(&pins, Path::new(&m.src), &mut errors))
            .collect();
        if planned.is_empty() {
            return Ok(ActionResult { count: 0, errors });
        }
        let mut result =
            sort::apply_moves(&ctx, &planned, Some(root.as_path()), recent_hours.min(72));
        errors.append(&mut result.errors);
        result.errors = errors;
        Ok(result)
    }

    /// Large & Old and Leftovers.
    pub fn trash_paths(&self, root: &str, paths: &[String]) -> Result<ActionResult, String> {
        let _one = lock(&self.acting);
        let ctx = self.ctx();
        let mut errors = Vec::new();
        let (root, ok) = self.scanned(&ctx, root, |m| offered(paths, &m.files, &mut errors))?;
        let ok = unpinned(&self.pins(&ctx), ok, &mut errors);
        Ok(self.recycle(&ctx, &ok, errors, |ps| {
            trash::to_recycle_bin(&ctx, ps, Some(root.as_path()))
        }))
    }

    pub fn trash_empty_dirs(&self, root: &str, paths: &[String]) -> Result<ActionResult, String> {
        let _one = lock(&self.acting);
        let ctx = self.ctx();
        let mut errors = Vec::new();
        let (root, ok) = self.scanned(&ctx, root, |m| offered(paths, &m.empty, &mut errors))?;
        let ok = unpinned(&self.pins(&ctx), ok, &mut errors);
        Ok(self.recycle(&ctx, &ok, errors, |ps| {
            trash::trash_empty_dirs(&ctx, ps, &root)
        }))
    }

    pub fn trash_duplicates(
        &self,
        root: &str,
        removals: &[DupRemoval],
    ) -> Result<ActionResult, String> {
        let _one = lock(&self.acting);
        let ctx = self.ctx();
        let mut errors = Vec::new();
        let (root, picked) = self.scanned(&ctx, root, |m| {
            let paths_of =
                |g: &DupGroup| -> Vec<String> { g.files.iter().map(|f| f.path.clone()).collect() };
            pick_groups(
                &m.dups,
                paths_of,
                removals.iter().map(|r| &r.remove),
                &mut errors,
            )
        })?;
        let pins = self.pins(&ctx);
        let mut batch = Vec::new();
        for (group, remove) in picked {
            let selected: HashSet<String> = remove.iter().cloned().collect();
            // SPEC rule 6, checked again on the shell's own copy of the group.
            if let Err(e) = dupes::check_dup_selection(std::slice::from_ref(&group), &selected) {
                errors.push(e);
                continue;
            }
            let remove = unpinned(
                &pins,
                remove.into_iter().map(PathBuf::from).collect(),
                &mut errors,
            );
            if !remove.is_empty() {
                batch.push((group, remove));
            }
        }
        let paths: Vec<PathBuf> = batch.iter().flat_map(|(_, r)| r.iter().cloned()).collect();
        Ok(self.recycle(&ctx, &paths, errors, |_| {
            dupes::trash_duplicates(&ctx, &batch, &root)
        }))
    }

    /// `removing`: duplicate copies going in the same batch. It can only make the core keep more, so it is
    /// passed on as given.
    pub fn trash_redownloads(
        &self,
        root: &str,
        pairs: &[Redownload],
        removing: &[String],
    ) -> Result<ActionResult, String> {
        let _one = lock(&self.acting);
        let ctx = self.ctx();
        let mut errors = Vec::new();
        let (root, known) = self.scanned(&ctx, root, |m| {
            let known: HashMap<String, &Redownload> = m
                .redownloads
                .iter()
                .map(|r| (key(&r.copy.path), r))
                .collect();
            let mut ok = Vec::new();
            for p in pairs {
                match known
                    .get(&key(&p.copy.path))
                    .filter(|k| key(&k.original.path) == key(&p.original.path))
                {
                    Some(k) => ok.push((*k).clone()),
                    None => errors.push(not_scanned(&p.copy.path)),
                }
            }
            ok
        })?;
        let pins = self.pins(&ctx);
        let known: Vec<Redownload> = known
            .into_iter()
            .filter(|r| allowed(&pins, Path::new(&r.copy.path), &mut errors))
            .collect();
        let copies: Vec<PathBuf> = known.iter().map(|r| PathBuf::from(&r.copy.path)).collect();
        let removing: Vec<PathBuf> = removing.iter().map(PathBuf::from).collect();
        Ok(self.recycle(&ctx, &copies, errors, |_| {
            redownloads::trash_redownloads(&ctx, &known, &root, &removing)
        }))
    }

    /// Similar pictures. The core has no wrapper for these, so the keep-one rule is enforced here.
    pub fn trash_similar(
        &self,
        root: &str,
        removals: &[SimilarRemoval],
    ) -> Result<ActionResult, String> {
        let _one = lock(&self.acting);
        let ctx = self.ctx();
        let mut errors = Vec::new();
        let (root, picked) = self.scanned(&ctx, root, |m| {
            let paths_of = |g: &SimilarGroup| -> Vec<String> {
                g.files.iter().map(|f| f.path.clone()).collect()
            };
            pick_groups(
                &m.similar,
                paths_of,
                removals.iter().map(|r| &r.remove),
                &mut errors,
            )
        })?;
        let pins = self.pins(&ctx);
        let mut paths = Vec::new();
        for (group, remove) in picked {
            let kept: Vec<&str> = group
                .files
                .iter()
                .map(|f| f.path.as_str())
                .filter(|p| !remove.iter().any(|r| r == p))
                .collect();
            if kept.is_empty() {
                errors
                    .push("Skipped a group of similar pictures: keep at least one of them.".into());
            } else if !kept.iter().any(|p| Path::new(p).is_file()) {
                errors.push("Skipped a group of similar pictures: the one you are keeping is no longer there.".into());
            } else {
                paths.extend(unpinned(
                    &pins,
                    remove.into_iter().map(PathBuf::from).collect(),
                    &mut errors,
                ));
            }
        }
        Ok(self.recycle(&ctx, &paths, errors, |ps| {
            trash::to_recycle_bin(&ctx, ps, Some(root.as_path()))
        }))
    }

    pub fn peek_undo(&self) -> Option<UndoInfo> {
        undo::peek_undo(&self.ctx())
    }

    pub fn undo_last_sort(&self) -> ActionResult {
        let _one = lock(&self.acting);
        undo::undo_last(&self.ctx())
    }

    // ---------- settings, presets, export ----------

    pub fn get_settings(&self) -> Settings {
        settings::load_settings(&self.ctx())
    }

    /// Returns the settings as stored (the core clamps and validates them).
    pub fn save_settings(&self, s: &Settings) -> Result<Settings, String> {
        let ctx = self.ctx();
        settings::save_settings(&ctx, s)?;
        Ok(settings::load_settings(&ctx))
    }

    pub fn add_pin(&self, path: &str) -> Result<Settings, String> {
        local_path(path)?;
        settings::add_pin(&self.ctx(), path)
    }

    pub fn remove_pin(&self, path: &str) -> Result<Settings, String> {
        settings::remove_pin(&self.ctx(), path)
    }

    pub fn list_presets(&self) -> Vec<Preset> {
        presets::list_presets(&self.ctx())
    }

    pub fn save_preset(&self, preset: &Preset) -> Result<Vec<Preset>, String> {
        presets::save_preset(&self.ctx(), preset)
    }

    pub fn delete_preset(&self, id: &str) -> Result<Vec<Preset>, String> {
        presets::delete_preset(&self.ctx(), id)
    }

    /// `path` comes from the native save dialog; only a `.csv` file on a local drive is written.
    pub fn export_csv(&self, path: &str, payload: &ExportPayload) -> Result<(), String> {
        let p = local_path(path)?;
        if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv")) {
            return Err("Save the export as a .csv file.".into());
        }
        let csv = match payload {
            ExportPayload::Sort { moves } => export::moves_csv(moves),
            ExportPayload::Duplicates { groups } => export::duplicates_csv(groups),
            ExportPayload::LargeOld { files } => export::large_old_csv(files),
            ExportPayload::Leftovers { items } => export::leftovers_csv(items),
            ExportPayload::Redownloads { pairs } => export::redownloads_csv(pairs),
            ExportPayload::Similar { groups } => export::similar_csv(groups),
            ExportPayload::EmptyDirs { dirs } => export::empty_dirs_csv(dirs),
        };
        export::write_csv(&p, &csv)
    }

    pub fn space_freed(&self) -> SpaceFreed {
        settings::space_freed(&self.ctx())
    }

    pub fn reset_space_freed(&self) -> Result<SpaceFreed, String> {
        settings::reset_freed(&self.ctx())
    }

    // ---------- Windows: Recycle Bin, Explorer, reminder ----------

    /// shortcut: folders in the Bin count as items but add no bytes (the trash crate reports only their entry
    /// count); move to SHQueryRecycleBinW behind `Platform` if exact totals matter.
    pub fn bin_info(&self) -> Result<BinInfo, String> {
        let items = ::trash::os_limited::list()
            .map_err(|_| "Could not read the Recycle Bin.".to_string())?;
        let bytes = items
            .iter()
            .filter_map(|i| ::trash::os_limited::metadata(i).ok()?.size.size())
            .sum();
        Ok(BinInfo {
            bytes,
            items: items.len() as u64,
        })
    }

    pub fn open_recycle_bin(&self) -> Result<(), String> {
        explorer()
            .arg("shell:RecycleBinFolder")
            .spawn()
            .map(drop)
            .map_err(|_| "Could not open the Recycle Bin.".into())
    }

    /// Opens Explorer with the item selected. `/select,` never runs or opens the item itself.
    pub fn reveal_in_explorer(&self, path: &str) -> Result<(), String> {
        let p = local_path(path)?;
        if p.symlink_metadata().is_err() {
            return Err("That item is no longer there.".into());
        }
        // Explorer wants `/select,"<path>"` as one token, which Rust's own argument quoting would split;
        // `local_path` has refused quotes, so the path cannot break out of it.
        explorer()
            .raw_arg(format!("/select,\"{}\"", p.display()))
            .spawn()
            .map(drop)
            .map_err(|_| "Could not open Explorer.".into())
    }

    pub fn reminder_status(&self) -> ReminderStatus {
        reminder::status(&self.ctx())
    }

    pub fn reminder_enable(&self) -> Result<ReminderStatus, String> {
        let exe = std::env::current_exe()
            .map_err(|_| "Could not find Declutter's program file.".to_string())?;
        reminder::enable(&self.ctx(), &exe)
    }

    pub fn reminder_disable(&self) -> Result<ReminderStatus, String> {
        reminder::disable(&self.ctx())
    }

    /// `declutter --remind`: scan the saved folder and return the notification text (None: nothing to say).
    /// It only reads (a walk and a sort plan, never an action), so no file is changed.
    pub fn remind(&self) -> Result<Option<String>, String> {
        let ctx = self.ctx();
        let s = self.engine.settings(&ctx);
        let Some(folder) = s.last_folder else {
            return Ok(None);
        };
        let root = self.engine.require_root(&ctx, &local_path(&folder)?)?;
        let pins = pin_paths(&s.pins);
        let scan = self.engine.walk(&ctx, &root, &pins);
        let plan = self.engine.plan_sort(
            &ctx,
            &root,
            &scan.entries,
            s.recent_hours.min(72),
            s.active_preset.as_deref(),
        )?;
        Ok(reminder::message(
            u32::try_from(plan.moves.len()).unwrap_or(u32::MAX),
        ))
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Windows paths compare case-insensitively.
fn key(p: impl AsRef<str>) -> String {
    p.as_ref().to_lowercase()
}

fn not_scanned(p: &str) -> String {
    format!("Skipped {p}: it was not in the last scan of this folder. Scan again and retry.")
}

/// A plain absolute path on a drive letter (`C:\...`). UNC and device paths are refused because touching one can
/// reach the network; quotes and control characters so a path can never break out of a quoted argument.
fn local_path(s: &str) -> Result<PathBuf, String> {
    let drive = matches!(s.as_bytes(), [d, b':', b'\\' | b'/', ..] if d.is_ascii_alphabetic());
    if !drive || s.chars().any(|c| c == '"' || c.is_control()) {
        return Err(
            "Choose a file or folder on this PC's drives (network locations are not supported)."
                .into(),
        );
    }
    Ok(PathBuf::from(s))
}

/// Pins spelled the way the scan spells paths: links resolved like the scanned root, so a pin made through a link
/// still holds. A pin that cannot be resolved (gone, or not on a local drive) is kept as written.
fn pin_paths(raw: &[String]) -> Vec<PathBuf> {
    raw.iter()
        .map(|s| {
            local_path(s)
                .ok()
                .and_then(|p| paths::resolve(&p))
                .unwrap_or_else(|| PathBuf::from(s))
        })
        .collect()
}

/// The paths the scan offered; the rest are reported.
fn offered(paths: &[String], known: &HashSet<String>, errors: &mut Vec<String>) -> Vec<PathBuf> {
    let mut ok = Vec::new();
    for p in paths {
        if known.contains(&key(p)) {
            ok.push(PathBuf::from(p));
        } else {
            errors.push(not_scanned(p));
        }
    }
    ok
}

/// True unless `p` is on the Never touch list (saved pins are re-read right before every action).
fn allowed(pins: &[PathBuf], p: &Path, errors: &mut Vec<String>) -> bool {
    let pinned = !pins.is_empty() && paths::is_pinned(p, pins);
    if pinned {
        errors.push(format!(
            "Skipped {}: it is on your Never touch list.",
            p.display()
        ));
    }
    !pinned
}

fn unpinned(pins: &[PathBuf], paths: Vec<PathBuf>, errors: &mut Vec<String>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|p| allowed(pins, p, errors))
        .collect()
}

/// For each stored group, every path the removals name in it, spelled as the scan did and without repeats.
/// Removals naming the same group are merged, so the keep-one check sees the whole selection. A removal naming
/// paths outside one stored group is refused.
fn pick_groups<'a, G: Clone>(
    groups: &[G],
    paths_of: impl Fn(&G) -> Vec<String>,
    removals: impl Iterator<Item = &'a Vec<String>>,
    errors: &mut Vec<String>,
) -> Vec<(G, Vec<String>)> {
    let mut index: HashMap<String, (usize, String)> = HashMap::new();
    for (i, g) in groups.iter().enumerate() {
        for p in paths_of(g) {
            index.insert(key(&p), (i, p));
        }
    }
    let mut picked: Vec<(usize, Vec<String>)> = Vec::new();
    for remove in removals.filter(|r| !r.is_empty()) {
        let hits: Option<Vec<&(usize, String)>> =
            remove.iter().map(|p| index.get(&key(p))).collect();
        let group = hits.as_ref().and_then(|h| {
            let i = h.first()?.0;
            h.iter().all(|(j, _)| *j == i).then_some(i)
        });
        match (group, hits) {
            (Some(i), Some(hits)) => {
                let at = picked.iter().position(|(j, _)| *j == i).unwrap_or_else(|| {
                    picked.push((i, Vec::new()));
                    picked.len() - 1
                });
                for (_, p) in hits {
                    if !picked[at].1.contains(p) {
                        picked[at].1.push(p.clone());
                    }
                }
            }
            _ => errors.push("Skipped a group that was not in the last scan of this folder. Scan again and retry.".into()),
        }
    }
    picked
        .into_iter()
        .map(|(i, remove)| (groups[i].clone(), remove))
        .collect()
}

/// `%SystemRoot%\explorer.exe` by full path, so nothing on PATH can stand in for it.
fn explorer() -> Command {
    let windir = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    Command::new(windir.join("explorer.exe"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_path_accepts_drive_paths_only() {
        assert!(local_path(r"C:\Users\me\Downloads").is_ok());
        assert!(local_path("d:/photos").is_ok());
        for bad in [
            r"\\server\share\x",
            r"\\?\C:\x",
            "relative\\x",
            "C:",
            "",
            "C:\\a\"b",
            "C:\\a\nb",
            "1:\\x",
        ] {
            assert!(local_path(bad).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn pick_groups_uses_the_stored_spelling_and_refuses_mixed_or_unknown_groups() {
        let groups = vec![
            vec![r"C:\r\A.txt".to_string(), r"C:\r\b.txt".into()],
            vec![r"C:\r\c.txt".to_string()],
        ];
        let removals = [
            vec![r"c:\R\a.TXT".to_string()],
            vec![r"C:\r\a.txt".to_string(), r"C:\r\c.txt".into()],
            vec![r"C:\x".to_string()],
            vec![],
        ];
        let mut errors = Vec::new();
        let picked = pick_groups(
            &groups,
            |g: &Vec<String>| g.clone(),
            removals.iter(),
            &mut errors,
        );
        assert_eq!(
            picked,
            vec![(groups[0].clone(), vec![r"C:\r\A.txt".to_string()])]
        );
        assert_eq!(errors.len(), 2, "{errors:?}");
    }

    #[test]
    fn pick_groups_merges_removals_on_the_same_group() {
        let groups = vec![vec![r"C:\r\A.txt".to_string(), r"C:\r\b.txt".into()]];
        let removals = [
            vec![r"C:\r\a.txt".to_string()],
            vec![r"C:\r\B.txt".to_string(), r"c:\r\A.TXT".into()],
        ];
        let mut errors = Vec::new();
        let picked = pick_groups(
            &groups,
            |g: &Vec<String>| g.clone(),
            removals.iter(),
            &mut errors,
        );
        assert_eq!(picked, vec![(groups[0].clone(), groups[0].clone())]);
        assert!(errors.is_empty(), "{errors:?}");
    }
}
