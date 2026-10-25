//! The seam between the core and Windows. Everything the golden fixtures patch in the Python oracle
//! (`_drive_type`, `_known_folder_path`, `send2trash`, `LOG_DIR`, `time.time`) goes through this trait, so
//! tests inject a fake and production uses `SystemPlatform`. The free functions below are Win32 helpers tests do
//! not need to fake (a non-replacing move, local-time conversion); all `unsafe` in the crate lives in this file.
//!
//! Owner: agent core-A.

use std::ffi::{c_void, OsString};
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::{Component, Path, PathBuf, Prefix};

use windows::core::{GUID, HSTRING};
use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
use windows::Win32::Storage::FileSystem::{GetDriveTypeW, MoveFileExW, MOVEFILE_COPY_ALLOWED};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Time::{
    FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime,
    TzSpecificLocalTimeToSystemTime,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Profile,
    SHGetKnownFolderPath, KF_FLAG_DEFAULT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownFolder {
    Downloads,
    Desktop,
    Documents,
}

/// `GetDriveTypeW` collapsed. Removals are allowed on `Fixed` only (SPEC rule 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind {
    Fixed,
    Removable,
    Network,
    Other,
}

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

impl SystemPlatform {
    /// `%<var>%\Declutter`, or `~\.config\Declutter` like v0.2.2 when the variable is missing.
    fn env_dir(&self, var: &str) -> PathBuf {
        std::env::var_os(var)
            .map(PathBuf::from)
            .unwrap_or_else(|| self.home().join(".config"))
            .join("Declutter")
    }
}

impl Platform for SystemPlatform {
    fn home(&self) -> PathBuf {
        // Python's Path.home() reads USERPROFILE; the shell is the fallback.
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .or_else(|| known_folder_path(&FOLDERID_Profile))
            .unwrap_or_default()
    }

    fn known_folder(&self, which: KnownFolder) -> Option<PathBuf> {
        known_folder_path(match which {
            KnownFolder::Downloads => &FOLDERID_Downloads,
            KnownFolder::Desktop => &FOLDERID_Desktop,
            KnownFolder::Documents => &FOLDERID_Documents,
        })
    }

    fn drive_kind(&self, path: &Path) -> DriveKind {
        let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let root = match abs.components().next() {
            Some(Component::Prefix(p)) => match p.kind() {
                Prefix::Disk(l) | Prefix::VerbatimDisk(l) => format!("{}:\\", l as char),
                Prefix::UNC(..) | Prefix::VerbatimUNC(..) => return DriveKind::Network,
                _ => return DriveKind::Other,
            },
            _ => return DriveKind::Other,
        };
        // SAFETY: the HSTRING is NUL-terminated and outlives the call, which only reads it.
        match unsafe { GetDriveTypeW(&HSTRING::from(root)) } {
            3 => DriveKind::Fixed,     // DRIVE_FIXED
            2 => DriveKind::Removable, // DRIVE_REMOVABLE
            4 => DriveKind::Network,   // DRIVE_REMOTE
            _ => DriveKind::Other,
        }
    }

    fn recycle(&self, path: &Path) -> io::Result<()> {
        // trash 5.2 deletes through IFileOperation with FOF_ALLOWUNDO | FOF_WANTNUKEWARNING and no UI, and checks
        // GetAnyOperationsAborted: an item that cannot go to the Bin comes back as an error, not destroyed.
        trash::delete(path).map_err(|e| io::Error::other(e.to_string()))
    }

    fn data_dir(&self) -> PathBuf {
        self.env_dir("LOCALAPPDATA")
    }

    fn legacy_data_dir(&self) -> PathBuf {
        self.env_dir("APPDATA")
    }
}

fn known_folder_path(id: &GUID) -> Option<PathBuf> {
    // SAFETY: `id` is a valid GUID. On success the shell returns a NUL-terminated string allocated with
    // CoTaskMemAlloc; it is copied, then freed exactly once and never touched again.
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let path = PathBuf::from(OsString::from_wide(p.as_wide()));
        CoTaskMemFree(Some(p.0 as *const c_void));
        Some(path)
    }
}

/// Move a file without ever replacing an existing one (`std::fs::rename` replaces). As with Python's
/// `shutil.move`, a move to another volume becomes copy + delete.
pub fn move_no_replace(src: &Path, dst: &Path) -> io::Result<()> {
    let (s, d) = (
        HSTRING::from(src.as_os_str()),
        HSTRING::from(dst.as_os_str()),
    );
    // SAFETY: both HSTRINGs are NUL-terminated and outlive the call, which only reads them.
    unsafe { MoveFileExW(&s, &d, MOVEFILE_COPY_ALLOWED) }.map_err(io::Error::from)
}

const FILETIME_UNIX_OFFSET_SECS: f64 = 11_644_473_600.0;

/// Windows FILETIME ticks (100 ns since 1601) as Unix seconds.
pub fn filetime_to_unix(ticks: u64) -> f64 {
    ticks as f64 / 1e7 - FILETIME_UNIX_OFFSET_SECS
}

/// Unix seconds as local time, `YYYY-MM-DD HH:MM:SS` (the undo log's `time`, as v0.2.2's strftime wrote it).
pub fn local_time_string(unix: f64) -> Option<String> {
    let ticks = (unix + FILETIME_UNIX_OFFSET_SECS) * 1e7;
    if !(0.0..9.0e18).contains(&ticks) {
        return None;
    }
    let ticks = ticks as u64;
    let ft = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let (mut utc, mut local) = (SYSTEMTIME::default(), SYSTEMTIME::default());
    // SAFETY: every pointer refers to a live, properly aligned local for the duration of each call.
    unsafe {
        FileTimeToSystemTime(&ft, &mut utc).ok()?;
        SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).ok()?;
    }
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        local.wYear, local.wMonth, local.wDay, local.wHour, local.wMinute, local.wSecond
    ))
}

/// Inverse of `local_time_string`; None for anything that is not a valid `YYYY-MM-DD HH:MM:SS` local time.
pub fn parse_local_time(s: &str) -> Option<f64> {
    let (date, time) = s.split_once(' ')?;
    let num = |part: Option<&str>| part?.parse::<u16>().ok();
    let (mut d, mut t) = (date.split('-'), time.split(':'));
    let local = SYSTEMTIME {
        wYear: num(d.next())?,
        wMonth: num(d.next())?,
        wDay: num(d.next())?,
        wHour: num(t.next())?,
        wMinute: num(t.next())?,
        wSecond: num(t.next())?,
        ..Default::default()
    };
    if d.next().is_some() || t.next().is_some() {
        return None;
    }
    let (mut utc, mut ft) = (SYSTEMTIME::default(), FILETIME::default());
    // SAFETY: every pointer refers to a live, properly aligned local; SystemTimeToFileTime rejects invalid dates.
    unsafe {
        TzSpecificLocalTimeToSystemTime(None, &local, &mut utc).ok()?;
        SystemTimeToFileTime(&utc, &mut ft).ok()?;
    }
    Some(filetime_to_unix(
        ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_time_round_trips() {
        let t = 1_760_000_000.0;
        let s = local_time_string(t).unwrap();
        assert_eq!(s.len(), 19);
        assert_eq!(parse_local_time(&s), Some(t));
    }

    #[test]
    fn bad_local_times_are_rejected() {
        for s in [
            "",
            "2026-01-01",
            "2026-13-01 10:00:00",
            "2026-01-01 10:00",
            "2026-01-01 10:00:00:00",
            "a b",
        ] {
            assert_eq!(parse_local_time(s), None, "{s}");
        }
        assert_eq!(local_time_string(f64::NAN), None);
    }

    #[test]
    fn unc_paths_are_network() {
        assert_eq!(
            SystemPlatform.drive_kind(Path::new(r"\\server\share\x.txt")),
            DriveKind::Network
        );
    }
}
