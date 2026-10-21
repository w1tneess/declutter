//! Plain-language formatting. Owner: core-A. Port of `human_size`, `ago`, `size_pair` in core.py.

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

/// "just now", "5 minutes ago", "1 day ago".
pub fn ago(seconds: f64) -> String {
    for (size, unit) in [(86400.0, "day"), (3600.0, "hour"), (60.0, "minute")] {
        if seconds >= size {
            let n = (seconds / size).floor() as u64;
            return format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
        }
    }
    "just now".into()
}

/// "2.0 MB vs 2.1 MB", or exact bytes when rounding would make two different sizes look equal.
pub fn size_pair(a: u64, b: u64) -> String {
    let (x, y) = (human_size(a as f64), human_size(b as f64));
    if x != y {
        format!("{x} vs {y}")
    } else {
        format!("{} B vs {} B", thousands(a), thousands(b))
    }
}

/// `1234567` -> `"1,234,567"`, like Python's `f"{n:,}"`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
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

    #[test]
    fn ago_picks_the_largest_unit() {
        assert_eq!(ago(0.0), "just now");
        assert_eq!(ago(59.9), "just now");
        assert_eq!(ago(60.0), "1 minute ago");
        assert_eq!(ago(3599.0), "59 minutes ago");
        assert_eq!(ago(7200.0), "2 hours ago");
        assert_eq!(ago(86400.0 * 3.5), "3 days ago");
        assert_eq!(ago(f64::NAN), "just now");
    }

    #[test]
    fn size_pair_falls_back_to_exact_bytes() {
        assert_eq!(
            size_pair(2 * 1024 * 1024, 3 * 1024 * 1024),
            "2.0 MB vs 3.0 MB"
        );
        assert_eq!(
            size_pair(1_048_576, 1_048_577),
            "1,048,576 B vs 1,048,577 B"
        );
        assert_eq!(size_pair(5, 5), "5 B vs 5 B");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
    }
}
