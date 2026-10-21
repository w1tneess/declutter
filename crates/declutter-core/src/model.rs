//! Data types shared by the core, the Tauri shell and the UI.
//!
//! Every type here is serialised as camelCase JSON and exported to TypeScript (`ui/src/api/generated/`) by
//! `cargo test -p declutter-core export_bindings`. Paths are absolute Windows paths as UTF-8 strings.
//! Times are Unix seconds (f64). Sizes are bytes. Change a type here and regenerate; never hand-edit the
//! generated `.ts`. See docs/API.md for how these map onto Tauri commands and events.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! dto {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $f:ident : $t:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
        #[serde(rename_all = "camelCase")]
        #[ts(export)]
        pub struct $name { $($(#[$fm])* pub $f : $t),* }
    };
}

dto! {
    /// Result of checking a folder the user picked (`paths::check_root`).
    pub struct FolderCheck { pub ok: bool, pub reason: String }
}

dto! {
    /// Downloads, Desktop and Documents as the shell reports them (they can be moved or redirected).
    pub struct QuickFolders { pub downloads: String, pub desktop: String, pub documents: String }
}

dto! {
    pub struct FileRef { pub path: String, #[ts(type = "number")] pub size: u64, pub mtime: f64 }
}

dto! {
    /// One planned move. `category` is "Images" or "Images/Screenshots".
    pub struct Move { pub src: String, pub dst: String, #[ts(type = "number")] pub size: u64, pub category: String }
}

dto! {
    pub struct SortPlan {
        pub moves: Vec<Move>,
        /// Files left alone because they were touched within the "recent hours" setting.
        pub recent_left: u32,
    }
}

dto! {
    /// Identical files. `files[0]` is the suggested keeper (oldest modified).
    pub struct DupGroup { #[ts(type = "number")] pub size: u64, pub files: Vec<FileRef> }
}

dto! {
    /// `name (1).ext` / `name - Copy.ext` beside the original.
    pub struct Redownload { pub original: FileRef, pub copy: FileRef }
}

dto! {
    /// A file that is probably no longer needed. Never preselected, never called "safe".
    pub struct Leftover { pub path: String, #[ts(type = "number")] pub size: u64, pub mtime: f64, pub reason: String }
}

dto! {
    pub struct SimilarFile {
        pub path: String, #[ts(type = "number")] pub size: u64, pub mtime: f64, pub width: u32, pub height: u32,
        /// Hamming distance to the group's first file (0 = identical hash).
        pub distance: u32,
    }
}

dto! {
    /// Visually similar images. Never preselected; the user keeps at least one.
    pub struct SimilarGroup { pub files: Vec<SimilarFile> }
}

dto! {
    /// A row in the folder-size or file-type breakdown. `name` is a path (folders) or a category (types).
    pub struct SizeEntry { pub name: String, #[ts(type = "number")] pub bytes: u64, #[ts(type = "number")] pub files: u64 }
}

dto! {
    pub struct Breakdown { pub folders: Vec<SizeEntry>, pub types: Vec<SizeEntry> }
}

dto! {
    /// Outcome of any action. One failure never aborts a batch: `errors` lists what was skipped and why.
    pub struct ActionResult { pub count: u32, pub errors: Vec<String> }
}

dto! {
    /// What "Undo last sort" would reverse.
    pub struct UndoInfo { pub folder: String, pub count: u32, pub age_seconds: Option<f64> }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Phase {
    Walking,
    Comparing,
    Hashing,
    Similar,
}

dto! {
    pub struct Progress {
        pub phase: Phase,
        #[ts(type = "number")] pub done: u64,
        /// None while the total is unknown (the walk). Some(n) once it is, so the bar can be determinate.
        #[ts(type = "number | null")] pub total: Option<u64>,
        #[ts(type = "number")] pub bytes: u64,
        pub eta_secs: Option<f64>,
        /// Plain-language status line, e.g. "Checking 14 possible duplicates in full".
        pub label: String,
    }
}

dto! {
    pub struct ScanOptions {
        pub min_mb: f64,
        pub older_days: u32,
        /// 0..=72; 0 = off.
        pub recent_hours: u32,
        pub similar_images: bool,
        /// Pinned ("never touch") files and folders.
        pub ignore: Vec<String>,
        /// Sort preset to use instead of the built-in categories.
        pub preset_id: Option<String>,
    }
}

/// Streamed from a scan. Results arrive as soon as each phase finishes, so Sort and Large & Old are on screen
/// while duplicates are still being hashed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum ScanEvent {
    Progress {
        progress: Progress,
    },
    Sort {
        plan: SortPlan,
    },
    LargeOld {
        files: Vec<FileRef>,
    },
    Leftovers {
        items: Vec<Leftover>,
    },
    EmptyDirs {
        dirs: Vec<String>,
    },
    Skipped {
        folders: Vec<String>,
    },
    Breakdown {
        breakdown: Breakdown,
    },
    Duplicates {
        groups: Vec<DupGroup>,
    },
    Redownloads {
        pairs: Vec<Redownload>,
    },
    Similar {
        groups: Vec<SimilarGroup>,
    },
    #[serde(rename_all = "camelCase")]
    Done {
        cancelled: bool,
        #[ts(type = "number")]
        took_ms: u64,
    },
    Error {
        message: String,
    },
}

dto! {
    /// One sort-preset rule: files with any of these extensions (lower case, with the dot) go to `folder`
    /// (relative to the chosen folder, at most two levels).
    pub struct PresetRule { pub extensions: Vec<String>, pub folder: String }
}

dto! {
    pub struct Preset { pub id: String, pub name: String, pub builtin: bool, pub rules: Vec<PresetRule> }
}

dto! {
    /// Persisted in `%LOCALAPPDATA%\Declutter\settings.json`. Must stay readable by v0.2.2 files.
    pub struct Settings {
        /// "system" | "light" | "dark"
        pub theme: String,
        pub recent_hours: u32,
        pub min_mb: f64,
        pub older_days: u32,
        pub last_folder: Option<String>,
        pub remember_last_folder: bool,
        pub pins: Vec<String>,
        pub active_preset: Option<String>,
    }
}

dto! {
    /// Local tally of what the user sent to the Recycle Bin through Declutter. Never leaves the machine.
    pub struct SpaceFreed { #[ts(type = "number")] pub bytes: u64, #[ts(type = "number")] pub files: u64, pub since: Option<String> }
}

dto! {
    pub struct ReminderStatus { pub enabled: bool, pub next_run: Option<String> }
}

dto! {
    pub struct BinInfo { #[ts(type = "number")] pub bytes: u64, #[ts(type = "number")] pub items: u64 }
}

dto! {
    /// The copies of one duplicate group the user chose to remove (the group is sent back so the shell can verify
    /// a kept copy still exists right now).
    pub struct DupRemoval { pub group: DupGroup, pub remove: Vec<String> }
}

dto! {
    pub struct SimilarRemoval { pub group: SimilarGroup, pub remove: Vec<String> }
}

dto! {
    pub struct AppInfo { pub version: String }
}

/// What `export_csv` writes. The UI sends the rows it is showing; nothing is re-scanned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ExportPayload {
    Sort { moves: Vec<Move> },
    Duplicates { groups: Vec<DupGroup> },
    LargeOld { files: Vec<FileRef> },
    Leftovers { items: Vec<Leftover> },
    Redownloads { pairs: Vec<Redownload> },
    Similar { groups: Vec<SimilarGroup> },
    EmptyDirs { dirs: Vec<String> },
}
