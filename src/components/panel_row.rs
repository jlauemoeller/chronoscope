//! One label-and-value row of the Session Time and Session Tokens panels
//! (product §3.2).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use super::text_fit::truncate_with_ellipsis;
use crate::theme::{Role, Theme};

/// The columns between a panel's border and its rows' text on each side:
/// one blank column and the border.
const SIDE: u16 = 2;

/// A row with a label on the left in `muted_foreground` and a value on the
/// right in `foreground`. The value is kept whole; the label gets what
/// remains less one blank cell, and is cut with `…` or left out when there is
/// no room. A value that is too wide for the row is cut itself.
#[derive(Debug, Clone, Copy)]
pub struct PanelRow<'a> {
    label: &'a str,
    value: &'a str,
    label_style: Style,
    value_style: Style,
}

impl<'a> PanelRow<'a> {
    /// A row of `label` and `value`, styled from `theme`.
    pub fn new(label: &'a str, value: &'a str, theme: &Theme) -> Self {
        Self {
            label,
            value,
            label_style: Style::new().fg(theme.role(Role::MutedForeground)),
            value_style: Style::new().fg(theme.role(Role::Foreground)),
        }
    }

    /// The area of row `index` of a panel drawn in `panel`: from column 2 to
    /// two columns before the right edge, below the border and the rows
    /// before it. `None` when the row does not fit inside the borders.
    pub fn area_in(panel: Rect, index: u16) -> Option<Rect> {
        let width = panel.width.checked_sub(2 * SIDE)?;
        let y = panel.y.checked_add(1)?.checked_add(index)?;
        // The bottom border is the last row of the panel.
        if width == 0 || y >= panel.bottom().saturating_sub(1) {
            return None;
        }
        Some(Rect::new(panel.x + SIDE, y, width, 1))
    }
}

impl Widget for PanelRow<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let width = usize::from(area.width);
        let value = truncate_with_ellipsis(self.value, width);
        let value_width = value.width();
        let value_x = area.x + area.width - u16::try_from(value_width).unwrap_or(area.width);
        buf.set_stringn(value_x, area.y, &value, value_width, self.value_style);
        let room = width.saturating_sub(value_width + 1);
        if room > 0 {
            let label = truncate_with_ellipsis(self.label, room);
            buf.set_stringn(area.x, area.y, &label, room, self.label_style);
        }
    }
}

/// The rows of a panel: all of them, or none when they do not all fit inside
/// the panel's borders. Drawn over the panel's whole area, border included.
#[derive(Debug, Clone)]
pub struct PanelRows<'a> {
    rows: Vec<(&'static str, String)>,
    theme: &'a Theme,
}

impl<'a> PanelRows<'a> {
    /// The `(label, value)` rows, styled from `theme`.
    pub fn new(rows: Vec<(&'static str, String)>, theme: &'a Theme) -> Self {
        Self { rows, theme }
    }
}

impl Widget for PanelRows<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let Ok(count) = u16::try_from(self.rows.len()) else {
            return;
        };
        // Rows show together: if the last one has no place, none is drawn.
        if count == 0 || PanelRow::area_in(area, count - 1).is_none() {
            return;
        }
        for (index, (label, value)) in (0..).zip(&self.rows) {
            if let Some(row) = PanelRow::area_in(area, index) {
                PanelRow::new(label, value, self.theme).render(row, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_text, render};
    use crate::theme::builtin;

    fn row(label: &str, value: &str, width: u16) -> String {
        let buffer = render(width, 1, |frame| {
            let area = frame.area();
            frame.render_widget(PanelRow::new(label, value, &builtin::default_theme()), area);
        });
        buffer_to_text(&buffer)
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned()
    }

    #[test]
    fn the_label_is_left_and_the_value_right() {
        assert_eq!(row("Total", "81h 33m", 20), "Total        81h 33m");
    }

    #[test]
    fn a_label_that_does_not_fit_is_cut_and_the_value_is_kept() {
        // 10 cells: the value takes 6, one is blank, 3 are left for the label.
        assert_eq!(row("Cache read", "862.3M", 10), "Ca… 862.3M");
        assert_eq!(row("Cache read", "862.3M", 17), "Cache read 862.3M");
    }

    #[test]
    fn a_label_with_no_room_is_not_drawn() {
        assert_eq!(row("Cache read", "862.3M", 7), " 862.3M");
        assert_eq!(row("Cache read", "862.3M", 6), "862.3M");
    }

    #[test]
    fn a_value_too_wide_is_cut() {
        assert_eq!(row("Total", "81h 33m", 4), "81h…");
        assert_eq!(row("Total", "81h 33m", 1), "…");
    }

    #[test]
    fn tiny_areas_do_not_panic() {
        for width in 0..12 {
            let _ = row("Cache write", "30.7M", width);
        }
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 2));
        PanelRow::new("Input", "15.8k", &builtin::default_theme())
            .render(Rect::new(2, 1, 20, 5), &mut buffer);
    }

    #[test]
    fn rows_show_together_or_not_at_all() {
        let theme = builtin::default_theme();
        let rows = || vec![("Total", "1m 0s".to_owned()), ("Agent", "5s".to_owned())];
        let drawn = |height| {
            let buffer = render(20, height, |frame| {
                let area = frame.area();
                frame.render_widget(PanelRows::new(rows(), &theme), area);
            });
            buffer_to_text(&buffer)
        };
        let text = drawn(4);
        assert!(text.contains("Total") && text.contains("Agent"), "{text}");
        for height in [0, 1, 2, 3] {
            let text = drawn(height);
            assert!(!text.contains("Total") && !text.contains("Agent"), "{text}");
        }
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 3));
        PanelRows::new(Vec::new(), &theme).render(buffer.area, &mut buffer);
    }

    #[test]
    fn rows_are_placed_one_column_inside_each_border() {
        let panel = Rect::new(3, 10, 31, 4);
        assert_eq!(PanelRow::area_in(panel, 0), Some(Rect::new(5, 11, 27, 1)));
        assert_eq!(PanelRow::area_in(panel, 1), Some(Rect::new(5, 12, 27, 1)));
        assert_eq!(PanelRow::area_in(panel, 2), None);
        assert_eq!(PanelRow::area_in(Rect::new(0, 0, 4, 4), 0), None);
        assert_eq!(
            PanelRow::area_in(Rect::new(0, 0, 5, 4), 0).map(|a| a.width),
            Some(1)
        );
        assert_eq!(PanelRow::area_in(Rect::new(0, 0, 0, 0), 0), None);
    }
}
