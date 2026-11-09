//! The Projects pane of the session selector (product §3.1, §7.2).

use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::double_click::{CLICK_CLOCK_ATTR, ClickClock, LastPress};
use super::list_pane::{PaneKey, PaneState, Part, RowParts, pane_key, render_rows};
use super::text_fit::truncate_with_ellipsis;
use super::titled_block::{border_role, titled_block};
use super::{payload_from_value, payload_value};
use crate::app::{Msg, Pane, RoutedMouse, SelectorMsg, UserEvent};
use crate::keymap::Keymap;
use crate::model::ProjectId;
use crate::theme::Theme;

/// The attribute that sets the rows ([`ProjectRows`]).
pub const PROJECT_ROWS_ATTR: Attribute = Attribute::Custom("project_rows");

/// The attribute that says whether the selector is filtered
/// (`AttrValue::Flag`): the Projects pane is then as at a start without the
/// prefix, and the wheel leaves it alone.
pub const PROJECTS_FILTERED_ATTR: Attribute = Attribute::Custom("projects_filtered");

/// The title of the pane.
const TITLE: &str = "Projects";

/// One project row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    /// The project's id, sent in [`SelectorMsg::ProjectSelected`].
    pub id: ProjectId,
    /// The display name.
    pub name: String,
}

/// The rows of the list and which one is selected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectRows {
    pub rows: Vec<ProjectRow>,
    pub selected: Option<usize>,
}

/// The Projects pane: one project per row, the selection on the focused
/// pane drawn as an inverted band. It takes focus.
///
/// Rows come in through [`PROJECT_ROWS_ATTR`]. Moving the selection reports
/// [`SelectorMsg::ProjectSelected`]; the app then shows that project's
/// sessions. While the selector is filtered ([`PROJECTS_FILTERED_ATTR`]) the
/// wheel does nothing. A left press on a row reports
/// [`SelectorMsg::ChooseProject`], with `open` when it completed a
/// double-click; the clock that is measured with is set with
/// [`with_click_clock`](Self::with_click_clock) or [`CLICK_CLOCK_ATTR`].
#[derive(Debug, Clone)]
pub struct ProjectList {
    pane: PaneState,
    keymap: Keymap,
    rows: Vec<ProjectRow>,
    filtered: bool,
    /// The last left press on a row, for double-clicks.
    presses: LastPress<ProjectId>,
}

impl ProjectList {
    /// An empty list that resolves keys with `keymap`.
    pub fn new(keymap: Keymap, theme: Theme) -> Self {
        Self {
            pane: PaneState::new(theme),
            keymap,
            rows: Vec::new(),
            filtered: false,
            presses: LastPress::default(),
        }
    }

    /// Replaces the clock double-clicks are measured with.
    #[must_use]
    pub fn with_click_clock(mut self, clock: ClickClock) -> Self {
        self.presses.set_clock(clock);
        self
    }

    fn selected_id(&self) -> Option<ProjectId> {
        self.pane
            .cursor
            .selected()
            .and_then(|i| self.rows.get(i))
            .map(|row| row.id.clone())
    }

    fn row_parts(&self, index: usize, width: usize) -> RowParts {
        let name = self.rows.get(index).map_or("", |row| row.name.as_str());
        RowParts {
            left: vec![(Part::Text, truncate_with_ellipsis(name, width))],
            stats: None,
        }
    }

    fn moved(&mut self, direction: Direction) -> Option<Msg> {
        match self.perform(Cmd::Move(direction)) {
            CmdResult::Changed(_) => self
                .selected_id()
                .map(|id| Msg::Selector(SelectorMsg::ProjectSelected(id))),
            _ => None,
        }
    }

    /// The wheel moves the selection like the keys, except while the
    /// selector is filtered; a left press on a row chooses the project (and
    /// says whether it completed a double-click), elsewhere in the pane it
    /// ends a double-click under way and asks for the focus (either ends the
    /// filter).
    fn on_mouse(&mut self, mouse: &RoutedMouse) -> Option<Msg> {
        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown if self.filtered => None,
            MouseEventKind::ScrollUp => self.moved(Direction::Up),
            MouseEventKind::ScrollDown => self.moved(Direction::Down),
            _ if mouse.is_left_press() => {
                let row = self.pane.row_at(mouse.column, mouse.row, mouse.area);
                let Some(id) = row.and_then(|i| self.rows.get(i)).map(|row| row.id.clone()) else {
                    self.presses.forget();
                    return Some(Msg::Selector(SelectorMsg::PaneClicked(Pane::Projects)));
                };
                let open = self.presses.press(id.clone());
                Some(Msg::Selector(SelectorMsg::ChooseProject { id, open }))
            }
            _ => None,
        }
    }
}

impl Component for ProjectList {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        let block = titled_block(&self.pane.theme, TITLE, border_role(self.pane.focused));
        frame.render_widget(block, area);
        render_rows(frame, area, &self.pane, |index, width| {
            self.row_parts(index, width)
        });
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        if attr == PROJECT_ROWS_ATTR {
            let rows = ProjectRows {
                rows: self.rows.clone(),
                selected: self.pane.cursor.selected(),
            };
            return Some(QueryResult::Owned(payload_value(rows)));
        }
        if attr == PROJECTS_FILTERED_ATTR {
            return Some(QueryResult::Owned(AttrValue::Flag(self.filtered)));
        }
        self.pane.query(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == PROJECT_ROWS_ATTR {
            if let Some(ProjectRows { rows, selected }) = payload_from_value(&value) {
                self.pane.cursor.set(rows.len(), selected);
                self.rows = rows;
            }
        } else if attr == PROJECTS_FILTERED_ATTR {
            if let AttrValue::Flag(filtered) = value {
                self.filtered = filtered;
            }
        } else if attr == CLICK_CLOCK_ATTR {
            if let Some(clock) = ClickClock::from_value(&value) {
                self.presses.set_clock(clock);
            }
        } else {
            self.pane.attr(attr, &value);
        }
    }

    fn state(&self) -> State {
        self.pane.state()
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        self.pane.perform(cmd)
    }
}

impl AppComponent<Msg, UserEvent> for ProjectList {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::User(UserEvent::Pointer(mouse)) => return self.on_mouse(mouse),
            Event::User(UserEvent::PointerElsewhere) => {
                self.presses.forget();
                return None;
            }
            _ => {}
        }
        match pane_key(&self.keymap, ev)? {
            PaneKey::Up => self.moved(Direction::Up),
            PaneKey::Down => self.moved(Direction::Down),
            PaneKey::FocusSessions | PaneKey::NextPane | PaneKey::PrevPane | PaneKey::Open => {
                Some(Msg::Selector(SelectorMsg::Focus(Pane::Sessions)))
            }
            PaneKey::FocusProjects => None,
            PaneKey::Search => Some(Msg::Selector(SelectorMsg::Search)),
            // The app resumes the session that is selected, if there is one.
            PaneKey::Resume => Some(Msg::Selector(SelectorMsg::Resume)),
            PaneKey::Cancel => Some(self.pane.cancel()),
            PaneKey::Harness(harness) => Some(Msg::Selector(SelectorMsg::SwitchHarness(harness))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::TestClock;
    use crate::components::list_cursor::{SELECTED_ATTR, VISIBLE_ROWS_ATTR, selected_value};
    use crate::components::list_pane::SESSION_OPEN_ATTR;
    use crate::test_support::{buffer_to_styled, render};
    use crate::theme::builtin;
    use tuirealm::event::{Key, KeyEvent, KeyModifiers};

    fn rows(names: &[&str], selected: Option<usize>) -> AttrValue {
        let rows = names
            .iter()
            .map(|n| ProjectRow {
                id: ProjectId::from(format!("/p/{n}")),
                name: (*n).to_owned(),
            })
            .collect();
        payload_value(ProjectRows { rows, selected })
    }

    fn list(names: &[&str], selected: Option<usize>, focused: bool) -> ProjectList {
        let mut list = ProjectList::new(Keymap::default(), builtin::default_theme());
        list.attr(PROJECT_ROWS_ATTR, rows(names, selected));
        list.attr(Attribute::Focus, AttrValue::Flag(focused));
        list
    }

    const NAMES: [&str; 4] = [
        "Agentmon",
        "BRIO",
        "Calira",
        "A very long project name indeed",
    ];

    fn key(code: Key) -> Event<UserEvent> {
        Event::Keyboard(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ch(c: char) -> Event<UserEvent> {
        key(Key::Char(c))
    }

    fn selected(id: &str) -> Option<Msg> {
        Some(Msg::Selector(SelectorMsg::ProjectSelected(
            ProjectId::from(format!("/p/{id}")),
        )))
    }

    fn focus(pane: Pane) -> Option<Msg> {
        Some(Msg::Selector(SelectorMsg::Focus(pane)))
    }

    fn draw(list: &mut ProjectList) -> String {
        buffer_to_styled(&render(36, 10, |frame| list.view(frame, frame.area())))
    }

    #[test]
    fn focused_list_snapshot() {
        insta::assert_snapshot!(draw(&mut list(&NAMES, Some(1), true)));
    }

    #[test]
    fn unfocused_list_snapshot() {
        insta::assert_snapshot!(draw(&mut list(&NAMES, Some(1), false)));
    }

    #[test]
    fn down_and_j_move_the_selection_and_report_the_project() {
        let mut l = list(&NAMES, Some(0), true);
        assert_eq!(l.on(&ch('j')), selected("BRIO"));
        assert_eq!(l.on(&key(Key::Down)), selected("Calira"));
        assert_eq!(
            l.state(),
            State::Single(tuirealm::state::StateValue::Usize(2))
        );
    }

    #[test]
    fn up_and_k_move_the_selection_and_report_the_project() {
        let mut l = list(&NAMES, Some(3), true);
        assert_eq!(l.on(&ch('k')), selected("Calira"));
        assert_eq!(l.on(&key(Key::Up)), selected("BRIO"));
    }

    #[test]
    fn there_is_no_wrap_around() {
        let mut l = list(&NAMES, Some(0), true);
        assert_eq!(l.on(&ch('k')), None);
        assert_eq!(l.on(&key(Key::Up)), None);
        let mut l = list(&NAMES, Some(3), true);
        assert_eq!(l.on(&ch('j')), None);
        assert_eq!(l.on(&key(Key::Down)), None);
        assert_eq!(
            l.state(),
            State::Single(tuirealm::state::StateValue::Usize(3))
        );
    }

    #[test]
    fn an_empty_list_reports_nothing() {
        let mut l = list(&[], None, true);
        assert_eq!(l.on(&ch('j')), None);
        assert_eq!(l.on(&ch('k')), None);
        assert_eq!(l.state(), State::None);
    }

    #[test]
    fn moving_is_a_command_with_a_result() {
        let mut l = list(&NAMES, Some(0), true);
        assert_eq!(
            l.perform(Cmd::Move(Direction::Down)),
            CmdResult::Changed(State::Single(tuirealm::state::StateValue::Usize(1)))
        );
        assert_eq!(
            l.perform(Cmd::Move(Direction::Up)),
            CmdResult::Changed(State::Single(tuirealm::state::StateValue::Usize(0)))
        );
        assert_eq!(l.perform(Cmd::Move(Direction::Up)), CmdResult::NoChange);
        assert_eq!(l.perform(Cmd::Tick), CmdResult::Invalid(Cmd::Tick));
    }

    #[test]
    fn tab_shift_tab_right_l_and_enter_focus_sessions() {
        let mut l = list(&NAMES, Some(0), true);
        for event in [
            key(Key::Tab),
            key(Key::BackTab),
            Event::Keyboard(KeyEvent::new(Key::Tab, KeyModifiers::SHIFT)),
            key(Key::Right),
            ch('l'),
            key(Key::Enter),
        ] {
            assert_eq!(l.on(&event), focus(Pane::Sessions), "{event:?}");
        }
    }

    #[test]
    fn left_and_h_do_nothing_in_projects() {
        let mut l = list(&NAMES, Some(0), true);
        assert_eq!(l.on(&key(Key::Left)), None);
        assert_eq!(l.on(&ch('h')), None);
    }

    #[test]
    fn esc_quits_without_an_open_session_and_closes_the_selector_with_one() {
        let mut l = list(&NAMES, Some(0), true);
        assert_eq!(l.on(&key(Key::Esc)), Some(Msg::Quit));
        l.attr(SESSION_OPEN_ATTR, AttrValue::Flag(true));
        assert_eq!(l.on(&key(Key::Esc)), Some(Msg::CloseSelector));
        l.attr(SESSION_OPEN_ATTR, AttrValue::Flag(false));
        assert_eq!(l.on(&key(Key::Esc)), Some(Msg::Quit));
    }

    #[test]
    fn enter_focuses_sessions_and_does_not_open_anything() {
        let mut l = list(&NAMES, Some(0), true);
        assert_eq!(l.on(&key(Key::Enter)), focus(Pane::Sessions));
    }

    #[test]
    fn an_empty_list_draws_only_its_border_and_title() {
        let text = crate::test_support::buffer_to_text(&render(36, 10, |frame| {
            list(&[], None, true).view(frame, frame.area())
        }));
        insta::assert_snapshot!(text);
    }

    #[test]
    fn harness_numbers_ask_for_a_switch() {
        let mut l = list(&NAMES, Some(0), true);
        for (c, harness) in [
            ('1', crate::model::HarnessId::Claude),
            ('2', crate::model::HarnessId::OpenCode),
            ('3', crate::model::HarnessId::Codex),
            ('4', crate::model::HarnessId::Pi),
        ] {
            assert_eq!(
                l.on(&ch(c)),
                Some(Msg::Selector(SelectorMsg::SwitchHarness(harness)))
            );
        }
    }

    #[test]
    fn other_keys_and_events_produce_nothing() {
        let mut l = list(&NAMES, Some(0), true);
        for event in [ch('x'), ch('q'), Event::Tick, Event::WindowResize(1, 1)] {
            assert_eq!(l.on(&event), None, "{event:?}");
        }
    }

    #[test]
    fn keys_follow_the_keymap() {
        let mut raw = crate::config::RawKeys::new();
        raw.entry("selector".to_owned())
            .or_default()
            .insert("down".to_owned(), vec!["n".to_owned()]);
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut l = ProjectList::new(keymap, builtin::default_theme());
        l.attr(PROJECT_ROWS_ATTR, rows(&NAMES, Some(0)));
        assert_eq!(l.on(&ch('n')), selected("BRIO"));
        assert_eq!(l.on(&ch('j')), None);
    }

    #[test]
    fn the_list_scrolls_to_keep_the_selection_visible() {
        let names: Vec<String> = (0..30).map(|n| format!("project-{n:02}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut l = list(&names, Some(0), true);
        l.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(7));
        for _ in 0..10 {
            l.on(&ch('j'));
        }
        let text = crate::test_support::buffer_to_text(&render(36, 10, |frame| {
            l.view(frame, frame.area())
        }));
        assert!(text.contains("project-10"), "{text}");
        assert!(text.contains("project-04"), "{text}");
        assert!(!text.contains("project-03"), "{text}");
    }

    #[test]
    fn rows_and_selection_pushed_through_attributes_are_queryable() {
        let mut l = list(&NAMES, Some(2), false);
        let queried = l.query(PROJECT_ROWS_ATTR).unwrap().into_attr();
        let got: ProjectRows = payload_from_value(&queried).unwrap();
        assert_eq!(got.selected, Some(2));
        assert_eq!(got.rows.len(), 4);
        l.attr(SELECTED_ATTR, selected_value(None));
        assert_eq!(l.state(), State::None);
        l.attr(SELECTED_ATTR, selected_value(Some(1)));
        assert_eq!(
            l.query(SELECTED_ATTR).unwrap().into_attr(),
            selected_value(Some(1))
        );
        assert_eq!(
            l.query(Attribute::Focus).unwrap().into_attr(),
            AttrValue::Flag(false)
        );
        l.attr(Attribute::Focus, AttrValue::Flag(true));
        assert_eq!(
            l.query(Attribute::Focus).unwrap().into_attr(),
            AttrValue::Flag(true)
        );
    }

    #[test]
    fn pushing_a_selection_sends_no_message() {
        let mut l = list(&NAMES, Some(0), true);
        l.attr(SELECTED_ATTR, selected_value(Some(2)));
        assert_eq!(
            l.state(),
            State::Single(tuirealm::state::StateValue::Usize(2))
        );
    }

    #[test]
    fn a_long_name_is_cut_with_an_ellipsis_before_the_margin() {
        let text = crate::test_support::buffer_to_text(&render(36, 10, |frame| {
            list(&NAMES, None, false).view(frame, frame.area())
        }));
        // Width 36: text from column 5 to column 32, 28 cells.
        assert!(
            text.contains("│    A very long project name in…  │"),
            "{text}"
        );
    }

    #[test]
    fn the_theme_can_be_replaced_through_an_attribute() {
        let mut l = list(&NAMES, None, false);
        let other = builtin::terminal_theme();
        l.attr(
            crate::components::THEME_ATTR,
            crate::components::theme_value(&other),
        );
        let queried = l.query(crate::components::THEME_ATTR).unwrap().into_attr();
        assert_eq!(crate::components::theme_from_value(&queried), Some(other));
    }

    #[test]
    fn a_tiny_area_does_not_panic() {
        for (w, h) in [(0, 0), (1, 1), (5, 2), (9, 3), (12, 4)] {
            let _ = render(20, 6, |frame| {
                list(&NAMES, Some(0), true).view(frame, Rect::new(0, 0, w, h));
            });
        }
    }

    // -- the mouse

    use ratatui::layout::Size;
    use tuirealm::event::{MouseButton, MouseEventKind};

    /// A pane of 36 x 10 shows 7 rows, from its row 2.
    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event<UserEvent> {
        Event::User(UserEvent::Pointer(RoutedMouse {
            kind,
            column,
            row,
            area: Size::new(36, 10),
        }))
    }

    fn press(column: u16, row: u16) -> Event<UserEvent> {
        mouse(MouseEventKind::Down(MouseButton::Left), column, row)
    }

    fn chosen(id: &str, open: bool) -> Option<Msg> {
        Some(Msg::Selector(SelectorMsg::ChooseProject {
            id: ProjectId::from(format!("/p/{id}")),
            open,
        }))
    }

    fn clicked(id: &str) -> Option<Msg> {
        chosen(id, false)
    }

    #[test]
    fn the_wheel_moves_the_selection_like_the_keys() {
        let mut l = list(&NAMES, Some(1), false);
        assert_eq!(
            l.on(&mouse(MouseEventKind::ScrollDown, 5, 4)),
            selected("Calira")
        );
        assert_eq!(
            l.on(&mouse(MouseEventKind::ScrollUp, 5, 4)),
            selected("BRIO")
        );
        assert_eq!(
            l.on(&mouse(MouseEventKind::ScrollUp, 5, 4)),
            selected("Agentmon")
        );
        assert_eq!(l.on(&mouse(MouseEventKind::ScrollUp, 5, 4)), None);
        assert_eq!(
            l.state(),
            State::Single(tuirealm::state::StateValue::Usize(0))
        );
        // Wherever in the pane the pointer is.
        assert_eq!(
            l.on(&mouse(MouseEventKind::ScrollDown, 0, 0)),
            selected("BRIO")
        );
    }

    #[test]
    fn while_the_selector_is_filtered_the_wheel_does_nothing_and_a_press_still_reports() {
        let mut l = list(&NAMES, Some(1), false);
        l.attr(PROJECTS_FILTERED_ATTR, AttrValue::Flag(true));
        assert_eq!(
            l.query(PROJECTS_FILTERED_ATTR).unwrap().into_attr(),
            AttrValue::Flag(true)
        );
        for kind in [MouseEventKind::ScrollDown, MouseEventKind::ScrollUp] {
            assert_eq!(l.on(&mouse(kind, 5, 4)), None, "{kind:?}");
        }
        assert_eq!(
            l.state(),
            State::Single(tuirealm::state::StateValue::Usize(1)),
            "the selection stays"
        );
        // A press ends the filter, as the app decides.
        assert_eq!(l.on(&press(5, 3)), clicked("BRIO"));
        assert_eq!(
            l.on(&press(5, 9)),
            Some(Msg::Selector(SelectorMsg::PaneClicked(Pane::Projects)))
        );
        // Once the filter ends, the wheel moves the selection again.
        l.attr(PROJECTS_FILTERED_ATTR, AttrValue::Flag(false));
        assert_eq!(
            l.on(&mouse(MouseEventKind::ScrollDown, 5, 4)),
            selected("Calira")
        );
    }

    #[test]
    fn a_press_on_a_row_reports_the_project() {
        let mut l = list(&NAMES, Some(0), true);
        assert_eq!(l.on(&press(5, 2)), clicked("Agentmon"));
        assert_eq!(l.on(&press(5, 3)), clicked("BRIO"));
        assert_eq!(
            l.on(&press(30, 5)),
            clicked("A very long project name indeed")
        );
    }

    #[test]
    fn a_press_where_there_is_no_row_asks_for_the_focus() {
        let mut l = list(&NAMES, Some(0), false);
        let pane = Some(Msg::Selector(SelectorMsg::PaneClicked(Pane::Projects)));
        // Under the last row, the blank row, the border, the margin.
        for (column, row) in [(5, 6), (5, 9), (5, 1), (5, 0), (0, 3), (35, 3)] {
            assert_eq!(l.on(&press(column, row)), pane, "{column}, {row}");
        }
    }

    #[test]
    fn other_mouse_events_do_nothing() {
        let mut l = list(&NAMES, Some(0), true);
        for kind in [
            MouseEventKind::Down(MouseButton::Right),
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::ScrollLeft,
            MouseEventKind::Moved,
        ] {
            assert_eq!(l.on(&mouse(kind, 5, 3)), None, "{kind:?}");
        }
        // The raw event, in screen coordinates, is not theirs to read.
        let raw = Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::ScrollDown,
            modifiers: KeyModifiers::NONE,
            column: 5,
            row: 3,
        });
        assert_eq!(l.on(&raw), None);
        assert_eq!(
            l.state(),
            State::Single(tuirealm::state::StateValue::Usize(0))
        );
    }

    #[test]
    fn a_press_on_the_second_page_hits_the_row_drawn_there() {
        let mut l = list(&NAMES, Some(0), true);
        l.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(2));
        l.on(&ch('j'));
        l.on(&ch('j'));
        // A pane of 36 x 5 shows two rows: rows 1 and 2 are drawn.
        let small = |row| {
            Event::User(UserEvent::Pointer(RoutedMouse {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row,
                area: Size::new(36, 5),
            }))
        };
        assert_eq!(l.on(&small(2)), clicked("BRIO"));
        assert_eq!(l.on(&small(3)), clicked("Calira"));
        assert_eq!(
            l.on(&small(4)),
            Some(Msg::Selector(SelectorMsg::PaneClicked(Pane::Projects)))
        );
    }
    // -- double-clicks

    /// A focused list whose double-clicks are measured with the returned
    /// clock.
    fn timed() -> (ProjectList, TestClock) {
        let clock = TestClock::new();
        let l = list(&NAMES, Some(0), true).with_click_clock(clock.click_clock());
        (l, clock)
    }

    fn at_ms(clock: &TestClock, ms: u64) {
        clock.set(std::time::Duration::from_millis(ms));
    }

    const ELSEWHERE: Event<UserEvent> = Event::User(UserEvent::PointerElsewhere);

    #[test]
    fn two_presses_on_a_row_at_most_400_ms_apart_open_the_project() {
        for (ms, open) in [(0, true), (400, true), (401, false)] {
            let (mut l, clock) = timed();
            assert_eq!(l.on(&press(5, 3)), chosen("BRIO", false));
            at_ms(&clock, ms);
            assert_eq!(l.on(&press(20, 3)), chosen("BRIO", open), "{ms} ms");
        }
    }

    #[test]
    fn a_third_press_starts_a_new_double_click() {
        let (mut l, clock) = timed();
        l.on(&press(5, 3));
        at_ms(&clock, 100);
        assert_eq!(l.on(&press(5, 3)), chosen("BRIO", true));
        at_ms(&clock, 200);
        assert_eq!(l.on(&press(5, 3)), chosen("BRIO", false));
        at_ms(&clock, 300);
        assert_eq!(l.on(&press(5, 3)), chosen("BRIO", true));
    }

    #[test]
    fn a_press_on_another_row_ends_a_double_click() {
        let (mut l, clock) = timed();
        l.on(&press(5, 3));
        at_ms(&clock, 100);
        assert_eq!(l.on(&press(5, 2)), chosen("Agentmon", false));
        at_ms(&clock, 200);
        assert_eq!(l.on(&press(5, 3)), chosen("BRIO", false));
    }

    #[test]
    fn a_press_not_on_a_row_ends_a_double_click() {
        // Under the last row, the blank row, the border, the margin.
        for (column, row) in [(5, 6), (5, 1), (5, 0), (0, 3)] {
            let (mut l, clock) = timed();
            l.on(&press(5, 3));
            at_ms(&clock, 100);
            assert_eq!(
                l.on(&press(column, row)),
                Some(Msg::Selector(SelectorMsg::PaneClicked(Pane::Projects)))
            );
            at_ms(&clock, 200);
            assert_eq!(l.on(&press(5, 3)), chosen("BRIO", false), "{column}, {row}");
        }
    }

    #[test]
    fn a_press_elsewhere_ends_a_double_click_and_answers_nothing() {
        let (mut l, clock) = timed();
        l.on(&press(5, 3));
        at_ms(&clock, 100);
        assert_eq!(l.on(&ELSEWHERE), None);
        assert_eq!(
            l.state(),
            State::Single(tuirealm::state::StateValue::Usize(0)),
            "the selection stays"
        );
        at_ms(&clock, 200);
        assert_eq!(l.on(&press(5, 3)), chosen("BRIO", false));
        // With no press under way it answers nothing either.
        assert_eq!(list(&NAMES, Some(0), true).on(&ELSEWHERE), None);
    }

    #[test]
    fn the_wheel_does_not_end_a_double_click() {
        let (mut l, clock) = timed();
        l.on(&press(5, 3));
        at_ms(&clock, 100);
        assert_eq!(
            l.on(&mouse(MouseEventKind::ScrollDown, 5, 3)),
            selected("BRIO")
        );
        at_ms(&clock, 200);
        assert_eq!(l.on(&press(5, 3)), chosen("BRIO", true));
    }

    #[test]
    fn the_click_clock_can_be_replaced_through_an_attribute() {
        let mut l = list(&NAMES, Some(0), true);
        let clock = TestClock::new();
        l.attr(CLICK_CLOCK_ATTR, clock.click_clock().value());
        l.on(&press(5, 3));
        at_ms(&clock, 401);
        assert_eq!(l.on(&press(5, 3)), chosen("BRIO", false));
    }

    #[test]
    fn the_system_clock_measures_double_clicks_by_default() {
        let mut l = list(&NAMES, Some(0), true);
        l.on(&press(5, 3));
        assert_eq!(l.on(&press(5, 3)), chosen("BRIO", true));
    }
}
