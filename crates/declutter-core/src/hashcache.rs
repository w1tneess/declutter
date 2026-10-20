//! Persistent hash cache keyed by (path, size, mtime) so a rescan after an action does not re-hash unchanged
//! files. Owner: core-B. Lives in `<data_dir>/hashcache.bin` (or similar), must tolerate corruption (treat as empty),
//! be written atomically, and never be trusted for a file whose size or mtime changed.

use std::path::Path;

use crate::ctx::Ctx;

pub struct HashCache { /* core-B */ }

impl HashCache {
    pub fn load(ctx: &Ctx) -> Self { todo!("core-B") }
    pub fn get(&self, path: &Path, size: u64, mtime: f64) -> Option<[u8; 32]> { todo!("core-B") }
    pub fn put(&mut self, path: &Path, size: u64, mtime: f64, sha256: [u8; 32]) { todo!("core-B") }
    pub fn save(&self, ctx: &Ctx) -> Result<(), String> { todo!("core-B") }
}
