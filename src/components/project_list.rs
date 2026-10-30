//! The Projects pane of the session selector (SPEC §4.1, §8.2).

use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::list_pane::{
    PaneKey, PaneState, Part, RowParts, border_role, pane_key, render_rows, titled_block,
};
use super::{payload_from_value, payload_value, truncate_with_ellipsis};
use crate::app::{Msg, Pane, SelectorMsg, UserEvent};
use crate::keymap::Keymap;
use crate::model::ProjectId;
use crate::theme::Theme;

/// The attribute that sets the rows ([`ProjectRows`]).
pub const PROJECT_ROWS_ATTR: Attribute = Attribute::Custom("project_rows");

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
/// sessions.
#[derive(Debug, Clone)]
pub struct ProjectList {
    pane: PaneState,
    keymap: Keymap,
    rows: Vec<ProjectRow>,
}

impl ProjectList {
    /// An empty list that resolves keys with `keymap`.
    pub fn new(keymap: Keymap, theme: Theme) -> Self {
        Self {
            pane: PaneState::new(theme),
            keymap,
            rows: Vec::new(),
        }
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
        self.pane.query(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == PROJECT_ROWS_ATTR {
            if let Some(ProjectRows { rows, selected }) = payload_from_value(&value) {
                self.pane.cursor.set(rows.len(), selected);
                self.rows = rows;
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
        match pane_key(&self.keymap, ev)? {
            PaneKey::Up => self.moved(Direction::Up),
            PaneKey::Down => self.moved(Direction::Down),
            PaneKey::FocusSessions | PaneKey::NextPane | PaneKey::PrevPane | PaneKey::Open => {
                Some(Msg::Selector(SelectorMsg::Focus(Pane::Sessions)))
            }
            PaneKey::FocusProjects => None,
            PaneKey::Cancel => Some(self.pane.cancel()),
            PaneKey::Harness(harness) => Some(Msg::Selector(SelectorMsg::SwitchHarness(harness))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{SELECTED_ATTR, SESSION_OPEN_ATTR, VISIBLE_ROWS_ATTR, selected_value};
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
}
