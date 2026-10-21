//! Duplicate finder. Owner: core-B. See CONTRACT "Duplicates". Port of find_duplicates / check_dup_selection /
//! trash_duplicates. size -> first+last 64 KiB -> full SHA-256 (1 MiB chunks), parallel, cancellable in every
//! stage, with the persisted hash cache (`hashcache`). Report `Phase::Comparing` / `Phase::Hashing` progress with
//! `done`, `total`, `bytes` and an ETA so the UI bar is determinate.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Instant;

use rayon::prelude::*;
use sha2::{Digest, Sha256};

use crate::ctx::Ctx;
use crate::hashcache::HashCache;
use crate::model::{ActionResult, DupGroup, FileRef, Phase, Progress};
use crate::paths::{ext_lower, ATTR_CLOUD_ONLY, DEAD_EXTS, SKIP_SORT_EXTS};
use crate::trash::to_recycle_bin;
use crate::walk::Entry;

/// Bytes hashed at each end in the cheap first pass; a file up to twice this long is covered whole by it.
const PARTIAL: usize = 64 * 1024;
const CHUNK: usize = 1024 * 1024;
const REPORT_MS: u64 = 50;

type Sha = [u8; 32];

/// Windows paths compare case-insensitively and accept either slash, as Python's `Path` equality does.
pub(crate) fn path_key(p: &Path) -> String {
    p.to_string_lossy().replace('/', "\\").to_lowercase()
}

pub(crate) fn name(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

pub(crate) fn file_ref(e: &Entry) -> FileRef {
    FileRef {
        path: e.path.to_string_lossy().into_owned(),
        size: e.size,
        mtime: e.mtime,
    }
}

/// Groups sorted by size x (copies - 1) descending; within a group oldest-modified first, ties by path. A cancel
/// returns no groups. Never reads online-only files.
pub fn find_duplicates(ctx: &Ctx, entries: &[Entry], min_size: u64) -> Vec<DupGroup> {
    let mut by_size: HashMap<u64, Vec<&Entry>> = HashMap::new();
    for e in entries {
        // Shortcuts and config files are never copies; old partial downloads are offered only as leftovers.
        let x = ext_lower(&e.path);
        if e.size >= min_size
            && e.attrs & ATTR_CLOUD_ONLY == 0
            && !SKIP_SORT_EXTS.contains(&x.as_str())
            && !DEAD_EXTS.contains(&x.as_str())
        {
            by_size.entry(e.size).or_default().push(e);
        }
    }
    let sized: Vec<&Entry> = by_size
        .into_values()
        .filter(|v| v.len() > 1)
        .flatten()
        .collect();

    let meter = Meter::start(
        ctx,
        Phase::Comparing,
        sized.len() as u64,
        format!("Comparing {} files of equal size…", sized.len()),
    );
    let ends = par_map(&sized, |buf, e| {
        let h = ends_hash(ctx, &e.path, buf);
        meter.add(1);
        h
    });
    let mut by_ends: HashMap<(u64, Sha), Vec<&Entry>> = HashMap::new();
    for (&e, h) in sized.iter().zip(ends) {
        if let Some(h) = h {
            by_ends.entry((e.size, h)).or_default().push(e);
        }
    }
    by_ends.retain(|_, v| v.len() > 1);

    let big: Vec<&Entry> = by_ends
        .iter()
        .filter(|((size, _), _)| *size > 2 * PARTIAL as u64)
        .flat_map(|(_, v)| v.iter().copied())
        .collect();
    let full = full_hashes(ctx, &big);

    let mut groups = Vec::new();
    for ((size, ends), fs) in by_ends {
        let mut buckets: HashMap<Sha, Vec<&Entry>> = HashMap::new();
        if size <= 2 * PARTIAL as u64 {
            buckets.insert(ends, fs); // the ends pass already covered the whole file
        } else {
            for e in fs {
                if let Some(h) = full.get(e.path.as_path()) {
                    buckets.entry(*h).or_default().push(e);
                }
            }
        }
        for mut same in buckets.into_values().filter(|v| v.len() > 1) {
            same.sort_by(|a, b| {
                a.mtime
                    .total_cmp(&b.mtime)
                    .then_with(|| a.path.as_os_str().cmp(b.path.as_os_str()))
            });
            groups.push(DupGroup {
                size,
                files: same.into_iter().map(file_ref).collect(),
            });
        }
    }
    if ctx.cancelled() {
        return Vec::new();
    }
    let reclaimable = |g: &DupGroup| u128::from(g.size) * g.files.len().saturating_sub(1) as u128;
    groups.sort_by(|a, b| {
        reclaimable(b).cmp(&reclaimable(a)).then_with(|| {
            a.files
                .first()
                .map(|f| &f.path)
                .cmp(&b.files.first().map(|f| &f.path))
        })
    });
    groups
}

/// Full SHA-256 of each file, from the persistent cache where its size and mtime still match.
fn full_hashes<'a>(ctx: &Ctx, files: &[&'a Entry]) -> HashMap<&'a Path, Sha> {
    let mut out = HashMap::new();
    if files.is_empty() {
        return out;
    }
    let mut cache = HashCache::load(ctx);
    let mut todo = Vec::new();
    for &e in files {
        match cache.get(&e.path, e.size, e.mtime) {
            Some(h) => {
                out.insert(e.path.as_path(), h);
            }
            None => todo.push(e),
        }
    }
    let meter = Meter::start(
        ctx,
        Phase::Hashing,
        todo.iter().map(|e| e.size).sum(),
        format!("Checking {} possible duplicates in full…", files.len()),
    );
    let hashed = par_map(&todo, |buf, e| full_hash(ctx, e, buf, &meter));
    for (&e, h) in todo.iter().zip(hashed) {
        if let Some(h) = h {
            cache.put(&e.path, e.size, e.mtime, h);
            out.insert(e.path.as_path(), h);
        }
    }
    // The cache only saves time on the next scan; failing to write it must not fail this one.
    let _ = cache.save(ctx);
    out
}

/// Map on a dedicated pool, each worker reusing one CHUNK buffer.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&mut [u8], &T) -> R + Sync) -> Vec<R> {
    // shortcut: min(4, cores) suits SSDs; a spinning disk would do better with 1-2 threads, which needs a
    // seek-penalty query (IOCTL_STORAGE_QUERY_PROPERTY) behind Platform, since unsafe Win32 lives only there.
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(4);
    let run = || -> Vec<R> {
        items
            .par_iter()
            .map_init(|| vec![0u8; CHUNK], |buf, t| f(buf.as_mut_slice(), t))
            .collect()
    };
    match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
        Ok(pool) => pool.install(run),
        Err(_) => {
            let mut buf = vec![0u8; CHUNK];
            items.iter().map(|t| f(buf.as_mut_slice(), t)).collect()
        }
    }
}

/// Read until `buf` is full or the file ends.
fn fill(f: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

/// SHA-256 of the first and last PARTIAL bytes, which is the whole file when it is at most 2 * PARTIAL long.
/// None on any error (vanished, locked, unreadable) or cancel. `buf` holds CHUNK >= PARTIAL bytes.
fn ends_hash(ctx: &Ctx, path: &Path, buf: &mut [u8]) -> Option<Sha> {
    if ctx.cancelled() {
        return None;
    }
    let mut f = File::open(path).ok()?;
    let size = f.metadata().ok()?.len();
    let mut h = Sha256::new();
    let head = fill(&mut f, &mut buf[..PARTIAL]).ok()?;
    h.update(&buf[..head]);
    f.seek(SeekFrom::Start(
        (head as u64).max(size.saturating_sub(PARTIAL as u64)),
    ))
    .ok()?;
    let tail = fill(&mut f, &mut buf[..PARTIAL]).ok()?;
    h.update(&buf[..tail]);
    Some(h.finalize().into())
}

/// Full SHA-256 in CHUNK reads, checking for cancel between chunks.
fn full_hash(ctx: &Ctx, e: &Entry, buf: &mut [u8], meter: &Meter) -> Option<Sha> {
    let mut f = File::open(&e.path).ok()?;
    let mut h = Sha256::new();
    let mut read = 0u64;
    loop {
        if ctx.cancelled() {
            return None;
        }
        let n = fill(&mut f, buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        read += n as u64;
        meter.add(n as u64);
    }
    // A file whose size changed since the walk is not the file that was grouped (or that the cache is keyed on).
    (read == e.size).then(|| h.finalize().into())
}

/// Throttled progress for one phase: `done` counts files (comparing) or bytes (hashing).
struct Meter<'a> {
    ctx: &'a Ctx,
    phase: Phase,
    total: u64,
    label: String,
    done: AtomicU64,
    start: Instant,
    last_ms: AtomicU64,
}

impl<'a> Meter<'a> {
    fn start(ctx: &'a Ctx, phase: Phase, total: u64, label: String) -> Self {
        let m = Meter {
            ctx,
            phase,
            total,
            label,
            done: AtomicU64::new(0),
            start: Instant::now(),
            last_ms: AtomicU64::new(0),
        };
        m.send(0);
        m
    }

    fn add(&self, n: u64) {
        let done = self.done.fetch_add(n, Relaxed) + n;
        let ms = self.start.elapsed().as_millis() as u64;
        let last = self.last_ms.load(Relaxed);
        // compare_exchange lets exactly one worker report per interval.
        if ms >= last + REPORT_MS
            && self
                .last_ms
                .compare_exchange(last, ms, Relaxed, Relaxed)
                .is_ok()
        {
            self.send(done);
        }
    }

    fn send(&self, done: u64) {
        let secs = self.start.elapsed().as_secs_f64();
        // No estimate until there is a measured rate; the first few milliseconds are noise.
        let eta_secs = (done > 0 && secs > 0.25)
            .then(|| self.total.saturating_sub(done) as f64 * secs / done as f64);
        let bytes = if self.phase == Phase::Hashing {
            done
        } else {
            0
        };
        self.ctx.report(Progress {
            phase: self.phase,
            done,
            total: Some(self.total),
            bytes,
            eta_secs,
            label: self.label.clone(),
        });
    }
}

/// `Err` with the user-visible message if any group would lose every copy.
pub fn check_dup_selection(groups: &[DupGroup], selected: &HashSet<String>) -> Result<(), String> {
    let chosen: HashSet<String> = selected.iter().map(|s| path_key(Path::new(s))).collect();
    for g in groups {
        let all = |f: &FileRef| chosen.contains(&path_key(Path::new(&f.path)));
        if let Some(first) = g.files.first().filter(|_| g.files.iter().all(all)) {
            return Err(format!(
                "All {} copies of {} are selected. Keep at least one.",
                g.files.len(),
                name(Path::new(&first.path))
            ));
        }
    }
    Ok(())
}

/// Recycle the chosen copies, but only where an intact kept copy of the same size exists on disk right now, and
/// only copies that are still byte-for-byte identical to it.
pub fn trash_duplicates(
    ctx: &Ctx,
    removals: &[(DupGroup, Vec<PathBuf>)],
    root: &Path,
) -> ActionResult {
    let mut out = ActionResult {
        count: 0,
        errors: Vec::new(),
    };
    for (group, remove) in removals {
        let gone: HashSet<String> = remove.iter().map(|p| path_key(p)).collect();
        let keeper = group.files.iter().map(|f| Path::new(&f.path)).find(|p| {
            !gone.contains(&path_key(p))
                && fs::symlink_metadata(p).is_ok_and(|m| m.is_file() && m.len() == group.size)
        });
        let Some(keeper) = keeper else {
            let first = group
                .files
                .first()
                .map(|f| name(Path::new(&f.path)))
                .unwrap_or_default();
            out.errors
                .push(format!("{first}: no intact copy would remain, skipped"));
            continue;
        };
        let mut sure = Vec::new();
        for p in remove {
            match fs::symlink_metadata(p) {
                // Missing paths and links go on to to_recycle_bin, which skips or refuses them with its own message.
                Err(e) if e.kind() == ErrorKind::NotFound => sure.push(p.clone()),
                Ok(m) if m.is_symlink() => sure.push(p.clone()),
                Ok(m) if m.is_file() => match same_bytes(keeper, p) {
                    Ok(true) => sure.push(p.clone()),
                    Ok(false) => out.errors.push(format!(
                        "{}: no longer identical to its kept copy, skipped",
                        name(p)
                    )),
                    Err(e) => out.errors.push(format!(
                        "{}: could not be compared with its kept copy ({e}), skipped",
                        name(p)
                    )),
                },
                Ok(_) => out.errors.push(format!(
                    "{}: no longer identical to its kept copy, skipped",
                    name(p)
                )),
                Err(e) => out.errors.push(format!("{}: {e}", name(p))),
            }
        }
        if !sure.is_empty() {
            let r = to_recycle_bin(ctx, &sure, Some(root));
            out.count += r.count;
            out.errors.extend(r.errors);
        }
    }
    out
}

/// Streamed byte-for-byte comparison.
fn same_bytes(a: &Path, b: &Path) -> std::io::Result<bool> {
    let (mut fa, mut fb) = (File::open(a)?, File::open(b)?);
    if fa.metadata()?.len() != fb.metadata()?.len() {
        return Ok(false);
    }
    let (mut ba, mut bb) = (vec![0u8; PARTIAL], vec![0u8; PARTIAL]);
    loop {
        let (n, m) = (fill(&mut fa, &mut ba)?, fill(&mut fb, &mut bb)?);
        if ba[..n] != bb[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::os::windows::fs::OpenOptionsExt;
    use std::sync::Arc;

    use super::*;
    use crate::platform::{DriveKind, KnownFolder, Platform};

    /// Home and data dir are a temp folder; no known folders; nothing in these tests recycles.
    pub(crate) struct TestPlatform(pub PathBuf);

    impl Platform for TestPlatform {
        fn home(&self) -> PathBuf {
            self.0.clone()
        }
        fn known_folder(&self, _: KnownFolder) -> Option<PathBuf> {
            None
        }
        fn drive_kind(&self, _: &Path) -> DriveKind {
            DriveKind::Fixed
        }
        fn recycle(&self, _: &Path) -> std::io::Result<()> {
            Err(std::io::Error::other("tests never recycle"))
        }
        fn data_dir(&self) -> PathBuf {
            self.0.clone()
        }
        fn legacy_data_dir(&self) -> PathBuf {
            self.0.clone()
        }
    }

    pub(crate) fn ctx(dir: &Path) -> Ctx {
        Ctx::new(Arc::new(TestPlatform(dir.to_path_buf())))
    }

    /// Writes each (relative path, content, mtime) under `dir` and returns walk-style entries for them.
    pub(crate) fn entries(dir: &Path, files: &[(&str, &str, f64)]) -> Vec<Entry> {
        files
            .iter()
            .map(|&(rel, data, mtime)| {
                let path = dir.join(rel);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, data).unwrap();
                Entry {
                    path,
                    size: data.len() as u64,
                    mtime,
                    created: mtime,
                    attrs: 0,
                }
            })
            .collect()
    }

    fn names(g: &DupGroup) -> Vec<String> {
        g.files.iter().map(|f| name(Path::new(&f.path))).collect()
    }

    #[test]
    fn zero_length_files_group_only_when_min_size_is_zero() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        let es = entries(dir.path(), &[("a", "", 1.0), ("b", "", 2.0)]);
        assert!(find_duplicates(&ctx, &es, 1).is_empty());
        let gs = find_duplicates(&ctx, &es, 0);
        assert_eq!(gs.len(), 1);
        assert_eq!(names(&gs[0]), ["a", "b"]);
    }

    #[test]
    fn mtime_ties_break_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let es = entries(
            dir.path(),
            &[("b", "same", 5.0), ("a", "same", 5.0), ("c", "same", 4.0)],
        );
        assert_eq!(
            names(&find_duplicates(&ctx(dir.path()), &es, 1)[0]),
            ["c", "a", "b"]
        );
    }

    #[test]
    fn vanished_and_locked_files_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let mut es = entries(
            dir.path(),
            &[
                ("a", "same", 1.0),
                ("b", "same", 2.0),
                ("locked", "same", 3.0),
            ],
        );
        es.push(Entry {
            path: dir.path().join("gone"),
            size: 4,
            mtime: 0.0,
            created: 0.0,
            attrs: 0,
        });
        let _lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(dir.path().join("locked"))
            .unwrap();
        let gs = find_duplicates(&ctx(dir.path()), &es, 1);
        assert_eq!(gs.len(), 1);
        assert_eq!(names(&gs[0]), ["a", "b"]);
    }

    #[test]
    fn big_files_need_a_full_match_and_fill_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        let x = "A".repeat(300_000);
        let mut y = x.clone();
        y.replace_range(150_000..150_001, "B"); // same ends, different middle
        let es = entries(
            dir.path(),
            &[
                ("x", x.as_str(), 1.0),
                ("y", y.as_str(), 2.0),
                ("z", x.as_str(), 3.0),
            ],
        );
        for _ in 0..2 {
            // the second round is answered from the cache
            let gs = find_duplicates(&ctx, &es, 1);
            assert_eq!(gs.len(), 1);
            assert_eq!(names(&gs[0]), ["x", "z"]);
        }
        let mut cache = HashCache::load(&ctx);
        assert!(es
            .iter()
            .all(|e| cache.get(&e.path, e.size, e.mtime).is_some()));
    }

    #[test]
    fn cancel_returns_no_groups() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        let es = entries(dir.path(), &[("a", "same", 1.0), ("b", "same", 2.0)]);
        ctx.cancel();
        assert!(find_duplicates(&ctx, &es, 1).is_empty());
    }

    #[test]
    fn selecting_every_copy_is_refused_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        let es = entries(
            dir.path(),
            &[("One.bin", "same", 1.0), ("two.bin", "same", 2.0)],
        );
        let gs = find_duplicates(&ctx(dir.path()), &es, 1);
        let key = |n: &str| dir.path().join(n).to_string_lossy().into_owned();
        assert_eq!(
            check_dup_selection(&gs, &HashSet::from([key("one.BIN")])),
            Ok(())
        );
        assert_eq!(
            check_dup_selection(&gs, &HashSet::from([key("one.BIN"), key("TWO.bin")])),
            Err("All 2 copies of One.bin are selected. Keep at least one.".to_string())
        );
    }

    #[test]
    fn a_copy_that_changed_since_the_scan_is_not_recycled() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        let es = entries(
            dir.path(),
            &[("keep.bin", "AAAA", 1.0), ("copy.bin", "AAAA", 2.0)],
        );
        let gs = find_duplicates(&ctx, &es, 1);
        fs::write(&es[1].path, b"AAAB").unwrap();
        let r = trash_duplicates(
            &ctx,
            &[(gs[0].clone(), vec![es[1].path.clone()])],
            dir.path(),
        );
        assert_eq!(r.count, 0);
        assert_eq!(
            r.errors,
            ["copy.bin: no longer identical to its kept copy, skipped"]
        );
        assert!(es[1].path.exists());
    }
}
