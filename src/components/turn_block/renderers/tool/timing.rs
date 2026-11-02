//! The duration of a finished tool call, shown after its description
//! (SPEC cycle 2 §3): ` · 340ms`, ` · 1.2s`, ` · 5m 2s`.

use std::time::Duration;

use crate::duration::format_duration;
use crate::model::{ToolCall, ToolStatus};

/// What stands between the description and the duration.
const SEPARATOR: &str = " · ";

/// A tool call's duration: milliseconds under a second (`340ms`), tenths of a
/// second under a minute (`1.2s`), and the shared format (§2.4) from a
/// minute on (`5m 2s`, `1h 5m`). Like the shared format, it truncates:
/// 999.9 ms is `999ms` and 59.99 s is `59.9s`.
pub fn format_tool_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    match millis {
        0..=999 => format!("{millis}ms"),
        1_000..=59_999 => format!("{}.{}s", millis / 1_000, millis % 1_000 / 100),
        _ => format_duration(duration),
    }
}

/// The duration of `call` as shown, with its separator (` · 1.2s`); `None`
/// for a pending call and for one without a known duration.
pub fn duration_suffix(call: &ToolCall) -> Option<String> {
    if call.status == ToolStatus::Pending {
        return None;
    }
    let duration = call.duration?;
    Some(format!("{SEPARATOR}{}", format_tool_duration(duration)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(millis: u64) -> String {
        format_tool_duration(Duration::from_millis(millis))
    }

    #[test]
    fn under_a_second_shows_milliseconds() {
        assert_eq!(fmt(0), "0ms");
        assert_eq!(fmt(340), "340ms");
        assert_eq!(fmt(999), "999ms");
        assert_eq!(
            format_tool_duration(Duration::from_micros(999_900)),
            "999ms"
        );
    }

    #[test]
    fn from_a_second_to_a_minute_shows_tenths() {
        assert_eq!(fmt(1_000), "1.0s");
        assert_eq!(fmt(1_049), "1.0s");
        assert_eq!(fmt(1_200), "1.2s");
        assert_eq!(fmt(45_678), "45.6s");
        assert_eq!(fmt(59_900), "59.9s");
        assert_eq!(fmt(59_999), "59.9s");
        assert_eq!(
            format_tool_duration(Duration::from_micros(59_999_999)),
            "59.9s"
        );
    }

    #[test]
    fn from_a_minute_on_it_is_the_shared_format() {
        assert_eq!(fmt(60_000), "1m 0s");
        assert_eq!(fmt(5 * 60_000 + 2_000), "5m 2s");
        assert_eq!(fmt(59 * 60_000 + 59_900), "59m 59s");
        assert_eq!(fmt(65 * 60_000), "1h 5m");
    }
}
