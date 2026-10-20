//! Sort presets (new in 1.0). Owner: core-C. Built-ins "Downloads", "Photos", "Work" plus user-defined ones stored
//! in `<data_dir>/presets.json`. A preset is a `Classifier` for `sort::plan_sort_with`, so preview, collision,
//! recency, top-level-only and undo behaviour are identical to the built-in sort. Validate on save: folder names
//! are relative, at most two levels, no `..`, no reserved Windows names, no protected folder names; extensions
//! are lower case with the dot; an extension may appear in only one rule.

use crate::ctx::Ctx;
use crate::model::Preset;
use crate::walk::Entry;

pub fn builtin_presets() -> Vec<Preset> { todo!("core-C") }
pub fn list_presets(ctx: &Ctx) -> Vec<Preset> { todo!("core-C") }
pub fn save_preset(ctx: &Ctx, preset: &Preset) -> Result<Vec<Preset>, String> { todo!("core-C") }
pub fn delete_preset(ctx: &Ctx, id: &str) -> Result<Vec<Preset>, String> { todo!("core-C") }

/// Where a file goes under `preset`; None leaves it in place (same exclusions as the built-in classifier).
pub fn classify(preset: &Preset, entry: &Entry) -> Option<Vec<String>> { todo!("core-C") }
