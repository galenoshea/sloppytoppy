//! Human-readable number formatting.

/// `1_234` → `1.2k`, `5_600_000` → `5.6M`.
#[must_use]
pub fn compact(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        1_000_000..1_000_000_000 => format!("{:.1}M", n as f64 / 1e6),
        _ => format!("{:.2}B", n as f64 / 1e9),
    }
}

/// Bytes as `MB` or `GB`.
#[must_use]
pub fn memory(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{mb:.0} MB")
    }
}

/// Seconds as `HH:MM:SS`, or `Nd HHh` beyond a day.
#[must_use]
pub fn duration(secs: u64) -> String {
    if secs >= 86_400 {
        format!("{}d {:02}h", secs / 86_400, secs % 86_400 / 3600)
    } else {
        format!(
            "{:02}:{:02}:{:02}",
            secs / 3600,
            secs % 3600 / 60,
            secs % 60
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1_234), "1.2k");
        assert_eq!(compact(5_600_000), "5.6M");
        assert_eq!(compact(2_500_000_000), "2.50B");
        assert_eq!(memory(300 * 1_048_576), "300 MB");
        assert_eq!(memory(3 * 1_073_741_824), "3.0 GB");
        assert_eq!(duration(3_725), "01:02:05");
        assert_eq!(duration(90_000), "1d 01h");
    }
}
