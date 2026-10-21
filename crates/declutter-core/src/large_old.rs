//! Large and old files. Owner: core-B. See CONTRACT "Large and old". Port of find_large_old.

use crate::ctx::Ctx;
use crate::dupes::{ext, file_ref, DEAD_EXTS};
use crate::model::FileRef;
use crate::walk::Entry;

/// size >= min_mb MiB AND older than `older_days`; 0 disables a filter; dead downloads excluded; size descending,
/// ties by path so the order is stable.
pub fn find_large_old(ctx: &Ctx, entries: &[Entry], min_mb: f64, older_days: u32) -> Vec<FileRef> {
    let cutoff = ctx.now() - f64::from(older_days) * 86400.0;
    let mut out: Vec<FileRef> = entries
        .iter()
        .filter(|e| {
            !(min_mb > 0.0 && (e.size as f64) < min_mb * 1024.0 * 1024.0)
                && !(older_days > 0 && e.mtime > cutoff)
                && !DEAD_EXTS.contains(&ext(&e.path).as_str())
        })
        .map(file_ref)
        .collect();
    out.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dupes::tests::{ctx, entries};

    #[test]
    fn equal_sizes_sort_by_path_and_dead_downloads_stay_out() {
        let dir = tempfile::tempdir().unwrap();
        let es = entries(
            dir.path(),
            &[
                ("b.bin", "xx", 1.0),
                ("a.bin", "xx", 1.0),
                ("c.part", "xxx", 1.0),
            ],
        );
        let got: Vec<String> = find_large_old(&ctx(dir.path()), &es, 0.0, 0)
            .into_iter()
            .map(|f| f.path)
            .collect();
        let want: Vec<String> = ["a.bin", "b.bin"]
            .iter()
            .map(|n| dir.path().join(n).to_string_lossy().into_owned())
            .collect();
        assert_eq!(got, want);
    }
}
