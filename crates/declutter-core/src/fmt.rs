//! Plain-language formatting. Owner: core-A. Port of `human_size`, `ago`, `size_pair` in core.py.

/// "512 B", "1.5 KB", "2.0 MB" ... one decimal above bytes, TB is the top unit.
pub fn human_size(n: f64) -> String { todo!("core-A") }

/// "just now", "5 minutes ago", "1 day ago".
pub fn ago(seconds: f64) -> String { todo!("core-A") }

/// "2.0 MB vs 2.1 MB", or exact bytes when rounding would make two different sizes look equal.
pub fn size_pair(a: u64, b: u64) -> String { todo!("core-A") }
