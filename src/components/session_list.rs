//! The Sessions pane of the session selector (product §3.1, §6.3, §7.2).

use std::sync::Arc;

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;
use unicode_width::UnicodeWidthStr;

use super::empty_state::{EmptyState, NO_MATCHING_SESSIONS, NO_SESSIONS};
use super::list_pane::{
    PaneKey, PaneState, Part, RowParts, border_role, pane_key, render_rows, title_width,
    titled_block, visible_rows,
};
use super::text_fit::truncate_with_ellipsis;
use super::{THEME_ATTR, payload_from_value, payload_value};
use crate::app::{Msg, Pane, RoutedMouse, SelectorMsg, UserEvent};
use crate::duration::format_duration;
use crate::keymap::Keymap;
use crate::model::{HarnessId, SessionId, SessionStats};
use crate::theme::Theme;

/// The attribute that sets the rows ([`SessionRows`]).
pub const SESSION_ROWS_ATTR: Attribute = Attribute::Custom("session_rows");
/// The attribute that sets the stats of one row ([`SessionStatsUpdate`]).
pub const SESSION_STATS_ATTR: Attribute = Attribute::Custom("session_stats");
/// The attribute that tells the list its rows are the sessions an id prefix
/// of the command line matches (a flag): `Esc` then leaves that filter
/// instead of closing the selector or quitting.
pub const SESSION_FILTERED_ATTR: Attribute = Attribute::Custom("session_filtered");
/// The attribute that sets which rows show the `●` marker
/// ([`ActiveSessions`]).
pub const ACTIVE_ATTR: Attribute = Attribute::Custom("active_sessions");
/// The attribute that moves the selection of the search results
/// (a payload of `tuirealm::command::Direction`: `Up` or `Down`), for the owner of the prompt
/// that has the input focus meanwhile.
pub const SEARCH_NAV_ATTR: Attribute = Attribute::Custom("search_nav");

/// The columns between the timestamp and the title, and between the title
/// and the stats.
const GAP: &str = "  ";
/// The cells the active marker slot takes: `● ` or two spaces.
const DOT_SLOT_WIDTH: usize = 2;
/// Stats are left out when fewer than this many cells would remain for the
/// title.
pub const MIN_TITLE_WIDTH: usize = 10;

/// One session row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRow {
    /// The session's id, sent in [`SelectorMsg::SessionSelected`].
    pub id: SessionId,
    /// The start time, already formatted.
    pub started: String,
    /// The title.
    pub title: String,
    /// The display name of the session's project, shown before the title in
    /// search results (`<project> › <title>`).
    pub project: Option<String>,
    /// Whether the session shows the `●` marker.
    pub active: bool,
    /// The stats, once they are computed.
    pub stats: Option<SessionStats>,
}

/// The rows of the list, the title of the pane and which row is selected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionRows {
    /// The display name of the project, for the title `Sessions in <name>`.
    pub project: String,
    /// The query when the rows are search results: the title then reads
    /// `Sessions matching "<query>"`, and no rows read `No matching
    /// sessions`.
    pub search: Option<String>,
    /// The harness the sessions belong to, sent in [`Msg::OpenSession`].
    pub harness: Option<HarnessId>,
    pub rows: Arc<Vec<SessionRow>>,
    pub selected: Option<usize>,
    /// Whether the harness has nothing to list at all. The pane then shows
    /// "No sessions for this harness". A project without sessions in a
    /// harness that has some does not.
    pub harness_empty: bool,
}

/// The stats of one session, for the row that has its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStatsUpdate {
    pub session: SessionId,
    pub stats: SessionStats,
}

/// The sessions that are active; every other row shows no marker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActiveSessions(pub Vec<SessionId>);

/// The stats of a row: `<n> turns · <duration>`, or `1 turn · <duration>`.
pub fn format_stats(stats: &SessionStats) -> String {
    let unit = if stats.turns == 1 { "turn" } else { "turns" };
    format!(
        "{} {unit} · {}",
        stats.turns,
        format_duration(stats.agent_time)
    )
}

/// The Sessions pane: `<start>  <title>` rows with muted stats on the right,
/// and `●` in the rows of active sessions. It takes focus.
///
/// Rows come in through [`SESSION_ROWS_ATTR`]; stats and the active flags
/// are updated one by one through [`SESSION_STATS_ATTR`] and
/// [`ACTIVE_ATTR`]. Moving the selection reports
/// [`SelectorMsg::SessionSelected`]; `Enter` on the selected session asks to
/// open it ([`Msg::OpenSession`]) and `R` to resume it in its harness
/// ([`Msg::RequestResume`]). When the app says the harness has nothing
/// to list, the pane shows the [`EmptyState`] "No sessions for this harness".
///
/// While the rows are filtered ([`SESSION_FILTERED_ATTR`]) `Esc` asks to
/// leave the filter ([`SelectorMsg::LeaveFilter`]).
///
/// Search results come in the same way, with [`SessionRows::search`] set: the
/// rows then carry their project's name. The list keeps the focus's look
/// while the prompt has the input focus; the app moves the selection with
/// [`SEARCH_NAV_ATTR`].
#[derive(Debug, Clone)]
pub struct SessionList {
    pane: PaneState,
    keymap: Keymap,
    project: String,
    search: Option<String>,
    /// Whether the rows are the matches of an id prefix.
    filtered: bool,
    harness: Option<HarnessId>,
    rows: Arc<Vec<SessionRow>>,
    harness_empty: bool,
    empty: EmptyState,
    no_matches: EmptyState,
}

impl SessionList {
    /// An empty list that resolves keys with `keymap`.
    pub fn new(keymap: Keymap, theme: Theme) -> Self {
        Self {
            empty: EmptyState::new(NO_SESSIONS, theme.clone()).without_background(),
            no_matches: EmptyState::new(NO_MATCHING_SESSIONS, theme.clone()).without_background(),
            pane: PaneState::new(theme),
            keymap,
            project: String::new(),
            search: None,
            filtered: false,
            harness: None,
            rows: Arc::default(),
            harness_empty: false,
        }
    }

    /// The request to open the selected session, if there is one.
    fn open(&self) -> Option<Msg> {
        let harness = self.harness?;
        self.selected_id()
            .map(|session| Msg::OpenSession(harness, session))
    }

    /// The request to resume the selected session, if there is one.
    fn resume(&self) -> Option<Msg> {
        let harness = self.harness?;
        self.selected_id()
            .map(|session| Msg::RequestResume(Some((harness, session))))
    }

    fn selected_id(&self) -> Option<SessionId> {
        self.pane
            .cursor
            .selected()
            .and_then(|i| self.rows.get(i))
            .map(|row| row.id.clone())
    }

    fn title(&self, pane_width: u16) -> String {
        let title = if let Some(query) = &self.search {
            // The query is cut inside the quotes, so the title still reads
            // as a quoted query.
            let room = title_width(pane_width).saturating_sub("Sessions matching \"\"".width());
            format!(
                "Sessions matching \"{}\"",
                truncate_with_ellipsis(query, room.max(1))
            )
        } else if self.project.is_empty() {
            "Sessions".to_owned()
        } else {
            format!("Sessions in {}", self.project)
        };
        truncate_with_ellipsis(&title, title_width(pane_width))
    }

    fn moved(&mut self, direction: Direction) -> Option<Msg> {
        match self.perform(Cmd::Move(direction)) {
            CmdResult::Changed(_) => self
                .selected_id()
                .map(|id| Msg::Selector(SelectorMsg::SessionSelected(id))),
            _ => None,
        }
    }

    /// The wheel moves the selection like the keys, or, with none selected,
    /// only scrolls the rows. A left press on a row selects it and reports
    /// the session; elsewhere in the pane it asks for the focus.
    fn on_mouse(&mut self, mouse: &RoutedMouse) -> Option<Msg> {
        let wheel = |list: &mut Self, direction| {
            if list.pane.cursor.selected().is_some() {
                return list.moved(direction);
            }
            let visible = visible_rows(mouse.area.height);
            let down = direction == Direction::Down;
            list.pane
                .cursor
                .scroll(down, visible)
                .then_some(Msg::Selector(SelectorMsg::ListScrolled))
        };
        match mouse.kind {
            MouseEventKind::ScrollUp => wheel(self, Direction::Up),
            MouseEventKind::ScrollDown => wheel(self, Direction::Down),
            _ if mouse.is_left_press() => {
                let row = self.pane.row_at(mouse.column, mouse.row, mouse.area);
                let Some(index) = row.filter(|&i| i < self.rows.len()) else {
                    return Some(Msg::Selector(SelectorMsg::PaneClicked(Pane::Sessions)));
                };
                // The list moves its own cursor: over search results and
                // filtered sessions the app does not track the selection.
                self.pane.cursor.select(Some(index));
                Some(Msg::Selector(SelectorMsg::SessionClicked(
                    self.rows[index].id.clone(),
                )))
            }
            _ => None,
        }
    }

    fn set_stats(&mut self, update: &SessionStatsUpdate) {
        if let Some(row) = Arc::make_mut(&mut self.rows)
            .iter_mut()
            .find(|row| row.id == update.session)
        {
            row.stats = Some(update.stats);
        }
    }

    fn set_active(&mut self, active: &ActiveSessions) {
        for row in Arc::make_mut(&mut self.rows).iter_mut() {
            row.active = active.0.contains(&row.id);
        }
    }
}

/// The project's name is cut to about a third of the row's room, so the
/// title stays visible after it.
const MIN_PROJECT_WIDTH: usize = 8;

/// What a row shows after the dot, in `room` cells: its title, after the
/// project's name in search results.
fn label(row: &SessionRow, room: usize) -> String {
    match &row.project {
        Some(project) => {
            let project = truncate_with_ellipsis(project, (room / 3).max(MIN_PROJECT_WIDTH));
            format!("{project} › {}", row.title)
        }
        None => row.title.clone(),
    }
}

/// The pieces of `row` for a text width of `width` columns: the start, the
/// label cut with `…`, and the stats on the right unless they would leave
/// fewer than [`MIN_TITLE_WIDTH`] cells for it.
fn row_parts(row: &SessionRow, width: usize) -> RowParts {
    let prefix = row.started.width() + GAP.width() + DOT_SLOT_WIDTH;
    let stats = row.stats.as_ref().map(format_stats);
    let with_stats = stats.as_ref().and_then(|text| {
        let room = width.saturating_sub(prefix + GAP.width() + text.width());
        (room >= MIN_TITLE_WIDTH).then_some((room, text.clone()))
    });
    let (room, stats) = match with_stats {
        Some((room, text)) => (room, Some(text)),
        None => (width.saturating_sub(prefix), None),
    };
    let dot = if row.active {
        (Part::Dot, "● ".to_owned())
    } else {
        (Part::Text, "  ".to_owned())
    };
    RowParts {
        left: vec![
            (Part::Text, row.started.clone()),
            (Part::Text, GAP.to_owned()),
            dot,
            (Part::Text, truncate_with_ellipsis(&label(row, room), room)),
        ],
        stats,
    }
}

impl Component for SessionList {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        let title = self.title(area.width);
        let block = titled_block(&self.pane.theme, &title, border_role(self.pane.focused));
        frame.render_widget(block, area);
        // Inside the border and the same 2-cell margin the rows keep.
        let interior = area.inner(Margin::new(3, 1));
        if self.harness_empty {
            self.empty.view(frame, interior);
            return;
        }
        if self.search.is_some() && self.rows.is_empty() {
            self.no_matches.view(frame, interior);
            return;
        }
        render_rows(frame, area, &self.pane, |index, width| {
            self.rows
                .get(index)
                .map_or_else(RowParts::default, |row| row_parts(row, width))
        });
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        if attr == SESSION_ROWS_ATTR {
            let rows = SessionRows {
                project: self.project.clone(),
                search: self.search.clone(),
                harness: self.harness,
                rows: Arc::clone(&self.rows),
                selected: self.pane.cursor.selected(),
                harness_empty: self.harness_empty,
            };
            return Some(QueryResult::Owned(payload_value(rows)));
        }
        self.pane.query(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match attr {
            SESSION_ROWS_ATTR => {
                if let Some(SessionRows {
                    project,
                    search,
                    harness,
                    rows,
                    selected,
                    harness_empty,
                }) = payload_from_value(&value)
                {
                    self.harness_empty = harness_empty;
                    self.pane.cursor.set(rows.len(), selected);
                    self.project = project;
                    self.search = search;
                    self.harness = harness;
                    self.rows = rows;
                }
            }
            SESSION_STATS_ATTR => {
                if let Some(update) = payload_from_value::<SessionStatsUpdate>(&value) {
                    self.set_stats(&update);
                }
            }
            ACTIVE_ATTR => {
                if let Some(active) = payload_from_value::<ActiveSessions>(&value) {
                    self.set_active(&active);
                }
            }
            SESSION_FILTERED_ATTR => {
                if let AttrValue::Flag(filtered) = value {
                    self.filtered = filtered;
                }
            }
            SEARCH_NAV_ATTR => {
                if let Some(direction) = payload_from_value::<Direction>(&value) {
                    self.pane.perform(Cmd::Move(direction));
                }
            }
            _ => {
                if attr == THEME_ATTR {
                    self.empty.attr(attr, value.clone());
                    self.no_matches.attr(attr, value.clone());
                }
                self.pane.attr(attr, &value);
            }
        }
    }

    fn state(&self) -> State {
        self.pane.state()
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        self.pane.perform(cmd)
    }
}

impl AppComponent<Msg, UserEvent> for SessionList {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        if let Event::User(UserEvent::Mouse(mouse)) = ev {
            return self.on_mouse(mouse);
        }
        match pane_key(&self.keymap, ev)? {
            PaneKey::Up => self.moved(Direction::Up),
            PaneKey::Down => self.moved(Direction::Down),
            PaneKey::FocusProjects | PaneKey::NextPane | PaneKey::PrevPane => {
                Some(Msg::Selector(SelectorMsg::Focus(Pane::Projects)))
            }
            PaneKey::FocusSessions => None,
            PaneKey::Search => Some(Msg::Selector(SelectorMsg::Search)),
            PaneKey::Resume => self.resume(),
            PaneKey::Open => self.open(),
            PaneKey::Cancel if self.filtered => Some(Msg::Selector(SelectorMsg::LeaveFilter)),
            PaneKey::Cancel => Some(self.pane.cancel()),
            PaneKey::Harness(harness) => Some(Msg::Selector(SelectorMsg::SwitchHarness(harness))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{SELECTED_ATTR, SESSION_OPEN_ATTR, VISIBLE_ROWS_ATTR, selected_value};
    use crate::model::HarnessId;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::{Role, builtin};
    use std::time::Duration;
    use tuirealm::event::{Key, KeyEvent, KeyModifiers};
    use tuirealm::state::StateValue;

    fn stats(turns: usize, secs: u64) -> SessionStats {
        SessionStats {
            turns,
            agent_time: Duration::from_secs(secs),
            run_time: Duration::ZERO,
            tokens: crate::model::TokenUsage::default(),
        }
    }

    fn row(id: &str, title: &str, active: bool, with_stats: Option<SessionStats>) -> SessionRow {
        SessionRow {
            id: SessionId::from(id),
            started: "2026-10-30 11:40Z".to_owned(),
            title: title.to_owned(),
            project: None,
            active,
            stats: with_stats,
        }
    }

    fn sample(with_stats: bool) -> Vec<SessionRow> {
        let s = |n, secs| with_stats.then(|| stats(n, secs));
        vec![
            row("a", "Fix the flaky login test", true, s(23, 3900)),
            row("b", "Refactor the index worker", false, s(1, 45)),
            row("c", "(untitled)", false, s(12, 720)),
        ]
    }

    fn rows_value(rows: Vec<SessionRow>, selected: Option<usize>) -> AttrValue {
        payload_value(SessionRows {
            search: None,
            project: "BRIO".to_owned(),
            harness: Some(HarnessId::Claude),
            rows: Arc::new(rows),
            selected,
            harness_empty: false,
        })
    }

    fn list(rows: Vec<SessionRow>, selected: Option<usize>, focused: bool) -> SessionList {
        let mut list = SessionList::new(Keymap::default(), builtin::default_theme());
        list.attr(SESSION_ROWS_ATTR, rows_value(rows, selected));
        list.attr(Attribute::Focus, AttrValue::Flag(focused));
        list
    }

    fn draw_sized(list: &mut SessionList, width: u16) -> ratatui::buffer::Buffer {
        render(width, 10, |frame| list.view(frame, frame.area()))
    }

    fn key(code: Key) -> Event<UserEvent> {
        Event::Keyboard(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ch(c: char) -> Event<UserEvent> {
        key(Key::Char(c))
    }

    fn selected(id: &str) -> Option<Msg> {
        Some(Msg::Selector(SelectorMsg::SessionSelected(
            SessionId::from(id),
        )))
    }

    // -- formatting

    #[test]
    fn stats_read_turns_and_duration() {
        assert_eq!(format_stats(&stats(12, 720)), "12 turns · 12m 0s");
        assert_eq!(format_stats(&stats(1, 45)), "1 turn · 45s");
        assert_eq!(format_stats(&stats(0, 0)), "0 turns · 0s");
        assert_eq!(format_stats(&stats(12, 59 * 60 + 59)), "12 turns · 59m 59s");
        assert_eq!(format_stats(&stats(23, 3900)), "23 turns · 1h 5m");
    }

    #[test]
    fn the_stats_use_the_shared_duration_format() {
        let duration = Duration::from_millis(59 * 60_000 + 59_900);
        let stats = SessionStats {
            turns: 3,
            agent_time: duration,
            ..stats(3, 0)
        };
        assert_eq!(
            format_stats(&stats),
            format!("3 turns · {}", format_duration(duration))
        );
        assert_eq!(format_stats(&stats), "3 turns · 59m 59s");
    }

    #[test]
    fn the_row_shows_agent_time_not_run_time() {
        let stats = SessionStats {
            run_time: Duration::from_secs(81 * 3600 + 33 * 60),
            ..stats(6, 17 * 3600 + 56 * 60)
        };
        assert_eq!(format_stats(&stats), "6 turns · 17h 56m");
    }

    #[test]
    fn a_session_without_turns_shows_zero() {
        assert_eq!(format_stats(&stats(0, 0)), "0 turns · 0s");
    }

    #[test]
    fn stats_need_ten_cells_for_the_title() {
        // Prefix 17 + 2 + 2 = 21; stats "1 turn · 45s" is 12 wide plus a
        // 2-cell gap: the title gets `width - 35` cells.
        let r = row(
            "a",
            "A long enough title for any room",
            false,
            Some(stats(1, 45)),
        );
        let parts = row_parts(&r, 45);
        assert_eq!(parts.stats.as_deref(), Some("1 turn · 45s"));
        assert_eq!(parts.left[3].1, "A long en…");
        assert_eq!(parts.left[3].1.width(), 10);

        let parts = row_parts(&r, 44);
        assert_eq!(parts.stats, None, "9 cells would remain");
        assert_eq!(parts.left[3].1.width(), 44 - 21);
    }

    #[test]
    fn seconds_under_an_hour_can_push_the_stats_out() {
        // Prefix 21 + 2 for the gap. In the old format the stats of this row
        // were `12 turns · 59m` (14 wide), leaving exactly 10 cells at a
        // text width of 47. The new `12 turns · 59m 59s` is 18 wide.
        let r = row(
            "a",
            "Fix the flaky login test",
            false,
            Some(stats(12, 59 * 60 + 59)),
        );
        let parts = row_parts(&r, 47);
        assert_eq!(parts.stats, None, "6 cells would remain");
        assert_eq!(parts.left[3].1, "Fix the flaky login test");

        let parts = row_parts(&r, 51);
        assert_eq!(parts.stats.as_deref(), Some("12 turns · 59m 59s"));
        assert_eq!(parts.left[3].1, "Fix the f…");

        // The same row with the old stats text would still have fitted.
        let old = row_parts(
            &row("a", "Fix the flaky login test", false, Some(stats(12, 59))),
            47,
        );
        assert_eq!(old.stats.as_deref(), Some("12 turns · 59s"));
    }

    #[test]
    fn a_short_title_keeps_its_stats_when_there_is_room() {
        let r = row("a", "Hi", false, Some(stats(1, 45)));
        let parts = row_parts(&r, 60);
        assert_eq!(parts.left[3].1, "Hi");
        assert!(parts.stats.is_some());
    }

    #[test]
    fn rows_without_stats_show_none() {
        let r = row("a", "Title", false, None);
        assert_eq!(row_parts(&r, 60).stats, None);
    }

    #[test]
    fn the_title_is_cut_with_an_ellipsis_and_never_exceeds_the_room() {
        let r = row("a", "日本語のとても長いタイトルです", false, None);
        for width in 22..60 {
            let parts = row_parts(&r, width);
            assert!(parts.left[3].1.width() <= width - 21, "{width}");
        }
        let r = row("a", "Fits", false, None);
        assert_eq!(row_parts(&r, 30).left[3].1, "Fits");
    }

    #[test]
    fn the_dot_slot_is_always_two_cells() {
        let active = row_parts(&row("a", "T", true, None), 60);
        let idle = row_parts(&row("a", "T", false, None), 60);
        assert_eq!(active.left[2], (Part::Dot, "● ".to_owned()));
        assert_eq!(idle.left[2], (Part::Text, "  ".to_owned()));
    }

    // -- drawing

    #[test]
    fn rows_with_stats_snapshot() {
        let mut l = list(sample(true), Some(0), true);
        insta::assert_snapshot!(buffer_to_styled(&draw_sized(&mut l, 60)));
    }

    #[test]
    fn stats_that_no_longer_fit_snapshot() {
        // A 55 column pane has 47 columns for the row: the title fits next
        // to `12 turns · 59m` but not next to `12 turns · 59m 59s`.
        let rows = vec![
            row(
                "a",
                "Fix the flaky login test",
                false,
                Some(stats(12, 59 * 60 + 59)),
            ),
            row("b", "Fix the flaky login test", false, Some(stats(12, 59))),
        ];
        let mut l = list(rows, Some(0), true);
        insta::assert_snapshot!(buffer_to_styled(&draw_sized(&mut l, 55)));
    }

    #[test]
    fn rows_without_stats_snapshot() {
        let mut l = list(sample(false), None, true);
        insta::assert_snapshot!(buffer_to_styled(&draw_sized(&mut l, 60)));
    }

    #[test]
    fn the_title_names_the_project() {
        let text = buffer_to_text(&draw_sized(&mut list(sample(false), None, false), 60));
        assert!(text.starts_with("╭─ Sessions in BRIO ─"), "{text}");
        let text = buffer_to_text(&draw_sized(&mut list(Vec::new(), None, false), 60));
        assert!(text.contains("Sessions in BRIO"));
    }

    #[test]
    fn a_long_title_is_cut_to_fit_the_border() {
        let mut l = list(sample(false), None, false);
        l.attr(
            SESSION_ROWS_ATTR,
            payload_value(SessionRows {
                search: None,
                project: "A project with a really long display name".to_owned(),
                harness: None,
                rows: Arc::default(),
                selected: None,
                harness_empty: false,
            }),
        );
        let text = buffer_to_text(&draw_sized(&mut l, 30));
        let first = text.lines().next().unwrap();
        assert_eq!(first.chars().count(), 30);
        assert!(first.contains("…"), "{first}");
        assert!(first.ends_with("─╮"), "{first}");
    }

    // -- search results

    fn result(id: &str, project: &str, title: &str) -> SessionRow {
        SessionRow {
            project: Some(project.to_owned()),
            ..row(id, title, false, None)
        }
    }

    fn results_list(rows: Vec<SessionRow>, selected: Option<usize>) -> SessionList {
        let mut l = list(Vec::new(), None, true);
        l.attr(
            SESSION_ROWS_ATTR,
            payload_value(SessionRows {
                project: String::new(),
                search: Some("brio".to_owned()),
                harness: Some(HarnessId::Claude),
                rows: Arc::new(rows),
                selected,
                harness_empty: false,
            }),
        );
        l
    }

    fn three_results() -> Vec<SessionRow> {
        vec![
            result("a", "Agentmon", "Brio importer"),
            result("b", "BRIO", "Plan the sprint"),
            result("c", "Calira", "Bring up the pipeline"),
        ]
    }

    #[test]
    fn results_are_titled_with_the_query_and_prefixed_with_their_project() {
        let mut l = results_list(three_results(), Some(0));
        let text = buffer_to_text(&draw_sized(&mut l, 60));
        assert!(
            text.starts_with("╭─ Sessions matching \"brio\" ─"),
            "{text}"
        );
        assert!(
            text.contains("2026-10-30 11:40Z    Agentmon › Brio importer"),
            "{text}"
        );
        assert!(text.contains("BRIO › Plan the sprint"), "{text}");
        assert!(text.contains("Calira › Bring up the pipeline"), "{text}");
        assert!(!text.contains("Sessions in"), "{text}");
    }

    #[test]
    fn a_result_row_keeps_the_dot_slot_before_the_project() {
        let mut active = result("a", "Agentmon", "Brio importer");
        active.active = true;
        let parts = row_parts(&active, 60);
        assert_eq!(parts.left[2], (Part::Dot, "● ".to_owned()));
        assert_eq!(parts.left[3].1, "Agentmon › Brio importer");
        let idle = row_parts(&result("a", "Agentmon", "Brio importer"), 60);
        assert_eq!(idle.left[2], (Part::Text, "  ".to_owned()));
    }

    #[test]
    fn the_project_and_the_title_are_cut_together_with_an_ellipsis() {
        let r = result("a", "Agentmon", "A really long title for this row");
        let parts = row_parts(&r, 21 + 14);
        assert_eq!(parts.left[3].1, "Agentmon › A …");
        assert_eq!(parts.left[3].1.width(), 14);
    }

    #[test]
    fn a_long_project_name_is_cut_and_leaves_the_title_visible() {
        let r = result(
            "a",
            "a-very-long-project-name-for-the-brio-integration",
            "Plan the sprint",
        );
        let parts = row_parts(&r, 40);
        let label = &parts.left[3].1;
        assert!(label.contains("… › Plan"), "{label}");
        assert!(label.width() <= 40 - 21);
        // A short name is not touched.
        let r = result("a", "BRIO", "Plan the sprint");
        assert_eq!(row_parts(&r, 60).left[3].1, "BRIO › Plan the sprint");
    }

    #[test]
    fn a_long_query_is_cut_inside_the_quotes() {
        let mut l = results_list(three_results(), Some(0));
        l.attr(
            SESSION_ROWS_ATTR,
            payload_value(SessionRows {
                search: Some("the quick brown fox jumps over the lazy dog".to_owned()),
                rows: Arc::new(three_results()),
                ..SessionRows::default()
            }),
        );
        let text = buffer_to_text(&draw_sized(&mut l, 40));
        let first = text.lines().next().unwrap();
        assert!(
            first.starts_with("╭─ Sessions matching \"the quick"),
            "{first}"
        );
        assert!(first.contains("…\" ─"), "{first}");
        assert_eq!(first.chars().count(), 40);
    }

    #[test]
    fn the_selected_result_is_the_focused_band() {
        let mut l = results_list(three_results(), Some(0));
        insta::assert_snapshot!(buffer_to_styled(&draw_sized(&mut l, 60)));
    }

    #[test]
    fn a_long_query_is_cut_in_the_title() {
        let mut l = results_list(three_results(), Some(0));
        l.attr(
            SESSION_ROWS_ATTR,
            payload_value(SessionRows {
                search: Some("a query that is much too long for this pane".to_owned()),
                rows: Arc::new(three_results()),
                ..SessionRows::default()
            }),
        );
        let text = buffer_to_text(&draw_sized(&mut l, 30));
        let first = text.lines().next().unwrap();
        assert_eq!(first.chars().count(), 30);
        assert!(first.contains("…"), "{first}");
    }

    #[test]
    fn no_results_show_the_message_centred_in_muted_foreground() {
        let mut l = results_list(Vec::new(), None);
        let theme = builtin::default_theme();
        let buffer = draw_sized(&mut l, 60);
        let text = buffer_to_text(&buffer);
        assert!(text.contains("Sessions matching \"brio\""), "{text}");
        let (y, line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("No matching sessions"))
            .unwrap();
        let x = line[..line.find("No matching").unwrap()].chars().count();
        // 58 interior columns less the 3 + 3 of margin: centred in the middle.
        assert_eq!(x, (60 - "No matching sessions".len()) / 2);
        assert_eq!(
            buffer[(x as u16, y as u16)].fg,
            theme.role(Role::MutedForeground)
        );
        assert!(!text.contains("No sessions for this harness"));
    }

    #[test]
    fn an_empty_list_that_is_not_a_search_keeps_its_own_message() {
        let mut l = list(Vec::new(), None, true);
        let text = buffer_to_text(&draw_sized(&mut l, 60));
        assert!(!text.contains("No matching sessions"), "{text}");
    }

    #[test]
    fn the_search_attribute_round_trips() {
        let l = results_list(three_results(), Some(1));
        let rows: SessionRows =
            payload_from_value(&l.query(SESSION_ROWS_ATTR).unwrap().into_attr()).unwrap();
        assert_eq!(rows.search.as_deref(), Some("brio"));
        assert_eq!(rows.selected, Some(1));
        assert_eq!(rows.rows[0].project.as_deref(), Some("Agentmon"));
    }

    fn nav(l: &mut SessionList, direction: Direction) {
        l.attr(SEARCH_NAV_ATTR, payload_value(direction));
    }

    fn selected_row(l: &SessionList) -> Option<usize> {
        match l.state() {
            State::Single(StateValue::Usize(i)) => Some(i),
            _ => None,
        }
    }

    #[test]
    fn the_search_nav_attribute_moves_the_selection_without_wrapping() {
        let mut l = results_list(three_results(), Some(0));
        nav(&mut l, Direction::Up);
        assert_eq!(selected_row(&l), Some(0));
        nav(&mut l, Direction::Down);
        nav(&mut l, Direction::Down);
        assert_eq!(selected_row(&l), Some(2));
        nav(&mut l, Direction::Down);
        assert_eq!(selected_row(&l), Some(2));
        nav(&mut l, Direction::Up);
        assert_eq!(selected_row(&l), Some(1));
        // Other directions and other payloads change nothing.
        nav(&mut l, Direction::Left);
        l.attr(SEARCH_NAV_ATTR, AttrValue::Flag(true));
        assert_eq!(selected_row(&l), Some(1));
    }

    #[test]
    fn the_search_key_asks_the_app_to_search() {
        let mut l = list(sample(false), Some(0), true);
        assert_eq!(l.on(&ch('/')), Some(Msg::Selector(SelectorMsg::Search)));
        let mut p =
            crate::components::ProjectList::new(Keymap::default(), builtin::default_theme());
        assert_eq!(p.on(&ch('/')), Some(Msg::Selector(SelectorMsg::Search)));
    }

    #[test]
    fn esc_leaves_the_filter_only_while_the_rows_are_filtered() {
        let mut l = list(sample(false), Some(0), true);
        assert_eq!(l.on(&key(Key::Esc)), Some(Msg::Quit));
        l.attr(SESSION_FILTERED_ATTR, AttrValue::Flag(true));
        assert_eq!(
            l.on(&key(Key::Esc)),
            Some(Msg::Selector(SelectorMsg::LeaveFilter))
        );
        // With a session open behind the selector it is the filter that goes
        // first, too.
        l.attr(SESSION_OPEN_ATTR, AttrValue::Flag(true));
        assert_eq!(
            l.on(&key(Key::Esc)),
            Some(Msg::Selector(SelectorMsg::LeaveFilter))
        );
        l.attr(SESSION_FILTERED_ATTR, AttrValue::Flag(false));
        assert_eq!(l.on(&key(Key::Esc)), Some(Msg::CloseSelector));
    }

    // -- events

    #[test]
    fn r_asks_to_resume_the_selected_session() {
        let mut l = list(sample(false), Some(1), true);
        let want = Some(Msg::RequestResume(Some((
            HarnessId::Claude,
            SessionId::from("b"),
        ))));
        assert_eq!(l.on(&ch('R')), want);
        let shifted = Event::Keyboard(KeyEvent::new(Key::Char('R'), KeyModifiers::SHIFT));
        assert_eq!(l.on(&shifted), want);
        // Asking does not move the selection or open the session.
        assert_eq!(l.state(), State::Single(StateValue::Usize(1)));
    }

    #[test]
    fn r_with_no_session_selected_does_nothing() {
        let mut l = list(sample(false), None, true);
        assert_eq!(l.on(&ch('R')), None);
        let mut empty = list(Vec::new(), None, true);
        assert_eq!(empty.on(&ch('R')), None);
    }

    #[test]
    fn r_in_the_projects_list_leaves_the_selected_session_to_the_app() {
        let mut p =
            crate::components::ProjectList::new(Keymap::default(), builtin::default_theme());
        assert_eq!(p.on(&ch('R')), Some(Msg::Selector(SelectorMsg::Resume)));
    }

    #[test]
    fn down_and_up_move_the_selection_and_report_the_session() {
        let mut l = list(sample(false), Some(0), true);
        assert_eq!(l.on(&ch('j')), selected("b"));
        assert_eq!(l.on(&key(Key::Down)), selected("c"));
        assert_eq!(l.on(&ch('k')), selected("b"));
        assert_eq!(l.on(&key(Key::Up)), selected("a"));
    }

    #[test]
    fn there_is_no_wrap_around() {
        let mut l = list(sample(false), Some(0), true);
        assert_eq!(l.on(&key(Key::Up)), None);
        let mut l = list(sample(false), Some(2), true);
        assert_eq!(l.on(&key(Key::Down)), None);
        assert_eq!(l.state(), State::Single(StateValue::Usize(2)));
    }

    #[test]
    fn tab_shift_tab_left_and_h_focus_projects() {
        let mut l = list(sample(false), Some(0), true);
        for event in [key(Key::Tab), key(Key::BackTab), key(Key::Left), ch('h')] {
            assert_eq!(
                l.on(&event),
                Some(Msg::Selector(SelectorMsg::Focus(Pane::Projects))),
                "{event:?}"
            );
        }
    }

    #[test]
    fn right_and_l_do_nothing_in_sessions() {
        let mut l = list(sample(false), Some(0), true);
        assert_eq!(l.on(&key(Key::Right)), None);
        assert_eq!(l.on(&ch('l')), None);
    }

    #[test]
    fn harness_numbers_ask_for_a_switch() {
        let mut l = list(sample(false), Some(0), true);
        assert_eq!(
            l.on(&ch('1')),
            Some(Msg::Selector(SelectorMsg::SwitchHarness(HarnessId::Claude)))
        );
        assert_eq!(
            l.on(&ch('2')),
            Some(Msg::Selector(SelectorMsg::SwitchHarness(
                HarnessId::OpenCode
            )))
        );
    }

    #[test]
    fn other_keys_and_events_produce_nothing() {
        let mut l = list(sample(false), Some(0), true);
        for event in [ch('x'), Event::Tick, Event::FocusGained] {
            assert_eq!(l.on(&event), None, "{event:?}");
        }
    }

    #[test]
    fn enter_asks_to_open_the_selected_session_of_the_harness() {
        let mut l = list(sample(false), Some(1), true);
        assert_eq!(
            l.on(&key(Key::Enter)),
            Some(Msg::OpenSession(HarnessId::Claude, SessionId::from("b")))
        );
        // The session in the message is the one the cursor is on.
        l.on(&ch('j'));
        assert_eq!(
            l.on(&key(Key::Enter)),
            Some(Msg::OpenSession(HarnessId::Claude, SessionId::from("c")))
        );
    }

    #[test]
    fn enter_without_a_selected_session_or_a_harness_does_nothing() {
        let mut l = list(sample(false), None, true);
        assert_eq!(l.on(&key(Key::Enter)), None);
        let mut l = list(Vec::new(), None, true);
        assert_eq!(l.on(&key(Key::Enter)), None);
        l.attr(
            SESSION_ROWS_ATTR,
            payload_value(SessionRows {
                project: String::new(),
                search: None,
                harness: None,
                rows: Arc::new(sample(false)),
                selected: Some(0),
                harness_empty: false,
            }),
        );
        assert_eq!(l.on(&key(Key::Enter)), None);
    }

    #[test]
    fn esc_quits_without_an_open_session_and_closes_the_selector_with_one() {
        let mut l = list(sample(false), Some(0), true);
        assert_eq!(l.on(&key(Key::Esc)), Some(Msg::Quit));
        l.attr(SESSION_OPEN_ATTR, AttrValue::Flag(true));
        assert_eq!(l.on(&key(Key::Esc)), Some(Msg::CloseSelector));
        assert_eq!(
            l.query(SESSION_OPEN_ATTR).unwrap().into_attr(),
            AttrValue::Flag(true)
        );
        l.attr(SESSION_OPEN_ATTR, AttrValue::Flag(false));
        assert_eq!(l.on(&key(Key::Esc)), Some(Msg::Quit));
    }

    #[test]
    fn a_list_of_an_empty_harness_shows_the_empty_state_in_muted_foreground() {
        let theme = builtin::default_theme();
        let mut l = list(Vec::new(), None, false);
        l.attr(
            SESSION_ROWS_ATTR,
            payload_value(SessionRows {
                harness_empty: true,
                ..SessionRows::default()
            }),
        );
        let buffer = render(60, 10, |frame| l.view(frame, frame.area()));
        let text = buffer_to_text(&buffer);
        let line = text.lines().nth(5).unwrap();
        assert!(line.contains("No sessions for this harness"), "{text}");
        // Centred in the 58 columns inside the border.
        let left = line.chars().position(|c| c == 'N').unwrap();
        assert_eq!(left, 1 + (58 - 28) / 2);
        assert_eq!(
            buffer[(left as u16, 5)].fg,
            theme.role(Role::MutedForeground)
        );
        // The pane keeps its border and its title.
        assert!(text.starts_with("╭─ Sessions ─"), "{text}");
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn the_empty_message_keeps_the_rows_margin_and_is_cut_in_a_narrow_pane() {
        let mut l = list(Vec::new(), None, false);
        l.attr(
            SESSION_ROWS_ATTR,
            payload_value(SessionRows {
                harness_empty: true,
                ..SessionRows::default()
            }),
        );
        // Pane of 30: 28 inside the border, 24 after the 2-cell margins.
        let text = buffer_to_text(&render(30, 10, |frame| l.view(frame, frame.area())));
        let line = text.lines().nth(5).unwrap();
        assert_eq!(line, "│  No sessions for this ha…  │", "{text}");
    }

    #[test]
    fn a_project_without_sessions_in_a_harness_that_has_some_shows_no_message() {
        let text = buffer_to_text(&draw_sized(&mut list(Vec::new(), None, false), 60));
        assert!(text.contains("Sessions in BRIO"), "{text}");
        assert!(!text.contains("No sessions"), "{text}");
    }

    #[test]
    fn a_list_with_rows_does_not_show_the_empty_state() {
        let text = buffer_to_text(&draw_sized(&mut list(sample(false), None, false), 60));
        assert!(!text.contains("No sessions"), "{text}");
    }

    #[test]
    fn the_list_scrolls_to_keep_the_selection_visible() {
        let rows: Vec<SessionRow> = (0..30)
            .map(|n| row(&format!("s{n}"), &format!("Title {n:02}"), false, None))
            .collect();
        let mut l = list(rows, Some(0), true);
        l.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(7));
        for _ in 0..12 {
            l.on(&ch('j'));
        }
        let text = buffer_to_text(&draw_sized(&mut l, 60));
        assert!(text.contains("Title 12"), "{text}");
        assert!(text.contains("Title 06"), "{text}");
        assert!(!text.contains("Title 05"), "{text}");
    }

    // -- attributes

    #[test]
    fn stats_arrive_one_row_at_a_time() {
        let mut l = list(sample(false), None, false);
        l.attr(
            SESSION_STATS_ATTR,
            payload_value(SessionStatsUpdate {
                session: SessionId::from("b"),
                stats: stats(3, 90),
            }),
        );
        let text = buffer_to_text(&draw_sized(&mut l, 60));
        assert!(text.contains("3 turns · 1m"), "{text}");
        assert_eq!(text.matches("turns").count(), 1);
        // An unknown id changes nothing.
        l.attr(
            SESSION_STATS_ATTR,
            payload_value(SessionStatsUpdate {
                session: SessionId::from("zzz"),
                stats: stats(9, 9),
            }),
        );
        assert_eq!(
            buffer_to_text(&draw_sized(&mut l, 60))
                .matches("turns")
                .count(),
            1
        );
    }

    #[test]
    fn active_flags_replace_the_old_ones() {
        let mut l = list(sample(false), None, false);
        let active = |ids: &[&str]| {
            payload_value(ActiveSessions(
                ids.iter().map(|id| SessionId::from(*id)).collect(),
            ))
        };
        l.attr(ACTIVE_ATTR, active(&["c"]));
        let text = buffer_to_text(&draw_sized(&mut l, 60));
        assert_eq!(text.matches('●').count(), 1);
        assert!(
            text.lines().nth(4).unwrap().contains("● (untitled)"),
            "{text}"
        );
        l.attr(ACTIVE_ATTR, active(&[]));
        assert_eq!(
            buffer_to_text(&draw_sized(&mut l, 60)).matches('●').count(),
            0
        );
    }

    #[test]
    fn rows_and_selection_are_queryable_and_selection_pushes_are_silent() {
        let mut l = list(sample(true), Some(1), false);
        let got: SessionRows =
            payload_from_value(&l.query(SESSION_ROWS_ATTR).unwrap().into_attr()).unwrap();
        assert_eq!(got.project, "BRIO");
        assert_eq!(got.selected, Some(1));
        assert_eq!(got.rows.len(), 3);
        assert_eq!(got.rows[0].stats, Some(stats(23, 3900)));
        l.attr(SELECTED_ATTR, selected_value(None));
        assert_eq!(l.state(), State::None);
    }

    // -- the mouse

    use ratatui::layout::Size;
    use tuirealm::event::{MouseButton, MouseEventKind};

    /// A pane `rows` list rows high and 60 wide: its rows start at row 2.
    fn mouse(kind: MouseEventKind, column: u16, row: u16, rows: u16) -> Event<UserEvent> {
        Event::User(UserEvent::Mouse(RoutedMouse {
            kind,
            column,
            row,
            area: Size::new(60, rows + 3),
        }))
    }

    fn press(row: u16, rows: u16) -> Event<UserEvent> {
        mouse(MouseEventKind::Down(MouseButton::Left), 10, row, rows)
    }

    fn clicked(id: &str) -> Option<Msg> {
        Some(Msg::Selector(SelectorMsg::SessionClicked(SessionId::from(
            id,
        ))))
    }

    fn offset_of(list: &SessionList, rows: usize) -> usize {
        list.pane.cursor.offset_for(rows)
    }

    #[test]
    fn the_wheel_moves_a_selection_like_the_keys() {
        let mut l = list(sample(true), Some(0), true);
        assert_eq!(
            l.on(&mouse(MouseEventKind::ScrollDown, 5, 3, 5)),
            selected("b")
        );
        assert_eq!(
            l.on(&mouse(MouseEventKind::ScrollDown, 5, 3, 5)),
            selected("c")
        );
        assert_eq!(l.on(&mouse(MouseEventKind::ScrollDown, 5, 3, 5)), None);
        assert_eq!(
            l.on(&mouse(MouseEventKind::ScrollUp, 5, 3, 5)),
            selected("b")
        );
    }

    #[test]
    fn the_wheel_scrolls_the_rows_when_nothing_is_selected() {
        let mut l = list(sample(false), None, false);
        let scrolled = Some(Msg::Selector(SelectorMsg::ListScrolled));
        // Room for one row: three rows scroll twice.
        assert_eq!(l.on(&mouse(MouseEventKind::ScrollDown, 5, 3, 1)), scrolled);
        assert_eq!(offset_of(&l, 1), 1);
        assert_eq!(l.on(&mouse(MouseEventKind::ScrollDown, 5, 3, 1)), scrolled);
        assert_eq!(l.on(&mouse(MouseEventKind::ScrollDown, 5, 3, 1)), None);
        assert_eq!(offset_of(&l, 1), 2);
        assert_eq!(l.on(&mouse(MouseEventKind::ScrollUp, 5, 3, 1)), scrolled);
        assert_eq!(offset_of(&l, 1), 1);
        // Nothing got selected.
        assert_eq!(l.state(), State::None);
        // A list that fits does not scroll.
        let mut l = list(sample(false), None, false);
        assert_eq!(l.on(&mouse(MouseEventKind::ScrollDown, 5, 3, 5)), None);
    }

    #[test]
    fn a_press_on_a_row_selects_it_and_reports_the_session() {
        let mut l = list(sample(true), Some(0), false);
        assert_eq!(l.on(&press(3, 5)), clicked("b"));
        assert_eq!(l.state(), State::Single(StateValue::Usize(1)));
        assert_eq!(l.on(&press(4, 5)), clicked("c"));
        assert_eq!(l.state(), State::Single(StateValue::Usize(2)));
        // From no selection too.
        let mut l = list(sample(true), None, false);
        assert_eq!(l.on(&press(4, 5)), clicked("c"));
        assert_eq!(l.state(), State::Single(StateValue::Usize(2)));
    }

    #[test]
    fn a_press_where_there_is_no_row_asks_for_the_focus_and_selects_nothing() {
        let mut l = list(sample(true), Some(1), false);
        let pane = Some(Msg::Selector(SelectorMsg::PaneClicked(Pane::Sessions)));
        for row in [0, 1, 5, 6, 7] {
            assert_eq!(l.on(&press(row, 5)), pane, "{row}");
        }
        assert_eq!(l.state(), State::Single(StateValue::Usize(1)));
    }

    #[test]
    fn an_empty_list_asks_for_the_focus() {
        let mut l = list(Vec::new(), None, false);
        assert_eq!(
            l.on(&press(2, 5)),
            Some(Msg::Selector(SelectorMsg::PaneClicked(Pane::Sessions)))
        );
    }

    #[test]
    fn other_mouse_events_and_the_raw_event_do_nothing() {
        let mut l = list(sample(true), Some(0), true);
        for kind in [
            MouseEventKind::Down(MouseButton::Right),
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::ScrollRight,
        ] {
            assert_eq!(l.on(&mouse(kind, 5, 3, 5)), None, "{kind:?}");
        }
        let raw = Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            modifiers: KeyModifiers::NONE,
            column: 10,
            row: 3,
        });
        assert_eq!(l.on(&raw), None);
        assert_eq!(l.state(), State::Single(StateValue::Usize(0)));
    }
}
