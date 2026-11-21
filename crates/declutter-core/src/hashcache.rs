//! Persistent hash cache keyed by (path, size, mtime) so a rescan after an action does not re-hash unchanged
//! files. Owner: core-B. Lives in `<data_dir>/hashcache.bin`, tolerates corruption (treated as empty), is written
//! atomically, and is never trusted for a file whose size or mtime changed.
//!
//! Binary rather than JSON: mtimes must round-trip bit for bit (serde_json does not guarantee that for f64 without
//! an extra feature), and a 100k-entry cache loads in one pass with no parsing beyond lengths.
//! Layout: MAGIC, then records of `u16 path_len, path (UTF-8, lower-cased), u64 size, u64 mtime bits,
//! u64 last_used (Unix secs), [u8; 32] sha256`, all little-endian.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::ctx::Ctx;
use crate::dupes::path_key;
use crate::settings::write_atomic;

const FILE: &str = "hashcache.bin";
/// Format and version tag. Anything else (another version, a damaged or cut-off file) loads as an empty cache.
const MAGIC: &[u8; 8] = b"DCHASH01";
/// Plenty for a big Downloads folder; about 100 bytes each on disk.
const MAX_ENTRIES: usize = 100_000;

struct Rec {
    size: u64,
    mtime: u64,
    used: u64,
    sha: [u8; 32],
}

pub struct HashCache {
    map: HashMap<String, Rec>,
    now: u64,
    dirty: bool,
}

impl HashCache {
    pub fn load(ctx: &Ctx) -> Self {
        let map = fs::read(ctx.platform.data_dir().join(FILE))
            .ok()
            .and_then(|b| parse(&b))
            .unwrap_or_default();
        Self {
            map,
            now: ctx.now() as u64,
            dirty: false,
        }
    }

    /// The stored hash, only if the file's size and mtime are exactly what they were when it was hashed.
    pub fn get(&mut self, path: &Path, size: u64, mtime: f64) -> Option<[u8; 32]> {
        let r = self
            .map
            .get_mut(&path_key(path))
            .filter(|r| r.size == size && r.mtime == mtime.to_bits())?;
        if r.used != self.now {
            r.used = self.now;
            self.dirty = true;
        }
        Some(r.sha)
    }

    pub fn put(&mut self, path: &Path, size: u64, mtime: f64, sha256: [u8; 32]) {
        self.map.insert(
            path_key(path),
            Rec {
                size,
                mtime: mtime.to_bits(),
                used: self.now,
                sha: sha256,
            },
        );
        self.dirty = true;
    }

    /// Temp file + rename, so a crash never leaves a half-written cache.
    pub fn save(&self, ctx: &Ctx) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        let mut buf = MAGIC.to_vec();
        for (k, r) in self.kept(MAX_ENTRIES) {
            let Ok(len) = u16::try_from(k.len()) else {
                continue;
            };
            buf.extend_from_slice(&len.to_le_bytes());
            buf.extend_from_slice(k.as_bytes());
            for v in [r.size, r.mtime, r.used] {
                buf.extend_from_slice(&v.to_le_bytes());
            }
            buf.extend_from_slice(&r.sha);
        }
        write_atomic(&ctx.platform.data_dir().join(FILE), &buf).map_err(|e| e.to_string())
    }

    /// Everything while under `cap`; over it, entries whose file still exists, most recently used first. Existence
    /// is only checked then, so a normal save costs no disk access per entry.
    fn kept(&self, cap: usize) -> Vec<(&String, &Rec)> {
        let mut v: Vec<_> = self.map.iter().collect();
        if v.len() > cap {
            v.retain(|(k, _)| Path::new(k).exists());
            v.sort_by_key(|(_, r)| std::cmp::Reverse(r.used));
            v.truncate(cap);
        }
        v
    }
}

fn take<'a>(b: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    let (head, tail) = b.split_at_checked(n)?;
    *b = tail;
    Some(head)
}

fn parse(mut b: &[u8]) -> Option<HashMap<String, Rec>> {
    if take(&mut b, MAGIC.len())? != MAGIC {
        return None;
    }
    let mut map = HashMap::new();
    while !b.is_empty() {
        let len = u16::from_le_bytes(take(&mut b, 2)?.try_into().ok()?);
        let k = std::str::from_utf8(take(&mut b, usize::from(len))?)
            .ok()?
            .to_owned();
        let mut num = || {
            take(&mut b, 8)
                .and_then(|s| s.try_into().ok())
                .map(u64::from_le_bytes)
        };
        let (size, mtime, used) = (num()?, num()?, num()?);
        let sha = take(&mut b, 32)?.try_into().ok()?;
        map.insert(
            k,
            Rec {
                size,
                mtime,
                used,
                sha,
            },
        );
    }
    Some(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dupes::tests::ctx;

    #[test]
    fn round_trip_and_invalidation() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        let p = Path::new(r"C:\Data\A.bin");
        let mut c = HashCache::load(&ctx);
        assert_eq!(c.get(p, 5, 1.5), None);
        c.put(p, 5, 1.5, [7; 32]);
        c.save(&ctx).unwrap();
        let mut c = HashCache::load(&ctx);
        assert_eq!(c.get(Path::new(r"c:\data\a.bin"), 5, 1.5), Some([7; 32]));
        assert_eq!(c.get(p, 6, 1.5), None, "size changed");
        assert_eq!(c.get(p, 5, 1.25), None, "mtime changed");
    }

    #[test]
    fn damaged_file_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        let p = Path::new(r"C:\Data\A.bin");
        let mut c = HashCache::load(&ctx);
        c.put(p, 5, 1.5, [7; 32]);
        c.save(&ctx).unwrap();
        let file = dir.path().join(FILE);
        let good = fs::read(&file).unwrap();
        for bad in [
            good[..good.len() - 1].to_vec(),
            b"garbage".to_vec(),
            Vec::new(),
        ] {
            fs::write(&file, &bad).unwrap();
            assert_eq!(HashCache::load(&ctx).get(p, 5, 1.5), None);
        }
    }

    #[test]
    fn over_the_cap_keeps_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx(dir.path());
        let real = dir.path().join("real.bin");
        fs::write(&real, b"x").unwrap();
        let mut c = HashCache::load(&ctx);
        c.put(Path::new(r"C:\gone\a.bin"), 1, 1.0, [1; 32]);
        c.put(&real, 1, 1.0, [2; 32]);
        let kept: Vec<String> = c.kept(1).into_iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(kept, [path_key(&real)]);
    }
}
