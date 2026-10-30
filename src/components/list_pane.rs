//! What `ProjectList` and `SessionList` share (SPEC §4.1, §8.2): the cursor
//! with its scroll offset, the pane border, the row layout and styles, and the
//! resolution of selector keys through the keymap.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, PropPayload, PropValue, QueryResult};
use tuirealm::state::{State, StateValue};
use unicode_width::UnicodeWidthStr;

use super::{THEME_ATTR, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::keymap::{Action, Context, Keymap};
use crate::model::HarnessId;
use crate::theme::{Role, Theme, Token};

/// The attribute that sets which row is selected (`None` for no selection).
/// Pushing it never produces a message.
pub const SELECTED_ATTR: Attribute = Attribute::Custom("selected");
/// The attribute that tells a list how many rows its pane shows, so scrolling
/// keeps the selection visible. Set from the layout, never from `view`.
pub const VISIBLE_ROWS_ATTR: Attribute = Attribute::Custom("visible_rows");
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

    /// Sets the number of rows and the selection (ignored when out of range)
    /// and scrolls the least that keeps the selection visible. Without a
    /// selection the list shows its first rows: a list nothing is selected
    /// in cannot have been scrolled, and the offset of the list it replaces
    /// means nothing for the new one.
    pub fn set(&mut self, len: usize, selected: Option<usize>) {
        self.len = len;
        self.selected = selected.filter(|&i| i < len);
        if self.selected.is_none() {
            self.offset = 0;
        }
        self.reveal();
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
            VISIBLE_ROWS_ATTR => AttrValue::Length(self.cursor.visible),
            SESSION_OPEN_ATTR => AttrValue::Flag(self.session_open),
            _ => return None,
        };
        Some(QueryResult::Owned(value))
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

/// The attribute value for [`SELECTED_ATTR`].
pub fn selected_value(selected: Option<usize>) -> AttrValue {
    AttrValue::Payload(match selected {
        Some(i) => PropPayload::Single(PropValue::Usize(i)),
        None => PropPayload::None,
    })
}

fn selected_from_value(value: &AttrValue) -> Option<Option<usize>> {
    match value {
        AttrValue::Payload(PropPayload::Single(PropValue::Usize(i))) => Some(Some(*i)),
        AttrValue::Payload(PropPayload::None) => Some(None),
        _ => None,
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
        Action::Harness1 => harness(1),
        Action::Harness2 => harness(2),
        Action::Harness3 => harness(3),
        Action::Harness4 => harness(4),
        _ => None,
    }
}

/// A rounded block whose border has the colour of `border` and whose title
/// reads `╭─ <title> ─…╮` in `foreground`.
pub(super) fn titled_block(theme: &Theme, title: &str, border: Role) -> Block<'static> {
    let border_style = Style::new().fg(theme.role(border));
    let title_style = Style::new().fg(theme.role(Role::Foreground));
    let title = Line::from(vec![
        Span::styled("─", border_style),
        Span::styled(format!(" {title} "), title_style),
    ]);
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(title)
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

/// How a row is drawn (SPEC §8.2).
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

/// The border role of a pane.
pub(super) fn border_role(focused: bool) -> Role {
    if focused { Role::Ring } else { Role::Border }
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
