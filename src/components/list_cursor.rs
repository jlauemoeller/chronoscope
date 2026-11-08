//! The selection of a list and its scroll offset, and the attributes that set
//! them: what `ProjectList`, `SessionList`, `ThemeSelector` and
//! `TurnSidebar` share.

use tuirealm::props::{AttrValue, Attribute, PropPayload, PropValue};

/// The attribute that sets which row is selected (`None` for no selection).
/// Pushing it never produces a message.
pub const SELECTED_ATTR: Attribute = Attribute::Custom("selected");
/// The attribute that tells a list how many rows its pane shows, so scrolling
/// keeps the selection visible. Set from the layout, never from `view`.
pub const VISIBLE_ROWS_ATTR: Attribute = Attribute::Custom("visible_rows");

/// The selection of a list and its scroll offset. It never wraps around.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListCursor {
    len: usize,
    selected: Option<usize>,
    offset: usize,
    /// How many rows are visible; 0 while not known.
    visible: usize,
}

impl ListCursor {
    /// The number of rows.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the list has no rows.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The selected row, if any.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The first visible row.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// How many rows are visible; 0 while not known.
    pub fn visible(&self) -> usize {
        self.visible
    }

    /// Sets the number of rows and the selection (ignored when out of range)
    /// and scrolls the least that keeps the selection visible. Without a
    /// selection the list shows its first rows: the offset of the list it
    /// replaces means nothing for the new one, so new rows also undo a
    /// scroll made with [`scroll`](Self::scroll) while nothing was selected.
    pub fn set(&mut self, len: usize, selected: Option<usize>) {
        self.len = len;
        self.selected = selected.filter(|&i| i < len);
        if self.selected.is_none() {
            self.offset = 0;
        }
        self.reveal();
    }

    /// Like [`set`](Self::set), starting from the first visible row `offset`
    /// (as an earlier [`offset`](Self::offset) of the same list): it scrolls
    /// only as far as needed from there, so a list that comes back is shown
    /// as it was left.
    pub fn restore(&mut self, len: usize, selected: Option<usize>, offset: usize) {
        self.offset = offset;
        self.set(len, selected);
    }

    /// Sets the selection, keeping the rows.
    pub fn select(&mut self, selected: Option<usize>) {
        self.set(self.len, selected);
    }

    /// Sets how many rows are visible.
    pub fn set_visible(&mut self, visible: usize) {
        self.visible = visible;
        self.reveal();
    }

    /// Scrolls the rows by one without moving the selection, keeping the
    /// first row between 0 and `len - visible` (`visible` rows are shown).
    /// Returns whether the list scrolled.
    pub fn scroll(&mut self, down: bool, visible: usize) -> bool {
        let last = self.len.saturating_sub(visible);
        let from = self.offset.min(last);
        let to = if down {
            (from + 1).min(last)
        } else {
            from.saturating_sub(1)
        };
        self.offset = to;
        to != from
    }

    /// Selects the previous row. Returns whether the selection changed; at
    /// the first row (or without a selection) it does not.
    pub fn step_up(&mut self) -> bool {
        match self.selected {
            Some(i) if i > 0 => {
                self.select(Some(i - 1));
                true
            }
            _ => false,
        }
    }

    /// Selects the next row; with no selection the first. Returns whether
    /// the selection changed; at the last row it does not.
    pub fn step_down(&mut self) -> bool {
        match self.selected {
            Some(i) if i + 1 < self.len => self.select(Some(i + 1)),
            None if self.len > 0 => self.select(Some(0)),
            _ => return false,
        }
        true
    }

    /// The first row to draw when `visible` rows are shown: the stored offset,
    /// moved the least that shows the selection. Changes nothing.
    pub fn offset_for(&self, visible: usize) -> usize {
        scrolled_offset(self.offset, self.selected, visible, self.len)
    }

    fn reveal(&mut self) {
        self.offset = scrolled_offset(self.offset, self.selected, self.visible, self.len);
    }
}

fn scrolled_offset(offset: usize, selected: Option<usize>, visible: usize, len: usize) -> usize {
    let mut offset = offset.min(len.saturating_sub(visible));
    if let Some(selected) = selected.filter(|_| visible > 0) {
        if selected < offset {
            offset = selected;
        } else if selected >= offset + visible {
            offset = selected + 1 - visible;
        }
    }
    offset
}

/// The attribute value for [`SELECTED_ATTR`].
pub fn selected_value(selected: Option<usize>) -> AttrValue {
    AttrValue::Payload(match selected {
        Some(i) => PropPayload::Single(PropValue::Usize(i)),
        None => PropPayload::None,
    })
}

pub fn selected_from_value(value: &AttrValue) -> Option<Option<usize>> {
    match value {
        AttrValue::Payload(PropPayload::Single(PropValue::Usize(i))) => Some(Some(*i)),
        AttrValue::Payload(PropPayload::None) => Some(None),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor(len: usize, selected: Option<usize>, visible: usize) -> ListCursor {
        let mut c = ListCursor::default();
        c.set_visible(visible);
        c.set(len, selected);
        c
    }

    #[test]
    fn stepping_does_not_wrap() {
        let mut c = cursor(3, Some(0), 5);
        assert!(!c.step_up());
        assert_eq!(c.selected(), Some(0));
        assert!(c.step_down() && c.step_down());
        assert!(!c.step_down());
        assert_eq!(c.selected(), Some(2));
    }

    #[test]
    fn stepping_down_without_a_selection_selects_the_first_row() {
        let mut c = cursor(3, None, 5);
        assert!(!c.step_up());
        assert!(c.step_down());
        assert_eq!(c.selected(), Some(0));
    }

    #[test]
    fn an_empty_list_cannot_be_stepped() {
        let mut c = cursor(0, None, 5);
        assert!(!c.step_up() && !c.step_down());
    }

    #[test]
    fn the_list_scrolls_the_least_that_keeps_the_selection_visible() {
        let mut c = cursor(20, Some(0), 5);
        for _ in 0..4 {
            c.step_down();
        }
        assert_eq!(c.offset(), 0);
        c.step_down();
        assert_eq!(c.selected(), Some(5));
        assert_eq!(c.offset(), 1);
        for _ in 0..3 {
            c.step_down();
        }
        assert_eq!(c.offset(), 4);
        // Moving back up inside the window does not scroll.
        c.step_up();
        c.step_up();
        assert_eq!(c.offset(), 4);
        // Past the top edge it does.
        c.step_up();
        c.step_up();
        assert_eq!((c.selected(), c.offset()), (Some(4), 4));
        c.step_up();
        assert_eq!((c.selected(), c.offset()), (Some(3), 3));
    }

    #[test]
    fn a_selection_set_far_down_scrolls_to_show_it() {
        let c = cursor(40, Some(30), 5);
        assert_eq!(c.offset(), 26);
    }

    #[test]
    fn fewer_rows_than_the_offset_pulls_the_offset_back() {
        let mut c = cursor(40, Some(30), 5);
        c.set(8, Some(2));
        assert_eq!(c.offset(), 2);
    }

    #[test]
    fn a_list_without_a_selection_shows_its_first_rows() {
        let mut c = cursor(40, Some(30), 5);
        assert_eq!(c.offset(), 26);
        c.set(10, None);
        assert_eq!(c.offset(), 0);
        let mut c = cursor(40, Some(30), 5);
        c.select(None);
        assert_eq!(c.offset(), 0);
        assert_eq!(c.offset_for(5), 0);
    }

    #[test]
    fn scrolling_moves_the_rows_by_one_and_keeps_the_selection_alone() {
        let mut c = cursor(10, None, 4);
        assert!(!c.scroll(false, 4), "already at the top");
        assert!(c.scroll(true, 4));
        assert_eq!((c.offset(), c.selected()), (1, None));
        for _ in 0..5 {
            assert!(c.scroll(true, 4));
        }
        assert_eq!(c.offset(), 6, "the last row is at the bottom");
        assert!(!c.scroll(true, 4));
        assert!(c.scroll(false, 4));
        assert_eq!(c.offset(), 5);
    }

    #[test]
    fn a_list_that_fits_does_not_scroll() {
        let mut c = cursor(3, None, 4);
        assert!(!c.scroll(true, 4) && !c.scroll(false, 4));
        let mut c = cursor(0, None, 4);
        assert!(!c.scroll(true, 4));
        assert_eq!(c.offset(), 0);
    }

    #[test]
    fn a_scroll_survives_a_selection_that_stays_none() {
        let mut c = cursor(10, None, 4);
        c.scroll(true, 4);
        c.set_visible(4);
        assert_eq!(c.offset_for(4), 1);
    }

    #[test]
    fn an_out_of_range_selection_is_dropped() {
        let c = cursor(3, Some(7), 5);
        assert_eq!(c.selected(), None);
    }

    #[test]
    fn offset_for_is_pure() {
        let c = cursor(40, Some(30), 5);
        assert_eq!(c.offset_for(10), 26);
        assert_eq!(c.offset_for(2), 29);
        assert_eq!(c.offset(), 26);
    }

    #[test]
    fn selected_value_round_trips() {
        for selected in [None, Some(0), Some(7)] {
            assert_eq!(
                selected_from_value(&selected_value(selected)),
                Some(selected)
            );
        }
        assert_eq!(selected_from_value(&AttrValue::Flag(true)), None);
    }
}
