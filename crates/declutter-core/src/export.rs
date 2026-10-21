//! CSV export of any preview (new in 1.0). Owner: core-C. Excel-friendly (UTF-8 BOM, CRLF), formula-injection safe
//! (a cell starting with = + - @ gets a leading apostrophe), no secrets, never writes inside the scanned folder
//! unless the user chose that path.

use std::path::Path;

use crate::fmt::human_size;
use crate::model::{DupGroup, FileRef, Leftover, Move, Redownload, SimilarGroup};
use crate::platform::local_time_string;

/// `header` and every row have the same length `N`, so the writer can never reject a record.
fn table<const N: usize>(header: [&str; N], rows: impl IntoIterator<Item = [String; N]>) -> String {
    let mut w = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    // Writing equal-length records into memory cannot fail.
    w.write_record(header).expect("in-memory CSV write");
    for row in rows {
        w.write_record(row.iter().map(|c| cell(c)))
            .expect("in-memory CSV write");
    }
    let bytes = w.into_inner().expect("in-memory CSV flush");
    format!("\u{feff}{}", String::from_utf8_lossy(&bytes))
}

/// Excel runs a cell that starts with one of these as a formula; the apostrophe makes it plain text.
fn cell(s: &str) -> String {
    if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_owned()
    }
}

fn human(n: u64) -> String {
    human_size(n as f64)
}

/// Unix seconds as local "YYYY-MM-DD HH:MM:SS" (Excel reads it as a date); empty when the time is unknown.
fn when(secs: f64) -> String {
    local_time_string(secs).unwrap_or_default()
}

pub fn moves_csv(moves: &[Move]) -> String {
    table(
        ["File", "Moves to", "Folder", "Size (bytes)", "Size"],
        moves.iter().map(|m| {
            [
                m.src.clone(),
                m.dst.clone(),
                m.category.clone(),
                m.size.to_string(),
                human(m.size),
            ]
        }),
    )
}

pub fn duplicates_csv(groups: &[DupGroup]) -> String {
    table(
        [
            "Group",
            "File",
            "Suggested keeper",
            "Size (bytes)",
            "Size",
            "Modified",
        ],
        groups.iter().enumerate().flat_map(|(g, group)| {
            group.files.iter().enumerate().map(move |(i, f)| {
                let keeper = if i == 0 { "Yes" } else { "No" };
                [
                    (g + 1).to_string(),
                    f.path.clone(),
                    keeper.into(),
                    f.size.to_string(),
                    human(f.size),
                    when(f.mtime),
                ]
            })
        }),
    )
}

pub fn large_old_csv(files: &[FileRef]) -> String {
    table(
        ["File", "Size (bytes)", "Size", "Modified"],
        files.iter().map(|f| {
            [
                f.path.clone(),
                f.size.to_string(),
                human(f.size),
                when(f.mtime),
            ]
        }),
    )
}

pub fn leftovers_csv(items: &[Leftover]) -> String {
    table(
        ["File", "Why", "Size (bytes)", "Size", "Modified"],
        items.iter().map(|l| {
            [
                l.path.clone(),
                l.reason.clone(),
                l.size.to_string(),
                human(l.size),
                when(l.mtime),
            ]
        }),
    )
}

pub fn redownloads_csv(pairs: &[Redownload]) -> String {
    table(
        [
            "Copy",
            "Original",
            "Copy size (bytes)",
            "Copy size",
            "Original size (bytes)",
            "Copy modified",
        ],
        pairs.iter().map(|p| {
            let (c, o) = (&p.copy, &p.original);
            [
                c.path.clone(),
                o.path.clone(),
                c.size.to_string(),
                human(c.size),
                o.size.to_string(),
                when(c.mtime),
            ]
        }),
    )
}

pub fn similar_csv(groups: &[SimilarGroup]) -> String {
    table(
        [
            "Group",
            "File",
            "Width",
            "Height",
            "Difference (0 = same picture)",
            "Size (bytes)",
            "Size",
            "Modified",
        ],
        groups.iter().enumerate().flat_map(|(g, group)| {
            group.files.iter().map(move |f| {
                [
                    (g + 1).to_string(),
                    f.path.clone(),
                    f.width.to_string(),
                    f.height.to_string(),
                    f.distance.to_string(),
                    f.size.to_string(),
                    human(f.size),
                    when(f.mtime),
                ]
            })
        }),
    )
}

pub fn empty_dirs_csv(dirs: &[String]) -> String {
    table(["Folder"], dirs.iter().map(|d| [d.clone()]))
}

/// `path` comes from the native save dialog; anything else (empty, relative) is refused rather than guessed.
pub fn write_csv(path: &Path, csv: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Choose where to save the file.".into());
    }
    crate::settings::write_atomic(path, csv.as_bytes())
        .map_err(|e| format!("Could not save the file: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(csv: &str) -> Vec<String> {
        let body = csv.strip_prefix('\u{feff}').expect("UTF-8 BOM first");
        let body = body.strip_suffix("\r\n").expect("ends with CRLF");
        assert!(
            !body.replace("\r\n", "").contains('\n'),
            "only CRLF line ends: {body:?}"
        );
        body.split("\r\n").map(str::to_owned).collect()
    }

    fn file(path: &str, size: u64) -> FileRef {
        FileRef {
            path: path.into(),
            size,
            mtime: 0.0,
        }
    }

    #[test]
    fn moves_have_plain_headers_bom_and_crlf() {
        let m = Move {
            src: r"C:\d\a.jpg".into(),
            dst: r"C:\d\Images\a.jpg".into(),
            size: 1536,
            category: "Images".into(),
        };
        assert_eq!(
            lines(&moves_csv(&[m])),
            [
                "File,Moves to,Folder,Size (bytes),Size",
                r"C:\d\a.jpg,C:\d\Images\a.jpg,Images,1536,1.5 KB"
            ]
        );
        assert_eq!(
            lines(&moves_csv(&[])),
            ["File,Moves to,Folder,Size (bytes),Size"]
        );
    }

    #[test]
    fn quoting_follows_rfc_4180() {
        let csv = large_old_csv(&[file("C:\\d\\a, \"b\"\nc.txt", 1)]);
        let body = csv.strip_prefix('\u{feff}').unwrap();
        assert!(
            body.contains("\"C:\\d\\a, \"\"b\"\"\nc.txt\",1,1 B,"),
            "{body:?}"
        );
        let mut r = csv::Reader::from_reader(body.as_bytes());
        let row = r.records().next().unwrap().unwrap();
        assert_eq!(&row[0], "C:\\d\\a, \"b\"\nc.txt");
    }

    #[test]
    fn formula_cells_are_neutralised() {
        for bad in ["=HYPERLINK(\"x\")", "+1", "-2", "@SUM(A1)", "\tx", "\rx"] {
            assert_eq!(cell(bad), format!("'{bad}"));
        }
        for fine in ["C:\\a", "a=b", "", "1"] {
            assert_eq!(cell(fine), fine);
        }
        let l = Leftover {
            path: "=cmd|' /C calc'!A0".into(),
            size: 1,
            mtime: 0.0,
            reason: "@x".into(),
        };
        let rows = lines(&leftovers_csv(&[l]));
        assert!(
            rows[1].starts_with("'=cmd|' /C calc'!A0,'@x,1,1 B,"),
            "{}",
            rows[1]
        );
    }

    #[test]
    fn every_preview_exports() {
        let g = DupGroup {
            size: 10,
            files: vec![file(r"C:\d\a", 10), file(r"C:\d\b", 10)],
        };
        let d = lines(&duplicates_csv(&[g.clone(), g]));
        assert_eq!(d.len(), 5);
        assert!(d[1].starts_with(r"1,C:\d\a,Yes,10,10 B,"));
        assert!(d[2].starts_with(r"1,C:\d\b,No,"));
        assert!(d[3].starts_with("2,"));

        let r = Redownload {
            original: file(r"C:\d\x.pdf", 2048),
            copy: file(r"C:\d\x (1).pdf", 2050),
        };
        assert!(lines(&redownloads_csv(&[r]))[1]
            .starts_with(r"C:\d\x (1).pdf,C:\d\x.pdf,2050,2.0 KB,2048,"));

        let f = crate::model::SimilarFile {
            path: r"C:\p\a.png".into(),
            size: 5_000_000,
            mtime: 0.0,
            width: 640,
            height: 480,
            distance: 3,
        };
        let s = lines(&similar_csv(&[SimilarGroup { files: vec![f] }]));
        assert!(
            s[1].starts_with(r"1,C:\p\a.png,640,480,3,5000000,4.8 MB,"),
            "{}",
            s[1]
        );

        assert_eq!(
            lines(&empty_dirs_csv(&[r"C:\d\e".into()])),
            ["Folder", r"C:\d\e"]
        );
    }

    #[test]
    fn write_csv_is_atomic_and_refuses_empty_paths() {
        let t = tempfile::tempdir().unwrap();
        assert!(write_csv(Path::new(""), "x").is_err());
        assert!(write_csv(Path::new("relative.csv"), "x").is_err());
        let p = t.path().join("out.csv");
        write_csv(&p, "old").unwrap();
        let csv = empty_dirs_csv(&["C:\\é".into()]);
        write_csv(&p, &csv).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), csv);
        assert!(std::fs::read(&p).unwrap().starts_with(&[0xEF, 0xBB, 0xBF]));
    }
}
