//! Scan-only reminder (new in 1.0). Owner: core-C. A per-user Windows Task Scheduler task (no admin) that runs
//! `declutter --remind`; that mode scans the saved folder and shows one notification ("212 files waiting"). It must
//! NEVER move, delete or otherwise change a file, and is off by default. Enabling, disabling and status must be
//! idempotent and reversible in one click. Build the `schtasks` argument list in a pure function so it can be unit
//! tested without touching the machine; quote the exe path safely (spaces) and never interpolate user text.
//!
//! Status reads the exit code and the task XML's element names, never `schtasks`' labels, which Windows translates.

use std::ffi::OsStr;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Output};

use crate::ctx::Ctx;
use crate::model::ReminderStatus;

pub const TASK_NAME: &str = "Declutter reminder";
/// `schtasks` refuses a `/TR` longer than this.
const TR_MAX: usize = 261;
/// Keeps a console window from flashing up when the GUI runs `schtasks`.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Arguments for `schtasks /Create ...`. Pure, for tests.
///
/// Runs as the signed-in user with limited rights (no `/RU`, `/RL LIMITED`), only while they are logged on, on
/// Mondays at 09:00. `/F` makes enabling twice simply replace the task.
pub fn create_args(exe: &Path) -> Result<Vec<String>, String> {
    let exe_text = exe.to_str().unwrap_or_default();
    // The path goes inside "..." in /TR: a quote would end it early, % would expand as a variable.
    let plain = !exe_text.contains(['"', '%']) && !exe_text.chars().any(char::is_control);
    if !exe.is_absolute() || !plain || !exe_text.to_lowercase().ends_with(".exe") {
        return Err("Declutter's location can't be used for a scheduled reminder.".into());
    }
    let tr = format!("\"{exe_text}\" --remind");
    if tr.chars().count() > TR_MAX {
        return Err(
            "Declutter is installed in a folder whose path is too long for a scheduled reminder."
                .into(),
        );
    }
    Ok([
        "/Create", "/TN", TASK_NAME, "/TR", &*tr, "/SC", "WEEKLY", "/D", "MON", "/ST", "09:00",
        "/RL", "LIMITED", "/F",
    ]
    .map(String::from)
    .to_vec())
}

/// Fixed program, argument list passed as-is (no shell), so nothing in it is ever interpreted.
fn schtasks<S: AsRef<OsStr>>(args: &[S]) -> std::io::Result<Output> {
    Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

/// `schtasks /XML` output arrives as UTF-16 on some Windows builds and in the console code page on others.
fn decode(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.get(1) == Some(&0) {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect();
        String::from_utf16_lossy(&units)
            .trim_start_matches('\u{feff}')
            .to_owned()
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// The command's output if it succeeded; a missing task makes `/Query` fail.
fn query(args: &[&str]) -> Option<String> {
    let out = schtasks(args).ok()?;
    out.status.success().then(|| decode(&out.stdout))
}

/// Second column of `schtasks /Query /FO CSV /NH`, shown as Windows formats it; None when it holds no date
/// (a disabled task says "N/A" in the user's language).
fn next_run(csv_text: &str) -> Option<String> {
    let mut r = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv_text.as_bytes());
    let record = r
        .records()
        .filter_map(Result::ok)
        .find(|rec| rec.len() >= 2)?;
    let when = record.get(1)?.trim();
    when.chars()
        .any(|c| c.is_ascii_digit())
        .then(|| when.to_owned())
}

fn off() -> ReminderStatus {
    ReminderStatus {
        enabled: false,
        next_run: None,
    }
}

pub fn enable(ctx: &Ctx, exe: &Path) -> Result<ReminderStatus, String> {
    let args = create_args(exe)?;
    let out =
        schtasks(args.as_slice()).map_err(|e| format!("Could not turn on the reminder: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "Could not turn on the reminder: {}",
            decode(&out.stderr).trim()
        ));
    }
    Ok(status(ctx))
}

/// Deleting a task that is not there fails, which is the state asked for, so the result is judged by a fresh query.
pub fn disable(ctx: &Ctx) -> Result<ReminderStatus, String> {
    let deleted = schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]);
    if query(&["/Query", "/TN", TASK_NAME]).is_some() {
        let why = match deleted {
            Ok(out) => decode(&out.stderr).trim().to_owned(),
            Err(e) => e.to_string(),
        };
        return Err(format!("Could not turn off the reminder: {why}"));
    }
    Ok(status(ctx))
}

/// `ctx` is unused today: the task itself is the source of truth, not a setting.
pub fn status(_ctx: &Ctx) -> ReminderStatus {
    let Some(xml) = query(&["/Query", "/TN", TASK_NAME, "/XML"]) else {
        return off();
    };
    ReminderStatus {
        // A task switched off in Task Scheduler still exists but will not run.
        enabled: !xml.contains("<Enabled>false</Enabled>"),
        next_run: query(&["/Query", "/TN", TASK_NAME, "/FO", "CSV", "/NH"])
            .and_then(|t| next_run(&t)),
    }
}

/// The text of the reminder notification for `n` files waiting (0 = say nothing).
pub fn message(files_waiting: u32) -> Option<String> {
    match files_waiting {
        0 => None,
        1 => Some("1 file is waiting to be sorted. Open Declutter to preview it; nothing moves until you confirm.".into()),
        n => Some(format!(
            "{n} files are waiting to be sorted. Open Declutter to preview them; nothing moves until you confirm."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = r"C:\Program Files\Declutter\declutter.exe";

    #[test]
    fn weekly_monday_nine() {
        let want = [
            "/Create",
            "/TN",
            "Declutter reminder",
            "/TR",
            r#""C:\Program Files\Declutter\declutter.exe" --remind"#,
            "/SC",
            "WEEKLY",
            "/D",
            "MON",
            "/ST",
            "09:00",
            "/RL",
            "LIMITED",
            "/F",
        ];
        assert_eq!(create_args(Path::new(EXE)).unwrap(), want);
    }

    #[test]
    fn unsafe_exe_paths_are_refused() {
        for bad in [
            r#"C:\a"b\declutter.exe"#,
            r"C:\%TEMP%\declutter.exe",
            "C:\\a\nb\\declutter.exe",
            "C:\\a\rb\\declutter.exe",
            r"declutter.exe",
            r"C:declutter.exe",
            r"C:\Declutter\declutter.bat",
            r"C:\Declutter\declutter",
            "",
        ] {
            assert!(create_args(Path::new(bad)).is_err(), "{bad:?}");
        }
        let long = format!(r"C:\{}\declutter.exe", "a".repeat(240));
        assert!(create_args(Path::new(&long)).is_err());
        let fits = format!(r"C:\{}\declutter.exe", "a".repeat(230));
        assert!(create_args(Path::new(&fits)).is_ok());
        assert!(create_args(Path::new(r"C:\Users\Zoë\Apps\Declutter.EXE")).is_ok());
    }

    #[test]
    fn decodes_utf16_and_code_page_output() {
        let text = "<Task><ScheduleByWeek/></Task>";
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode(&utf16), text);
        let bare: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode(&bare), text);
        assert_eq!(decode(text.as_bytes()), text);
        assert_eq!(decode(b""), "");
        assert_eq!(decode(&[0xFF]), "\u{fffd}");
    }

    #[test]
    fn reads_next_run_whatever_the_language() {
        assert_eq!(
            next_run("\r\n\"\\Declutter reminder\",\"27/10/2026 09:00:00\",\"Ready\"\r\n")
                .as_deref(),
            Some("27/10/2026 09:00:00")
        );
        assert_eq!(
            next_run("\"\\Declutter reminder\",\"27.10.2026 09:00:00\",\"Bereit\"").as_deref(),
            Some("27.10.2026 09:00:00")
        );
        assert_eq!(
            next_run("\"\\Declutter reminder\",\"N/A\",\"Disabled\""),
            None
        );
        assert_eq!(
            next_run("\"\\Declutter reminder\",\"Nicht zutreffend\",\"Deaktiviert\""),
            None
        );
        assert_eq!(next_run(""), None);
        assert_eq!(next_run("garbage"), None);
    }

    #[test]
    fn notification_text() {
        assert_eq!(message(0), None);
        assert!(message(1).unwrap().starts_with("1 file is waiting"));
        let m = message(212).unwrap();
        assert!(
            m.starts_with("212 files are waiting") && m.contains("nothing moves until you confirm"),
            "{m}"
        );
    }
}
