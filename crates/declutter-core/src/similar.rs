//! Similar-image review (new in 1.0). Owner: core-C. Perceptual hash (e.g. `image_hasher` dHash/pHash) over
//! common image types, grouped by Hamming distance. Never preselected; same keep-one rule as duplicates (the user
//! must keep at least one per group); Recycle Bin only. Exact duplicates (identical bytes) belong to `dupes`, not here.
//! Must be cancellable, report `Phase::Similar` progress, skip online-only and hidden files (already excluded by
//! the walk), and never panic on a corrupt image (skip it).
//!
//! Hash: `image_hasher`'s Gradient algorithm (dHash) on an 8x8 grid, 64 bits. It survives resizing, recompression
//! and brightness changes, and is the cheapest of the robust options (DCT/pHash costs 50-100 % more per image).
//! Grouping: union-find over pairs within `MAX_DISTANCE`, so a chain A~B~C forms one group and `distance` (always
//! relative to the group's first file) can exceed the threshold for the far end of a chain.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use image::{ImageReader, Limits};
use image_hasher::HasherConfig;
use rayon::prelude::*;

use crate::ctx::Ctx;
use crate::model::{Phase, Progress, SimilarFile, SimilarGroup};
use crate::walk::Entry;

/// 6 of 64 bits: in dHash practice 0-5 is "same picture, resized or recompressed", 6-10 starts catching merely
/// alike shots (burst photos), above that unrelated pictures.
const MAX_DISTANCE: u32 = 6;
const EXTS: [&str; 9] = [
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff", "ico",
];
/// Bigger files are almost certainly not photos worth comparing, and decoding them costs too much memory.
const MAX_FILE_BYTES: u64 = 256 << 20;
const MAX_SIDE: u32 = 20_000;
/// Per decode; with one decode per rayon thread this bounds the scan's memory to threads x 256 MiB.
const MAX_ALLOC: u64 = 256 << 20;
/// The hasher shrinks to 9x8 with Lanczos3; doing that from a full photo costs more than the decode, so a cheap
/// box-filter thumbnail goes first.
const HASH_INPUT: u32 = 64;
const THROTTLE: Duration = Duration::from_millis(50);

struct Hashed<'a> {
    entry: &'a Entry,
    hash: u64,
    width: u32,
    height: u32,
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// (hash, width, height), or None for anything unreadable, corrupt, too big or that makes a decoder panic.
fn hash_file(config: &HasherConfig<[u8; 8]>, path: &Path) -> Option<(u64, u32, u32)> {
    catch_unwind(AssertUnwindSafe(|| {
        let mut reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        limits.max_alloc = Some(MAX_ALLOC);
        reader.limits(limits);
        let img = reader.decode().ok()?;
        let (width, height) = (img.width(), img.height());
        let img = if width > HASH_INPUT || height > HASH_INPUT {
            img.thumbnail(HASH_INPUT, HASH_INPUT)
        } else {
            img
        };
        let hash = config.to_hasher().hash_image(&img);
        Some((u64::from_le_bytes(hash.into_inner()), width, height))
    }))
    .ok()
    .flatten()
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// Component root for each of `hashes` (distinct values) when hashes within `d` bits are linked, plus the
/// number of distance checks made.
///
/// Not all pairs: by pigeonhole, two hashes differing in at most d bits agree exactly on at least one of d + 1
/// disjoint bit blocks, so only hashes sharing a block value are compared. For d = 6 the blocks are 9-10 bits wide,
/// so n spread-out hashes cost about 7 n^2 / 1024 checks instead of n^2 / 2.
fn components(hashes: &[u64], d: u32) -> (Vec<usize>, u64) {
    let blocks = d + 1;
    let mut parent: Vec<usize> = (0..hashes.len()).collect();
    let mut checks = 0u64;
    for b in 0..blocks {
        let (lo, hi) = (b * 64 / blocks, (b + 1) * 64 / blocks);
        let mask = if hi - lo == 64 {
            u64::MAX
        } else {
            (1u64 << (hi - lo)) - 1
        };
        let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
        for (i, h) in hashes.iter().enumerate() {
            let bucket = buckets.entry((h >> lo) & mask).or_default();
            for &j in bucket.iter() {
                checks += 1;
                if (h ^ hashes[j]).count_ones() <= d {
                    let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                    parent[ri] = rj;
                }
            }
            bucket.push(i);
        }
    }
    let roots = (0..hashes.len()).map(|i| find(&mut parent, i)).collect();
    (roots, checks)
}

fn progress(done: u64, total: u64, bytes: u64, start: Instant) -> Progress {
    let secs = start.elapsed().as_secs_f64();
    let eta_secs = (done > 0).then(|| secs / done as f64 * total.saturating_sub(done) as f64);
    Progress {
        phase: Phase::Similar,
        done,
        total: Some(total),
        bytes,
        eta_secs,
        label: format!("Comparing pictures ({done} of {total})"),
    }
}

/// Groups pictures within `MAX_DISTANCE` bits of each other. A cancel keeps the groups found among the pictures hashed so far; each of them is still a real match.
pub fn find_similar(ctx: &Ctx, entries: &[Entry]) -> Vec<SimilarGroup> {
    let todo: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.size > 0 && e.size <= MAX_FILE_BYTES && is_image(&e.path))
        .collect();
    let total = todo.len() as u64;
    let (done, bytes) = (AtomicU64::new(0), AtomicU64::new(0));
    let start = Instant::now();
    let last_report = Mutex::new(start);
    let config = HasherConfig::with_bytes_type::<[u8; 8]>();

    let hashed: Vec<Hashed> = todo
        .par_iter()
        .map(|&entry| {
            if ctx.cancelled() {
                return None;
            }
            let out = hash_file(&config, &entry.path).map(|(hash, width, height)| Hashed {
                entry,
                hash,
                width,
                height,
            });
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            let b = bytes.fetch_add(entry.size, Ordering::Relaxed) + entry.size;
            // try_lock: a thread that finds another one reporting just carries on.
            if let Ok(mut last) = last_report.try_lock() {
                if last.elapsed() >= THROTTLE {
                    *last = Instant::now();
                    ctx.report(progress(n, total, b, start));
                }
            }
            out
        })
        .flatten()
        .collect();
    ctx.report(progress(
        done.load(Ordering::Relaxed),
        total,
        bytes.load(Ordering::Relaxed),
        start,
    ));

    let mut uniq: Vec<u64> = hashed.iter().map(|h| h.hash).collect();
    uniq.sort_unstable();
    uniq.dedup();
    let (roots, _) = components(&uniq, MAX_DISTANCE);
    let mut by_root: HashMap<usize, Vec<&Hashed>> = HashMap::new();
    for h in &hashed {
        if let Ok(i) = uniq.binary_search(&h.hash) {
            by_root.entry(roots[i]).or_default().push(h);
        }
    }

    let mut groups: Vec<(u64, SimilarGroup)> = by_root
        .into_values()
        .filter(|members| members.len() >= 2)
        .map(|mut members| {
            // Largest picture first: the natural one to keep, and the reference for `distance`.
            members.sort_by(|a, b| {
                let pixels = |h: &Hashed| u64::from(h.width) * u64::from(h.height);
                pixels(b)
                    .cmp(&pixels(a))
                    .then(b.entry.size.cmp(&a.entry.size))
                    .then_with(|| a.entry.path.cmp(&b.entry.path))
            });
            let first = members[0].hash; // every group has at least two members (filter above)
            let reclaimable = members
                .iter()
                .skip(1)
                .fold(0u64, |s, h| s.saturating_add(h.entry.size));
            let files = members
                .iter()
                .map(|h| SimilarFile {
                    path: h.entry.path.to_string_lossy().into_owned(),
                    size: h.entry.size,
                    mtime: h.entry.mtime,
                    width: h.width,
                    height: h.height,
                    distance: (h.hash ^ first).count_ones(),
                })
                .collect();
            (reclaimable, SimilarGroup { files })
        })
        .collect();
    groups.sort_by(|(ra, a), (rb, b)| {
        rb.cmp(ra)
            .then_with(|| a.files[0].path.cmp(&b.files[0].path))
    });
    groups.into_iter().map(|(_, g)| g).collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use super::*;
    use crate::settings::test_support::ctx;

    /// The same smooth picture at any resolution (coordinates are normalised), optionally brightened or inverted.
    fn picture(w: u32, h: u32, brighten: u8, negative: bool) -> image::RgbImage {
        image::RgbImage::from_fn(w, h, |x, y| {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let v = 128.0 + 100.0 * (fx * 9.0).sin() * (fy * 5.0 + fx * 3.0).cos();
            let v = if negative { 255.0 - v } else { v };
            let v = v as u8 + brighten;
            image::Rgb([v, v / 2, 255 - v])
        })
    }

    fn entry(path: &Path) -> Entry {
        let size = fs::metadata(path).unwrap().len();
        Entry {
            path: path.to_path_buf(),
            size,
            mtime: 0.0,
            created: 0.0,
            attrs: 0,
        }
    }

    /// Six candidate images (four alike, one negative, one corrupt) plus two files that are not candidates.
    fn fixture(dir: &Path) -> Vec<Entry> {
        picture(300, 200, 0, false).save(dir.join("a.png")).unwrap();
        picture(300, 200, 10, false)
            .save(dir.join("b_bright.png"))
            .unwrap();
        picture(150, 100, 0, false)
            .save(dir.join("c_small.bmp"))
            .unwrap();
        fs::copy(dir.join("a.png"), dir.join("d_copy.PNG")).unwrap();
        picture(300, 200, 0, true)
            .save(dir.join("e_negative.png"))
            .unwrap();
        fs::write(dir.join("f_broken.jpg"), b"not really a jpeg").unwrap();
        fs::write(dir.join("g_empty.png"), b"").unwrap();
        fs::write(dir.join("notes.txt"), b"hello").unwrap();
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        names.sort();
        names.iter().map(|p| entry(p)).collect()
    }

    fn name(f: &SimilarFile) -> String {
        Path::new(&f.path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn groups_near_copies_and_skips_the_rest() {
        let t = tempfile::tempdir().unwrap();
        let entries = fixture(t.path());
        let groups = find_similar(&ctx(t.path()), &entries);
        assert_eq!(groups.len(), 1, "{groups:?}");
        let files = &groups[0].files;
        let mut names: Vec<String> = files.iter().map(name).collect();
        names.sort();
        assert_eq!(
            names,
            ["a.png", "b_bright.png", "c_small.bmp", "d_copy.PNG"]
        );
        assert_eq!(
            (files[0].width, files[0].height, files[0].distance),
            (300, 200, 0)
        );
        let small = files.iter().find(|f| name(f) == "c_small.bmp").unwrap();
        assert_eq!((small.width, small.height), (150, 100));
        assert!(
            files.iter().all(|f| f.distance <= MAX_DISTANCE),
            "{files:?}"
        );
        let copy = files.iter().find(|f| name(f) == "d_copy.PNG").unwrap();
        let a = files.iter().find(|f| name(f) == "a.png").unwrap();
        assert_eq!(
            copy.distance, a.distance,
            "byte-identical files hash the same"
        );
    }

    #[test]
    fn reports_progress_and_honours_cancel() {
        let t = tempfile::tempdir().unwrap();
        let entries = fixture(t.path());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let c = ctx(t.path()).with_progress(move |p| sink.lock().unwrap().push(p));
        find_similar(&c, &entries);
        let seen = seen.lock().unwrap();
        let last = seen.last().expect("a final report");
        assert_eq!(
            (last.phase, last.done, last.total),
            (Phase::Similar, 6, Some(6))
        );
        assert!(seen
            .iter()
            .all(|p| p.phase == Phase::Similar && p.done <= 6));

        let c = ctx(t.path());
        c.cancel();
        assert!(find_similar(&c, &entries).is_empty());
    }

    #[test]
    fn nothing_to_compare_is_fine() {
        let t = tempfile::tempdir().unwrap();
        assert!(find_similar(&ctx(t.path()), &[]).is_empty());
        assert!(find_similar(&ctx(t.path()), &[entry_missing()]).is_empty());
    }

    fn entry_missing() -> Entry {
        Entry {
            path: "C:\\nowhere\\gone.jpg".into(),
            size: 10,
            mtime: 0.0,
            created: 0.0,
            attrs: 0,
        }
    }

    /// Deterministic xorshift so the tests need no extra crate.
    fn rng(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }

    fn canonical(roots: &[usize]) -> Vec<usize> {
        // Map each element to the smallest index in its component, so two partitions compare equal.
        let mut first: HashMap<usize, usize> = HashMap::new();
        roots
            .iter()
            .enumerate()
            .map(|(i, r)| *first.entry(*r).or_insert(i))
            .collect()
    }

    #[test]
    fn bucketed_grouping_matches_all_pairs() {
        let mut seed = 0x9E37_79B9_7F4A_7C15;
        let mut hashes = Vec::new();
        for _ in 0..300 {
            let base = rng(&mut seed);
            hashes.push(base);
            for k in 0..4 {
                let mut h = base;
                for _ in 0..(k * 3) {
                    h ^= 1 << (rng(&mut seed) % 64);
                }
                hashes.push(h);
            }
        }
        hashes.sort_unstable();
        hashes.dedup();
        for d in [0, 1, 3, 6, 10, 16] {
            let mut parent: Vec<usize> = (0..hashes.len()).collect();
            for i in 0..hashes.len() {
                for j in 0..i {
                    if (hashes[i] ^ hashes[j]).count_ones() <= d {
                        let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                        parent[ri] = rj;
                    }
                }
            }
            let brute: Vec<usize> = (0..hashes.len()).map(|i| find(&mut parent, i)).collect();
            assert_eq!(
                canonical(&components(&hashes, d).0),
                canonical(&brute),
                "d = {d}"
            );
        }
    }

    #[test]
    fn grouping_is_far_from_quadratic() {
        let mut seed = 42;
        let n = 20_000u64;
        let hashes: Vec<u64> = (0..n).map(|_| rng(&mut seed)).collect();
        let (_, checks) = components(&hashes, MAX_DISTANCE);
        // All pairs would be n^2 / 2 = 2e8; the block index needs about 7 n^2 / 1024 = 2.7e6.
        assert!(checks < n * n / 50, "{checks} checks");
    }
}
