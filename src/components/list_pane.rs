//! What `ProjectList` and `SessionList` share (product §3.1, §7.2): the state
//! and attributes of a selector list, the row layout and styles, the pane
//! title's width, and the resolution of selector keys through the keymap.
//! The cursor is in [`list_cursor`](super::list_cursor) and the pane border
//! in [`titled_block`](super::titled_block), which other components use too.

use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::{State, StateValue};
use unicode_width::UnicodeWidthStr;

use super::list_cursor::{
    ListCursor, SELECTED_ATTR, VISIBLE_ROWS_ATTR, selected_from_value, selected_value,
};
use super::{THEME_ATTR, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::keymap::{Action, Context, Keymap};
use crate::model::HarnessId;
use crate::theme::{Role, Theme, Token};

/// The attribute that tells a list whether a session is open behind the
/// selector (`AttrValue::Flag`). It decides what `Esc` does: close the
/// selector, or quit.
pub const SESSION_OPEN_ATTR: Attribute = Attribute::Custom("session_open");

/// The rows above the list inside a pane: the border and one blank row.
const ROWS_ABOVE_LIST: u16 = 2;
/// The rows of a pane that are not list rows: top border, blank row, bottom
/// border.
const PANE_CHROME_ROWS: u16 = 3;
/// The columns of a pane that do not hold row text: the border, two spaces,
/// the marker, the band's first cell, and two margin cells before the border
/// (`1 + 2 + 1 + 1 + 2 + 1`).
const ROW_CHROME_COLUMNS: usize = 8;

/// How many list rows a pane of `pane_height` rows shows.
pub fn visible_rows(pane_height: u16) -> usize {
    usize::from(pane_height.saturating_sub(PANE_CHROME_ROWS))
}

/// The columns available to a row's text (and stats) in a pane of
/// `pane_width` columns: from column 5 to column `pane_width - 4`.
pub(super) fn content_width(pane_width: usize) -> usize {
    pane_width.saturating_sub(ROW_CHROME_COLUMNS)
}

/// The state and attributes every selector list has: theme, focus and cursor.
#[derive(Debug, Clone)]
pub(super) struct PaneState {
    pub theme: Theme,
    pub focused: bool,
    pub cursor: ListCursor,
    /// Whether a session is open behind the selector.
    pub session_open: bool,
}

impl PaneState {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            focused: false,
            cursor: ListCursor::default(),
            session_open: false,
        }
    }

    /// What `Esc` asks for when there is no search to clear: close the
    /// selector if a session is open behind it, else quit.
    pub fn cancel(&self) -> Msg {
        if self.session_open {
            Msg::CloseSelector
        } else {
            Msg::Quit
        }
    }

    /// Applies the attributes all lists understand. Returns whether `attr` was
    /// one of them.
    pub fn attr(&mut self, attr: Attribute, value: &AttrValue) -> bool {
        match attr {
            Attribute::Focus => {
                if let AttrValue::Flag(focused) = value {
                    self.focused = *focused;
                }
            }
            THEME_ATTR => {
                if let Some(theme) = theme_from_value(value) {
                    self.theme = theme;
                }
            }
            SELECTED_ATTR => {
                if let Some(selected) = selected_from_value(value) {
                    self.cursor.select(selected);
                }
            }
            VISIBLE_ROWS_ATTR => {
                if let AttrValue::Length(rows) = value {
                    self.cursor.set_visible(*rows);
                }
            }
            SESSION_OPEN_ATTR => {
                if let AttrValue::Flag(open) = value {
                    self.session_open = *open;
                }
            }
            _ => return false,
        }
        true
    }

    /// Answers the queries all lists understand.
    pub fn query(&self, attr: Attribute) -> Option<QueryResult<'static>> {
        let value = match attr {
            Attribute::Focus => AttrValue::Flag(self.focused),
            THEME_ATTR => theme_value(&self.theme),
            SELECTED_ATTR => selected_value(self.cursor.selected()),
            VISIBLE_ROWS_ATTR => AttrValue::Length(self.cursor.visible()),
            SESSION_OPEN_ATTR => AttrValue::Flag(self.session_open),
            _ => return None,
        };
        Some(QueryResult::Owned(value))
    }

    /// The row of the list at (`column`, `row`) of a pane of `size` (as
    /// drawn: the border, a blank row, then the rows), or `None` on the
    /// border, the blank row, the margin and below the last row.
    pub fn row_at(&self, column: u16, row: u16, size: Size) -> Option<usize> {
        let line = usize::from(row.checked_sub(ROWS_ABOVE_LIST)?);
        let inside = column >= 1 && column < size.width.saturating_sub(1);
        let visible = visible_rows(size.height);
        if !inside || line >= visible {
            return None;
        }
        let index = self.cursor.offset_for(visible) + line;
        (index < self.cursor.len()).then_some(index)
    }

    /// Moves the selection one row for `Cmd::Move(Up | Down)`.
    pub fn perform(&mut self, cmd: Cmd) -> CmdResult {
        let changed = match cmd {
            Cmd::Move(Direction::Up) => self.cursor.step_up(),
            Cmd::Move(Direction::Down) => self.cursor.step_down(),
            other => return CmdResult::Invalid(other),
        };
        match (changed, self.cursor.selected()) {
            (true, Some(i)) => CmdResult::Changed(State::Single(StateValue::Usize(i))),
            _ => CmdResult::NoChange,
        }
    }

    pub fn state(&self) -> State {
        self.cursor
            .selected()
            .map_or(State::None, |i| State::Single(StateValue::Usize(i)))
    }
}

/// A selector key, resolved through the keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PaneKey {
    Up,
    Down,
    FocusProjects,
    FocusSessions,
    NextPane,
    PrevPane,
    Open,
    Cancel,
    Search,
    Resume,
    Harness(HarnessId),
}

/// The selector action `event` is bound to in `keymap`, if it is one a list
/// handles.
pub(super) fn pane_key(keymap: &Keymap, event: &Event<UserEvent>) -> Option<PaneKey> {
    let Event::Keyboard(key) = event else {
        return None;
    };
    let harness = |n| HarnessId::from_number(n).map(PaneKey::Harness);
    match keymap.resolve(Context::Selector, key)? {
        Action::Up => Some(PaneKey::Up),
        Action::Down => Some(PaneKey::Down),
        Action::FocusProjects => Some(PaneKey::FocusProjects),
        Action::FocusSessions => Some(PaneKey::FocusSessions),
        Action::NextPane => Some(PaneKey::NextPane),
        Action::PrevPane => Some(PaneKey::PrevPane),
        Action::Open => Some(PaneKey::Open),
        Action::Cancel => Some(PaneKey::Cancel),
        Action::Search => Some(PaneKey::Search),
        Action::Resume => Some(PaneKey::Resume),
        Action::Harness1 => harness(1),
        Action::Harness2 => harness(2),
        Action::Harness3 => harness(3),
        Action::Harness4 => harness(4),
        _ => None,
    }
}

/// The most columns a pane title can use: the corner, a dash and a space
/// either side leave this much of the width.
pub(super) fn title_width(pane_width: u16) -> usize {
    usize::from(pane_width).saturating_sub(6)
}

/// Which part of a row a piece of text is; it decides the style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Part {
    Text,
    Dot,
}

/// The pieces of one row: the left side, and stats ending at the right edge.
#[derive(Debug, Clone, Default)]
pub(super) struct RowParts {
    pub left: Vec<(Part, String)>,
    pub stats: Option<String>,
}

/// How a row is drawn (product §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    Unselected,
    SelectedFocused,
    SelectedUnfocused,
}

impl RowKind {
    fn of(selected: bool, focused: bool) -> Self {
        match (selected, focused) {
            (false, _) => Self::Unselected,
            (true, true) => Self::SelectedFocused,
            (true, false) => Self::SelectedUnfocused,
        }
    }
}

/// The styles of one row's pieces.
struct RowStyles {
    marker: Style,
    fill: Style,
    text: Style,
    dot: Style,
    stats: Style,
}

impl RowStyles {
    fn new(theme: &Theme, kind: RowKind) -> Self {
        let muted = Style::new().fg(theme.role(Role::MutedForeground));
        let dot = Style::new().fg(theme.role(Role::Primary));
        match kind {
            RowKind::Unselected => Self {
                marker: Style::new(),
                fill: Style::new(),
                text: muted,
                dot,
                stats: muted,
            },
            RowKind::SelectedUnfocused => {
                let secondary = Style::new().fg(theme.role(Role::SecondaryForeground));
                Self {
                    marker: secondary,
                    fill: Style::new(),
                    text: secondary.add_modifier(Modifier::BOLD),
                    dot,
                    stats: muted,
                }
            }
            RowKind::SelectedFocused => {
                let band = Style::new()
                    .fg(theme.role(Role::Foreground))
                    .add_modifier(Modifier::REVERSED);
                Self {
                    marker: theme.style(Token::SelectionMarker),
                    fill: band,
                    text: band,
                    dot: band,
                    stats: band,
                }
            }
        }
    }

    fn part(&self, part: Part) -> Style {
        match part {
            Part::Text => self.text,
            Part::Dot => self.dot,
        }
    }
}

/// One row of a pane `pane_width` columns wide, from column 1 to the column
/// before the right border.
///
/// The marker is in column 3 and the text starts in column 5. A selected
/// row in the focused pane has one inverted band from column 4 to column
/// `pane_width - 4`; stats end at that column. Two cells are left before the
/// border.
fn row_line(theme: &Theme, kind: RowKind, pane_width: usize, parts: &RowParts) -> Line<'static> {
    let styles = RowStyles::new(theme, kind);
    let marker = if kind == RowKind::Unselected {
        " "
    } else {
        ">"
    };
    let mut spans = vec![
        Span::raw("  "),
        Span::styled(marker, styles.marker),
        Span::styled(" ", styles.fill),
    ];
    let mut used = 0;
    for (part, text) in &parts.left {
        used += text.width();
        spans.push(Span::styled(text.clone(), styles.part(*part)));
    }
    let stats_width = parts.stats.as_deref().map_or(0, UnicodeWidthStr::width);
    let padding = content_width(pane_width).saturating_sub(used + stats_width);
    spans.push(Span::styled(" ".repeat(padding), styles.fill));
    if let Some(stats) = &parts.stats {
        spans.push(Span::styled(stats.clone(), styles.stats));
    }
    spans.push(Span::raw("  "));
    Line::from(spans)
}

/// Draws the visible rows of a list into the pane at `area` (whose border
/// the caller has drawn). `row` makes the pieces of row `index` for a text
/// width of `content_width` columns.
pub(super) fn render_rows(
    frame: &mut Frame,
    area: Rect,
    state: &PaneState,
    mut row: impl FnMut(usize, usize) -> RowParts,
) {
    let rows = visible_rows(area.height);
    if rows == 0 || area.width <= u16::try_from(ROW_CHROME_COLUMNS).unwrap_or(u16::MAX) {
        return;
    }
    let cursor = &state.cursor;
    let first = cursor.offset_for(rows);
    let last = cursor.len().min(first + rows);
    let width = usize::from(area.width);
    for (n, index) in (first..last).enumerate() {
        let kind = RowKind::of(cursor.selected() == Some(index), state.focused);
        let parts = row(index, content_width(width));
        let line = row_line(&state.theme, kind, width, &parts);
        // `n` is below `rows`, which fits in the pane's height.
        let y = area.y + ROWS_ABOVE_LIST + n as u16;
        let rect = Rect::new(area.x + 1, y, area.width - 2, 1);
        frame.render_widget(Paragraph::new(line), rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(len: usize, selected: Option<usize>) -> PaneState {
        let mut pane = PaneState::new(crate::theme::builtin::default_theme());
        pane.cursor.set(len, selected);
        pane
    }

    #[test]
    fn row_at_maps_a_cell_to_the_row_drawn_there() {
        // A pane of 20 x 8 shows 5 rows, from its row 2.
        let size = Size::new(20, 8);
        let pane = pane(3, None);
        assert_eq!(pane.row_at(5, 2, size), Some(0));
        assert_eq!(pane.row_at(1, 3, size), Some(1));
        assert_eq!(pane.row_at(18, 4, size), Some(2));
        // Under the last row, on the blank row and the border, and the
        // border columns.
        assert_eq!(pane.row_at(5, 5, size), None);
        assert_eq!(pane.row_at(5, 1, size), None);
        assert_eq!(pane.row_at(5, 0, size), None);
        assert_eq!(pane.row_at(0, 2, size), None);
        assert_eq!(pane.row_at(19, 2, size), None);
    }

    #[test]
    fn row_at_follows_the_scroll_offset_and_the_rows_that_fit() {
        let size = Size::new(20, 5);
        // Two rows fit; the selection at 7 scrolls the list to rows 6 and 7.
        let pane = pane(10, Some(7));
        assert_eq!(pane.row_at(5, 2, size), Some(6));
        assert_eq!(pane.row_at(5, 3, size), Some(7));
        assert_eq!(pane.row_at(5, 4, size), None, "the bottom border");
    }

    #[test]
    fn visible_rows_leave_room_for_border_and_blank_row() {
        assert_eq!(visible_rows(24), 21);
        assert_eq!(visible_rows(8), 5);
        assert_eq!(visible_rows(4), 1);
        assert_eq!(visible_rows(2), 0);
        assert_eq!(visible_rows(0), 0);
    }

    #[test]
    fn content_width_is_the_pane_width_minus_eight() {
        assert_eq!(content_width(36), 28);
        assert_eq!(content_width(5), 0);
    }
}
