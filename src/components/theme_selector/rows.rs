//! The theme rows of the theme selector (SPEC §8.2, §8.4).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::components::truncate_with_ellipsis;
use crate::theme::{Role, Theme};

/// The columns before the marker, counted from the inside of the border: the
/// marker is in column 3 of the overlay, its border being column 0.
const MARKER_INDENT: &str = "  ";
/// What stands in place of the marker on the rows that are not selected.
const NO_MARKER: &str = " ";
/// What stands between the marker and the label.
const LABEL_GAP: &str = " ";
/// The columns of a row that are not label: the indent, the marker, the gap
/// and one column before the border.
const ROW_CHROME: usize = 5;

/// One theme as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeItem {
    /// The id used in `config.toml`.
    pub id: String,
    /// The text of the row: the theme's name, or `<id> (invalid)`.
    pub label: String,
    /// Whether the cursor can stop on the row.
    pub selectable: bool,
}

impl ThemeItem {
    /// A theme that can be selected, shown as `label`.
    pub fn valid(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selectable: true,
        }
    }

    /// A user theme that failed to load: shown as `<id> (invalid)`, never
    /// selected.
    pub fn invalid(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            label: format!("{id} (invalid)"),
            id,
            selectable: false,
        }
    }
}

/// The visible part of the list: `items` from `offset`, as many as fit in the
/// area it is rendered into, one per row. The row at `selected` has the
/// marker and the bright bold label; the others are muted.
pub struct ThemeRows<'a> {
    pub items: &'a [ThemeItem],
    pub selected: Option<usize>,
    pub offset: usize,
    pub theme: &'a Theme,
}

impl ThemeRows<'_> {
    /// The row for `item`, its label cut to `width` columns.
    fn line(&self, index: usize, item: &ThemeItem, width: usize) -> Line<'static> {
        let label = truncate_with_ellipsis(&item.label, width.saturating_sub(ROW_CHROME));
        if self.selected == Some(index) {
            let marker = Style::new().fg(self.theme.role(Role::Primary));
            let bold = Style::new()
                .fg(self.theme.role(Role::Foreground))
                .add_modifier(Modifier::BOLD);
            Line::from(vec![
                Span::raw(MARKER_INDENT),
                Span::styled(">", marker),
                Span::raw(LABEL_GAP),
                Span::styled(label, bold),
            ])
        } else {
            let muted = Style::new().fg(self.theme.role(Role::MutedForeground));
            Line::from(Span::styled(
                format!("{MARKER_INDENT}{NO_MARKER}{LABEL_GAP}{label}"),
                muted,
            ))
        }
    }
}

impl Widget for ThemeRows<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self
            .items
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(usize::from(area.height))
            .map(|(index, item)| self.line(index, item, usize::from(area.width)))
            .collect();
        Paragraph::new(lines).render(area, buf);
    }
}
