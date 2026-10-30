//! The screen layout (SPEC §4, §1.4): a pure function from the screen state
//! and size to the area of every visible component.
//!
//! `App::view` draws from it, and the mouse subscriptions will be built from
//! it, so both always agree on where a component is.

use ratatui::layout::Rect;

use crate::components::{MAX_TOAST_WIDTH, TOAST_CHROME};

use super::Id;

/// The help line starts this many columns inside the selector overlay.
pub const HELP_LINE_INSET: u16 = 5;
/// The help line is this many rows above the overlay's bottom border
/// (help line, blank row, border).
pub const HELP_LINE_ROWS_FROM_BOTTOM: u16 = 3;
/// The toast's right edge is this many columns from the screen's right edge.
pub const TOAST_RIGHT_MARGIN: u16 = 1;
/// The toast's bottom border is on this many rows above the screen's last row.
pub const TOAST_BOTTOM_MARGIN: u16 = 1;
/// The toast box is this many rows high.
pub const TOAST_HEIGHT: u16 = 3;

/// The terminal is too small below this many columns.
pub const MIN_WIDTH: u16 = 60;
/// The terminal is too small below this many rows.
pub const MIN_HEIGHT: u16 = 20;
/// The selector overlay is at most this wide.
pub const MAX_OVERLAY_WIDTH: u16 = 160;

/// Which screen is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// The session selector (the startup screen).
    #[default]
    Selector,
}

/// What the layout depends on besides the size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutInput {
    /// The screen being shown.
    pub screen: Screen,
    /// The display width of the toast text, if a toast is shown.
    pub toast_text_width: Option<usize>,
}

/// The area of each visible component, in drawing order (overlays last).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenLayout {
    areas: Vec<(Id, Rect)>,
}

impl ScreenLayout {
    /// The visible components with their areas, in drawing order.
    pub fn areas(&self) -> &[(Id, Rect)] {
        &self.areas
    }

    /// The area of `id`, or `None` if it is not visible.
    pub fn area(&self, id: Id) -> Option<Rect> {
        self.areas.iter().find(|(i, _)| *i == id).map(|(_, r)| *r)
    }
}

/// Whether a screen of this size is too small to show the UI.
pub fn too_small(area: Rect) -> bool {
    area.width < MIN_WIDTH || area.height < MIN_HEIGHT
}

/// Computes the layout for a screen of `area`.
///
/// When the screen is too small (under 60 columns or 20 rows) only
/// [`Id::EmptyState`] is visible, filling the screen. Otherwise the selector
/// overlay is [`Id::SelectorFrame`]: as wide as the screen up to 160 columns
/// and centred, with one empty row above and below. Inside it, [`Id::HelpLine`]
/// is one row, two rows above the bottom border, from column 5 to width - 6.
///
/// The toast, when `input` has one, comes last (on top) in the bottom-right
/// corner of the screen, except on the too-small screen.
pub fn layout(input: &LayoutInput, area: Rect) -> ScreenLayout {
    if too_small(area) {
        return ScreenLayout {
            areas: vec![(Id::EmptyState, area)],
        };
    }
    let mut areas = match input.screen {
        Screen::Selector => {
            let overlay = selector_overlay(area);
            vec![
                (Id::SelectorFrame, overlay),
                (Id::HelpLine, selector_help_line(overlay)),
            ]
        }
    };
    if let Some(text_width) = input.toast_text_width {
        areas.push((Id::Toast, toast_rect(area, text_width)));
    }
    ScreenLayout { areas }
}

/// The help line inside the selector `overlay`.
fn selector_help_line(overlay: Rect) -> Rect {
    Rect {
        x: overlay.x + HELP_LINE_INSET,
        y: overlay.y + overlay.height - HELP_LINE_ROWS_FROM_BOTTOM,
        width: overlay.width.saturating_sub(2 * HELP_LINE_INSET),
        height: 1,
    }
}

/// The toast box for text of `text_width` columns: the text plus four columns
/// (borders and padding), at most `min(60, width - 4)`, with its last column
/// one column from the screen's right edge and its bottom border on row
/// `height - 2`.
fn toast_rect(area: Rect, text_width: usize) -> Rect {
    let most = MAX_TOAST_WIDTH.min(area.width.saturating_sub(4));
    let wanted = u16::try_from(text_width).unwrap_or(u16::MAX);
    let width = wanted.saturating_add(TOAST_CHROME).min(most);
    Rect {
        x: area.right().saturating_sub(TOAST_RIGHT_MARGIN + width),
        y: area
            .bottom()
            .saturating_sub(TOAST_BOTTOM_MARGIN + TOAST_HEIGHT),
        width,
        height: TOAST_HEIGHT,
    }
}

/// The selector overlay rect. `area` is at least [`MIN_WIDTH`] x
/// [`MIN_HEIGHT`].
fn selector_overlay(area: Rect) -> Rect {
    let width = area.width.min(MAX_OVERLAY_WIDTH);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + 1,
        width,
        height: area.height - 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(width: u16, height: u16) -> ScreenLayout {
        layout(&LayoutInput::default(), Rect::new(0, 0, width, height))
    }

    #[test]
    fn narrow_terminal_shows_only_the_empty_state() {
        let l = at(59, 24);
        assert_eq!(l.areas(), &[(Id::EmptyState, Rect::new(0, 0, 59, 24))]);
    }

    #[test]
    fn short_terminal_shows_only_the_empty_state() {
        let l = at(80, 19);
        assert_eq!(l.areas(), &[(Id::EmptyState, Rect::new(0, 0, 80, 19))]);
    }

    #[test]
    fn smallest_usable_terminal_shows_the_overlay() {
        let l = at(60, 20);
        assert_eq!(l.area(Id::SelectorFrame), Some(Rect::new(0, 1, 60, 18)));
    }

    #[test]
    fn overlay_at_80x24() {
        let l = at(80, 24);
        assert_eq!(l.area(Id::SelectorFrame), Some(Rect::new(0, 1, 80, 22)));
        assert_eq!(l.area(Id::EmptyState), None);
    }

    #[test]
    fn overlay_at_120x40_is_full_width() {
        assert_eq!(
            at(120, 40).area(Id::SelectorFrame),
            Some(Rect::new(0, 1, 120, 38))
        );
    }

    #[test]
    fn overlay_at_160_columns_is_full_width() {
        assert_eq!(
            at(160, 30).area(Id::SelectorFrame),
            Some(Rect::new(0, 1, 160, 28))
        );
    }

    #[test]
    fn overlay_at_200x50_is_160_wide_and_centred() {
        assert_eq!(
            at(200, 50).area(Id::SelectorFrame),
            Some(Rect::new(20, 1, 160, 48))
        );
    }

    #[test]
    fn odd_extra_width_puts_the_spare_column_on_the_right() {
        assert_eq!(
            at(201, 24).area(Id::SelectorFrame),
            Some(Rect::new(20, 1, 160, 22))
        );
    }

    #[test]
    fn degenerate_sizes_do_not_panic() {
        for (w, h) in [(0, 0), (1, 1), (0, 24), (80, 0), (80, 1), (80, 2)] {
            let l = at(w, h);
            assert_eq!(l.areas(), &[(Id::EmptyState, Rect::new(0, 0, w, h))]);
        }
    }

    #[test]
    fn the_layout_follows_the_area_origin() {
        let l = layout(&LayoutInput::default(), Rect::new(3, 2, 80, 24));
        assert_eq!(l.area(Id::SelectorFrame), Some(Rect::new(3, 3, 80, 22)));
    }

    fn with_toast(width: u16, height: u16, text_width: usize) -> ScreenLayout {
        let input = LayoutInput {
            toast_text_width: Some(text_width),
            ..LayoutInput::default()
        };
        layout(&input, Rect::new(0, 0, width, height))
    }

    #[test]
    fn help_line_at_80x24_is_on_row_20_from_column_5_to_74() {
        assert_eq!(at(80, 24).area(Id::HelpLine), Some(Rect::new(5, 20, 70, 1)));
    }

    #[test]
    fn help_line_at_120x40_is_on_row_36_from_column_5_to_114() {
        assert_eq!(
            at(120, 40).area(Id::HelpLine),
            Some(Rect::new(5, 36, 110, 1))
        );
    }

    #[test]
    fn help_line_is_two_rows_above_the_bottom_border() {
        for (w, h) in [(60, 20), (80, 24), (120, 40), (200, 50)] {
            let l = at(w, h);
            let overlay = l.area(Id::SelectorFrame).unwrap();
            let help = l.area(Id::HelpLine).unwrap();
            assert_eq!(help.y, overlay.y + overlay.height - 3, "{w}x{h}");
            assert_eq!(help.x, overlay.x + 5, "{w}x{h}");
            assert_eq!(help.right(), overlay.right() - 5, "{w}x{h}");
        }
    }

    #[test]
    fn help_line_follows_a_centred_overlay() {
        assert_eq!(
            at(200, 50).area(Id::HelpLine),
            Some(Rect::new(25, 46, 150, 1))
        );
    }

    #[test]
    fn there_is_no_toast_without_one_in_the_input() {
        assert_eq!(at(80, 24).area(Id::Toast), None);
    }

    #[test]
    fn toast_at_80x24_sits_in_the_bottom_right_corner() {
        let l = with_toast(80, 24, 20);
        // Width 24, right edge at column 78, bottom border on row 22.
        assert_eq!(l.area(Id::Toast), Some(Rect::new(55, 20, 24, 3)));
    }

    #[test]
    fn toast_at_120x40_sits_in_the_bottom_right_corner() {
        let l = with_toast(120, 40, 10);
        assert_eq!(l.area(Id::Toast), Some(Rect::new(105, 36, 14, 3)));
    }

    #[test]
    fn toast_edges_are_one_column_and_two_rows_inside_the_screen() {
        for (w, h) in [(60, 20), (80, 24), (120, 40), (200, 50)] {
            let toast = with_toast(w, h, 8).area(Id::Toast).unwrap();
            assert_eq!(toast.right(), w - 1, "{w}x{h}");
            assert_eq!(toast.bottom(), h - 1, "bottom border on row h-2: {w}x{h}");
        }
    }

    #[test]
    fn long_toast_text_is_capped_at_60_or_width_minus_4() {
        assert_eq!(with_toast(80, 24, 500).area(Id::Toast).unwrap().width, 60);
        assert_eq!(with_toast(120, 40, 500).area(Id::Toast).unwrap().width, 60);
        assert_eq!(with_toast(62, 24, 500).area(Id::Toast).unwrap().width, 58);
        assert_eq!(with_toast(60, 24, 500).area(Id::Toast).unwrap().width, 56);
    }

    #[test]
    fn toast_is_drawn_last() {
        let l = with_toast(80, 24, 5);
        assert_eq!(l.areas().last().map(|(id, _)| *id), Some(Id::Toast));
    }

    #[test]
    fn too_small_screen_shows_no_toast() {
        assert_eq!(with_toast(59, 24, 5).area(Id::Toast), None);
    }
}
