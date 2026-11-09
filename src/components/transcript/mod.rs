//! The transcript (product §3.2, §4, §8.2): the open session's turns as a
//! vertically scrolling stack of [`TurnBlock`](super::TurnBlock)s.
//!
//! [`Transcript`] is one mounted component, virtualised: it lays blocks out
//! lazily for the current width, keeps their heights in a [`LayoutCache`] and
//! draws only the blocks that overlap the viewport, clipping the first and
//! the last. It owns the turns, the fold states and the selection (keyed by
//! [`SectionId`], so they survive updates), and the scroll offset.
//!
//! # Props
//!
//! Data flows in through [`attr`](Component::attr): [`THEME_ATTR`],
//! [`DIM_UNSELECTED_ATTR`], [`SHOW_SYSTEM_ATTR`], [`TRUNCATE_OUTPUT_LINES_ATTR`],
//! [`OPEN_SUBAGENT_KEY_ATTR`], [`TIMESTAMP_FORMATTER_ATTR`], [`MODEL_UPDATE_ATTR`] (an `Arc<ModelUpdate>`
//! payload, see [`model_update_value`]) and [`VIEWPORT_ATTR`]. The search
//! has its own attributes ([`SEARCH_QUERY_ATTR`], [`SEARCH_REQUEST_ATTR`],
//! [`SEARCH_RESULTS_ATTR`], [`SEARCH_POSITION_ATTR`]); see the `search`
//! module.
//!
//! # The pre-render step
//!
//! `view` never changes the transcript. The width and height a layout is for
//! are announced beforehand with [`VIEWPORT_ATTR`] (or
//! [`Transcript::set_viewport`]): the App sets it whenever the transcript's
//! area changes. That step re-wraps for a new width, fills the cache for the
//! visible blocks and prepares the window `view` draws. Every later change
//! (a `Cmd`, an update, a prop) refreshes it again. If `view` is given an
//! area the viewport was not announced for, it computes the window for that
//! area on a copy of the cache and discards it: slower, but correct, and
//! still without side effects.
//!
//! # Following
//!
//! While the viewport shows the last row the transcript *follows*: an
//! `Appended` or `Updated` batch keeps the view pinned to the end (and moves
//! the selection to the new last section if it was on the last one). Once the
//! user leaves the end it keeps the position and counts the sections that
//! arrive ([`NEW_SECTIONS_ATTR`]); `G` / `End`, or scrolling back to the end,
//! resumes following. Until `Loaded` arrives (see [`Transcript::loading`])
//! the view stays where it is. A key that changes the count answers
//! [`Msg::NewSections`]; the App reads the count after the updates it
//! forwards. See the `follow` module.
//!
//! # Keys and `Cmd`s
//!
//! `on` resolves a key with the shared [`Keymap`] for the `browser` context
//! (a [`KeyResolver`] of its own holds a pending chord) and turns the action
//! into a `Cmd` that `perform` handles; it answers [`Msg::TranscriptChanged`]
//! when the key changed what is shown (the App then redraws), else nothing.
//! The `Cmd`s:
//!
//! - `Scroll(Up/Down)` scrolls one row;
//! - `Move(Up/Down)` selects the previous/next section (paging through one
//!   taller than the viewport first);
//! - `GoTo(Begin/End)` selects the first/last section and scrolls there;
//! - `Custom(`[`NEXT_TURN`]`/`[`PREV_TURN`]`)` jump between turn blocks;
//! - `Custom(`[`HALF_PAGE_DOWN`]`/`[`HALF_PAGE_UP`]`/`[`PAGE_DOWN`]`/`[`PAGE_UP`]`)`
//!   scroll and move the selection into view;
//! - `Submit` toggles the selected section's fold, or, on the `… N more
//!   lines` row of a truncated tool call, expands its output;
//!   `Custom(`[`FOLD`]`/`[`UNFOLD`]`/`[`FOLD_ALL`]`/`[`UNFOLD_ALL`]`)` fold
//!   and unfold;
//! - `Custom(`[`TOGGLE_SYSTEM`]`)` shows or hides system sections;
//! - `Custom(`[`SEARCH_NEXT`]`/`[`SEARCH_PREV`]`)` go to the next/previous
//!   match, wrapping, and `Custom(`[`CLEAR_SEARCH`]`)` clears the search.
//!   `/` is not a `Cmd`: the key answers [`Msg::OpenSearch`].
//!
//! The mouse (see the `mouse` module) arrives as a [`RoutedMouse`](crate::app::RoutedMouse)
//! the router made for the pointer's position in the transcript's area. A wheel
//! step is `Custom(`[`WHEEL_UP`]`/`[`WHEEL_DOWN`]`)`: it scrolls [`WHEEL_ROWS`]
//! rows the way the page keys scroll. A left press selects the section under
//! the pointer, toggles its fold when it is on the section's first row, and
//! expands the output on a `… N more lines` row; the borders, the padding and
//! the blank rows are not sections. Wherever it lands, a left press answers
//! [`Msg::TranscriptPressed`], which asks for the focus.
//!
//! The keys that focus the turns sidebar are not `Cmd`s either: they answer
//! [`Msg::FocusTurns`]. The sidebar's selection reaches the transcript through
//! [`GO_TO_TURN_ATTR`], which goes to a block the way `]` / `[` do.
//!
//! The subagent keys are not `Cmd`s either: `a` on a subagent section
//! answers [`Msg::OpenSubagent`] (nothing on any other), and the back key of
//! a [`nested`](Transcript::nested) transcript [`Msg::CloseSubagent`].

mod cache;
mod content;
mod details;
mod folding;
mod follow;
mod keys;
mod mouse;
mod nav;
mod paging;
mod search;
mod subagent;
mod text;
mod updates;
mod window;

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::sync::Arc;

use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use ratatui::style::Style;
use ratatui::widgets::Block;
use tuirealm::command::{Cmd, CmdResult, Direction, Position};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::{State, StateValue};

pub(crate) use cache::LayoutCache;
pub use keys::{
    FOLD, FOLD_ALL, HALF_PAGE_DOWN, HALF_PAGE_UP, NEXT_TURN, PAGE_DOWN, PAGE_UP, PREV_TURN,
    TOGGLE_SYSTEM, UNFOLD, UNFOLD_ALL, WHEEL_DOWN, WHEEL_ROWS, WHEEL_UP,
};
pub use search::{
    CLEAR_SEARCH, SEARCH_NEXT, SEARCH_POSITION_ATTR, SEARCH_PREV, SEARCH_QUERY_ATTR,
    SEARCH_REQUEST_ATTR, SEARCH_RESULTS_ATTR, SearchStart, search_results_value,
    search_start_value,
};

use self::content::{Content, Measurer};
use self::folding::FoldOp;
use self::window::Window;
use super::turn_rows::{TURN_ROWS_ATTR, TurnRows};
use super::{THEME_ATTR, payload_from_value, payload_value, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::components::turn_block::SectionId;
use crate::components::turn_block::export::ExportMeta;
use crate::keymap::{Action, Context, KeyResolver, Keymap, Resolved};
use crate::model::{ModelUpdate, Turn, TurnId};
use crate::theme::{Role, Theme};
use crate::timestamp::TimestampFormatter;
use crate::transcript_search::SearchMatches;

/// Whether unselected sections are drawn in `muted_foreground` (a flag).
pub const DIM_UNSELECTED_ATTR: Attribute = Attribute::Custom("dim_unselected");
/// Whether system / injected sections are shown (a flag).
pub const SHOW_SYSTEM_ATTR: Attribute = Attribute::Custom("show_system");
/// The `truncate_output_lines` setting (a length).
pub const TRUNCATE_OUTPUT_LINES_ATTR: Attribute = Attribute::Custom("truncate_output_lines");
/// The label of the key that opens a subagent transcript, shown in the hint
/// of subagent sections (a string).
pub const OPEN_SUBAGENT_KEY_ATTR: Attribute = Attribute::Custom("open_subagent_key");
/// The [`TimestampFormatter`] for block titles (a payload).
pub const TIMESTAMP_FORMATTER_ATTR: Attribute = Attribute::Custom("timestamp_formatter");
/// A [`ModelUpdate`] to apply (an `Arc<ModelUpdate>` payload).
pub const MODEL_UPDATE_ATTR: Attribute = Attribute::Custom("model_update");
/// The number of turns the transcript holds (a length; query only). The rows
/// the turns sidebar lists are asked with [`TURN_ROWS_ATTR`].
pub const TURN_COUNT_ATTR: Attribute = Attribute::Custom("turn_count");
/// The number of sections that arrived while the user was away from the end,
/// the `N` of `↓ N new` (a length; query only). A `Component::attr` cannot
/// answer a `Msg`, so the App reads it with `query` after the updates it
/// forwards; keys report changes themselves as [`Msg::NewSections`].
pub const NEW_SECTIONS_ATTR: Attribute = Attribute::Custom("new_sections");
/// Whether the transcript follows new content (a flag; query only).
pub const FOLLOWING_ATTR: Attribute = Attribute::Custom("following");
/// The selected section (a [`SectionId`] payload; query only; nothing when
/// no section is selected). The App reads it to keep the turns sidebar on the
/// turn that contains it.
pub const SELECTED_SECTION_ATTR: Attribute = Attribute::Custom("selected_section");
/// What the transcript keeps of its presentation: the scroll offset, the
/// selected section and the fold states (a payload; query only).
pub const PRESENTATION_ATTR: Attribute = Attribute::Custom("presentation");
/// The turn whose block to go to (a [`TurnId`] payload; set only): the same
/// scroll and selection as `]` / `[` give, for the turns sidebar. A key
/// reports its changes as a `Msg`; an attribute cannot, so the App reads the
/// count of new sections with [`NEW_SECTIONS_ATTR`] afterwards.
pub const GO_TO_TURN_ATTR: Attribute = Attribute::Custom("go_to_turn");
/// The [`Size`] of the area the transcript is drawn in (a payload).
pub const VIEWPORT_ATTR: Attribute = Attribute::Custom("viewport");

/// The attribute value that applies `update` to the transcript.
pub fn model_update_value(update: ModelUpdate) -> AttrValue {
    payload_value(Arc::new(update))
}

/// The attribute value that announces the viewport size.
pub fn viewport_value(size: Size) -> AttrValue {
    payload_value(size)
}

/// The presentation state of a transcript: what the user changed by looking
/// at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presentation {
    /// The top row of the viewport.
    pub offset: usize,
    /// The selected section.
    pub selected: Option<SectionId>,
    /// Which sections are folded or unfolded, and which outputs expanded.
    pub folds: crate::components::turn_block::FoldState,
}

/// The virtualised transcript component.
#[derive(Debug, Clone)]
pub struct Transcript {
    content: Content,
    cache: LayoutCache,
    /// The top row of the viewport, in transcript rows.
    offset: usize,
    viewport: Option<Size>,
    /// The key bindings of the `browser` context.
    keymap: Keymap,
    /// Resolves keys; remembers the first key of a chord.
    resolver: KeyResolver,
    /// The blocks overlapping the viewport, laid out; `None` until a
    /// viewport is announced.
    window: Option<Window>,
    /// Whether new content scrolls into view: the viewport shows the last
    /// row.
    following: bool,
    /// Whether the initial load is still streaming: nothing scrolls until
    /// it is done.
    loading: bool,
    /// The sections that arrived since the user left the end.
    new_sections: usize,
    /// The first turn whose growth counts as new content while the user is
    /// away from the end.
    tail_from: usize,
    /// The search: what is waited for, the matches and what it unfolded.
    search: search::SearchState,
    /// What the header of an export says about the session.
    export_meta: Option<ExportMeta>,
    /// Whether this is the transcript of a subagent, opened over another
    /// (`Backspace` goes back to it).
    nested: bool,
    /// How many block layouts were run to measure or draw blocks (read by
    /// tests).
    #[cfg_attr(not(test), allow(dead_code))]
    layouts_run: usize,
}

impl Transcript {
    /// An empty transcript with the given theme and timestamp format.
    pub fn new(theme: Theme, formatter: TimestampFormatter) -> Self {
        Self {
            content: Content::new(theme, formatter),
            cache: LayoutCache::new(),
            offset: 0,
            viewport: None,
            keymap: Keymap::default(),
            resolver: KeyResolver::new(Context::Browser),
            window: None,
            following: true,
            loading: false,
            new_sections: 0,
            tail_from: 0,
            search: search::SearchState::default(),
            export_meta: None,
            nested: false,
            layouts_run: 0,
        }
    }

    /// Sets the keymap keys are resolved with (default: the built-in one).
    #[must_use]
    pub fn keymap(mut self, keymap: Keymap) -> Self {
        self.keymap = keymap;
        self.resolver.reset();
        self
    }

    /// Sets what the header of an export says about the session.
    #[must_use]
    pub fn export_meta(mut self, meta: ExportMeta) -> Self {
        self.export_meta = Some(meta);
        self
    }

    /// Marks this as the transcript of a subagent, opened over another one:
    /// the back key then answers [`Msg::CloseSubagent`].
    #[must_use]
    pub fn nested(mut self, nested: bool) -> Self {
        self.nested = nested;
        self
    }

    /// Sets whether unselected sections are dimmed.
    #[must_use]
    pub fn dim_unselected(mut self, dim: bool) -> Self {
        self.set_dim_unselected(dim);
        self
    }

    /// Sets whether system sections are shown.
    #[must_use]
    pub fn show_system(mut self, show: bool) -> Self {
        self.set_show_system(show);
        self
    }

    /// Sets the viewport the first layout is for.
    #[must_use]
    pub fn viewport(mut self, size: Size) -> Self {
        self.set_viewport(size);
        self
    }

    /// Adds `turns` at the end.
    #[must_use]
    pub fn with_turns(mut self, turns: Vec<Arc<Turn>>) -> Self {
        self.apply_update(&ModelUpdate::Appended(turns));
        self
    }

    /// The selected section.
    pub fn selected(&self) -> Option<&SectionId> {
        self.content.selected.as_ref()
    }

    /// The scroll offset: the top row of the viewport.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The scroll offset, the selection and the fold states.
    pub fn presentation(&self) -> Presentation {
        Presentation {
            offset: self.offset,
            selected: self.content.selected.clone(),
            folds: self.content.folds.clone(),
        }
    }

    /// The number of turns.
    pub fn turn_count(&self) -> usize {
        self.content.turns.len()
    }

    /// The turn at `index`.
    pub fn turn(&self, index: usize) -> Option<&Arc<Turn>> {
        self.content.turns.get(index)
    }

    /// The `truncate_output_lines` setting.
    pub fn truncate_output_lines(&self) -> usize {
        self.content.truncate_output_lines
    }

    /// The layout cache (tests read its counters).
    #[cfg(test)]
    pub(crate) fn cache(&self) -> &LayoutCache {
        &self.cache
    }

    /// How many block layouts were run to measure or draw blocks (not
    /// counting the ones navigation and selection need).
    #[cfg(test)]
    pub(crate) fn layouts_run(&self) -> usize {
        self.layouts_run
    }

    /// The indices of the blocks the prepared window draws.
    pub(crate) fn visible_blocks(&self) -> std::ops::Range<usize> {
        self.window.as_ref().map_or(0..0, Window::block_range)
    }

    /// Announces the area the transcript is drawn in: the pre-render step.
    ///
    /// A new width re-wraps every block; the top visible block stays at the
    /// top. The offset is clamped to the new height.
    pub fn set_viewport(&mut self, size: Size) {
        if self.viewport == Some(size) {
            return;
        }
        let resized = self.viewport.is_some();
        let pinned = resized && self.following && !self.loading;
        let anchor = match self.viewport {
            Some(old) if old.width != size.width && !pinned => self.anchor(),
            _ => None,
        };
        self.viewport = Some(size);
        self.cache.set_width(size.width);
        if let Some((block, within)) = anchor {
            self.offset = self.measured(|cache, m| {
                let top = cache.top(block, m);
                top + within.min(cache.height(block, m).saturating_sub(1))
            });
        }
        self.ensure_selection();
        if pinned {
            // At the end of the transcript, a new size keeps it there.
            self.pin_to_bottom();
        }
        self.refresh();
        self.update_following();
    }

    /// The block at the top of the viewport and how far into it the viewport
    /// starts.
    fn anchor(&mut self) -> Option<(usize, usize)> {
        let offset = self.offset;
        let found = self.measured(|cache, m| cache.locate(offset, m))?;
        Some((found.block, offset - found.top))
    }

    /// Runs `f` with the cache and a measurer for the cache's width.
    fn measured<R>(&mut self, f: impl FnOnce(&mut LayoutCache, &mut Measurer<'_>) -> R) -> R {
        let mut measurer = Measurer::new(&self.content, self.cache.width());
        let result = f(&mut self.cache, &mut measurer);
        self.layouts_run += measurer.calls;
        result
    }

    /// Like [`measured`](Self::measured), with `layouts` of blocks that the
    /// measurer hands out instead of laying them out again.
    fn measured_reusing<R>(
        &mut self,
        layouts: std::collections::HashMap<usize, crate::components::turn_block::BlockLayout>,
        f: impl FnOnce(&mut LayoutCache, &mut Measurer<'_>) -> R,
    ) -> R {
        let mut measurer = Measurer::new(&self.content, self.cache.width()).reusing(layouts);
        let result = f(&mut self.cache, &mut measurer);
        self.layouts_run += measurer.calls;
        result
    }

    /// Brings the window up to date with the content, the offset and the
    /// viewport. Called after every change.
    fn refresh(&mut self) {
        let Some(size) = self.viewport else {
            self.window = None;
            return;
        };
        let offset = self.offset;
        let window = self.measured(|cache, m| Window::build_clamped(cache, m, size, offset));
        self.offset = window.offset;
        self.window = Some(window);
    }

    /// Like [`refresh`](Self::refresh) after a change of the offset alone:
    /// the layouts of the blocks that stay in the window are reused, and only
    /// the newly exposed blocks are laid out.
    fn refresh_scrolled(&mut self) {
        let Some(size) = self.viewport else {
            return;
        };
        let reuse = self.window.take().map(Window::into_layouts);
        let offset = self.offset;
        let window = self.measured_reusing(reuse.unwrap_or_default(), |cache, m| {
            Window::build_clamped(cache, m, size, offset)
        });
        self.offset = window.offset;
        self.window = Some(window);
    }

    /// The window for `size`: the prepared one, or one computed on a copy of
    /// the cache so that `view` leaves the transcript untouched.
    fn window_for(&self, size: Size) -> Cow<'_, Window> {
        match &self.window {
            Some(window) if window.size == size => Cow::Borrowed(window),
            _ => {
                let mut cache = self.cache.clone();
                cache.set_width(size.width);
                let mut measurer = Measurer::new(&self.content, size.width);
                let offset = self.offset;
                Cow::Owned(Window::build_clamped(
                    &mut cache,
                    &mut measurer,
                    size,
                    offset,
                ))
            }
        }
    }

    fn paint_background(&self, frame: &mut Frame, area: Rect) {
        if !self.content.theme.transparent_background() {
            let style = Style::new().bg(self.content.theme.role(Role::Background));
            frame.render_widget(Block::new().style(style), area);
        }
    }

    fn set_dim_unselected(&mut self, dim: bool) {
        self.content.dim_unselected = dim;
        self.refresh();
    }

    fn set_theme(&mut self, theme: Theme) {
        self.content.theme = theme;
        self.refresh();
    }

    fn set_formatter(&mut self, formatter: TimestampFormatter) {
        self.content.formatter = formatter;
        self.refresh();
    }

    /// Sets `truncate_output_lines`. Tool bodies depend on it, so every
    /// height is forgotten; a selected truncation row that no longer exists
    /// is replaced by the nearest section.
    fn set_truncate_output_lines(&mut self, lines: usize) {
        if self.content.truncate_output_lines == lines {
            return;
        }
        let before = self.sections_if(|_| true);
        self.content.truncate_output_lines = lines;
        self.relayout_all(before);
    }

    /// Sets the label of the `open_subagent` key and lays out again.
    fn set_open_subagent_key(&mut self, label: String) {
        if self.content.open_subagent_key == label {
            return;
        }
        let before = self.sections_if(|_| true);
        self.content.open_subagent_key = label;
        self.relayout_all(before);
    }

    /// Forgets every height after a setting that changes bodies, and keeps
    /// the selection on a section that still exists.
    fn relayout_all(&mut self, before: Option<Vec<SectionId>>) {
        self.cache.invalidate_all();
        self.restore_selection(before);
        self.refresh();
    }
}

impl Component for Transcript {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        self.paint_background(frame, area);
        let window = self.window_for(area.as_size());
        window.render(&self.content, area, frame.buffer_mut());
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        let value = if attr == THEME_ATTR {
            theme_value(&self.content.theme)
        } else if attr == DIM_UNSELECTED_ATTR {
            AttrValue::Flag(self.content.dim_unselected)
        } else if attr == SHOW_SYSTEM_ATTR {
            AttrValue::Flag(self.content.show_system)
        } else if attr == TRUNCATE_OUTPUT_LINES_ATTR {
            AttrValue::Length(self.content.truncate_output_lines)
        } else if attr == OPEN_SUBAGENT_KEY_ATTR {
            AttrValue::String(self.content.open_subagent_key.clone())
        } else if attr == TIMESTAMP_FORMATTER_ATTR {
            payload_value(self.content.formatter.clone())
        } else if attr == TURN_COUNT_ATTR {
            AttrValue::Length(self.content.turns.len())
        } else if attr == TURN_ROWS_ATTR {
            // One pass over the turns, deriving each one's sections only:
            // nothing is laid out or rendered.
            payload_value(TurnRows::from_turns(
                self.content.turns.iter().map(AsRef::as_ref),
            ))
        } else if attr == NEW_SECTIONS_ATTR {
            AttrValue::Length(self.new_sections)
        } else if attr == FOLLOWING_ATTR {
            AttrValue::Flag(self.following)
        } else if attr == PRESENTATION_ATTR {
            payload_value(self.presentation())
        } else if attr == SELECTED_SECTION_ATTR {
            payload_value(self.content.selected.clone()?)
        } else if attr == VIEWPORT_ATTR {
            payload_value(self.viewport?)
        } else if attr == SEARCH_REQUEST_ATTR {
            payload_value(self.search_request()?)
        } else if attr == SEARCH_POSITION_ATTR {
            payload_value(self.search_position())
        } else {
            return None;
        };
        Some(QueryResult::Owned(value))
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == THEME_ATTR {
            if let Some(theme) = theme_from_value(&value) {
                self.set_theme(theme);
            }
        } else if attr == DIM_UNSELECTED_ATTR {
            if let AttrValue::Flag(dim) = value {
                self.set_dim_unselected(dim);
            }
        } else if attr == SHOW_SYSTEM_ATTR {
            if let AttrValue::Flag(show) = value {
                self.set_show_system(show);
            }
        } else if attr == TRUNCATE_OUTPUT_LINES_ATTR {
            if let AttrValue::Length(lines) = value {
                self.set_truncate_output_lines(lines);
            }
        } else if attr == OPEN_SUBAGENT_KEY_ATTR {
            if let AttrValue::String(label) = value {
                self.set_open_subagent_key(label);
            }
        } else if attr == TIMESTAMP_FORMATTER_ATTR {
            if let Some(formatter) = payload_from_value::<TimestampFormatter>(&value) {
                self.set_formatter(formatter);
            }
        } else if attr == MODEL_UPDATE_ATTR {
            if let Some(update) = payload_from_value::<Arc<ModelUpdate>>(&value) {
                self.apply_update(&update);
            }
        } else if attr == GO_TO_TURN_ATTR {
            if let Some(id) = payload_from_value::<TurnId>(&value) {
                self.go_to_turn(&id);
            }
        } else if attr == SEARCH_QUERY_ATTR {
            if let Some(start) = payload_from_value::<SearchStart>(&value) {
                self.begin_search(start);
            }
        } else if attr == SEARCH_RESULTS_ATTR {
            if let Some(results) = payload_from_value::<SearchMatches>(&value) {
                self.apply_search_results(&results);
            }
        } else if attr == VIEWPORT_ATTR
            && let Some(size) = payload_from_value::<Size>(&value)
        {
            self.set_viewport(size);
        }
    }

    /// The scroll offset.
    fn state(&self) -> State {
        State::Single(StateValue::Usize(self.offset))
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        let result = self.run(cmd);
        // Whatever moved the selection: a search's unfolding ends with it.
        self.settle_temp_unfold();
        self.update_following();
        result
    }
}

impl Transcript {
    /// [`Msg::SaveSection`] for the selected section with `text`.
    fn save_msg(&self, text: Option<String>) -> Option<Msg> {
        Some(Msg::SaveSection {
            text: text?,
            kind: self.file_kind()?,
        })
    }

    /// [`Msg::Export`] with the session as markdown.
    fn export_msg(&self, full: bool) -> Msg {
        Msg::Export {
            full,
            document: self.export_document(full),
        }
    }

    /// Runs a `Cmd` (`perform` then decides whether the view follows).
    fn run(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Scroll(Direction::Down) => self.scroll_down(),
            Cmd::Scroll(Direction::Up) => self.scroll_up(),
            Cmd::Move(Direction::Down) => self.move_selection(nav::Step::Next),
            Cmd::Move(Direction::Up) => self.move_selection(nav::Step::Previous),
            Cmd::GoTo(Position::Begin) => self.jump_to_start(),
            Cmd::GoTo(Position::End) => self.jump_to_end(),
            Cmd::Submit => self.fold_command(FoldOp::Toggle),
            Cmd::Custom(name) => self.custom(name),
            Cmd::None => CmdResult::NoChange,
            other => CmdResult::Invalid(other),
        }
    }
}

impl AppComponent<Msg, UserEvent> for Transcript {
    /// Resolves the key in the `browser` context and performs the `Cmd` of
    /// its action. Returns [`Msg::OpenSearch`] for the search key,
    /// [`Msg::SearchPosition`] when a key moved among the matches or cleared
    /// the search, [`Msg::NewSections`] when it changed the count of new
    /// sections (`G` resuming following), else [`Msg::TranscriptChanged`]
    /// when that changed what is shown, so that the App redraws. The copy
    /// keys answer [`Msg::CopyText`] with the text of the selected section
    /// (as displayed, or in full); with nothing selected they do nothing.
    /// The save keys answer [`Msg::SaveSection`] the same way, the export
    /// keys [`Msg::Export`] with the whole document. The open key answers
    /// [`Msg::OpenSubagent`] on a subagent section (nothing on any other),
    /// and the back key [`Msg::CloseSubagent`] when the transcript is
    /// [`nested`](Self::nested). The resume key answers
    /// [`Msg::RequestResume`] for the open session. The details key answers
    /// [`Msg::OpenDetails`] with the rows of the selected section (nothing
    /// with nothing selected).
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        let key = match ev {
            Event::Keyboard(key) => key,
            // Only what the router sent here: the raw event reaches the
            // focused component in screen coordinates.
            Event::User(UserEvent::Pointer(mouse)) => return self.on_mouse(mouse),
            _ => return None,
        };
        let Resolved::Action(action) = self.resolver.feed(&self.keymap, key) else {
            return None;
        };
        match action {
            Action::Search => return Some(Msg::OpenSearch),
            Action::OpenSubagent => return self.open_subagent_msg(),
            Action::Back => return self.nested.then_some(Msg::CloseSubagent),
            Action::Copy => return self.displayed_text().map(Msg::CopyText),
            Action::CopyFull => return self.full_text().map(Msg::CopyText),
            Action::Save => return self.save_msg(self.displayed_text()),
            Action::SaveFull => return self.save_msg(self.full_text()),
            Action::Export => return Some(self.export_msg(false)),
            Action::ExportFull => return Some(self.export_msg(true)),
            Action::Resume => return Some(Msg::RequestResume(None)),
            Action::Details => return self.details_msg(),
            // The keys that hand the focus to the turns sidebar; the App
            // decides whether there is one.
            Action::FocusTurns | Action::NextPane | Action::PrevPane => {
                return Some(Msg::FocusTurns);
            }
            _ => {}
        }
        let cmd = keys::cmd_for(action)?;
        let new_before = self.new_sections;
        let result = self.perform(cmd);
        let changed = !matches!(result, CmdResult::NoChange | CmdResult::Invalid(_));
        if changed
            && matches!(
                action,
                Action::SearchNext | Action::SearchPrev | Action::ClearSearch
            )
        {
            return Some(Msg::SearchPosition(self.search_position()));
        }
        // The count changed (`G` resumed following): the App shows it.
        if self.new_sections != new_before {
            return Some(Msg::NewSections(self.new_sections));
        }
        changed.then_some(Msg::TranscriptChanged)
    }
}
