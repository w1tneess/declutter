//! Plain-language formatting. Owner: core-A. Port of `human_size` in core.py.

/// "512 B", "1.5 KB", "2.0 MB" ... one decimal above bytes, TB is the top unit.
pub fn human_size(n: f64) -> String {
    let mut n = n;
    for unit in ["B", "KB", "MB", "GB"] {
        if n < 1024.0 {
            return if unit == "B" {
                format!("{n:.0} B")
            } else {
                format!("{n:.1} {unit}")
            };
        }
        n /= 1024.0;
    }
    format!("{n:.1} TB")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_size_matches_python() {
        assert_eq!(human_size(0.0), "0 B");
        assert_eq!(human_size(1023.0), "1023 B");
        assert_eq!(human_size(1024.0), "1.0 KB");
        assert_eq!(human_size(1536.0), "1.5 KB");
        assert_eq!(human_size(1280.0), "1.2 KB"); // 1.25 exactly: Python rounds half to even
        assert_eq!(human_size(2.0 * 1024.0 * 1024.0), "2.0 MB");
        assert_eq!(human_size(1024f64.powi(4) * 5000.0), "5000.0 TB");
    }
}
