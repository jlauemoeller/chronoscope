//! Update tests for the selector's fuzzy search (no terminal) and its screen
//! snapshots. All data comes from [`fixtures`].

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use tuirealm::command::Direction;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::listener::{EventListenerCfg, Poll, PortResult};
use tuirealm::props::{AttrValue, Attribute};
use tuirealm::state::{State, StateValue};

use super::fixtures::{self, message, search_snapshot};
use super::redraw::RedrawRequest;
use super::selector_tests::{
    CLAUDE, OPENCODE, empty_app_in, init, key_msg, press, row_ids, screen, send, session_rows,
    styled, text, theme,
};
use super::{App, Id, Msg, Pane, SearchPort, SearchUpdate, SearchWorker, UserEvent};
use crate::model::{HarnessId, SessionId};
use crate::test_support::buffer_to_text;

/// An app of 120x40 that has the search fixture of Claude, started in
/// `/nowhere`, with two sessions' stats.
fn searchable() -> App {
    let mut app = empty_app_in(&[CLAUDE], theme("default"), "/nowhere");
    send(&mut app, search_snapshot(CLAUDE));
    send(&mut app, fixtures::stats_event(CLAUDE, "s1", 23, 3900));
    send(&mut app, fixtures::stats_event(CLAUDE, "s2", 1, 45));
    send(&mut app, fixtures::stats_event(CLAUDE, "b1", 4, 600));
    app
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, Key::Char(c));
    }
}

/// Opens the search with `/`, pressed in the focused list.
fn open_search(app: &mut App) {
    press(app, Key::Char('/'));
}

fn searching(query: &str) -> App {
    let mut app = searchable();
    open_search(&mut app);
    type_text(&mut app, query);
    app
}

fn focus_of(app: &App) -> Option<Id> {
    app.application().focus().copied()
}

/// Whether the list `id` is drawn as the focused one.
fn looks_focused(app: &App, id: Id) -> bool {
    let value = app
        .application()
        .query(&id, Attribute::Focus)
        .unwrap()
        .unwrap()
        .into_attr();
    value == AttrValue::Flag(true)
}

fn prompt_text(app: &App) -> String {
    match app.application().state(&Id::Prompt).unwrap() {
        State::Single(StateValue::String(text)) => text,
        other => panic!("{other:?}"),
    }
}

fn ids(app: &App) -> Vec<&str> {
    app.search_results().iter().map(SessionId::as_str).collect()
}

fn selected(app: &App) -> Option<String> {
    app.selected_result().map(|id| id.as_str().to_owned())
}

// -- the prompt and the focus stack

#[test]
fn slash_in_projects_mounts_the_prompt_and_gives_it_the_focus() {
    let mut app = searchable();
    assert!(!app.application().mounted(&Id::Prompt));
    assert_eq!(focus_of(&app), Some(Id::ProjectList));
    open_search(&mut app);
    assert!(app.application().mounted(&Id::Prompt));
    assert_eq!(focus_of(&app), Some(Id::Prompt));
    assert_eq!(app.search_query(), Some(""));
}

#[test]
fn slash_in_sessions_opens_the_prompt_too() {
    let mut app = searchable();
    press(&mut app, Key::Enter);
    assert_eq!(focus_of(&app), Some(Id::SessionList));
    open_search(&mut app);
    assert_eq!(focus_of(&app), Some(Id::Prompt));
}

#[test]
fn slash_gives_sessions_the_look_of_focus_and_selects_the_newest_session() {
    let mut app = searchable();
    open_search(&mut app);
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s1"));
    assert!(looks_focused(&app, Id::SessionList));
    assert!(!looks_focused(&app, Id::ProjectList));
    // With an empty query the pane shows its normal content.
    let rows = session_rows(&app);
    assert_eq!(rows.search, None);
    assert_eq!(rows.project, "Agentmon");
    assert_eq!(rows.selected, Some(0));
}

#[test]
fn cancelling_from_projects_returns_the_focus_to_projects() {
    let mut app = searchable();
    open_search(&mut app);
    press(&mut app, Key::Esc);
    assert!(!app.application().mounted(&Id::Prompt));
    assert_eq!(focus_of(&app), Some(Id::ProjectList));
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert!(looks_focused(&app, Id::ProjectList));
    assert!(!looks_focused(&app, Id::SessionList));
    assert_eq!(app.search_query(), None);
}

#[test]
fn cancelling_from_sessions_returns_the_focus_to_sessions() {
    let mut app = searchable();
    press(&mut app, Key::Enter);
    open_search(&mut app);
    press(&mut app, Key::Esc);
    assert!(!app.application().mounted(&Id::Prompt));
    assert_eq!(focus_of(&app), Some(Id::SessionList));
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert!(looks_focused(&app, Id::SessionList));
    assert!(!looks_focused(&app, Id::ProjectList));
}

#[test]
fn the_focus_stack_is_empty_again_after_every_search() {
    let mut app = searchable();
    for _ in 0..3 {
        open_search(&mut app);
        type_text(&mut app, "br");
        press(&mut app, Key::Esc);
        assert_eq!(focus_of(&app), Some(Id::ProjectList));
        assert!(app.focus_stack.is_empty());
    }
}

#[test]
fn the_prompt_takes_the_help_lines_place() {
    let mut app = searchable();
    open_search(&mut app);
    let layout = super::layout::layout(
        &app.layout_input(),
        ratatui::layout::Rect::new(0, 0, 120, 40),
    );
    assert!(layout.area(Id::Prompt).is_some());
    assert_eq!(layout.area(Id::HelpLine), None);
    press(&mut app, Key::Esc);
    let layout = super::layout::layout(
        &app.layout_input(),
        ratatui::layout::Rect::new(0, 0, 120, 40),
    );
    assert!(layout.area(Id::Prompt).is_none());
    assert!(layout.area(Id::HelpLine).is_some());
}

#[test]
fn opening_a_search_twice_keeps_the_first() {
    let mut app = searchable();
    open_search(&mut app);
    type_text(&mut app, "br");
    app.update(Msg::Selector(super::SelectorMsg::Search));
    assert_eq!(app.search_query(), Some("br"));
    press(&mut app, Key::Esc);
    assert_eq!(focus_of(&app), Some(Id::ProjectList));
    assert!(app.focus_stack.is_empty());
}

// -- global keys are off while typing

/// A port that delivers the events sent through its channel.
struct KeyPort(Receiver<Event<UserEvent>>);

impl Poll<UserEvent> for KeyPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        Ok(self.0.try_recv().ok())
    }
}

/// An app whose events come from the returned sender, as the terminal's do.
fn app_with_keys() -> (App, Sender<Event<UserEvent>>) {
    let (tx, rx) = channel();
    let listener =
        EventListenerCfg::default().add_port(Box::new(KeyPort(rx)), Duration::from_millis(2), 8);
    let mut app = App::new(
        init(&[CLAUDE], theme("default")),
        listener,
        PathBuf::from("/nowhere"),
    )
    .unwrap();
    app.set_clock(fixtures::now);
    app.update(Msg::Resized(120, 40));
    app.update(message(search_snapshot(CLAUDE)));
    (app, tx)
}

fn char_event(c: char) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent::new(Key::Char(c), KeyModifiers::NONE))
}

/// Ticks until the app returned something, then a few times more, and
/// returns everything, applying nothing.
fn collect(app: &mut App) -> Vec<Msg> {
    let mut msgs = Vec::new();
    for _ in 0..200 {
        msgs.extend(app.tick().unwrap());
        if !msgs.is_empty() {
            break;
        }
    }
    for _ in 0..5 {
        msgs.extend(app.tick().unwrap());
    }
    msgs
}

#[test]
fn q_typed_in_the_prompt_does_not_quit_but_is_typed() {
    let (mut app, tx) = app_with_keys();
    open_search(&mut app);
    tx.send(char_event('q')).unwrap();
    let msgs = collect(&mut app);
    assert_eq!(msgs, [Msg::PromptChanged("q".to_owned())]);
    for msg in msgs {
        app.update(msg);
    }
    assert!(!app.should_quit());
    assert_eq!(prompt_text(&app), "q");
}

#[test]
fn the_other_default_quit_keys_are_typed_or_ignored_in_the_prompt_too() {
    let (mut app, tx) = app_with_keys();
    open_search(&mut app);
    for event in [
        char_event('Q'),
        Event::Keyboard(KeyEvent::new(Key::Char('q'), KeyModifiers::CONTROL)),
        Event::Keyboard(KeyEvent::new(Key::Char('w'), KeyModifiers::CONTROL)),
    ] {
        tx.send(event).unwrap();
    }
    let msgs = collect(&mut app);
    assert!(!msgs.contains(&Msg::Quit), "{msgs:?}");
    for msg in msgs {
        app.update(msg);
    }
    assert!(!app.should_quit());
}

#[test]
fn without_the_prompt_the_same_key_quits() {
    // The control of the test above: the key does reach `GlobalEvents`.
    let (mut app, tx) = app_with_keys();
    tx.send(char_event('q')).unwrap();
    let msgs = collect(&mut app);
    assert_eq!(msgs, [Msg::Quit]);
    for msg in msgs {
        app.update(msg);
    }
    assert!(app.should_quit());
}

#[test]
fn after_the_prompt_closes_the_global_keys_work_again() {
    let (mut app, tx) = app_with_keys();
    open_search(&mut app);
    press(&mut app, Key::Esc);
    tx.send(char_event('q')).unwrap();
    assert_eq!(collect(&mut app), [Msg::Quit]);
}

#[test]
fn ctrl_c_quits_from_the_prompt() {
    let (mut app, tx) = app_with_keys();
    open_search(&mut app);
    tx.send(Event::Keyboard(KeyEvent::new(
        Key::Char('c'),
        KeyModifiers::CONTROL,
    )))
    .unwrap();
    let msgs = collect(&mut app);
    assert!(msgs.contains(&Msg::Quit), "{msgs:?}");
}

#[test]
fn resizes_and_index_events_still_arrive_while_the_prompt_has_focus() {
    let (mut app, tx) = app_with_keys();
    open_search(&mut app);
    tx.send(Event::WindowResize(100, 30)).unwrap();
    let msgs = collect(&mut app);
    assert!(msgs.contains(&Msg::Resized(100, 30)), "{msgs:?}");
    // An index update is applied and the results follow it.
    type_text(&mut app, "brio");
    let before = app.search_results().len();
    app.update(message(fixtures::snapshot_event(
        CLAUDE,
        true,
        Vec::new(),
        Default::default(),
    )));
    assert!(before > 0);
    assert_eq!(app.search_results().len(), 0);
}

// -- typing a query

#[test]
fn typing_brio_lists_the_matches_of_every_project() {
    let app = searching("brio");
    assert_eq!(app.search_query(), Some("brio"));
    let mut found = ids(&app);
    found.sort_unstable();
    assert_eq!(found, ["b1", "b2", "c2", "s2", "s3"]);
    let rows = session_rows(&app);
    assert_eq!(rows.search.as_deref(), Some("brio"));
    assert_eq!(row_ids(&rows), ids(&app));
    let projects: Vec<_> = rows
        .rows
        .iter()
        .map(|r| r.project.clone().unwrap())
        .collect();
    for (row, project) in rows.rows.iter().zip(&projects) {
        let expected = match row.id.as_str() {
            "s2" | "s3" => "Agentmon",
            "b1" | "b2" => "BRIO",
            "c2" => "Calira",
            other => panic!("{other}"),
        };
        assert_eq!(project, expected);
    }
}

#[test]
fn the_first_result_is_selected_and_the_projects_keep_their_selection() {
    let app = searching("brio");
    assert_eq!(session_rows(&app).selected, Some(0));
    assert_eq!(selected(&app), Some(ids(&app)[0].to_owned()));
    assert_eq!(super::selector_tests::project_rows(&app).selected, Some(0));
    assert_eq!(
        app.selected_project().map(|p| p.as_str().to_owned()),
        Some("/home/jacob/Development/projects/Agentmon".to_owned())
    );
}

#[test]
fn results_carry_the_stats_the_start_time_and_the_marker() {
    let app = searching("brio");
    let rows = session_rows(&app);
    let b1 = rows.rows.iter().find(|r| r.id.as_str() == "b1").unwrap();
    assert_eq!(b1.stats.map(|s| s.turns), Some(4));
    assert_eq!(b1.started, "2026-10-27 09:00Z");
    assert!(rows.rows.iter().all(|r| !r.active), "s1 is not a result");
    // `s1` is active; a query that finds it shows the marker.
    let app = searching("flaky");
    let rows = session_rows(&app);
    assert_eq!(row_ids(&rows), ["s1"]);
    assert!(rows.rows[0].active);
    assert_eq!(app.active_sessions(), [SessionId::from("s1")]);
}

#[test]
fn a_query_that_matches_nothing_lists_nothing() {
    let app = searching("zzzz");
    let rows = session_rows(&app);
    assert_eq!(rows.search.as_deref(), Some("zzzz"));
    assert!(rows.rows.is_empty());
    assert_eq!(rows.selected, None);
    assert!(app.search_results().is_empty());
}

#[test]
fn deleting_the_query_shows_the_projects_sessions_again() {
    let mut app = searching("br");
    assert!(session_rows(&app).search.is_some());
    press(&mut app, Key::Backspace);
    press(&mut app, Key::Backspace);
    let rows = session_rows(&app);
    assert_eq!(rows.search, None);
    assert_eq!(rows.project, "Agentmon");
    assert_eq!(row_ids(&rows), ["s1", "s2", "s3", "s4", "s5"]);
    assert_eq!(rows.selected, Some(0));
    assert!(app.search_results().is_empty());
    assert_eq!(focus_of(&app), Some(Id::Prompt), "the prompt stays open");
}

#[test]
fn a_blank_query_is_an_empty_one() {
    let app = searching("  ");
    assert_eq!(session_rows(&app).search, None);
}

#[test]
fn only_sessions_of_the_current_harness_are_listed() {
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/nowhere");
    send(&mut app, search_snapshot(CLAUDE));
    // OpenCode has a session `oc` in BRIO that Claude does not have.
    let projects = fixtures::projects(OPENCODE);
    let mut sessions = crate::index::SessionMap::new();
    let at = fixtures::at(2026, 10, 29, 9, 0, 0);
    sessions.insert(
        projects[1].id.clone(),
        vec![fixtures::session(&projects[1], "oc", "Brio brio", at, at)],
    );
    send(
        &mut app,
        fixtures::snapshot_event(OPENCODE, true, projects, sessions),
    );
    open_search(&mut app);
    type_text(&mut app, "brio");
    assert!(!ids(&app).contains(&"oc"), "{:?}", ids(&app));
    assert!(ids(&app).contains(&"b1"));
}

// -- moving through the results

#[test]
fn up_and_down_move_the_selection_of_the_results() {
    let mut app = searching("brio");
    let all: Vec<String> = ids(&app).iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(selected(&app).as_ref(), Some(&all[0]));
    press(&mut app, Key::Down);
    assert_eq!(selected(&app).as_ref(), Some(&all[1]));
    assert_eq!(session_rows(&app).selected, Some(1));
    press(&mut app, Key::Down);
    assert_eq!(selected(&app).as_ref(), Some(&all[2]));
    press(&mut app, Key::Up);
    assert_eq!(selected(&app).as_ref(), Some(&all[1]));
    press(&mut app, Key::Up);
    press(&mut app, Key::Up);
    assert_eq!(selected(&app).as_ref(), Some(&all[0]), "no wrap-around");
    for _ in 0..10 {
        press(&mut app, Key::Down);
    }
    assert_eq!(selected(&app).as_ref(), all.last());
}

#[test]
fn prompt_nav_reaches_the_list_as_a_message_and_asks_for_a_redraw() {
    let mut app = searching("brio");
    assert_eq!(
        key_msg(&mut app, Key::Down),
        Some(Msg::PromptNav(Direction::Down))
    );
    app.take_redraw();
    app.update(Msg::PromptNav(Direction::Down));
    assert_eq!(app.take_redraw(), RedrawRequest::Dirty);
}

#[test]
fn nav_keys_do_nothing_without_results() {
    let mut app = searchable();
    open_search(&mut app);
    press(&mut app, Key::Down);
    assert_eq!(app.selected_result(), None);
    assert_eq!(session_rows(&app).selected, Some(0));
}

#[test]
fn typing_again_selects_the_first_result_again() {
    let mut app = searching("brio");
    press(&mut app, Key::Down);
    press(&mut app, Key::Down);
    type_text(&mut app, "x");
    assert_eq!(session_rows(&app).selected, None);
    press(&mut app, Key::Backspace);
    assert_eq!(session_rows(&app).selected, Some(0));
}

// -- keys that must not move the panes

#[test]
fn tab_arrows_h_and_l_do_not_change_the_pane_during_a_search() {
    let mut app = searching("br");
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(key_msg(&mut app, Key::Tab), None);
    assert_eq!(key_msg(&mut app, Key::BackTab), None);
    for code in [Key::Left, Key::Right] {
        press(&mut app, code);
    }
    for c in ['h', 'l'] {
        press(&mut app, Key::Char(c));
    }
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(focus_of(&app), Some(Id::Prompt));
    assert!(looks_focused(&app, Id::SessionList));
    assert!(!looks_focused(&app, Id::ProjectList));
    // `h` and `l` were typed.
    assert_eq!(prompt_text(&app), "brhl");
}

#[test]
fn the_text_cursor_moves_with_left_and_right_and_asks_for_a_redraw() {
    let mut app = searching("brio");
    app.take_redraw();
    press(&mut app, Key::Left);
    assert_eq!(app.take_redraw(), RedrawRequest::Dirty);
    type_text(&mut app, "X");
    assert_eq!(prompt_text(&app), "briXo");
}

#[test]
fn number_keys_j_k_t_and_question_mark_are_typed() {
    let mut app = searchable();
    open_search(&mut app);
    type_text(&mut app, "1j2k3t4?");
    assert_eq!(prompt_text(&app), "1j2k3t4?");
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert!(!app.should_quit());
}

// -- Enter

#[test]
fn enter_opens_the_selected_result() {
    let mut app = searching("brio");
    press(&mut app, Key::Down);
    let wanted = selected(&app).unwrap();
    assert_eq!(
        key_msg(&mut app, Key::Enter),
        Some(Msg::PromptSubmit("brio".to_owned()))
    );
    press(&mut app, Key::Enter);
    assert_eq!(
        app.take_queued(),
        [Msg::OpenSession(CLAUDE, SessionId::from(wanted.as_str()))]
    );
}

#[test]
fn queued_open_requests_come_out_of_tick() {
    let (mut app, _tx) = app_with_keys();
    open_search(&mut app);
    type_text(&mut app, "brio");
    press(&mut app, Key::Enter);
    let first = app.search_results()[0].clone();
    let msgs = app.tick().unwrap();
    assert_eq!(msgs, [Msg::OpenSession(CLAUDE, first)]);
    assert_eq!(app.tick().unwrap(), []);
}

#[test]
fn enter_opens_a_result_of_another_project_than_the_selected_one() {
    let mut app = searching("calira");
    press(&mut app, Key::Enter);
    let queued = app.take_queued();
    assert_eq!(queued.len(), 1);
    assert!(
        matches!(&queued[0], Msg::OpenSession(h, id)
        if *h == CLAUDE && ["c1", "c2"].contains(&id.as_str())),
        "{queued:?}"
    );
}

#[test]
fn enter_without_a_result_does_nothing() {
    let mut app = searching("zzzz");
    press(&mut app, Key::Enter);
    assert_eq!(app.take_queued(), []);
    assert_eq!(focus_of(&app), Some(Id::Prompt));
    // Nor does it with an empty query.
    let mut app = searchable();
    open_search(&mut app);
    press(&mut app, Key::Enter);
    assert_eq!(app.take_queued(), []);
    assert_eq!(focus_of(&app), Some(Id::Prompt));
}

// -- Esc

#[test]
fn esc_from_projects_without_a_session_returns_to_exactly_that() {
    let mut app = searchable();
    let before = (
        app.focused_pane(),
        app.selected_session().cloned(),
        session_rows(&app),
        super::selector_tests::project_rows(&app),
    );
    assert_eq!((before.0, &before.1), (Pane::Projects, &None));
    open_search(&mut app);
    type_text(&mut app, "brio");
    press(&mut app, Key::Esc);
    let after = (
        app.focused_pane(),
        app.selected_session().cloned(),
        session_rows(&app),
        super::selector_tests::project_rows(&app),
    );
    assert_eq!(after, before);
    assert_eq!(focus_of(&app), Some(Id::ProjectList));
    assert_eq!(app.search_query(), None);
    assert!(app.search_results().is_empty());
}

#[test]
fn esc_from_sessions_returns_to_sessions_with_the_previous_selection() {
    let mut app = searchable();
    press(&mut app, Key::Enter);
    press(&mut app, Key::Down);
    press(&mut app, Key::Down);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s3"));
    let before = session_rows(&app);
    open_search(&mut app);
    type_text(&mut app, "brio");
    press(&mut app, Key::Down);
    press(&mut app, Key::Esc);
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s3"));
    assert_eq!(session_rows(&app), before);
    assert_eq!(focus_of(&app), Some(Id::SessionList));
}

#[test]
fn esc_with_an_empty_query_restores_the_same_state() {
    let mut app = searchable();
    open_search(&mut app);
    press(&mut app, Key::Esc);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(app.selected_session(), None);
    assert_eq!(session_rows(&app).selected, None);
}

#[test]
fn esc_from_projects_with_a_session_selected_returns_to_that_state() {
    let mut app = searchable();
    press(&mut app, Key::Enter);
    press(&mut app, Key::Tab);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert!(app.selected_session().is_some());
    open_search(&mut app);
    type_text(&mut app, "brio");
    press(&mut app, Key::Esc);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s1"));
    assert_eq!(focus_of(&app), Some(Id::ProjectList));
}

#[test]
fn esc_quits_nothing_and_a_second_esc_acts_on_the_selector_again() {
    let mut app = searching("brio");
    press(&mut app, Key::Esc);
    assert!(!app.should_quit());
    // Back in the selector at startup, `Esc` quits.
    assert_eq!(key_msg(&mut app, Key::Esc), Some(Msg::Quit));
}

// -- live updates during a search

#[test]
fn a_new_snapshot_refreshes_the_results_and_keeps_the_selected_one() {
    let mut app = searching("brio");
    press(&mut app, Key::Down);
    let keep = selected(&app).unwrap();
    // A snapshot with one more matching session in Calira.
    let event = search_snapshot(CLAUDE);
    let crate::index::IndexEvent::Snapshot {
        harness,
        available,
        projects,
        sessions,
    } = event
    else {
        unreachable!()
    };
    let mut sessions = (*sessions).clone();
    let calira = &projects[2];
    let at = fixtures::at(2026, 10, 30, 11, 30, 0);
    sessions
        .get_mut(&calira.id)
        .unwrap()
        .push(fixtures::session(calira, "c3", "Brio again", at, at));
    send(
        &mut app,
        fixtures::snapshot_event(harness, available, (*projects).clone(), sessions),
    );
    assert!(ids(&app).contains(&"c3"));
    assert_eq!(selected(&app), Some(keep));
    assert_eq!(app.search_query(), Some("brio"));
    assert_eq!(focus_of(&app), Some(Id::Prompt));
    assert_eq!(
        app.selected_project().map(|p| !p.as_str().is_empty()),
        Some(true)
    );
}

#[test]
fn stats_of_a_result_in_another_project_reach_its_row() {
    let mut app = searching("brio");
    let c2 = |app: &App| {
        session_rows(app)
            .rows
            .iter()
            .find(|r| r.id.as_str() == "c2")
            .cloned()
            .unwrap()
    };
    assert_eq!(c2(&app).stats, None);
    send(&mut app, fixtures::stats_event(CLAUDE, "c2", 9, 120));
    assert_eq!(c2(&app).stats.map(|s| s.turns), Some(9));
}

#[test]
fn the_marker_stays_on_result_rows_of_other_projects_after_a_tick() {
    let mut app = searching("brio");
    assert!(session_rows(&app).rows.iter().all(|r| !r.active));
    // The harness says `c2`, a session of Calira, is running (not idle).
    let crate::index::IndexEvent::Snapshot {
        harness,
        projects,
        sessions,
        ..
    } = search_snapshot(CLAUDE)
    else {
        unreachable!()
    };
    let mut sessions = (*sessions).clone();
    for session in sessions.values_mut().flatten() {
        session.idle = Some(session.id.as_str() != "c2");
    }
    send(
        &mut app,
        fixtures::snapshot_event(harness, true, (*projects).clone(), sessions),
    );
    let active = |app: &App| -> Vec<String> {
        session_rows(app)
            .rows
            .iter()
            .filter(|r| r.active)
            .map(|r| r.id.as_str().to_owned())
            .collect()
    };
    assert_eq!(active(&app), ["c2"]);
    app.update(Msg::Tick);
    assert_eq!(active(&app), ["c2"], "a tick must not lose the marker");
    assert_eq!(session_rows(&app).search.as_deref(), Some("brio"));
}

// -- the search worker

/// An app that searches on a worker, with its port in the listener.
fn app_with_worker() -> App {
    let (worker, results) = SearchWorker::start();
    let listener = EventListenerCfg::default().add_port(
        Box::new(SearchPort::new(results)),
        Duration::from_millis(2),
        8,
    );
    let mut app = App::new(
        init(&[CLAUDE], theme("default")),
        listener,
        PathBuf::from("/nowhere"),
    )
    .unwrap();
    app.set_clock(fixtures::now);
    app.update(Msg::Resized(120, 40));
    app.update(message(search_snapshot(CLAUDE)));
    app.set_search_worker(worker);
    app
}

/// Ticks until the answer to the latest request is among the messages and
/// returns every search result that came, applying nothing.
fn answers(app: &mut App) -> Vec<Msg> {
    let latest = app.search_seq;
    let mut found = Vec::new();
    for _ in 0..500 {
        for msg in app.tick().unwrap() {
            if matches!(&msg, Msg::SearchResults(_)) {
                found.push(msg);
            }
        }
        if found
            .iter()
            .any(|m| matches!(m, Msg::SearchResults(u) if u.seq() == latest))
        {
            return found;
        }
    }
    panic!("no answer to request {latest}: {found:?}");
}

#[test]
fn with_a_worker_the_results_arrive_as_an_event() {
    let mut app = app_with_worker();
    open_search(&mut app);
    app.update(Msg::PromptChanged("brio".to_owned()));
    // Nothing is listed until the worker answers.
    assert!(app.search_results().is_empty());
    assert_eq!(session_rows(&app).search, None);
    for msg in answers(&mut app) {
        app.update(msg);
    }
    assert_eq!(session_rows(&app).search.as_deref(), Some("brio"));
    let mut found = ids(&app);
    found.sort_unstable();
    assert_eq!(found, ["b1", "b2", "c2", "s2", "s3"]);
    assert_eq!(session_rows(&app).selected, Some(0));
    app.shutdown();
}

#[test]
fn answers_to_older_queries_are_ignored() {
    let mut app = app_with_worker();
    open_search(&mut app);
    let mut all = Vec::new();
    for query in ["b", "br", "bri", "brio"] {
        app.update(Msg::PromptChanged(query.to_owned()));
        all.extend(answers_to_latest_or_none(&mut app));
    }
    let latest = app.search_seq;
    let has_latest = |all: &[Msg]| {
        all.iter()
            .any(|m| matches!(m, Msg::SearchResults(u) if u.seq() == latest))
    };
    if !has_latest(&all) {
        all.extend(answers(&mut app));
    }
    // Apply the answers newest first: the older ones must change nothing.
    all.reverse();
    for msg in all {
        app.update(msg);
    }
    assert_eq!(session_rows(&app).search.as_deref(), Some("brio"));
    assert_eq!(app.search_query(), Some("brio"));
    app.shutdown();
}

/// Whatever answers are in already; unlike [`answers`] it does not wait.
fn answers_to_latest_or_none(app: &mut App) -> Vec<Msg> {
    app.tick()
        .unwrap()
        .into_iter()
        .filter(|m| matches!(m, Msg::SearchResults(_)))
        .collect()
}

#[test]
fn a_stale_answer_after_a_newer_one_is_ignored() {
    let mut app = app_with_worker();
    open_search(&mut app);
    app.update(Msg::PromptChanged("calira".to_owned()));
    let first = answers(&mut app);
    app.update(Msg::PromptChanged("brio".to_owned()));
    for msg in answers(&mut app) {
        app.update(msg);
    }
    let brio = session_rows(&app);
    for msg in first {
        app.update(msg);
    }
    assert_eq!(session_rows(&app), brio);
    app.shutdown();
}

#[test]
fn enter_before_the_answer_opens_the_first_result_when_it_arrives() {
    let mut app = app_with_worker();
    open_search(&mut app);
    app.update(Msg::PromptChanged("brio".to_owned()));
    app.update(Msg::PromptSubmit("brio".to_owned()));
    assert_eq!(app.take_queued(), [], "nothing to open yet");
    for msg in answers(&mut app) {
        app.update(msg);
    }
    let first = app.search_results()[0].clone();
    assert_eq!(app.take_queued(), [Msg::OpenSession(CLAUDE, first)]);
    app.shutdown();
}

#[test]
fn esc_before_the_answer_leaves_the_selector_as_it_was() {
    let mut app = app_with_worker();
    let before = session_rows(&app);
    open_search(&mut app);
    app.update(Msg::PromptChanged("brio".to_owned()));
    app.update(Msg::PromptCancel);
    for msg in answers_after_cancel(&mut app) {
        app.update(msg);
    }
    assert_eq!(session_rows(&app), before);
    assert_eq!(app.search_query(), None);
    app.shutdown();
}

/// The answers that arrive in the next moments (there may be none we can
/// wait for: the search was cancelled).
fn answers_after_cancel(app: &mut App) -> Vec<Msg> {
    let mut found = Vec::new();
    for _ in 0..20 {
        found.extend(answers_to_latest_or_none(app));
    }
    found
}

#[test]
fn a_snapshot_while_a_keystroke_is_pending_still_selects_the_first_result() {
    let mut app = app_with_worker();
    open_search(&mut app);
    app.update(Msg::PromptChanged("brio".to_owned()));
    for msg in answers(&mut app) {
        app.update(msg);
    }
    app.update(Msg::PromptNav(Direction::Down));
    app.update(Msg::PromptNav(Direction::Down));
    assert_eq!(session_rows(&app).selected, Some(2));
    // A longer query whose results still contain the selected session, and
    // a live snapshot before the worker answers it.
    app.update(Msg::PromptChanged("brio ".to_owned()));
    app.update(message(append_session()));
    for msg in answers(&mut app) {
        app.update(msg);
    }
    assert_eq!(session_rows(&app).search.as_deref(), Some("brio "));
    assert_eq!(session_rows(&app).selected, Some(0));
    app.shutdown();
}

#[test]
fn a_snapshot_with_the_same_query_keeps_the_selection_with_a_worker() {
    let mut app = app_with_worker();
    open_search(&mut app);
    app.update(Msg::PromptChanged("brio".to_owned()));
    for msg in answers(&mut app) {
        app.update(msg);
    }
    app.update(Msg::PromptNav(Direction::Down));
    let keep = selected(&app).unwrap();
    app.update(message(append_session()));
    for msg in answers(&mut app) {
        app.update(msg);
    }
    assert_eq!(selected(&app), Some(keep));
    app.shutdown();
}

/// The search fixture with one more session, matching `brio`, in Calira.
fn append_session() -> crate::index::IndexEvent {
    let crate::index::IndexEvent::Snapshot {
        harness,
        available,
        projects,
        sessions,
    } = search_snapshot(CLAUDE)
    else {
        unreachable!()
    };
    let mut sessions = (*sessions).clone();
    let calira = &projects[2];
    let at = fixtures::at(2026, 10, 30, 11, 30, 0);
    sessions
        .get_mut(&calira.id)
        .unwrap()
        .push(fixtures::session(calira, "c3", "Brio again", at, at));
    fixtures::snapshot_event(harness, available, (*projects).clone(), sessions)
}

#[test]
fn an_enter_waiting_for_an_answer_is_dropped_when_the_query_changes() {
    let mut app = app_with_worker();
    open_search(&mut app);
    app.update(Msg::PromptChanged("brio".to_owned()));
    app.update(Msg::PromptSubmit("brio".to_owned()));
    app.update(Msg::PromptChanged("calira".to_owned()));
    for msg in answers(&mut app) {
        app.update(msg);
    }
    assert_eq!(app.take_queued(), []);
    app.shutdown();
}

#[test]
fn deleting_the_query_while_a_search_runs_drops_its_answer() {
    let mut app = app_with_worker();
    open_search(&mut app);
    app.update(Msg::PromptChanged("brio".to_owned()));
    app.update(Msg::PromptChanged(String::new()));
    for msg in answers_after_cancel(&mut app) {
        app.update(msg);
    }
    assert_eq!(session_rows(&app).search, None);
    assert!(app.search_results().is_empty());
    app.shutdown();
}

#[test]
fn a_stopped_worker_falls_back_to_searching_in_place() {
    let mut app = app_with_worker();
    if let Some(worker) = app.search_worker.as_mut() {
        worker.shutdown();
    }
    open_search(&mut app);
    app.update(Msg::PromptChanged("brio".to_owned()));
    assert_eq!(session_rows(&app).search.as_deref(), Some("brio"));
    assert_eq!(app.search_results().len(), 5);
}

// -- screens

#[test]
fn prompt_open_with_an_empty_query_120x40_text() {
    let mut app = searchable();
    open_search(&mut app);
    insta::assert_snapshot!(text(&mut app, 120, 40));
}

#[test]
fn prompt_open_with_an_empty_query_120x40_styled() {
    let mut app = searchable();
    open_search(&mut app);
    insta::assert_snapshot!(styled(&mut app, 120, 40));
}

#[test]
fn query_brio_120x40_text() {
    insta::assert_snapshot!(text(&mut searching("brio"), 120, 40));
}

#[test]
fn query_brio_120x40_styled() {
    insta::assert_snapshot!(styled(&mut searching("brio"), 120, 40));
}

#[test]
fn query_brio_80x24_text() {
    insta::assert_snapshot!(text(&mut searching("brio"), 80, 24));
}

#[test]
fn query_with_no_results_80x24_text() {
    insta::assert_snapshot!(text(&mut searching("zzzz"), 80, 24));
}

#[test]
fn query_with_no_results_80x24_styled() {
    insta::assert_snapshot!(styled(&mut searching("zzzz"), 80, 24));
}

#[test]
fn query_with_no_results_message_is_centred_in_muted_foreground() {
    let mut app = searching("zzzz");
    let buffer = screen(&mut app, 80, 24);
    let theme = app.theme().clone();
    let lines: Vec<String> = buffer_to_text(&buffer).lines().map(str::to_owned).collect();
    let (y, line) = lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("No matching sessions"))
        .unwrap();
    let x = line
        .find("No matching")
        .map(|byte| line[..byte].chars().count())
        .unwrap();
    let cell = &buffer[(x as u16, y as u16)];
    assert_eq!(cell.fg, theme.role(crate::theme::Role::MutedForeground));
}

#[test]
fn after_esc_the_screen_is_the_startup_screen_120x40_text() {
    let mut app = searchable();
    let startup = text(&mut app, 120, 40);
    open_search(&mut app);
    type_text(&mut app, "brio");
    press(&mut app, Key::Esc);
    let after = text(&mut app, 120, 40);
    assert_eq!(after, startup);
    insta::assert_snapshot!(after);
}

#[test]
fn after_esc_the_styles_are_the_startup_styles_too() {
    let mut app = searchable();
    let startup = styled(&mut app, 120, 40);
    open_search(&mut app);
    type_text(&mut app, "brio");
    press(&mut app, Key::Esc);
    assert_eq!(styled(&mut app, 120, 40), startup);
}

/// The cost of one keystroke over 5 000 sessions with first prompts of about
/// 430 characters, of which every session matches the short queries: the
/// whole path of `PromptChanged` on the UI thread (matching, sorting, the
/// rows, handing them to the list), and then what the UI thread pays when
/// the worker matches (sending the request, and listing the answer). Run
/// with `cargo test --release -- --ignored --nocapture five_thousand`; the
/// numbers are in the task's build notes.
#[test]
#[ignore = "a measurement for release builds"]
fn five_thousand_sessions_one_keystroke_timing() {
    use std::time::Instant;
    let (projects, sessions) = fixtures::big_data(5000);
    let mut app = empty_app_in(&[CLAUDE], theme("default"), "/nowhere");
    send(
        &mut app,
        fixtures::snapshot_event(HarnessId::Claude, true, projects, sessions),
    );
    open_search(&mut app);
    let queries = ["i", "im", "imp", "impo", "brio", "flaky timeout", "zzz"];
    let runs = 20;
    println!("on the UI thread (no worker):");
    for query in queries {
        let mut worst = Duration::ZERO;
        let mut total = Duration::ZERO;
        for _ in 0..runs {
            let start = Instant::now();
            app.update(Msg::PromptChanged(query.to_owned()));
            let took = start.elapsed();
            worst = worst.max(took);
            total += took;
        }
        println!(
            "  {query:?}: {} results, mean {:?}, worst {worst:?}",
            app.search_results().len(),
            total / runs
        );
    }
    println!("with the worker, on the UI thread (request + listing the answer):");
    let (worker, results) = SearchWorker::start();
    app.set_search_worker(worker);
    for query in queries {
        let mut worst = Duration::ZERO;
        let mut total = Duration::ZERO;
        for _ in 0..runs {
            let start = Instant::now();
            app.update(Msg::PromptChanged(query.to_owned()));
            let sent = start.elapsed();
            let answer = results.recv().unwrap();
            let start = Instant::now();
            app.update(Msg::SearchResults(SearchUpdate::new(answer)));
            let took = sent + start.elapsed();
            worst = worst.max(took);
            total += took;
        }
        println!("  {query:?}: mean {:?}, worst {worst:?}", total / runs);
    }
}
