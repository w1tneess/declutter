//! Persistent user data. Owner: core-C. `<data_dir>/settings.json` must stay readable from v0.2.2 files (read
//! `src/declutter/theme.py` for the existing keys and clamping, `recent_hours_setting` in core.py) and tolerate
//! junk (fall back to defaults field by field). Writes are atomic and merge, never clobber unknown keys.
//! Also the local "space freed" tally (`<data_dir>/stats.json`) and the preset store (`presets.rs` uses it).

use crate::ctx::Ctx;
use crate::model::{Settings, SpaceFreed};

pub fn load_settings(ctx: &Ctx) -> Settings { todo!("core-C") }
pub fn save_settings(ctx: &Ctx, settings: &Settings) -> Result<(), String> { todo!("core-C") }

pub fn add_pin(ctx: &Ctx, path: &str) -> Result<Settings, String> { todo!("core-C") }
pub fn remove_pin(ctx: &Ctx, path: &str) -> Result<Settings, String> { todo!("core-C") }

pub fn space_freed(ctx: &Ctx) -> SpaceFreed { todo!("core-C") }
/// Add only what was actually sent to the Recycle Bin.
pub fn record_freed(ctx: &Ctx, bytes: u64, files: u64) -> Result<SpaceFreed, String> { todo!("core-C") }
pub fn reset_freed(ctx: &Ctx) -> Result<SpaceFreed, String> { todo!("core-C") }
