//! The seam between the core and Windows. Everything the golden fixtures patch in the Python oracle
//! (`_drive_type`, `_known_folder_path`, `send2trash`, `LOG_DIR`, `time.time`) goes through this trait, so
//! tests inject a fake and production uses `SystemPlatform`.
//!
//! Owner: agent core-A.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownFolder { Downloads, Desktop, Documents }

/// `GetDriveTypeW` collapsed. Removals are allowed on `Fixed` only (SPEC rule 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind { Fixed, Removable, Network, Other }

pub trait Platform: Send + Sync {
    /// Unix seconds. Injected so tests control "now".
    fn now(&self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
    fn home(&self) -> PathBuf;
    /// Real location of Downloads/Desktop/Documents (`SHGetKnownFolderPath`); None if the shell cannot say.
    fn known_folder(&self, which: KnownFolder) -> Option<PathBuf>;
    fn drive_kind(&self, path: &Path) -> DriveKind;
    /// Send one file or folder to the Recycle Bin. Never delete permanently.
    fn recycle(&self, path: &Path) -> std::io::Result<()>;
    /// `%LOCALAPPDATA%\Declutter`
    fn data_dir(&self) -> PathBuf;
    /// `%APPDATA%\Declutter`, where v0.1 kept its files.
    fn legacy_data_dir(&self) -> PathBuf;
}

/// The real thing. Windows only.
pub struct SystemPlatform;

impl Platform for SystemPlatform {
    fn home(&self) -> PathBuf { todo!("core-A: USERPROFILE / SHGetKnownFolderPath(Profile)") }
    fn known_folder(&self, which: KnownFolder) -> Option<PathBuf> { todo!("core-A: SHGetKnownFolderPath") }
    fn drive_kind(&self, path: &Path) -> DriveKind { todo!("core-A: GetDriveTypeW on the drive root") }
    fn recycle(&self, path: &Path) -> std::io::Result<()> { todo!("core-A: trash crate / IFileOperation") }
    fn data_dir(&self) -> PathBuf { todo!("core-A: %LOCALAPPDATA%\\Declutter") }
    fn legacy_data_dir(&self) -> PathBuf { todo!("core-A: %APPDATA%\\Declutter") }
}
