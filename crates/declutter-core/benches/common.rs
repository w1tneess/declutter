//! Shared by the benches (owner: release): a platform that touches nothing real, and the tree to measure.
//! `DECLUTTER_BENCH_TREE` names a tree made by `tools/gen_tree.py`; without it a small one is generated in a temp dir.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use declutter_core::platform::{DriveKind, KnownFolder, Platform};
use declutter_core::Ctx;
use tempfile::TempDir;

struct BenchPlatform {
    data: PathBuf,
}

impl Platform for BenchPlatform {
    fn home(&self) -> PathBuf {
        self.data.clone()
    }
    fn known_folder(&self, _which: KnownFolder) -> Option<PathBuf> {
        None
    }
    fn drive_kind(&self, _path: &Path) -> DriveKind {
        DriveKind::Fixed
    }
    fn recycle(&self, _path: &Path) -> std::io::Result<()> {
        Err(std::io::Error::other("benches never delete"))
    }
    fn data_dir(&self) -> PathBuf {
        self.data.clone()
    }
    fn legacy_data_dir(&self) -> PathBuf {
        self.data.join("legacy")
    }
}

/// A context whose data dir (hash cache) is `data`: use a fresh dir for a cold cache, the same one for a warm cache.
pub fn ctx(data: &Path) -> Ctx {
    Ctx::new(Arc::new(BenchPlatform { data: data.to_path_buf() }))
}

pub struct Tree {
    pub root: PathBuf,
    _temp: Option<TempDir>,
}

impl Tree {
    pub fn open() -> Self {
        if let Some(root) = std::env::var_os("DECLUTTER_BENCH_TREE") {
            return Self { root: PathBuf::from(root), _temp: None };
        }
        let temp = TempDir::new().expect("temp dir");
        // Old enough that the walk's "leave recently touched files alone" rule keeps them.
        let old = SystemTime::now() - Duration::from_secs(30 * 86_400);
        for i in 0..3000u64 {
            let dir = temp.path().join(format!("d{}", i % 60 / 10)).join(format!("s{}", i % 60));
            fs::create_dir_all(&dir).expect("create dir");
            // Every 20th file is an exact copy of its predecessor; sizes also collide across different content.
            let seed = if i % 20 == 19 { i - 1 } else { i };
            let path = dir.join(format!("f{i:05}.bin"));
            fs::write(&path, content(seed)).expect("write file");
            OpenOptions::new().write(true).open(&path).and_then(|f| f.set_modified(old)).expect("set mtime");
        }
        Self { root: temp.path().to_path_buf(), _temp: Some(temp) }
    }
}

fn content(seed: u64) -> Vec<u8> {
    let len = 512 + (seed % 7) * 1024 + (seed % 13) * 37;
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}
