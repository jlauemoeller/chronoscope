//! The Sessions pane of the session selector (SPEC §4.1, §7.3, §8.2).

use std::sync::Arc;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;
use unicode_width::UnicodeWidthStr;

use super::empty_state::{EmptyState, NO_SESSIONS};
use super::list_pane::{
    PaneKey, PaneState, Part, RowParts, border_role, pane_key, render_rows, title_width,
    titled_block,
};
use super::{THEME_ATTR, payload_from_value, payload_value, truncate_with_ellipsis};
use crate::app::{Msg, Pane, SelectorMsg, UserEvent};
use crate::keymap::Keymap;
use crate::model::{HarnessId, SessionId, SessionStats};
use crate::theme::Theme;

/// The attribute that sets the rows ([`SessionRows`]).
pub const SESSION_ROWS_ATTR: Attribute = Attribute::Custom("session_rows");
/// The attribute that sets the stats of one row ([`SessionStatsUpdate`]).
pub const SESSION_STATS_ATTR: Attribute = Attribute::Custom("session_stats");
/// The attribute that sets which rows show the `●` marker
/// ([`ActiveSessions`]).
pub const ACTIVE_ATTR: Attribute = Attribute::Custom("active_sessions");

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
    /// The harness the sessions belong to, sent in [`Msg::OpenSession`].
    pub harness: Option<HarnessId>,
    pub rows: Arc<Vec<SessionRow>>,
    pub selected: Option<usize>,
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

/// How long a session ran, as shown in its row: `45s` under a minute, `12m`
/// under an hour, and `1h 05m` from then on.
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m", secs / 60),
        _ => format!("{}h {:02}m", secs / 3600, secs % 3600 / 60),
    }
}

/// The stats of a row: `<n> turns · <duration>`, or `1 turn · <duration>`.
pub fn format_stats(stats: &SessionStats) -> String {
    let unit = if stats.turns == 1 { "turn" } else { "turns" };
    format!(
        "{} {unit} · {}",
        stats.turns,
        format_duration(stats.duration)
    )
}

/// The Sessions pane: `<start>  <title>` rows with muted stats on the right,
/// and `●` in the rows of active sessions. It takes focus.
///
/// Rows come in through [`SESSION_ROWS_ATTR`]; stats and the active flags
/// are updated one by one through [`SESSION_STATS_ATTR`] and
/// [`ACTIVE_ATTR`]. Moving the selection reports
/// [`SelectorMsg::SessionSelected`]; `Enter` on the selected session asks to
/// open it ([`Msg::OpenSession`]). With no rows, the pane shows the
/// [`EmptyState`] "No sessions for this harness".
#[derive(Debug, Clone)]
pub struct SessionList {
    pane: PaneState,
    keymap: Keymap,
    project: String,
    harness: Option<HarnessId>,
    rows: Arc<Vec<SessionRow>>,
    empty: EmptyState,
}

impl SessionList {
    /// An empty list that resolves keys with `keymap`.
    pub fn new(keymap: Keymap, theme: Theme) -> Self {
        Self {
            empty: EmptyState::new(NO_SESSIONS, theme.clone()).without_background(),
            pane: PaneState::new(theme),
            keymap,
            project: String::new(),
            harness: None,
            rows: Arc::default(),
        }
    }

    /// The request to open the selected session, if there is one.
    fn open(&self) -> Option<Msg> {
        let harness = self.harness?;
        self.selected_id()
            .map(|session| Msg::OpenSession(harness, session))
    }

    fn selected_id(&self) -> Option<SessionId> {
        self.pane
            .cursor
            .selected()
            .and_then(|i| self.rows.get(i))
            .map(|row| row.id.clone())
    }

    fn title(&self, pane_width: u16) -> String {
        let title = if self.project.is_empty() {
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

/// The pieces of `row` for a text width of `width` columns: the start, the
/// title cut with `…`, and the stats on the right unless they would leave
/// fewer than [`MIN_TITLE_WIDTH`] cells for the title.
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
            (Part::Text, truncate_with_ellipsis(&row.title, room)),
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
        if self.rows.is_empty() {
            self.empty.view(frame, area.inner(Margin::new(1, 1)));
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
                harness: self.harness,
                rows: Arc::clone(&self.rows),
                selected: self.pane.cursor.selected(),
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
                    harness,
                    rows,
                    selected,
                }) = payload_from_value(&value)
                {
                    self.pane.cursor.set(rows.len(), selected);
                    self.project = project;
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
            _ => {
                if attr == THEME_ATTR {
                    self.empty.attr(attr, value.clone());
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
        match pane_key(&self.keymap, ev)? {
            PaneKey::Up => self.moved(Direction::Up),
            PaneKey::Down => self.moved(Direction::Down),
            PaneKey::FocusProjects | PaneKey::NextPane | PaneKey::PrevPane => {
                Some(Msg::Selector(SelectorMsg::Focus(Pane::Projects)))
            }
            PaneKey::FocusSessions => None,
            PaneKey::Open => self.open(),
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
    use tuirealm::event::{Key, KeyEvent, KeyModifiers};
    use tuirealm::state::StateValue;

    fn stats(turns: usize, secs: u64) -> SessionStats {
        SessionStats {
            turns,
            duration: Duration::from_secs(secs),
        }
    }

    fn row(id: &str, title: &str, active: bool, with_stats: Option<SessionStats>) -> SessionRow {
        SessionRow {
            id: SessionId::from(id),
            started: "2026-10-30 11:40Z".to_owned(),
            title: title.to_owned(),
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
            project: "BRIO".to_owned(),
            harness: Some(HarnessId::Claude),
            rows: Arc::new(rows),
            selected,
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
    fn durations_use_seconds_minutes_and_hours() {
        let d = |secs| format_duration(Duration::from_secs(secs));
        assert_eq!(d(0), "0s");
        assert_eq!(d(45), "45s");
        assert_eq!(d(59), "59s");
        assert_eq!(d(60), "1m");
        assert_eq!(d(12 * 60), "12m");
        assert_eq!(d(12 * 60 + 59), "12m");
        assert_eq!(d(59 * 60 + 59), "59m");
        assert_eq!(d(3600), "1h 00m");
        assert_eq!(d(3600 + 5 * 60), "1h 05m");
        assert_eq!(d(26 * 3600 + 30 * 60 + 12), "26h 30m");
    }

    #[test]
    fn stats_read_turns_and_duration() {
        assert_eq!(format_stats(&stats(12, 720)), "12 turns · 12m");
        assert_eq!(format_stats(&stats(1, 45)), "1 turn · 45s");
        assert_eq!(format_stats(&stats(0, 0)), "0 turns · 0s");
        assert_eq!(format_stats(&stats(23, 3900)), "23 turns · 1h 05m");
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
                project: "A project with a really long display name".to_owned(),
                harness: None,
                rows: Arc::default(),
                selected: None,
            }),
        );
        let text = buffer_to_text(&draw_sized(&mut l, 30));
        let first = text.lines().next().unwrap();
        assert_eq!(first.chars().count(), 30);
        assert!(first.contains("…"), "{first}");
        assert!(first.ends_with("─╮"), "{first}");
    }

    // -- events

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
                harness: None,
                rows: Arc::new(sample(false)),
                selected: Some(0),
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
    fn a_list_without_rows_shows_the_empty_state_in_muted_foreground() {
        let theme = builtin::default_theme();
        let mut l = list(Vec::new(), None, false);
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
        assert!(text.starts_with("╭─ Sessions in BRIO ─"), "{text}");
        insta::assert_snapshot!(buffer_to_styled(&buffer));
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
}
