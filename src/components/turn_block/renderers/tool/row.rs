//! The description row of a tool call: the description, the agent type of a
//! subagent and the muted duration after them.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::super::{ellipsize, word_wrap};

/// What stays at the end of a folded row when the description is cut.
#[derive(Debug, Clone, Copy)]
pub(super) struct Tail<'a> {
    /// ` (agent)`, in the style of the description.
    pub agent: Option<&'a str>,
    /// ` · 1.2s`, muted.
    pub duration: Option<&'a str>,
}

impl Tail<'_> {
    fn width(&self) -> usize {
        [self.agent, self.duration]
            .into_iter()
            .flatten()
            .map(UnicodeWidthStr::width)
            .sum()
    }
}

/// The folded row: `description` is cut with `…` first and the `tail` stays.
/// When the tail leaves no room for the description (it needs at least one
/// cell and the `…`), the whole row is cut instead.
pub(super) fn folded(
    description: &str,
    tail: Tail<'_>,
    width: usize,
    style: Style,
    muted: Style,
) -> Line<'static> {
    let tail_width = tail.width();
    if tail_width == 0 {
        let line = Line::from(Span::styled(description.to_owned(), style));
        return ellipsize(line, width, false);
    }
    let mut tail_spans = Vec::new();
    if let Some(agent) = tail.agent {
        tail_spans.push(Span::styled(agent.to_owned(), style));
    }
    if let Some(duration) = tail.duration {
        tail_spans.push(Span::styled(duration.to_owned(), muted));
    }
    if tail_width + 1 >= width {
        let mut spans = vec![Span::styled(description.to_owned(), style)];
        spans.extend(tail_spans);
        return cut(Line::from(spans), width);
    }
    let line = Line::from(Span::styled(description.to_owned(), style));
    let mut line = ellipsize(line, width - tail_width, false);
    line.spans.extend(tail_spans);
    line
}

/// `line` cut to `width` cells. The `…` takes the style of the text it
/// replaces, not that of the last span of the line.
fn cut(line: Line<'static>, width: usize) -> Line<'static> {
    if line.width() <= width {
        return line;
    }
    let mut cut = ellipsize(line, width, false);
    if let Some(style) = cut
        .spans
        .len()
        .checked_sub(2)
        .map(|before| cut.spans[before].style)
        && let Some(ellipsis) = cut.spans.last_mut()
    {
        ellipsis.style = style;
    }
    cut
}

/// The unfolded rows: `text` word-wrapped at `width`, and the muted
/// `duration` (` · 1.2s`) after it. The duration is kept in one piece: it
/// goes on the last row when it fits there, else on a row of its own
/// (`· 1.2s`), and is broken only when it is wider than a row.
pub(super) fn wrapped(
    text: &str,
    duration: Option<&str>,
    width: u16,
    style: Style,
    muted: Style,
) -> Vec<Line<'static>> {
    let mut rows = word_wrap(text, width, style);
    let Some(duration) = duration else {
        return rows;
    };
    let room = usize::from(width.max(1));
    let alone = duration.trim_start();
    let last = rows.last_mut().expect("word_wrap gives a row");
    if last.width() + duration.width() <= room {
        last.spans.push(Span::styled(duration.to_owned(), muted));
    } else if alone.width() <= room {
        rows.push(Line::from(Span::styled(alone.to_owned(), muted)));
    } else {
        rows.extend(word_wrap(alone, width, muted));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color::{Blue, Red};

    fn base() -> Style {
        Style::new().fg(Red)
    }

    fn muted() -> Style {
        Style::new().fg(Blue)
    }

    fn texts(rows: &[Line<'_>]) -> Vec<String> {
        rows.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn the_duration_follows_the_last_row_when_it_fits() {
        let rows = wrapped("Do it", Some(" · 1.2s"), 20, base(), muted());
        assert_eq!(texts(&rows), ["Do it · 1.2s"]);
        let styles: Vec<_> = rows[0].spans.iter().map(|s| s.style.fg).collect();
        assert_eq!(styles, [Some(Red), Some(Blue)]);
    }

    #[test]
    fn the_duration_goes_to_a_row_of_its_own_rather_than_be_split() {
        // `Do it now ·` would fit, `5m 2s` would not.
        let rows = wrapped("Do it now", Some(" · 5m 2s"), 12, base(), muted());
        assert_eq!(texts(&rows), ["Do it now", "· 5m 2s"]);
        assert_eq!(rows[1].spans[0].style.fg, Some(Blue));
        // The full width is still usable.
        let rows = wrapped("Do it now", Some(" · 5m 2s"), 17, base(), muted());
        assert_eq!(texts(&rows), ["Do it now · 5m 2s"]);
    }

    #[test]
    fn a_wrapped_description_keeps_its_style_and_the_duration_is_muted() {
        let rows = wrapped("aaa bbb ccc ddd", Some(" · 340ms"), 8, base(), muted());
        assert_eq!(texts(&rows), ["aaa bbb", "ccc ddd", "· 340ms"]);
        assert!(rows[..2].iter().all(|r| r.spans[0].style.fg == Some(Red)));
        assert_eq!(rows[2].spans[0].style.fg, Some(Blue));
    }

    #[test]
    fn a_duration_wider_than_a_row_is_broken() {
        let rows = wrapped("Do", Some(" · 1.2s"), 4, base(), muted());
        assert_eq!(texts(&rows), ["Do", "·", "1.2s"]);
        assert!(rows[1..].iter().all(|r| r.spans[0].style.fg == Some(Blue)));
    }

    #[test]
    fn no_duration_wraps_as_before() {
        let rows = wrapped("aaa bbb ccc", None, 8, base(), muted());
        assert_eq!(texts(&rows), ["aaa bbb", "ccc"]);
    }

    #[test]
    fn the_folded_row_cuts_the_description_and_keeps_the_tail() {
        let tail = Tail {
            agent: Some(" (agent)"),
            duration: Some(" · 5m 2s"),
        };
        let row = folded("A long description", tail, 30, base(), muted());
        assert_eq!(row.to_string(), "A long descri… (agent) · 5m 2s");
        let fgs: Vec<_> = row.spans.iter().map(|s| s.style.fg).collect();
        assert_eq!(fgs, [Some(Red), Some(Red), Some(Red), Some(Blue)]);
    }

    #[test]
    fn a_tail_without_room_for_the_description_cuts_the_whole_row() {
        let tail = Tail {
            agent: None,
            duration: Some(" · 5m 2s"),
        };
        let row = folded("A long description", tail, 8, base(), muted());
        assert_eq!(row.to_string(), "A long …");
        // The `…` replaces description text.
        assert_eq!(row.spans.last().unwrap().style.fg, Some(Red));
    }
}
