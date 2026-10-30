//! The screen layout (SPEC §4, §1.4): a pure function from the screen state
//! and size to the area of every visible component.
//!
//! `App::view` draws from it, and the mouse subscriptions will be built from
//! it, so both always agree on where a component is.

use ratatui::layout::Rect;

use super::Id;

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
/// and centred, with one empty row above and below.
pub fn layout(input: &LayoutInput, area: Rect) -> ScreenLayout {
    if too_small(area) {
        return ScreenLayout {
            areas: vec![(Id::EmptyState, area)],
        };
    }
    let areas = match input.screen {
        Screen::Selector => vec![(Id::SelectorFrame, selector_overlay(area))],
    };
    ScreenLayout { areas }
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
        assert_eq!(l.areas(), &[(Id::SelectorFrame, Rect::new(0, 1, 60, 18))]);
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
}
