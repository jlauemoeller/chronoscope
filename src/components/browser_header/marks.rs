//! What is right-aligned on the header's working-directory row: the
//! parse-warning note and the `● live` marker.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{Role, Theme};

/// The dot of the live marker, drawn in `primary`.
pub(super) const LIVE_DOT: &str = "●";
/// The word after the dot, drawn in `muted_foreground`.
pub(super) const LIVE_WORD: &str = " live";

/// The note that entries could not be parsed, in `muted_foreground`, cut
/// at the area's right edge.
#[derive(Debug, Clone, Copy)]
pub(super) struct ParseWarningNote<'a> {
    pub(super) text: &'a str,
    pub(super) theme: &'a Theme,
}

impl Widget for ParseWarningNote<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let muted = Style::new().fg(self.theme.role(Role::MutedForeground));
        Line::from(Span::styled(self.text, muted)).render(area, buf);
    }
}

/// `● live`: the dot in `primary` and the word in `muted_foreground`, cut
/// at the area's right edge.
#[derive(Debug, Clone, Copy)]
pub(super) struct LiveMarker<'a> {
    pub(super) theme: &'a Theme,
}

impl Widget for LiveMarker<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dot = Style::new().fg(self.theme.role(Role::Primary));
        let word = Style::new().fg(self.theme.role(Role::MutedForeground));
        Line::from(vec![
            Span::styled(LIVE_DOT, dot),
            Span::styled(LIVE_WORD, word),
        ])
        .render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::*;
    use crate::theme::builtin;

    fn theme() -> Theme {
        builtin::default_theme()
            .with_role(Role::Primary, Color::Rgb(7, 200, 7))
            .with_role(Role::MutedForeground, Color::Rgb(2, 250, 2))
            .with_role(Role::Background, Color::Rgb(9, 9, 9))
    }

    /// `widget` drawn in a `width` x 1 area at column 1 of a buffer two
    /// columns wider, which is painted with `background` first.
    fn draw(widget: impl Widget, width: u16, theme: &Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width + 2, 1));
        buf.set_style(buf.area, Style::new().bg(theme.role(Role::Background)));
        widget.render(Rect::new(1, 0, width, 1), &mut buf);
        buf
    }

    fn cells(buf: &Buffer) -> Vec<(String, Color, Color)> {
        buf.content()
            .iter()
            .map(|cell| (cell.symbol().to_owned(), cell.fg, cell.bg))
            .collect()
    }

    #[test]
    fn the_note_is_drawn_in_muted_foreground_on_what_is_under_it() {
        let theme = theme();
        let buf = draw(
            ParseWarningNote {
                text: "1 entry",
                theme: &theme,
            },
            7,
            &theme,
        );
        let (muted, bg) = (
            theme.role(Role::MutedForeground),
            theme.role(Role::Background),
        );
        let mut expected = vec![(" ".to_owned(), Color::Reset, bg)];
        expected.extend("1 entry".chars().map(|c| (c.to_string(), muted, bg)));
        expected.push((" ".to_owned(), Color::Reset, bg));
        assert_eq!(cells(&buf), expected);
    }

    #[test]
    fn the_note_is_cut_at_the_edge_of_its_area() {
        let theme = theme();
        let buf = draw(
            ParseWarningNote {
                text: "1 entry",
                theme: &theme,
            },
            3,
            &theme,
        );
        let symbols: String = cells(&buf).into_iter().map(|(s, _, _)| s).collect();
        assert_eq!(symbols, " 1 e ");
    }

    #[test]
    fn the_marker_is_a_primary_dot_and_a_muted_word() {
        let theme = theme();
        let buf = draw(LiveMarker { theme: &theme }, 6, &theme);
        let (primary, muted) = (theme.role(Role::Primary), theme.role(Role::MutedForeground));
        let bg = theme.role(Role::Background);
        let expected: Vec<(String, Color, Color)> = [
            (" ", Color::Reset),
            ("●", primary),
            (" ", muted),
            ("l", muted),
            ("i", muted),
            ("v", muted),
            ("e", muted),
            (" ", Color::Reset),
        ]
        .into_iter()
        .map(|(s, fg)| (s.to_owned(), fg, bg))
        .collect();
        assert_eq!(cells(&buf), expected);
    }

    #[test]
    fn the_marker_is_cut_at_the_edge_of_its_area() {
        let theme = theme();
        let buf = draw(LiveMarker { theme: &theme }, 3, &theme);
        let symbols: String = cells(&buf).into_iter().map(|(s, _, _)| s).collect();
        assert_eq!(symbols, " ● l ");
    }
}
