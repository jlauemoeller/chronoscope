//! Update tests for the session selector (no terminal) and its screen
//! snapshots. All data comes from [`fixtures`]: fixed times, a fixed clock
//! and UTC timestamps.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::listener::EventListenerCfg;

use super::fixtures::{self, fixture_snapshot, message, now};
use super::redraw::RedrawRequest;
use super::{App, AppInit, Id, IndexPort, Msg, Pane, SelectorMsg, UserEvent};
use crate::components::{
    CWD_ATTR, PROJECT_ROWS_ATTR, ProjectRows, SESSION_ROWS_ATTR, SessionRows, TABS_ATTR, Tab, Tabs,
    VISIBLE_ROWS_ATTR, payload_from_value,
};
use crate::config::Config;
use crate::index::{IndexEvent, SessionMap};
use crate::keymap::Keymap;
use crate::model::{HarnessId, ProjectId, SessionId};
use crate::test_support::{buffer_to_styled, buffer_to_text, render};
use crate::theme::{Role, TerminalEnv, Theme, ThemeRegistry, resolve_by_id};
use tuirealm::props::AttrValue;

pub(super) const CLAUDE: HarnessId = HarnessId::Claude;
pub(super) const OPENCODE: HarnessId = HarnessId::OpenCode;

pub(super) fn theme(id: &str) -> Theme {
    let env = TerminalEnv {
        colorterm: Some("truecolor".to_owned()),
        no_color: false,
    };
    resolve_by_id(&ThemeRegistry::builtin(), id, &env, None).unwrap()
}

/// The default theme with every role the cell-level checks look at set to
/// its own colour, so a check cannot pass by confusing two roles.
fn distinct_theme() -> Theme {
    let theme = theme("default")
        .with_role(Role::Foreground, Color::Rgb(250, 1, 1))
        .with_role(Role::MutedForeground, Color::Rgb(2, 250, 2))
        .with_role(Role::Primary, Color::Rgb(3, 3, 250))
        .with_role(Role::SecondaryForeground, Color::Rgb(250, 250, 4))
        .with_role(Role::Ring, Color::Rgb(5, 250, 250))
        .with_role(Role::Border, Color::Rgb(250, 6, 250));
    let roles = [
        Role::Foreground,
        Role::MutedForeground,
        Role::Primary,
        Role::SecondaryForeground,
        Role::Ring,
        Role::Border,
    ];
    for (i, a) in roles.iter().enumerate() {
        for b in &roles[i + 1..] {
            assert_ne!(theme.role(*a), theme.role(*b), "{a:?} and {b:?}");
        }
    }
    theme
}

/// The id of the built-in theme that has `theme`'s name (`default` for any
/// other).
pub(super) fn id_of(theme: &Theme) -> String {
    let registry = ThemeRegistry::builtin();
    let entry = registry
        .entries()
        .iter()
        .find(|e| e.theme().is_some_and(|t| t.name() == theme.name()));
    entry.map_or("default", |e| e.id()).to_owned()
}

/// A truecolor terminal, as `theme` resolves for.
pub(super) fn truecolor() -> TerminalEnv {
    TerminalEnv {
        colorterm: Some("truecolor".to_owned()),
        no_color: false,
    }
}

pub(super) fn init(harnesses: &[HarnessId], theme: Theme) -> AppInit {
    let config = Config {
        utc: true,
        ..Config::default()
    };
    AppInit {
        config,
        registry: ThemeRegistry::builtin(),
        theme_id: id_of(&theme),
        env: truecolor(),
        config_path: None,
        theme,
        keymap: Keymap::default(),
        harnesses: harnesses.to_vec(),
        warnings: Vec::new(),
    }
}

/// An app of 120x40 with a fixed clock, before any index data arrived.
pub(super) fn empty_app(harnesses: &[HarnessId], theme: Theme) -> App {
    empty_app_in(harnesses, theme, "/nowhere")
}

/// Like [`empty_app`], started in the directory `working_dir`.
pub(super) fn empty_app_in(harnesses: &[HarnessId], theme: Theme, working_dir: &str) -> App {
    let mut app = App::new(
        init(harnesses, theme),
        EventListenerCfg::default(),
        PathBuf::from(working_dir),
    )
    .unwrap();
    app.set_clock(now);
    app.update(Msg::Resized(120, 40));
    app
}

pub(super) fn send(app: &mut App, event: IndexEvent) {
    app.update(message(event));
}

/// An app that has the fixture snapshot of Claude and two sessions' stats.
fn loaded_with(theme: Theme) -> App {
    let mut app = empty_app(&[CLAUDE], theme);
    send(&mut app, fixture_snapshot(CLAUDE));
    send(&mut app, fixtures::stats_event(CLAUDE, "s1", 23, 3900));
    send(&mut app, fixtures::stats_event(CLAUDE, "s2", 1, 45));
    app
}

pub(super) fn loaded() -> App {
    loaded_with(theme("default"))
}

pub(super) fn query<T: Clone + 'static>(app: &App, id: Id, attr: tuirealm::props::Attribute) -> T {
    let value = app
        .application()
        .query(&id, attr)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

pub(super) fn project_rows(app: &App) -> ProjectRows {
    query(app, Id::ProjectList, PROJECT_ROWS_ATTR)
}

pub(super) fn session_rows(app: &App) -> SessionRows {
    query(app, Id::SessionList, SESSION_ROWS_ATTR)
}

pub(super) fn tabs(app: &App) -> Tabs {
    query(app, Id::HarnessTabs, TABS_ATTR)
}

pub(super) fn cwd(app: &App) -> String {
    match app
        .application()
        .query(&Id::WorkingDir, CWD_ATTR)
        .unwrap()
        .unwrap()
        .into_attr()
    {
        AttrValue::String(cwd) => cwd,
        other => panic!("{other:?}"),
    }
}

pub(super) fn row_ids(rows: &SessionRows) -> Vec<&str> {
    rows.rows.iter().map(|r| r.id.as_str()).collect()
}

/// Presses `code` in the focused component and returns its message, if any,
/// without applying it.
pub(super) fn key_msg(app: &mut App, code: Key) -> Option<Msg> {
    let event: Event<UserEvent> = Event::Keyboard(KeyEvent::new(code, KeyModifiers::NONE));
    let focus = app.application().focus().cloned().unwrap();
    app.application
        .get_component_mut(&focus)
        .unwrap()
        .on(&event)
}

/// Presses `code` in the focused component and applies the message, if any.
pub(super) fn press(app: &mut App, code: Key) {
    if let Some(msg) = key_msg(app, code) {
        app.update(msg);
    }
}

pub(super) fn focus_sessions(app: &mut App) {
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Sessions)));
}

// -- data flow

#[test]
fn a_snapshot_message_sets_the_domain_state_and_fills_the_components() {
    let mut app = loaded();
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(
        app.selected_project().map(ProjectId::as_str),
        Some("/home/jacob/Development/projects/Agentmon")
    );
    assert_eq!(app.selected_session(), None);
    assert_eq!(app.focused_pane(), Pane::Projects);

    let projects = project_rows(&app);
    let names: Vec<&str> = projects.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Agentmon",
            "BRIO",
            "Calira",
            "CameraSettings",
            "FactoryFight",
            "Nayra"
        ]
    );
    assert_eq!(projects.selected, Some(0));

    let sessions = session_rows(&app);
    assert_eq!(sessions.project, "Agentmon");
    assert_eq!(row_ids(&sessions), ["s1", "s2", "s3", "s4", "s5"]);
    assert_eq!(sessions.selected, None);
    assert_eq!(sessions.rows[0].started, "2026-10-30 11:40Z");
    assert_eq!(sessions.rows[0].title, "Fix the flaky login test");

    assert_eq!(cwd(&app), "/home/jacob/Development/projects/Agentmon");
    assert_eq!(
        tabs(&app),
        Tabs {
            tabs: vec![Tab {
                id: CLAUDE,
                available: true
            }],
            selected: Some(CLAUDE)
        }
    );
    assert_eq!(app.take_redraw(), RedrawRequest::Resized);
}

#[test]
fn before_any_snapshot_nothing_is_selected_and_the_tab_is_muted() {
    let app = empty_app(&[CLAUDE], theme("default"));
    assert_eq!(app.selected_harness(), None);
    assert_eq!(app.selected_project(), None);
    assert_eq!(app.application().focus(), Some(&Id::ProjectList));
    assert_eq!(
        tabs(&app),
        Tabs {
            tabs: vec![Tab {
                id: CLAUDE,
                available: false
            }],
            selected: None
        }
    );
    assert_eq!(project_rows(&app).rows.len(), 0);
}

#[test]
fn an_unavailable_harness_is_not_selected_at_startup() {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    send(
        &mut app,
        fixtures::snapshot_event(CLAUDE, false, Vec::new(), SessionMap::new()),
    );
    assert_eq!(app.selected_harness(), None);
    assert!(!tabs(&app).tabs[0].available);
}

#[test]
fn the_first_available_harness_by_number_is_shown() {
    // The first available snapshot decides: the index worker builds the
    // harnesses in tab order, so a lower number arrives first.
    let mut app = empty_app(&[CLAUDE, OPENCODE], theme("default"));
    send(&mut app, fixture_snapshot(OPENCODE));
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    let mut app = empty_app(&[CLAUDE, OPENCODE], theme("default"));
    send(
        &mut app,
        fixtures::snapshot_event(CLAUDE, false, Vec::new(), SessionMap::new()),
    );
    send(&mut app, fixture_snapshot(OPENCODE));
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert!(!tabs(&app).tabs[0].available);
}

#[test]
fn a_snapshot_of_an_unregistered_harness_is_ignored() {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    send(&mut app, fixture_snapshot(OPENCODE));
    assert_eq!(app.selected_harness(), None);
    assert_eq!(project_rows(&app).rows.len(), 0);
}

#[test]
fn a_snapshot_of_another_harness_changes_only_the_tabs() {
    let mut app = empty_app(&[CLAUDE, OPENCODE], theme("default"));
    send(&mut app, fixture_snapshot(CLAUDE));
    assert!(!tabs(&app).tabs[1].available);
    let before = (project_rows(&app), session_rows(&app));
    send(&mut app, fixture_snapshot(OPENCODE));
    assert_eq!((project_rows(&app), session_rows(&app)), before);
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert!(tabs(&app).tabs.iter().all(|t| t.available));
}

#[test]
fn a_snapshot_travels_from_the_port_through_global_events_to_update() {
    let (tx, rx) = mpsc::channel();
    let listener = EventListenerCfg::default().add_port(
        Box::new(IndexPort::new(rx)),
        Duration::from_millis(5),
        8,
    );
    let mut app = App::new(
        init(&[CLAUDE], theme("default")),
        listener,
        PathBuf::from("/nowhere"),
    )
    .unwrap();
    tx.send(fixture_snapshot(CLAUDE)).unwrap();
    let mut msgs = Vec::new();
    for _ in 0..100 {
        msgs.extend(app.tick().unwrap());
        if msgs.iter().any(|m| matches!(m, Msg::Index(_))) {
            break;
        }
    }
    let indexes = msgs.iter().filter(|m| matches!(m, Msg::Index(_))).count();
    assert_eq!(indexes, 1, "{msgs:?}");
    assert_eq!(app.selected_harness(), None, "nothing applied yet");
    for msg in msgs {
        app.update(msg);
    }
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(project_rows(&app).rows.len(), 6);
}

// -- focus and selection

#[test]
fn focusing_sessions_selects_the_newest_session() {
    let mut app = loaded();
    focus_sessions(&mut app);
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(app.application().focus(), Some(&Id::SessionList));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s1"));
    assert_eq!(session_rows(&app).selected, Some(0));
}

#[test]
fn focusing_projects_keeps_the_session_selection() {
    let mut app = loaded();
    focus_sessions(&mut app);
    app.update(Msg::Selector(SelectorMsg::SessionSelected(
        SessionId::from("s3"),
    )));
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    assert_eq!(app.application().focus(), Some(&Id::ProjectList));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s3"));
    // Back to Sessions: the selection stays where it was.
    focus_sessions(&mut app);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s3"));
}

#[test]
fn focusing_the_pane_that_has_focus_does_nothing() {
    let mut app = loaded();
    app.take_redraw();
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    focus_sessions(&mut app);
    app.update(Msg::Selector(SelectorMsg::SessionSelected(
        SessionId::from("s2"),
    )));
    focus_sessions(&mut app);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s2"));
}

#[test]
fn focusing_sessions_of_an_empty_project_selects_nothing() {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    let projects = fixtures::projects(CLAUDE);
    let mut sessions = SessionMap::new();
    sessions.insert(projects[0].id.clone(), Vec::new());
    send(
        &mut app,
        fixtures::snapshot_event(CLAUDE, true, projects, sessions),
    );
    focus_sessions(&mut app);
    assert_eq!(app.selected_session(), None);
    assert_eq!(app.focused_pane(), Pane::Sessions);
}

#[test]
fn moving_the_project_selection_pushes_its_sessions_and_cwd() {
    let mut app = loaded();
    let brio = ProjectId::from("/home/jacob/Development/projects/BRIO");
    app.update(Msg::Selector(SelectorMsg::ProjectSelected(brio.clone())));
    assert_eq!(app.selected_project(), Some(&brio));
    let sessions = session_rows(&app);
    assert_eq!(sessions.project, "BRIO");
    assert_eq!(row_ids(&sessions), ["other1"]);
    assert_eq!(sessions.selected, None);
    assert_eq!(cwd(&app), "/home/jacob/Development/projects/BRIO");
}

#[test]
fn moving_to_another_project_clears_the_session_selection() {
    let mut app = loaded();
    focus_sessions(&mut app);
    assert!(app.selected_session().is_some());
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    let calira = ProjectId::from("/home/jacob/Development/projects/Calira");
    app.update(Msg::Selector(SelectorMsg::ProjectSelected(calira)));
    assert_eq!(app.selected_session(), None);
    assert_eq!(session_rows(&app).selected, None);
}

#[test]
fn an_unknown_project_or_session_selection_is_ignored() {
    let mut app = loaded();
    app.update(Msg::Selector(SelectorMsg::ProjectSelected(
        ProjectId::from("/nope"),
    )));
    assert_eq!(project_rows(&app).selected, Some(0));
    focus_sessions(&mut app);
    app.update(Msg::Selector(SelectorMsg::SessionSelected(
        SessionId::from("nope"),
    )));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s1"));
}

#[test]
fn keys_drive_the_selection_through_the_focused_component() {
    let mut app = loaded();
    press(&mut app, Key::Char('j'));
    assert_eq!(project_rows(&app).selected, Some(1));
    assert_eq!(
        app.selected_project().map(ProjectId::as_str),
        Some("/home/jacob/Development/projects/BRIO")
    );
    assert_eq!(session_rows(&app).project, "BRIO");
    press(&mut app, Key::Tab);
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(
        app.selected_session().map(SessionId::as_str),
        Some("other1")
    );
    press(&mut app, Key::BackTab);
    assert_eq!(app.focused_pane(), Pane::Projects);
    press(&mut app, Key::Char('k'));
    assert_eq!(project_rows(&app).selected, Some(0));
    assert_eq!(app.selected_session(), None);
    press(&mut app, Key::Enter);
    assert_eq!(app.focused_pane(), Pane::Sessions);
    press(&mut app, Key::Down);
    press(&mut app, Key::Down);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s3"));
    press(&mut app, Key::Left);
    assert_eq!(app.focused_pane(), Pane::Projects);
    press(&mut app, Key::Right);
    assert_eq!(app.focused_pane(), Pane::Sessions);
}

#[test]
fn moving_in_either_list_asks_for_a_redraw() {
    let mut app = loaded();
    app.take_redraw();
    press(&mut app, Key::Char('j'));
    assert_eq!(app.take_redraw(), RedrawRequest::Dirty, "Projects");
    press(&mut app, Key::Tab);
    app.take_redraw();
    press(&mut app, Key::Char('j'));
    assert_eq!(
        app.selected_session().map(SessionId::as_str),
        Some("other1")
    );
    // Only one session: a move that goes nowhere asks for nothing.
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    press(&mut app, Key::BackTab);
    press(&mut app, Key::Char('k'));
    press(&mut app, Key::Tab);
    app.take_redraw();
    press(&mut app, Key::Char('j'));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s2"));
    assert_eq!(app.take_redraw(), RedrawRequest::Dirty, "Sessions");
}

// -- harness switching

pub(super) fn two_harnesses() -> App {
    two_harnesses_with(theme("default"))
}

fn two_harnesses_with(theme: Theme) -> App {
    let mut app = empty_app(&[CLAUDE, OPENCODE], theme);
    send(&mut app, fixture_snapshot(CLAUDE));
    let mut projects = fixtures::projects(OPENCODE);
    projects.truncate(2);
    projects[0].name = "Fake".to_owned();
    let mut sessions = SessionMap::new();
    sessions.insert(
        projects[0].id.clone(),
        fixtures::many_sessions(&projects[0], 3),
    );
    sessions.insert(projects[1].id.clone(), Vec::new());
    send(
        &mut app,
        fixtures::snapshot_event(OPENCODE, true, projects, sessions),
    );
    app
}

#[test]
fn the_number_of_a_harness_switches_to_its_first_project() {
    let mut app = two_harnesses();
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    press(&mut app, Key::Char('2'));
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(app.selected_session(), None, "Projects has focus");
    assert_eq!(project_rows(&app).rows.len(), 2);
    assert_eq!(project_rows(&app).selected, Some(0));
    assert_eq!(session_rows(&app).project, "Fake");
    assert_eq!(tabs(&app).selected, Some(OPENCODE));
    press(&mut app, Key::Char('1'));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(project_rows(&app).rows.len(), 6);
    assert_eq!(project_rows(&app).selected, Some(0));
    assert_eq!(cwd(&app), "/home/jacob/Development/projects/Agentmon");
}

#[test]
fn a_switch_selects_the_newest_session_only_when_sessions_has_focus() {
    let mut app = two_harnesses();
    focus_sessions(&mut app);
    app.update(Msg::Selector(SelectorMsg::SessionSelected(
        SessionId::from("s4"),
    )));
    press(&mut app, Key::Char('2'));
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m00"));
    assert_eq!(session_rows(&app).selected, Some(0));
    assert_eq!(app.focused_pane(), Pane::Sessions);
    press(&mut app, Key::Char('1'));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s1"));
}

#[test]
fn a_switch_to_a_harness_without_a_tab_or_data_does_nothing() {
    let mut app = two_harnesses();
    for key in ['3', '4'] {
        press(&mut app, Key::Char(key));
        assert_eq!(app.selected_harness(), Some(CLAUDE), "key {key}");
    }
    // Registered, but its store does not exist.
    let mut app = empty_app(&[CLAUDE, OPENCODE], theme("default"));
    send(&mut app, fixture_snapshot(CLAUDE));
    send(
        &mut app,
        fixtures::snapshot_event(OPENCODE, false, Vec::new(), SessionMap::new()),
    );
    app.take_redraw();
    press(&mut app, Key::Char('2'));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    // Registered but no snapshot yet.
    let mut app = empty_app(&[CLAUDE, OPENCODE], theme("default"));
    send(&mut app, fixture_snapshot(CLAUDE));
    press(&mut app, Key::Char('2'));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
}

#[test]
fn switching_to_the_shown_harness_keeps_the_selection() {
    let mut app = loaded();
    press(&mut app, Key::Char('j'));
    press(&mut app, Key::Char('1'));
    assert_eq!(project_rows(&app).selected, Some(1));
}

#[test]
fn the_shown_harness_becoming_unavailable_gives_way_to_an_available_one() {
    let mut app = two_harnesses();
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    send(
        &mut app,
        fixtures::snapshot_event(CLAUDE, false, Vec::new(), SessionMap::new()),
    );
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(tabs(&app).selected, Some(OPENCODE));
    assert!(!tabs(&app).tabs[0].available);
    assert_eq!(project_rows(&app).rows.len(), 2);
    send(
        &mut app,
        fixtures::snapshot_event(OPENCODE, false, Vec::new(), SessionMap::new()),
    );
    assert_eq!(app.selected_harness(), None);
    assert_eq!(tabs(&app).selected, None);
    assert_eq!(project_rows(&app).rows.len(), 0);
    assert_eq!(session_rows(&app).rows.len(), 0);
}

#[test]
fn stats_of_vanished_sessions_are_dropped_on_a_snapshot() {
    let mut app = loaded();
    send(
        &mut app,
        edited_snapshot(|projects, sessions| {
            sessions.get_mut(&projects[0].id).unwrap().remove(0);
        }),
    );
    // s1 is back in a later snapshot, without its old stats.
    send(&mut app, fixture_snapshot(CLAUDE));
    let sessions = session_rows(&app);
    assert_eq!(sessions.rows[0].stats, None);
    assert!(sessions.rows[1].stats.is_some());
}

// -- live updates

/// Claude's fixture snapshot with `edit` applied to its projects and
/// sessions.
pub(super) fn edited_snapshot(
    edit: impl FnOnce(&mut Vec<crate::model::Project>, &mut SessionMap),
) -> IndexEvent {
    let IndexEvent::Snapshot {
        projects, sessions, ..
    } = fixture_snapshot(CLAUDE)
    else {
        unreachable!()
    };
    let mut projects = (*projects).clone();
    let mut sessions = (*sessions).clone();
    edit(&mut projects, &mut sessions);
    fixtures::snapshot_event(CLAUDE, true, projects, sessions)
}

pub(super) fn selected_project_name(app: &App) -> String {
    let rows = project_rows(app);
    rows.rows[rows.selected.unwrap()].name.clone()
}

#[test]
fn a_live_snapshot_keeps_the_selected_project_and_session_by_id() {
    let mut app = loaded();
    app.update(Msg::Selector(SelectorMsg::ProjectSelected(
        ProjectId::from("/home/jacob/Development/projects/Agentmon"),
    )));
    focus_sessions(&mut app);
    app.update(Msg::Selector(SelectorMsg::SessionSelected(
        SessionId::from("s3"),
    )));
    // A new project and a new session come in at the top: every index moves.
    send(
        &mut app,
        edited_snapshot(|projects, sessions| {
            let newest =
                fixtures::project(CLAUDE, "Brand New", fixtures::at(2026, 10, 30, 11, 59, 59));
            sessions.insert(newest.id.clone(), Vec::new());
            projects.insert(0, newest);
            let agentmon = projects[1].id.clone();
            let list = sessions.get_mut(&agentmon).unwrap();
            let fresh = fixtures::session(
                &projects[1],
                "s0",
                "Newest",
                fixtures::at(2026, 10, 30, 11, 55, 0),
                fixtures::at(2026, 10, 30, 11, 56, 0),
            );
            list.insert(0, fresh);
        }),
    );
    let projects = project_rows(&app);
    assert_eq!(projects.selected, Some(1));
    assert_eq!(selected_project_name(&app), "Agentmon");
    let sessions = session_rows(&app);
    assert_eq!(sessions.selected, Some(3));
    assert_eq!(sessions.rows[3].id.as_str(), "s3");
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s3"));
    assert_eq!(cwd(&app), "/home/jacob/Development/projects/Agentmon");
}

#[test]
fn a_live_snapshot_keeps_the_selection_by_id_when_indices_move_down() {
    let mut app = loaded();
    let nayra = ProjectId::from("/home/jacob/Development/projects/Nayra");
    app.update(Msg::Selector(SelectorMsg::ProjectSelected(nayra.clone())));
    send(
        &mut app,
        edited_snapshot(|projects, _| {
            projects.remove(1);
            projects.remove(1);
        }),
    );
    assert_eq!(app.selected_project(), Some(&nayra));
    assert_eq!(project_rows(&app).selected, Some(3));
}

#[test]
fn a_vanished_session_selection_clamps_to_the_same_index() {
    let mut app = loaded();
    focus_sessions(&mut app);
    app.update(Msg::Selector(SelectorMsg::SessionSelected(
        SessionId::from("s3"),
    )));
    send(
        &mut app,
        edited_snapshot(|projects, sessions| {
            sessions.get_mut(&projects[0].id).unwrap().remove(2);
        }),
    );
    // "s4" moved up into index 2.
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s4"));
    assert_eq!(session_rows(&app).selected, Some(2));

    // The last row vanishes: clamp to the new last row.
    app.update(Msg::Selector(SelectorMsg::SessionSelected(
        SessionId::from("s5"),
    )));
    send(
        &mut app,
        edited_snapshot(|projects, sessions| {
            let list = sessions.get_mut(&projects[0].id).unwrap();
            list.truncate(2);
        }),
    );
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s2"));
    assert_eq!(session_rows(&app).selected, Some(1));
}

#[test]
fn a_vanished_project_selection_clamps_to_the_same_index_and_clears_the_session() {
    let mut app = loaded();
    app.update(Msg::Selector(SelectorMsg::ProjectSelected(
        ProjectId::from("/home/jacob/Development/projects/Calira"),
    )));
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Sessions)));
    assert!(app.selected_session().is_some());
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    send(
        &mut app,
        edited_snapshot(|projects, _| {
            projects.remove(2);
        }),
    );
    // CameraSettings took index 2.
    assert_eq!(project_rows(&app).selected, Some(2));
    assert_eq!(selected_project_name(&app), "CameraSettings");
    assert_eq!(app.selected_session(), None);
    assert_eq!(session_rows(&app).project, "CameraSettings");

    // The last project vanishes: clamp to the new last one.
    app.update(Msg::Selector(SelectorMsg::ProjectSelected(
        ProjectId::from("/home/jacob/Development/projects/Nayra"),
    )));
    send(
        &mut app,
        edited_snapshot(|projects, _| {
            projects.truncate(3);
        }),
    );
    assert_eq!(project_rows(&app).selected, Some(2));
}

#[test]
fn a_vanished_project_with_sessions_focused_selects_the_newest_session_of_its_successor() {
    let mut app = loaded();
    focus_sessions(&mut app);
    send(
        &mut app,
        edited_snapshot(|projects, _| {
            projects.remove(0);
        }),
    );
    assert_eq!(selected_project_name(&app), "BRIO");
    assert_eq!(
        app.selected_session().map(SessionId::as_str),
        Some("other1")
    );
}

#[test]
fn a_snapshot_with_nothing_clears_the_lists() {
    let mut app = loaded();
    focus_sessions(&mut app);
    send(
        &mut app,
        fixtures::snapshot_event(CLAUDE, true, Vec::new(), SessionMap::new()),
    );
    assert_eq!(app.selected_project(), None);
    assert_eq!(app.selected_session(), None);
    assert_eq!(project_rows(&app).rows.len(), 0);
    assert_eq!(session_rows(&app).rows.len(), 0);
    assert_eq!(cwd(&app), "");
}

#[test]
fn a_selection_made_while_a_session_is_selected_and_titles_refresh_survives() {
    let mut app = loaded();
    focus_sessions(&mut app);
    send(
        &mut app,
        edited_snapshot(|projects, sessions| {
            sessions.get_mut(&projects[0].id).unwrap()[0].title = "A new title".to_owned();
        }),
    );
    let sessions = session_rows(&app);
    assert_eq!(sessions.rows[0].title, "A new title");
    assert_eq!(sessions.selected, Some(0));
}

// -- stats

#[test]
fn stats_events_fill_the_matching_row() {
    let app = loaded();
    let sessions = session_rows(&app);
    let stats = |i: usize| {
        sessions.rows[i]
            .stats
            .map(|s| (s.turns, s.agent_time.as_secs()))
    };
    assert_eq!(stats(0), Some((23, 3900)));
    assert_eq!(stats(1), Some((1, 45)));
    assert_eq!(stats(2), None);
}

#[test]
fn a_stats_event_updates_a_row_that_is_shown_and_survives_a_snapshot() {
    let mut app = loaded();
    send(&mut app, fixtures::stats_event(CLAUDE, "s3", 4, 720));
    assert_eq!(session_rows(&app).rows[2].stats.map(|s| s.turns), Some(4));
    send(&mut app, fixtures::stats_event(CLAUDE, "s3", 5, 900));
    assert_eq!(session_rows(&app).rows[2].stats.map(|s| s.turns), Some(5));
    send(&mut app, fixture_snapshot(CLAUDE));
    assert_eq!(session_rows(&app).rows[2].stats.map(|s| s.turns), Some(5));
}

#[test]
fn stats_of_a_session_that_is_not_shown_are_kept_for_later() {
    let mut app = loaded();
    app.take_redraw();
    send(&mut app, fixtures::stats_event(CLAUDE, "other1", 7, 70));
    assert_eq!(
        app.take_redraw(),
        RedrawRequest::None,
        "nothing visible changed"
    );
    app.update(Msg::Selector(SelectorMsg::ProjectSelected(
        ProjectId::from("/home/jacob/Development/projects/BRIO"),
    )));
    assert_eq!(session_rows(&app).rows[0].stats.map(|s| s.turns), Some(7));
}

// -- activity

#[test]
fn an_active_session_shows_the_marker_and_the_flag_clears_after_60_seconds() {
    let clock = std::rc::Rc::new(std::cell::Cell::new(now()));
    let mut app = empty_app(&[CLAUDE], theme("default"));
    let handle = std::rc::Rc::clone(&clock);
    app.set_clock(move || handle.get());
    send(&mut app, fixture_snapshot(CLAUDE));
    assert!(session_rows(&app).rows[0].active);
    assert!(session_rows(&app).rows[1..].iter().all(|r| !r.active));
    assert_eq!(app.active_sessions(), [SessionId::from("s1")]);

    // 10 s old at `now`: still active at +50 s, not at +51 s.
    clock.set(now() + chrono::Duration::seconds(50));
    app.update(Msg::Tick);
    assert!(session_rows(&app).rows[0].active);
    clock.set(now() + chrono::Duration::seconds(51));
    app.take_redraw();
    app.update(Msg::Tick);
    assert!(!session_rows(&app).rows[0].active);
    assert!(app.active_sessions().is_empty());
    assert_eq!(app.take_redraw(), RedrawRequest::Dirty);
}

#[test]
fn the_active_flags_are_pushed_only_when_they_change() {
    let mut app = loaded();
    app.take_redraw();
    app.update(Msg::Tick);
    app.update(Msg::Tick);
    assert_eq!(app.take_redraw(), RedrawRequest::None);
}

#[test]
fn a_harness_that_says_it_is_not_idle_is_active_whatever_its_age() {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    send(
        &mut app,
        edited_snapshot(|projects, sessions| {
            sessions.get_mut(&projects[0].id).unwrap()[3].idle = Some(false);
        }),
    );
    let flags: Vec<bool> = session_rows(&app).rows.iter().map(|r| r.active).collect();
    assert_eq!(flags, [true, false, false, true, false]);
}

// -- sizes

#[test]
fn the_lists_are_told_how_many_rows_their_pane_shows() {
    let visible = |app: &App, id: Id| match app
        .application()
        .query(&id, VISIBLE_ROWS_ATTR)
        .unwrap()
        .unwrap()
        .into_attr()
    {
        AttrValue::Length(n) => n,
        other => panic!("{other:?}"),
    };
    let mut app = loaded();
    // 120x40: Projects is 24 rows, Sessions 13 (the panels take 11).
    assert_eq!(visible(&app, Id::ProjectList), 21);
    assert_eq!(visible(&app, Id::SessionList), 10);
    app.update(Msg::Resized(80, 24));
    assert_eq!(visible(&app, Id::ProjectList), 5);
    assert_eq!(visible(&app, Id::SessionList), 5);
}

#[test]
fn each_list_follows_its_own_pane_across_a_resize_between_32_and_33_rows() {
    let visible = |app: &App, id: Id| match app
        .application()
        .query(&id, VISIBLE_ROWS_ATTR)
        .unwrap()
        .unwrap()
        .into_attr()
    {
        AttrValue::Length(n) => n,
        other => panic!("{other:?}"),
    };
    let mut app = loaded();
    app.update(Msg::Resized(100, 32));
    // Overlay 30 rows, panes 16: no panels.
    assert_eq!(visible(&app, Id::ProjectList), 13);
    assert_eq!(visible(&app, Id::SessionList), 13);
    app.update(Msg::Resized(100, 33));
    // Panes 17: Sessions 6, three list rows.
    assert_eq!(visible(&app, Id::ProjectList), 14);
    assert_eq!(visible(&app, Id::SessionList), 3);
    app.update(Msg::Resized(100, 32));
    assert_eq!(visible(&app, Id::ProjectList), 13);
    assert_eq!(visible(&app, Id::SessionList), 13);
}

#[test]
fn the_selection_stays_visible_when_moving_down_a_long_list() {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    app.update(Msg::Resized(80, 24));
    send(&mut app, long_session_snapshot(40));
    focus_sessions(&mut app);
    for _ in 0..30 {
        press(&mut app, Key::Char('j'));
    }
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m30"));
    let text = buffer_to_text(&screen(&mut app, 80, 24));
    assert!(text.contains("Session number 30"), "{text}");
    assert!(!text.contains("Session number 0\n"));
    // No wrap-around at the end.
    for _ in 0..20 {
        press(&mut app, Key::Char('j'));
    }
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m39"));
    press(&mut app, Key::Char('j'));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m39"));
}

#[test]
fn a_scrolled_sessions_list_does_not_carry_its_offset_to_the_next_project() {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    app.update(Msg::Resized(80, 24));
    let projects = fixtures::projects(CLAUDE);
    let mut sessions = SessionMap::new();
    sessions.insert(
        projects[0].id.clone(),
        fixtures::many_sessions(&projects[0], 40),
    );
    let others = (0..10)
        .map(|n| {
            let start = fixtures::at(2026, 3, 1, 10, 0, 0) - chrono::Duration::days(n);
            fixtures::session(
                &projects[1],
                &format!("o{n}"),
                &format!("Other session {n}"),
                start,
                start,
            )
        })
        .collect();
    sessions.insert(projects[1].id.clone(), others);
    send(
        &mut app,
        fixtures::snapshot_event(CLAUDE, true, projects, sessions),
    );
    focus_sessions(&mut app);
    for _ in 0..30 {
        press(&mut app, Key::Char('j'));
    }
    assert!(text(&mut app, 80, 24).contains("Session number 30"));
    press(&mut app, Key::BackTab);
    press(&mut app, Key::Char('j'));
    assert_eq!(app.selected_session(), None);
    let shown = text(&mut app, 80, 24);
    assert!(shown.contains("Sessions in BRIO"), "{shown}");
    assert!(shown.contains("Other session 0"), "{shown}");
    assert!(shown.contains("Other session 4"), "{shown}");
    assert!(!shown.contains("Other session 5"), "{shown}");
    // Back in Sessions the newest session is selected and shown at the top.
    press(&mut app, Key::Tab);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("o0"));
    assert!(text(&mut app, 80, 24).contains("> 2026-03-01"));
}

// -- screens

pub(super) fn screen(app: &mut App, width: u16, height: u16) -> Buffer {
    app.update(Msg::Resized(width, height));
    render(width, height, |frame| app.view(frame))
}

pub(super) fn text(app: &mut App, width: u16, height: u16) -> String {
    buffer_to_text(&screen(app, width, height))
}

pub(super) fn styled(app: &mut App, width: u16, height: u16) -> String {
    buffer_to_styled(&screen(app, width, height))
}

pub(super) fn long_session_snapshot(count: usize) -> IndexEvent {
    let projects = fixtures::projects(CLAUDE);
    let mut sessions = SessionMap::new();
    sessions.insert(
        projects[0].id.clone(),
        fixtures::many_sessions(&projects[0], count),
    );
    fixtures::snapshot_event(CLAUDE, true, projects, sessions)
}

fn long_title_snapshot() -> IndexEvent {
    edited_snapshot(|projects, sessions| {
        let list = sessions.get_mut(&projects[0].id).unwrap();
        list[0].title = "Investigate why the continuous integration pipeline keeps timing out on the nightly build".to_owned();
        list[1].title = "Ship it".to_owned();
        list[2].title =
            "Rewrite 日本語のタイトル transcript renderer so that it handles wide characters"
                .to_owned();
    })
}

#[test]
fn startup_80x24_text() {
    insta::assert_snapshot!(text(&mut loaded(), 80, 24));
}

#[test]
fn startup_120x40_text() {
    insta::assert_snapshot!(text(&mut loaded(), 120, 40));
}

#[test]
fn startup_120x40_styled() {
    insta::assert_snapshot!(styled(&mut loaded(), 120, 40));
}

#[test]
fn startup_200x50_text() {
    insta::assert_snapshot!(text(&mut loaded(), 200, 50));
}

fn sessions_focused() -> App {
    let mut app = loaded();
    focus_sessions(&mut app);
    app
}

#[test]
fn sessions_focused_120x40_text() {
    insta::assert_snapshot!(text(&mut sessions_focused(), 120, 40));
}

#[test]
fn sessions_focused_120x40_styled() {
    insta::assert_snapshot!(styled(&mut sessions_focused(), 120, 40));
}

#[test]
fn sessions_focused_80x24_text() {
    insta::assert_snapshot!(text(&mut sessions_focused(), 80, 24));
}

#[test]
fn long_titles_80x24_text() {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    send(&mut app, long_title_snapshot());
    send(&mut app, fixtures::stats_event(CLAUDE, "s1", 23, 3900));
    insta::assert_snapshot!(text(&mut app, 80, 24));
}

#[test]
fn long_titles_with_stats_120x40_text() {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    send(&mut app, long_title_snapshot());
    send(&mut app, fixtures::stats_event(CLAUDE, "s1", 23, 3900));
    send(&mut app, fixtures::stats_event(CLAUDE, "s3", 1, 45));
    insta::assert_snapshot!(text(&mut app, 120, 40));
}

// State 4 (the active marker inside the band) is `sessions_focused_120x40_styled`
// (the active session is the newest, which is selected) and the cell checks in
// `the_active_dot_is_primary_outside_the_band_and_inverted_inside_it`.
#[test]
fn the_active_marker_in_an_unselected_row_above_a_band_120x40_styled() {
    let mut app = sessions_focused();
    app.update(Msg::Selector(SelectorMsg::SessionSelected(
        SessionId::from("s2"),
    )));
    insta::assert_snapshot!(styled(&mut app, 120, 40));
}

#[test]
fn a_scrolled_list_80x24_text() {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    app.update(Msg::Resized(80, 24));
    send(&mut app, long_session_snapshot(40));
    focus_sessions(&mut app);
    for _ in 0..30 {
        press(&mut app, Key::Char('j'));
    }
    insta::assert_snapshot!(text(&mut app, 80, 24));
}

#[test]
fn a_working_directory_longer_than_the_box_is_cut_from_the_left_80x24_text() {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    send(
        &mut app,
        edited_snapshot(|projects, _| {
            projects[0].cwd = Some(
                "/home/jacob/Development/projects/clients/very-long-customer-name/monorepo/services/session-index-worker"
                    .into(),
            );
        }),
    );
    let text = text(&mut app, 80, 24);
    assert!(text.contains("│  …"), "{text}");
    insta::assert_snapshot!(text);
}

macro_rules! theme_snapshot {
    ($name:ident, $id:literal) => {
        #[test]
        fn $name() {
            let mut app = loaded_with(theme($id));
            focus_sessions(&mut app);
            insta::assert_snapshot!(styled(&mut app, 120, 40));
        }
    };
}

theme_snapshot!(sessions_focused_theme_default, "default");
theme_snapshot!(sessions_focused_theme_terminal, "terminal");
theme_snapshot!(sessions_focused_theme_catppuccin, "catppuccin");
theme_snapshot!(sessions_focused_theme_gruvbox, "gruvbox");
theme_snapshot!(sessions_focused_theme_nord, "nord");
theme_snapshot!(sessions_focused_theme_tokyo_night, "tokyo-night");
theme_snapshot!(sessions_focused_theme_solarized, "solarized");

// -- cell-level checks of the geometry and styles (SPEC §8.2)

/// The overlay's panes at 120x40: Projects at (4, 6) 36x24, Sessions at
/// (41, 6) 75x13 (the panels take the other 11 rows). The first list row is two rows below a pane's top.
const LIST_ROW_Y: u16 = 8;
const SESSIONS_X: u16 = 41;
const PROJECTS_X: u16 = 4;
const SESSIONS_WIDTH: u16 = 75;

pub(super) fn cell(buffer: &Buffer, x: u16, y: u16) -> &ratatui::buffer::Cell {
    &buffer[(x, y)]
}

#[test]
fn the_focused_selection_is_one_inverted_band_with_a_primary_marker() {
    let theme = distinct_theme();
    let mut app = loaded_with(theme.clone());
    focus_sessions(&mut app);
    let buffer = screen(&mut app, 120, 40);
    let marker = SESSIONS_X + 3;
    let band_first = SESSIONS_X + 4;
    let band_last = SESSIONS_X + SESSIONS_WIDTH - 4;
    let reversed = |x: u16| {
        cell(&buffer, x, LIST_ROW_Y)
            .modifier
            .contains(Modifier::REVERSED)
    };
    assert_eq!(cell(&buffer, marker, LIST_ROW_Y).symbol(), ">");
    assert_eq!(
        cell(&buffer, marker, LIST_ROW_Y).fg,
        theme.role(Role::Primary)
    );
    assert!(!reversed(marker), "the marker is never inverted");
    assert!(!reversed(band_first - 2));
    for x in band_first..=band_last {
        assert!(reversed(x), "column {x} is in the band");
        assert_eq!(
            cell(&buffer, x, LIST_ROW_Y).fg,
            theme.role(Role::Foreground)
        );
        assert!(
            !cell(&buffer, x, LIST_ROW_Y)
                .modifier
                .contains(Modifier::BOLD)
        );
    }
    assert!(!reversed(band_last + 1), "the margin is outside the band");
    assert!(!reversed(band_last + 2));
    // The stats end at the band's last column.
    assert_eq!(cell(&buffer, band_last, LIST_ROW_Y).symbol(), "m");
    let row = buffer_to_text(&buffer);
    let line = row.lines().nth(usize::from(LIST_ROW_Y)).unwrap();
    assert!(
        line.contains("2026-10-30 11:40Z  ● Fix the flaky login test"),
        "{line}"
    );
    assert!(line.contains("23 turns · 1h 5m"), "{line}");
}

#[test]
fn the_unfocused_selection_is_bold_secondary_without_a_band() {
    let theme = distinct_theme();
    let mut app = loaded_with(theme.clone());
    focus_sessions(&mut app);
    let buffer = screen(&mut app, 120, 40);
    let marker = PROJECTS_X + 3;
    assert_eq!(cell(&buffer, marker, LIST_ROW_Y).symbol(), ">");
    assert_eq!(
        cell(&buffer, marker, LIST_ROW_Y).fg,
        theme.role(Role::SecondaryForeground)
    );
    assert!(
        !cell(&buffer, marker, LIST_ROW_Y)
            .modifier
            .contains(Modifier::BOLD)
    );
    let text_cell = cell(&buffer, PROJECTS_X + 5, LIST_ROW_Y);
    assert_eq!(text_cell.symbol(), "A");
    assert_eq!(text_cell.fg, theme.role(Role::SecondaryForeground));
    assert!(text_cell.modifier.contains(Modifier::BOLD));
    for x in PROJECTS_X..PROJECTS_X + 36 {
        assert!(
            !cell(&buffer, x, LIST_ROW_Y)
                .modifier
                .contains(Modifier::REVERSED)
        );
    }
    // An unselected row is muted.
    let other = cell(&buffer, PROJECTS_X + 5, LIST_ROW_Y + 1);
    assert_eq!(other.fg, theme.role(Role::MutedForeground));
}

#[test]
fn the_unfocused_selection_in_sessions_is_bold_secondary_without_a_band() {
    let theme = distinct_theme();
    let mut app = loaded_with(theme.clone());
    focus_sessions(&mut app);
    app.update(Msg::Selector(SelectorMsg::SessionSelected(
        SessionId::from("s2"),
    )));
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    let buffer = screen(&mut app, 120, 40);
    let y = LIST_ROW_Y + 1;
    let secondary = theme.role(Role::SecondaryForeground);
    let marker = cell(&buffer, SESSIONS_X + 3, y);
    assert_eq!(marker.symbol(), ">");
    assert_eq!(marker.fg, secondary);
    assert!(!marker.modifier.contains(Modifier::BOLD));
    // Timestamp, title and every cell up to the stats: bold, not inverted.
    for x in [
        SESSIONS_X + 5,
        SESSIONS_X + 20,
        SESSIONS_X + 26,
        SESSIONS_X + 40,
    ] {
        let c = cell(&buffer, x, y);
        assert_eq!(c.fg, secondary, "column {x}");
        assert!(c.modifier.contains(Modifier::BOLD), "column {x}");
    }
    for x in SESSIONS_X..SESSIONS_X + SESSIONS_WIDTH {
        assert!(!cell(&buffer, x, y).modifier.contains(Modifier::REVERSED));
    }
    // Stats stay muted.
    let last = cell(&buffer, SESSIONS_X + SESSIONS_WIDTH - 4, y);
    assert_eq!(last.symbol(), "s");
    assert_eq!(last.fg, theme.role(Role::MutedForeground));
    assert!(!last.modifier.contains(Modifier::BOLD));
}

#[test]
fn the_focused_pane_has_the_ring_border() {
    let theme = distinct_theme();
    let mut app = loaded_with(theme.clone());
    let buffer = screen(&mut app, 120, 40);
    assert_eq!(cell(&buffer, PROJECTS_X, 6).fg, theme.role(Role::Ring));
    assert_eq!(cell(&buffer, SESSIONS_X, 6).fg, theme.role(Role::Border));
    focus_sessions(&mut app);
    let buffer = screen(&mut app, 120, 40);
    assert_eq!(cell(&buffer, PROJECTS_X, 6).fg, theme.role(Role::Border));
    assert_eq!(cell(&buffer, SESSIONS_X, 6).fg, theme.role(Role::Ring));
}

#[test]
fn the_active_dot_is_primary_outside_the_band_and_inverted_inside_it() {
    let theme = distinct_theme();
    let mut app = loaded_with(theme.clone());
    let buffer = screen(&mut app, 120, 40);
    // Pane column 5 + timestamp (17) + two spaces = column 24 is the dot.
    let dot_x = SESSIONS_X + 5 + 17 + 2;
    let dot = cell(&buffer, dot_x, LIST_ROW_Y);
    assert_eq!(dot.symbol(), "●");
    assert_eq!(dot.fg, theme.role(Role::Primary));
    assert!(!dot.modifier.contains(Modifier::REVERSED));
    focus_sessions(&mut app);
    let buffer = screen(&mut app, 120, 40);
    let dot = cell(&buffer, dot_x, LIST_ROW_Y);
    assert_eq!(dot.symbol(), "●");
    assert_eq!(dot.fg, theme.role(Role::Foreground));
    assert!(dot.modifier.contains(Modifier::REVERSED));
    // An inactive row has no dot.
    assert_eq!(cell(&buffer, dot_x, LIST_ROW_Y + 1).symbol(), " ");
}

#[test]
fn the_selected_tab_is_primary_with_a_primary_underline_of_its_label_width() {
    let theme = distinct_theme();
    let mut app = two_harnesses_with(theme.clone());
    let buffer = screen(&mut app, 120, 40);
    // Tabs at column 5, rows 3 and 4.
    let label = "[1] Claude";
    for (i, ch) in label.chars().enumerate() {
        let c = cell(&buffer, 5 + i as u16, 3);
        assert_eq!(c.symbol(), ch.to_string());
        assert_eq!(c.fg, theme.role(Role::Primary));
        let u = cell(&buffer, 5 + i as u16, 4);
        assert_eq!(u.symbol(), "━");
        assert_eq!(u.fg, theme.role(Role::Primary));
    }
    assert_eq!(cell(&buffer, 5 + 10, 4).symbol(), " ");
    let other = cell(&buffer, 5 + 12, 3);
    assert_eq!(other.symbol(), "[");
    assert_eq!(other.fg, theme.role(Role::MutedForeground));
}

#[test]
fn the_screen_fits_the_smallest_terminal() {
    let mut app = loaded();
    let text = text(&mut app, 60, 20);
    assert!(text.contains("Projects"), "{text}");
    assert!(text.contains("Agentmon"), "{text}");
}
