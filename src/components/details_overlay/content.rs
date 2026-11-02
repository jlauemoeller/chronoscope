//! What the details overlay shows (SPEC §5.3): groups of `label  value`
//! rows, and how they are laid out in the overlay.
//!
//! The content is plain data, so it can ride in a message and be snapshotted
//! as a whole. How the rows are made from the model is in [`super::rows`].

use ratatui::layout::Size;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::theme::{Role, Theme, Token};

/// The overlay is at least this wide.
pub const MIN_WIDTH: u16 = 40;
/// The columns in front of a group heading, counted inside the border (the
/// heading is at overlay column 3).
const HEADING_INDENT: usize = 2;
/// The columns in front of a row (the row is at overlay column 5).
const ROW_INDENT: usize = 4;
/// The columns between the end of the widest label and the values.
const LABEL_GAP: usize = 2;
/// The columns the overlay is wider than its widest row: the indent of the
/// rows, two columns of padding and the right border, less the left border
/// that is part of the indent.
const WIDTH_CHROME: usize = 8;
/// The rows of the border.
const BORDER_ROWS: usize = 2;
/// The text of the row that stands for a group whose source could not be
/// read.
pub const UNAVAILABLE: &str = "unavailable";

/// One `label  value` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailsRow {
    pub label: String,
    pub value: String,
    /// Whether the value is drawn in `muted_foreground`.
    pub muted: bool,
}

impl DetailsRow {
    /// A row whose value is drawn in `foreground`.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            muted: false,
        }
    }

    /// A row whose value is drawn in `muted_foreground`.
    pub fn muted(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            muted: true,
            ..Self::new(label, value)
        }
    }

    /// The muted row of a group whose source cannot be read (the file is
    /// gone, the row was deleted, the line does not parse).
    pub fn unavailable() -> Self {
        Self::muted(UNAVAILABLE, "")
    }
}

/// A heading and its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailsGroup {
    pub title: String,
    pub rows: Vec<DetailsRow>,
}

impl DetailsGroup {
    pub fn new(title: impl Into<String>, rows: Vec<DetailsRow>) -> Self {
        Self {
            title: title.into(),
            rows,
        }
    }
}

/// The groups of the details overlay, in the order they are shown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailsContent {
    groups: Vec<DetailsGroup>,
}

/// A row of the laid-out overlay, before it is styled.
enum Laid<'a> {
    Blank,
    Heading(&'a str),
    /// A part of a row's value. Only the first part of a row has the label;
    /// the others are the wrapped rest, aligned to the value column.
    Row {
        label: Option<&'a str>,
        text: String,
        muted: bool,
    },
}

impl DetailsContent {
    /// The content of `groups`. A group with no rows is left out.
    pub fn new(groups: Vec<DetailsGroup>) -> Self {
        Self {
            groups: groups.into_iter().filter(|g| !g.rows.is_empty()).collect(),
        }
    }

    /// The groups, in the order they are shown.
    pub fn groups(&self) -> &[DetailsGroup] {
        &self.groups
    }

    /// Whether there is nothing to show.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// The width of the label column: the widest label of all groups, so the
    /// values start in one column.
    fn label_width(&self) -> usize {
        self.groups
            .iter()
            .flat_map(|g| &g.rows)
            .map(|row| row.label.width())
            .max()
            .unwrap_or(0)
    }

    /// The column of the values, counted from the start of a row.
    fn value_column(&self) -> usize {
        self.label_width() + LABEL_GAP
    }

    /// The width of the widest row: the label column and the widest value.
    fn widest_row(&self) -> usize {
        let value = self
            .groups
            .iter()
            .flat_map(|g| &g.rows)
            .flat_map(|row| row.value.split('\n'))
            .map(UnicodeWidthStr::width)
            .max()
            .unwrap_or(0);
        self.value_column() + value
    }

    /// The width a value can take in an interior `interior_width` columns
    /// wide: what is left of the row after the indent, the label column and
    /// the padding. Never less than one column.
    fn value_width(&self, interior_width: u16) -> usize {
        let row = usize::from(interior_width).saturating_sub(WIDTH_CHROME - 2);
        row.saturating_sub(self.value_column()).max(1)
    }

    /// The rows for an interior `interior_width` columns wide: a blank row,
    /// then for each group its heading, a blank row, its rows and a blank
    /// row. A value wider than the interior is char-wrapped.
    fn layout(&self, interior_width: u16) -> Vec<Laid<'_>> {
        let value_width = self.value_width(interior_width);
        let mut laid = vec![Laid::Blank];
        for group in &self.groups {
            laid.push(Laid::Heading(&group.title));
            laid.push(Laid::Blank);
            for row in &group.rows {
                let mut label = Some(row.label.as_str());
                for line in row.value.split('\n') {
                    for text in char_wrap(line, value_width) {
                        laid.push(Laid::Row {
                            label: label.take(),
                            text,
                            muted: row.muted,
                        });
                    }
                }
            }
            laid.push(Laid::Blank);
        }
        laid
    }

    /// The number of rows the content takes in an interior
    /// `interior_width` columns wide.
    pub fn row_count(&self, interior_width: u16) -> usize {
        self.layout(interior_width).len()
    }

    /// The size the overlay wants, borders included: as wide as the widest
    /// row plus eight columns (at least [`MIN_WIDTH`], at most `max_width`),
    /// and as high as the content at that width plus the border.
    pub fn desired_size(&self, max_width: u16) -> Size {
        let wanted = (self.widest_row() + WIDTH_CHROME).max(usize::from(MIN_WIDTH));
        let width = u16::try_from(wanted).unwrap_or(u16::MAX).min(max_width);
        let rows = self.row_count(width.saturating_sub(2));
        Size::new(width, u16::try_from(rows + BORDER_ROWS).unwrap_or(u16::MAX))
    }

    /// The rows, top to bottom, styled with `theme`, for an interior
    /// `interior_width` columns wide.
    pub fn lines(&self, theme: &Theme, interior_width: u16) -> Vec<Line<'static>> {
        let heading = Style::new()
            .fg(theme.role(Role::Foreground))
            .add_modifier(Modifier::BOLD);
        let label_style = theme.style(Token::Label);
        let value_style = Style::new().fg(theme.role(Role::Foreground));
        let muted_style = Style::new().fg(theme.role(Role::MutedForeground));
        let label_width = self.label_width();
        self.layout(interior_width)
            .into_iter()
            .map(|laid| match laid {
                Laid::Blank => Line::default(),
                Laid::Heading(title) => Line::from(vec![
                    Span::raw(" ".repeat(HEADING_INDENT)),
                    Span::styled(title.to_owned(), heading),
                ]),
                Laid::Row { label, text, muted } => {
                    let label = label.unwrap_or_default();
                    let padding = label_width.saturating_sub(label.width()) + LABEL_GAP;
                    let style = if muted { muted_style } else { value_style };
                    // A muted row with a label only (`unavailable`) is
                    // muted as a whole.
                    let label_style = if muted && text.is_empty() {
                        muted_style
                    } else {
                        label_style
                    };
                    Line::from(vec![
                        Span::raw(" ".repeat(ROW_INDENT)),
                        Span::styled(label.to_owned(), label_style),
                        Span::raw(" ".repeat(padding)),
                        Span::styled(text, style),
                    ])
                }
            })
            .collect()
    }
}

/// `text` cut into pieces at most `width` columns wide, at any character
/// (paths and long tokens are never hidden, SPEC §5.5). Always at least one
/// piece. A character wider than `width` gets a piece of its own.
fn char_wrap(text: &str, width: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let cols = ch.width().unwrap_or(0);
        if used + cols > width && !current.is_empty() {
            pieces.push(std::mem::take(&mut current));
            used = 0;
        }
        current.push(ch);
        used += cols;
    }
    pieces.push(current);
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::builtin;

    fn row(label: &str, value: &str) -> DetailsRow {
        DetailsRow::new(label, value)
    }

    fn content() -> DetailsContent {
        DetailsContent::new(vec![
            DetailsGroup::new(
                "Turn",
                vec![row("model", "claude-opus-5-5"), row("effort", "high")],
            ),
            DetailsGroup::new("Tool call", vec![row("duration", "1m 2s")]),
        ])
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn a_group_without_rows_is_left_out() {
        let content = DetailsContent::new(vec![
            DetailsGroup::new("Turn", Vec::new()),
            DetailsGroup::new("Tool call", vec![row("status", "ok")]),
        ]);
        assert_eq!(content.groups().len(), 1);
        assert_eq!(content.groups()[0].title, "Tool call");
        assert!(DetailsContent::new(vec![DetailsGroup::new("Turn", Vec::new())]).is_empty());
    }

    #[test]
    fn the_rows_are_a_blank_row_then_per_group_a_heading_a_blank_row_the_rows_and_a_blank_row() {
        let theme = builtin::default_theme();
        let lines = content().lines(&theme, 60);
        let texts: Vec<String> = lines.iter().map(text).collect();
        assert_eq!(
            texts,
            [
                "",
                "  Turn",
                "",
                "    model     claude-opus-5-5",
                "    effort    high",
                "",
                "  Tool call",
                "",
                "    duration  1m 2s",
                "",
            ]
        );
        assert_eq!(content().row_count(60), texts.len());
    }

    #[test]
    fn the_values_start_in_one_column_across_the_groups() {
        let theme = builtin::default_theme();
        let lines = content().lines(&theme, 60);
        // The widest label of any group is `duration` (8): 4 + 8 + 2.
        for index in [3, 4, 8] {
            let spans = &lines[index].spans;
            let before: usize = spans[..3].iter().map(|s| s.content.width()).sum();
            assert_eq!(before, 14, "{}", text(&lines[index]));
        }
    }

    #[test]
    fn the_styles_come_from_the_theme() {
        let theme = builtin::default_theme();
        let lines = content().lines(&theme, 60);
        let heading = &lines[1].spans[1];
        assert_eq!(heading.style.fg, Some(theme.role(Role::Foreground)));
        assert!(heading.style.add_modifier.contains(Modifier::BOLD));
        let label = &lines[3].spans[1];
        assert_eq!(label.style, theme.style(Token::Label));
        let value = &lines[3].spans[3];
        assert_eq!(value.style.fg, Some(theme.role(Role::Foreground)));
    }

    #[test]
    fn a_muted_row_is_drawn_in_muted_foreground() {
        let theme = builtin::default_theme();
        let content = DetailsContent::new(vec![DetailsGroup::new(
            "Message",
            vec![row("id", "x"), DetailsRow::muted("note", "later")],
        )]);
        let lines = content.lines(&theme, 60);
        assert_eq!(
            lines[3].spans[3].style.fg,
            Some(theme.role(Role::Foreground))
        );
        assert_eq!(
            lines[4].spans[3].style.fg,
            Some(theme.role(Role::MutedForeground))
        );
    }

    #[test]
    fn the_unavailable_row_is_one_muted_row() {
        let theme = builtin::default_theme();
        let content = DetailsContent::new(vec![DetailsGroup::new(
            "Message",
            vec![DetailsRow::unavailable()],
        )]);
        let lines = content.lines(&theme, 60);
        assert_eq!(text(&lines[3]).trim(), "unavailable");
        assert_eq!(
            lines[3].spans[1].style.fg,
            Some(theme.role(Role::MutedForeground))
        );
    }

    #[test]
    fn a_long_value_is_char_wrapped_with_a_hanging_indent_at_the_value_column() {
        let theme = builtin::default_theme();
        let content = DetailsContent::new(vec![DetailsGroup::new(
            "Environment",
            vec![
                row("cwd", "/home/user/projects/brio/src"),
                row("version", "2"),
            ],
        )]);
        // An interior of 28: 28 - 6 for the padding = 22 for the row, less
        // the value column (7 + 2) = 13 columns for a value.
        let texts: Vec<String> = content.lines(&theme, 28).iter().map(text).collect();
        assert_eq!(
            texts[3..7],
            [
                "    cwd      /home/user/pr",
                "             ojects/brio/s",
                "             rc",
                "    version  2",
            ]
        );
        assert_eq!(content.row_count(28), texts.len());
    }

    #[test]
    fn a_wrapped_value_never_loses_a_character() {
        let theme = builtin::default_theme();
        let value = "a-very-long-token-with-no-spaces-at-all-in-it-0123456789";
        let content = DetailsContent::new(vec![DetailsGroup::new("G", vec![row("k", value)])]);
        for width in [8, 9, 12, 20, 40] {
            let joined: String = content
                .lines(&theme, width)
                .iter()
                .skip(3)
                .take_while(|l| !text(l).is_empty())
                .map(|l| l.spans[3].content.to_string())
                .collect();
            assert_eq!(joined, value, "width {width}");
        }
    }

    #[test]
    fn a_value_with_newlines_takes_a_row_per_line() {
        let theme = builtin::default_theme();
        let content =
            DetailsContent::new(vec![DetailsGroup::new("G", vec![row("error", "one\ntwo")])]);
        let texts: Vec<String> = content.lines(&theme, 40).iter().map(text).collect();
        assert_eq!(texts[3], "    error  one");
        assert_eq!(texts[4], "           two");
    }

    #[test]
    fn wide_characters_wrap_by_columns() {
        assert_eq!(char_wrap("日本語", 4), ["日本", "語"]);
        assert_eq!(char_wrap("日本語", 1), ["日", "本", "語"]);
        assert_eq!(char_wrap("", 5), [""]);
    }

    #[test]
    fn a_degenerate_width_does_not_panic() {
        let theme = builtin::default_theme();
        for width in [0, 1, 2, 5, 6, 7] {
            let lines = content().lines(&theme, width);
            assert_eq!(lines.len(), content().row_count(width));
        }
    }

    #[test]
    fn the_desired_size_is_the_widest_row_plus_eight_and_the_content_plus_two() {
        let content = DetailsContent::new(vec![DetailsGroup::new(
            "G",
            vec![row("duration", &"x".repeat(50))],
        )]);
        // 8 + 2 + 50 + 8, over 1 + 3 + 1 rows... a blank, the heading, a
        // blank, the row and a blank, plus the border.
        assert_eq!(content.desired_size(200), Size::new(68, 5 + 2));
    }

    #[test]
    fn the_desired_width_is_at_least_forty() {
        assert_eq!(content().desired_size(200).width, MIN_WIDTH);
    }

    #[test]
    fn the_desired_size_wraps_at_the_most_width() {
        let content = DetailsContent::new(vec![DetailsGroup::new(
            "G",
            vec![row("cwd", &"x".repeat(100))],
        )]);
        let size = content.desired_size(50);
        assert_eq!(size.width, 50);
        // 48 - 6 = 42 for the row, less 5 for the value column: 37 columns,
        // so 100 characters take three rows (a blank row, the heading, a
        // blank row, the three and a blank row).
        assert_eq!(content.row_count(48), 7);
        assert_eq!(usize::from(size.height), 7 + 2);
    }
}
