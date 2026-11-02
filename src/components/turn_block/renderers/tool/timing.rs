//! The duration of a finished tool call, shown after its description
//! (SPEC cycle 2 §3): ` · 340ms`, ` · 1.2s`, ` · 5m 2s`.

use std::time::Duration;

use ratatui::style::Style;
use ratatui::text::{Line, Span};

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

/// Gives the last characters of `lines` that read `tail` (white space
/// aside, which a word-wrap may have dropped at a break) the style `muted`.
/// The rows before and after the tail keep their styles.
pub fn mute_tail(lines: Vec<Line<'static>>, tail: &str, muted: Style) -> Vec<Line<'static>> {
    let mut left = tail.chars().filter(|c| !c.is_whitespace()).count();
    let mut out: Vec<Line<'static>> = Vec::with_capacity(lines.len());
    for line in lines.into_iter().rev() {
        if left == 0 {
            out.push(line);
            continue;
        }
        out.push(mute_line_tail(line, &mut left, muted));
    }
    out.reverse();
    out
}

/// Mutes the last `left` non-white-space characters of `line` (fewer if it
/// has fewer, and `left` then holds what is still to do).
fn mute_line_tail(line: Line<'static>, left: &mut usize, muted: Style) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.spans.len() + 1);
    for span in line.spans.into_iter().rev() {
        if *left == 0 {
            spans.push(span);
            continue;
        }
        let text = span.content.as_ref();
        let mut split = 0;
        for (at, c) in text.char_indices().rev() {
            if !c.is_whitespace() {
                *left -= 1;
            }
            if *left == 0 && !c.is_whitespace() {
                split = at;
                break;
            }
        }
        let (keep, mute) = text.split_at(split);
        if !mute.is_empty() {
            spans.push(Span::styled(mute.to_owned(), span.style.patch(muted)));
        }
        if !keep.is_empty() {
            spans.push(Span::styled(keep.to_owned(), span.style));
        }
    }
    spans.reverse();
    Line::from(spans)
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

    fn row(spans: &[(&str, Style)]) -> Line<'static> {
        Line::from(
            spans
                .iter()
                .map(|(text, style)| Span::styled((*text).to_owned(), *style))
                .collect::<Vec<_>>(),
        )
    }

    fn styled(line: &Line<'_>) -> Vec<(String, Option<ratatui::style::Color>)> {
        line.spans
            .iter()
            .map(|s| (s.content.to_string(), s.style.fg))
            .collect()
    }

    #[test]
    fn the_tail_is_muted_inside_one_row() {
        use ratatui::style::Color::{Blue, Red};
        let base = Style::new().fg(Red);
        let muted = Style::new().fg(Blue);
        let out = mute_tail(vec![row(&[("Do it · 1.2s", base)])], " · 1.2s", muted);
        assert_eq!(
            styled(&out[0]),
            [("Do it ".into(), Some(Red)), ("· 1.2s".into(), Some(Blue))]
        );
    }

    #[test]
    fn the_tail_is_muted_across_a_wrap() {
        use ratatui::style::Color::{Blue, Red};
        let base = Style::new().fg(Red);
        let muted = Style::new().fg(Blue);
        let out = mute_tail(
            vec![row(&[("Do it ·", base)]), row(&[("5m 2s", base)])],
            " · 5m 2s",
            muted,
        );
        assert_eq!(
            styled(&out[0]),
            [("Do it ".into(), Some(Red)), ("·".into(), Some(Blue))]
        );
        assert_eq!(styled(&out[1]), [("5m 2s".into(), Some(Blue))]);
    }
}
