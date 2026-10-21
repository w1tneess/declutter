//! The scan pipeline: one walk, then every analysis on its entries, each emitted as soon as it finishes in the order
//! docs/API.md fixes. The core calls go through `Engine`, so tests can stand in for them and check ordering and
//! cancel behaviour without the real analyses.

use std::path::{Path, PathBuf};

use declutter_core::model::{
    Breakdown, DupGroup, FileRef, Leftover, Redownload, ScanEvent, ScanOptions, Settings,
    SimilarGroup, SortPlan,
};
use declutter_core::sort::dto_path;
use declutter_core::walk::{Entry, Scan};
use declutter_core::{
    dupes, empty, large_old, leftovers, paths, presets, redownloads, settings, similar, sizes,
    sort, walk, Ctx,
};

/// Rows in each Large & Old breakdown table.
const BREAKDOWN_TOP: usize = 10;

/// The core operations a scan (and `--remind`) uses. Every method defaults to the real core; `Core` is that.
pub trait Engine: Send + Sync {
    fn require_root(&self, ctx: &Ctx, path: &Path) -> Result<PathBuf, String> {
        paths::require_root(ctx, path)
    }
    fn settings(&self, ctx: &Ctx) -> Settings {
        settings::load_settings(ctx)
    }
    fn walk(&self, ctx: &Ctx, root: &Path, ignore: &[PathBuf]) -> Scan {
        walk::walk_files(ctx, root, ignore)
    }
    fn plan_sort(
        &self,
        ctx: &Ctx,
        root: &Path,
        entries: &[Entry],
        recent_hours: u32,
        preset: Option<&str>,
    ) -> Result<SortPlan, String> {
        let Some(id) = preset else {
            return sort::plan_sort(ctx, root, entries, recent_hours);
        };
        let preset = presets::list_presets(ctx)
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| {
                "The chosen sort preset no longer exists. Pick another one in Settings.".to_string()
            })?;
        sort::plan_sort_with(ctx, root, entries, recent_hours, &|e: &Entry| {
            presets::classify(&preset, e)
        })
    }
    fn large_old(
        &self,
        ctx: &Ctx,
        entries: &[Entry],
        min_mb: f64,
        older_days: u32,
    ) -> Vec<FileRef> {
        large_old::find_large_old(ctx, entries, min_mb, older_days)
    }
    fn leftovers(&self, ctx: &Ctx, entries: &[Entry]) -> Vec<Leftover> {
        leftovers::find_leftovers(ctx, entries)
    }
    fn breakdown(&self, root: &Path, entries: &[Entry]) -> Breakdown {
        sizes::breakdown(root, entries, BREAKDOWN_TOP)
    }
    fn empty_dirs(
        &self,
        ctx: &Ctx,
        root: &Path,
        ignore: &[PathBuf],
    ) -> Result<Vec<PathBuf>, String> {
        empty::find_empty_dirs(ctx, root, ignore)
    }
    fn duplicates(&self, ctx: &Ctx, entries: &[Entry]) -> Vec<DupGroup> {
        // 1 byte: v0.2.2's default, so empty files are never "duplicates".
        dupes::find_duplicates(ctx, entries, 1)
    }
    fn redownloads(&self, entries: &[Entry], exclude: &[PathBuf]) -> Vec<Redownload> {
        redownloads::find_redownloads(entries, exclude)
    }
    fn similar(&self, ctx: &Ctx, entries: &[Entry]) -> Vec<SimilarGroup> {
        similar::find_similar(ctx, entries)
    }
}

/// The real core.
pub struct Core;
impl Engine for Core {}

/// Runs every phase on `root` (already validated). Returns `Ok(true)` when cancelled. A phase that finishes after
/// a cancel is dropped, since its result may be cut short; everything already emitted stays complete and valid.
pub(crate) fn pipeline(
    e: &dyn Engine,
    ctx: &Ctx,
    root: &Path,
    opts: &ScanOptions,
    ignore: &[PathBuf],
    emit: &dyn Fn(ScanEvent),
) -> Result<bool, String> {
    macro_rules! send {
        ($ev:expr) => {{
            let ev = $ev;
            if ctx.cancelled() {
                return Ok(true);
            }
            emit(ev);
        }};
    }
    let scan = e.walk(ctx, root, ignore);
    let entries = &scan.entries;
    send!(ScanEvent::Skipped {
        folders: scan.skipped.iter().map(|p| dto_path(p.as_path())).collect()
    });
    send!(ScanEvent::Sort {
        plan: e.plan_sort(
            ctx,
            root,
            entries,
            opts.recent_hours,
            opts.preset_id.as_deref()
        )?
    });
    send!(ScanEvent::LargeOld {
        files: e.large_old(ctx, entries, opts.min_mb, opts.older_days)
    });
    send!(ScanEvent::Leftovers {
        items: e.leftovers(ctx, entries)
    });
    send!(ScanEvent::Breakdown {
        breakdown: e.breakdown(root, entries)
    });
    send!(ScanEvent::EmptyDirs {
        dirs: e
            .empty_dirs(ctx, root, ignore)?
            .iter()
            .map(|p| dto_path(p.as_path()))
            .collect()
    });
    let groups = e.duplicates(ctx, entries);
    // Re-downloads leave out anything already listed as an exact copy (as v0.2.2 did).
    let exact: Vec<PathBuf> = groups
        .iter()
        .flat_map(|g| g.files.iter().map(|f| PathBuf::from(&f.path)))
        .collect();
    send!(ScanEvent::Duplicates { groups });
    send!(ScanEvent::Redownloads {
        pairs: e.redownloads(entries, &exact)
    });
    if opts.similar_images {
        send!(ScanEvent::Similar {
            groups: e.similar(ctx, entries)
        });
    }
    Ok(false)
}
