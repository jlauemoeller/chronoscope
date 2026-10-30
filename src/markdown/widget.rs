//! The `Markdown` widget.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use super::render_markdown;
use crate::theme::{Role, Theme};

/// Draws markdown into an area, laid out for the area's width.
///
/// The layout is recomputed on every render, so a resize re-wraps. Lines
/// beyond the height of the area are cut off. It paints no background.
#[derive(Debug, Clone, Copy)]
pub struct Markdown<'a> {
    source: &'a str,
    theme: &'a Theme,
    base: Style,
}

impl<'a> Markdown<'a> {
    /// A widget for `source` whose text style is `foreground`.
    pub fn new(source: &'a str, theme: &'a Theme) -> Self {
        Self {
            source,
            theme,
            base: Style::new().fg(theme.role(Role::Foreground)),
        }
    }

    /// Sets the text style the element styles are derived from.
    #[must_use]
    pub fn base(mut self, base: Style) -> Self {
        self.base = base;
        self
    }
}

impl Widget for Markdown<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = render_markdown(self.source, area.width, self.base, self.theme);
        for (row, line) in lines.iter().take(usize::from(area.height)).enumerate() {
            let y = area.y + u16::try_from(row).unwrap_or(u16::MAX);
            buf.set_line(area.x, y, line, area.width);
        }
    }
}
