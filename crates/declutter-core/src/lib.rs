//! Declutter core: scanning, planning and the safe actions (Recycle Bin, undoable sort).
//!
//! No UI or Tauri code lives here. Behaviour is specified by `docs/CONTRACT.md` and proven by the golden
//! fixtures in `tests/golden/`, which are generated from the Python implementation. Read
//! `docs/ARCHITECTURE.md` for module ownership and conventions before changing anything.

// Stubs carry `todo!()` bodies until their owner lands them; the integration step removes this allow.
#![allow(unused_variables, dead_code)]

pub mod ctx;
pub mod model;
pub mod platform;

// foundation (core-A)
pub mod empty;
pub mod fmt;
pub mod paths;
pub mod sort;
pub mod trash;
pub mod undo;
pub mod walk;

// analysis (core-B)
pub mod dupes;
pub mod hashcache;
pub mod large_old;
pub mod leftovers;
pub mod redownloads;

// extras and new features (core-C)
pub mod export;
pub mod presets;
pub mod reminder;
pub mod settings;
pub mod similar;
pub mod sizes;

pub use ctx::Ctx;
