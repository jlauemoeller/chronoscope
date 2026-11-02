//! Duration display (SPEC §2.4): one format for the session selector's rows
//! and the stats footer, and for every later place that shows a duration.

use std::time::Duration;

/// A duration as `45s`, `4m 3s` or `3h 4m`.
///
/// Under a minute it shows seconds, under an hour minutes and seconds, and
/// from an hour on hours and minutes. Numbers are not zero-padded, and the
/// value is truncated, not rounded: `59m 59.9s` reads `59m 59s`.
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m {}s", secs / 60, secs % 60),
        _ => format!("{}h {}m", secs / 3600, secs % 3600 / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(millis: u64) -> String {
        format_duration(Duration::from_millis(millis))
    }

    fn secs(secs: u64) -> String {
        format_duration(Duration::from_secs(secs))
    }

    #[test]
    fn under_a_minute_shows_seconds() {
        assert_eq!(secs(0), "0s");
        assert_eq!(secs(45), "45s");
        assert_eq!(fmt(59_900), "59s");
    }

    #[test]
    fn under_an_hour_shows_minutes_and_seconds() {
        assert_eq!(secs(60), "1m 0s");
        assert_eq!(secs(4 * 60 + 3), "4m 3s");
        assert_eq!(secs(5 * 60 + 2), "5m 2s");
        assert_eq!(secs(12 * 60), "12m 0s");
        assert_eq!(secs(59 * 60 + 59), "59m 59s");
        assert_eq!(fmt(59 * 60_000 + 59_900), "59m 59s");
    }

    #[test]
    fn an_hour_or_more_shows_hours_and_minutes() {
        assert_eq!(secs(3600), "1h 0m");
        assert_eq!(secs(3600 + 5 * 60), "1h 5m");
        assert_eq!(secs(3 * 3600 + 4 * 60), "3h 4m");
        assert_eq!(secs(2 * 3600 + 45 * 60), "2h 45m");
        assert_eq!(secs(26 * 3600 + 30 * 60), "26h 30m");
        assert_eq!(secs(100 * 3600 + 30 * 60), "100h 30m");
    }

    #[test]
    fn values_are_truncated_not_rounded() {
        assert_eq!(secs(3600 + 5 * 60 + 59), "1h 5m");
        assert_eq!(fmt(3_599_999), "59m 59s");
        assert_eq!(fmt(999), "0s");
    }

    #[test]
    fn numbers_are_not_zero_padded() {
        assert_eq!(secs(60), "1m 0s");
        assert_eq!(secs(3600), "1h 0m");
        assert_eq!(secs(3600 + 60), "1h 1m");
    }
}
