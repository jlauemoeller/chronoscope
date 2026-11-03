//! The turns sidebar (SPEC §6): a fixed pane to the left of the transcript
//! that lists the turns and compaction events of the session, so the shape of
//! a long session is visible.
//!
//! # Props
//!
//! Data flows in through [`attr`](Component::attr): [`THEME_ATTR`],
//! `Attribute::Focus`, [`KEY_LABEL_ATTR`] (the label of the key that focuses
//! the pane, shown in the title), [`TURN_ROWS_ATTR`] (the [`TurnRows`] of a
//! transcript that was opened or closed), [`MODEL_UPDATE_ATTR`] (the
//! `Appended`, `Updated` and `Reset` updates the transcript gets), the
//! selection ([`SELECTED_ATTR`], an entry, or [`SELECTED_TURN_ATTR`], a turn;
//! pushing either produces no message), [`COVER_ATTR`] (a transcript is
//! opened over the one the rows are of, or closed again) and
//! [`VISIBLE_ROWS_ATTR`] (set from the layout, never from `view`).
//!
//! The component keeps no turns, only the [`TurnRows`] built from them. It
//! lays out and renders nothing but its own rows, and draws only the rows it
//! shows.
//!
//! The pane paints its own area only; the column after it is `SidebarGap`'s.
//!
//! # Keys
//!
//! `on` resolves a key with the shared [`Keymap`] for the `turns` context.
//! `Up` / `Down` are `Cmd::Move`s that `perform` handles: they select the
//! previous or next entry (the first one when nothing is selected, never
//! wrapping) and answer [`Msg::TurnSelected`] with the turn's id; at either
//! end, or on an empty list, they answer nothing. `Open` answers
//! [`Msg::OpenTurn`] for the selected turn, and the pane keys
//! [`Msg::FocusTranscript`]. Which component has focus is the `App`'s to
//! decide; the pane only draws it (`Attribute::Focus`).
//!
//! The pane takes no mouse yet.

mod line;
mod rows;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod snapshots;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::{State, StateValue};

use self::line::Look;
pub use self::rows::{Entry, EntryKind, Row, TurnRows, entry_for};
use super::list_pane::{ListCursor, border_role, titled_block};
use super::{
    MODEL_UPDATE_ATTR, SELECTED_ATTR, THEME_ATTR, VISIBLE_ROWS_ATTR, payload_from_value,
    payload_value, selected_from_value, selected_value, theme_from_value, theme_value,
};
use crate::app::{Msg, UserEvent};
use crate::keymap::{Action, Context, KeyResolver, Keymap, Resolved};
use crate::model::{ModelUpdate, TurnId};
use crate::theme::{Role, Theme};

/// The rows of a transcript, as a [`TurnRows`] payload. Setting it on the
/// sidebar replaces its rows; querying a `Transcript` builds them in one pass
/// over its turns.
pub const TURN_ROWS_ATTR: Attribute = Attribute::Custom("turn_rows");
/// The turn to select (a [`TurnId`] payload). A turn without an entry leaves
/// the selection as it is. Setting it produces no message.
pub const SELECTED_TURN_ATTR: Attribute = Attribute::Custom("selected_turn");
/// A flag that says a transcript is opened over the one whose rows the pane
/// shows (`true`; the pane remembers its selection and scroll offset), or
/// that it was closed (`false`; the next rows set bring the remembered
/// selection and offset back, so the pane is as it was left).
pub const COVER_ATTR: Attribute = Attribute::Custom("sidebar_cover");
/// The label of the key that focuses the pane, shown in its title (a
/// string; empty when the action has no key: the title is then just `Turns`).
pub const KEY_LABEL_ATTR: Attribute = Attribute::Custom("sidebar_key");

/// The key label the title shows until another is set.
pub const DEFAULT_KEY_LABEL: &str = "1";

/// The rows above the list inside the pane: the border and two blank rows.
const ROWS_ABOVE_LIST: u16 = 3;
/// The rows of the pane that are not list rows: the rows above the list, and
/// the bottom border.
const PANE_CHROME_ROWS: u16 = 4;

/// How many list rows a pane of `pane_height` rows shows.
pub fn sidebar_list_rows(pane_height: u16) -> usize {
    usize::from(pane_height.saturating_sub(PANE_CHROME_ROWS))
}

/// What the pane remembers of the rows of a transcript that is covered.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Covered {
    selected: Option<TurnId>,
    offset: usize,
}

/// The turns sidebar.
#[derive(Debug, Clone)]
pub struct TurnSidebar {
    theme: Theme,
    focused: bool,
    key_label: String,
    /// The key bindings of the `turns` context.
    keymap: Keymap,
    /// Resolves keys; remembers the first key of a chord.
    resolver: KeyResolver,
    rows: TurnRows,
    /// The selected turn. It always has an entry.
    selected: Option<TurnId>,
    /// The scroll offset over the rows of the list; its selection is the row
    /// of the selected entry.
    cursor: ListCursor,
    /// The selection and offset of each transcript a later one covers,
    /// outermost first.
    covered: Vec<Covered>,
    /// What the next rows set bring back: the pane of a transcript that was
    /// uncovered.
    restore: Option<Covered>,
}

impl TurnSidebar {
    /// An empty sidebar (`Start` and `End` only) in `theme`.
    pub fn new(theme: Theme) -> Self {
        let mut sidebar = Self {
            theme,
            focused: false,
            key_label: DEFAULT_KEY_LABEL.to_owned(),
            keymap: Keymap::default(),
            resolver: KeyResolver::new(Context::Turns),
            rows: TurnRows::default(),
            selected: None,
            cursor: ListCursor::default(),
            covered: Vec::new(),
            restore: None,
        };
        sidebar.sync();
        sidebar
    }

    /// Sets the rows, from the turns of a transcript.
    #[must_use]
    pub fn with_rows(mut self, rows: TurnRows) -> Self {
        self.set_rows(rows);
        self
    }

    /// Sets the label of the key that focuses the pane.
    #[must_use]
    pub fn key_label(mut self, label: impl Into<String>) -> Self {
        self.key_label = label.into();
        self
    }

    /// Sets the keymap keys are resolved with (default: the built-in one).
    #[must_use]
    pub fn keymap(mut self, keymap: Keymap) -> Self {
        self.keymap = keymap;
        self.resolver.reset();
        self
    }

    /// Sets the selection to the entry `entry` (`None`: nothing selected). An
    /// entry that does not exist selects nothing.
    #[must_use]
    pub fn with_selected(mut self, entry: Option<usize>) -> Self {
        self.select(entry);
        self
    }

    /// Sets whether the pane has focus.
    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// The rows.
    pub fn rows(&self) -> &TurnRows {
        &self.rows
    }

    /// The selected entry.
    pub fn selected_entry(&self) -> Option<usize> {
        self.selected
            .as_ref()
            .and_then(|id| self.rows.entry_index(id))
    }

    /// The first list row drawn in a pane of `pane_height` rows.
    pub fn offset(&self, pane_height: u16) -> usize {
        self.cursor.offset_for(sidebar_list_rows(pane_height))
    }

    fn set_rows(&mut self, rows: TurnRows) {
        self.rows = rows;
        match self.restore.take() {
            Some(saved) => {
                self.selected = saved.selected;
                self.sync_from(Some(saved.offset));
            }
            None => self.sync(),
        }
    }

    /// Remembers (`covered`) or recalls the selection and offset of the rows
    /// that a transcript opened over (or closed again) leaves.
    fn cover(&mut self, covered: bool) {
        if covered {
            self.covered.push(Covered {
                selected: self.selected.clone(),
                offset: self.cursor.offset(),
            });
        } else {
            self.restore = self.covered.pop();
        }
    }

    /// Selects the entry `entry` (`None`: nothing). An entry that does not
    /// exist selects nothing. The same as setting [`SELECTED_ATTR`].
    pub fn select(&mut self, entry: Option<usize>) {
        self.selected = entry
            .and_then(|e| self.rows.entry(e))
            .map(|(id, _)| id.clone());
        self.sync();
    }

    /// Selects the turn `id`, when it has an entry. A turn without one (a
    /// system section in a turn of system content only) leaves the selection
    /// where it is: it neither clears nor jumps. The same as setting
    /// [`SELECTED_TURN_ATTR`].
    pub fn select_turn(&mut self, id: &TurnId) {
        if self.rows.entry_index(id).is_some() {
            self.selected = Some(id.clone());
            self.sync();
        }
    }

    /// Selects the previous or next entry (`down`): the first when nothing is
    /// selected, in either direction. It never wraps, so at the first or last
    /// entry, or on an empty list, nothing changes. `Changed` when the
    /// selection moved.
    fn step(&mut self, down: bool) -> CmdResult {
        let entries = self.rows.entry_count();
        let next = match (self.selected_entry(), down) {
            (None, _) if entries > 0 => 0,
            (Some(entry), true) if entry + 1 < entries => entry + 1,
            (Some(entry), false) if entry > 0 => entry - 1,
            _ => return CmdResult::NoChange,
        };
        self.select(Some(next));
        CmdResult::Changed(Component::state(self))
    }

    /// Applies a model update: `Appended`, `Updated` and `Reset` change the
    /// rows, the other updates do not. The same as setting
    /// [`MODEL_UPDATE_ATTR`], for callers that have no `Arc` of the update.
    pub fn apply_update(&mut self, update: &ModelUpdate) {
        self.rows.apply(update);
        self.sync();
    }

    /// Brings the cursor in line with the rows: a selected turn that has no
    /// entry any more is deselected, and the offset moves the least that
    /// keeps the selected row visible.
    fn sync(&mut self) {
        self.sync_from(None);
    }

    /// [`sync`](Self::sync), starting from the first visible row `offset`
    /// when given.
    fn sync_from(&mut self, offset: Option<usize>) {
        let row = self
            .selected
            .as_ref()
            .and_then(|id| self.rows.entry_index(id))
            .map(TurnRows::row_of_entry);
        if row.is_none() {
            self.selected = None;
        }
        match offset {
            Some(offset) => self.cursor.restore(self.rows.row_count(), row, offset),
            None => self.cursor.set(self.rows.row_count(), row),
        }
    }

    /// How the row `index` looks.
    fn look(&self, index: usize) -> Look {
        if self.cursor.selected() != Some(index) {
            Look::Plain
        } else if self.focused {
            Look::SelectedFocused
        } else {
            Look::SelectedUnfocused
        }
    }

    /// Paints the background of the pane (not of the gap after it: that is
    /// [`Id::SidebarGap`](crate::app::Id::SidebarGap)'s).
    fn paint_background(&self, frame: &mut Frame, area: Rect) {
        if self.theme.transparent_background() {
            return;
        }
        let style = Style::new().bg(self.theme.role(Role::Background));
        frame.render_widget(Block::new().style(style), area);
    }

    /// Draws the rows the pane shows, from its fourth row.
    fn draw_rows(&self, frame: &mut Frame, area: Rect) {
        let shown = sidebar_list_rows(area.height);
        let first = self.cursor.offset_for(shown);
        let last = self.rows.row_count().min(first + shown);
        let width = usize::from(area.width);
        for (n, index) in (first..last).enumerate() {
            let Some(row) = self.rows.row(index) else {
                break;
            };
            let Some(line) = line::row_line(&self.theme, row, self.look(index), width) else {
                return;
            };
            // `n` is below `shown`, which fits in the pane's height.
            let y = area.y + ROWS_ABOVE_LIST + n as u16;
            let rect = Rect::new(area.x + 1, y, area.width - 2, 1);
            frame.render_widget(Paragraph::new(line), rect);
        }
    }
}

impl Component for TurnSidebar {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        self.paint_background(frame, area);
        let title = if self.key_label.is_empty() {
            "Turns ".to_owned()
        } else {
            format!("[{}] Turns ", self.key_label)
        };
        frame.render_widget(
            titled_block(&self.theme, &title, border_role(self.focused)),
            area,
        );
        self.draw_rows(frame, area);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        let value = if attr == Attribute::Focus {
            AttrValue::Flag(self.focused)
        } else if attr == THEME_ATTR {
            theme_value(&self.theme)
        } else if attr == KEY_LABEL_ATTR {
            AttrValue::String(self.key_label.clone())
        } else if attr == SELECTED_ATTR {
            selected_value(self.selected_entry())
        } else if attr == VISIBLE_ROWS_ATTR {
            AttrValue::Length(self.cursor.visible())
        } else if attr == TURN_ROWS_ATTR {
            payload_value(self.rows.clone())
        } else {
            return None;
        };
        Some(QueryResult::Owned(value))
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus {
            if let AttrValue::Flag(focused) = value {
                self.focused = focused;
            }
        } else if attr == THEME_ATTR {
            if let Some(theme) = theme_from_value(&value) {
                self.theme = theme;
            }
        } else if attr == KEY_LABEL_ATTR {
            if let AttrValue::String(label) = value {
                self.key_label = label;
            }
        } else if attr == SELECTED_ATTR {
            if let Some(selected) = selected_from_value(&value) {
                self.select(selected);
            }
        } else if attr == SELECTED_TURN_ATTR {
            if let Some(id) = payload_from_value::<TurnId>(&value) {
                self.select_turn(&id);
            }
        } else if attr == COVER_ATTR {
            if let AttrValue::Flag(covered) = value {
                self.cover(covered);
            }
        } else if attr == VISIBLE_ROWS_ATTR {
            if let AttrValue::Length(rows) = value {
                self.cursor.set_visible(rows);
            }
        } else if attr == TURN_ROWS_ATTR {
            if let Some(rows) = payload_from_value::<TurnRows>(&value) {
                self.set_rows(rows);
            }
        } else if attr == MODEL_UPDATE_ATTR
            && let Some(update) = payload_from_value::<Arc<ModelUpdate>>(&value)
        {
            self.apply_update(&update);
        }
    }

    /// The selected entry.
    fn state(&self) -> State {
        self.selected_entry()
            .map_or(State::None, |e| State::Single(StateValue::Usize(e)))
    }

    /// `Move(Up)` and `Move(Down)` select the previous and next entry:
    /// `Changed` with the new state when the selection moved, else
    /// `NoChange`. `Submit` is `Submit` with the state of the selected entry
    /// (`NoChange` with none).
    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Move(Direction::Up) => self.step(false),
            Cmd::Move(Direction::Down) => self.step(true),
            Cmd::Submit => match self.selected_entry() {
                Some(entry) => CmdResult::Submit(State::Single(StateValue::Usize(entry))),
                None => CmdResult::NoChange,
            },
            other => CmdResult::Invalid(other),
        }
    }
}

impl TurnSidebar {
    /// The selected turn as a [`Msg`] built by `msg`.
    fn selected_msg(&self, msg: fn(TurnId) -> Msg) -> Option<Msg> {
        self.selected.clone().map(msg)
    }
}

impl AppComponent<Msg, UserEvent> for TurnSidebar {
    /// Resolves the key in the `turns` context. `Up` / `Down` answer
    /// [`Msg::TurnSelected`] when the selection moved, `Open`
    /// [`Msg::OpenTurn`] for the selected turn, and the pane keys and
    /// `focus_transcript` [`Msg::FocusTranscript`]. Anything else, the mouse
    /// included, answers nothing.
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        let Event::Keyboard(key) = ev else {
            return None;
        };
        let Resolved::Action(action) = self.resolver.feed(&self.keymap, key) else {
            return None;
        };
        match action {
            Action::Up | Action::Down => {
                let direction = if action == Action::Up {
                    Direction::Up
                } else {
                    Direction::Down
                };
                match self.perform(Cmd::Move(direction)) {
                    CmdResult::Changed(_) => self.selected_msg(Msg::TurnSelected),
                    _ => None,
                }
            }
            Action::Open => match self.perform(Cmd::Submit) {
                CmdResult::Submit(_) => self.selected_msg(Msg::OpenTurn),
                _ => None,
            },
            Action::NextPane | Action::PrevPane | Action::FocusTranscript => {
                Some(Msg::FocusTranscript)
            }
            _ => None,
        }
    }
}
