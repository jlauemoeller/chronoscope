//! Update tests and screen snapshots for the selector's selection rules
//! (SPEC §4.1, §6.6): startup preselection, clearing and keeping the session
//! selection, `Enter`, `Esc` and the empty state.

use tuirealm::event::Key;

use super::fixtures::{self, at, project_in, snapshot_of, twelve_projects};
use super::selector_tests::{
    CLAUDE, OPENCODE, cell, cwd, edited_snapshot, empty_app_in, focus_sessions, key_msg, loaded,
    press, project_rows, query, row_ids, screen, selected_project_name, send, session_rows, styled,
    tabs, text, theme,
};
use super::{App, Id, Msg, Pane, SelectorMsg};
use crate::components::{PROJECT_ROWS_ATTR, ProjectRows};
use crate::index::SessionMap;
use crate::model::{HarnessId, ProjectId, SessionId};
use crate::theme::Role;

fn dir(app: &App) -> Option<&str> {
    app.selected_project().map(ProjectId::as_str)
}

fn two_apps_data(app: &mut App, claude: &[(&str, &str)], opencode: &[(&str, &str)]) {
    let projects = |harness, list: &[(&str, &str)]| {
        list.iter()
            .enumerate()
            .map(|(n, (name, cwd))| {
                let last = at(2026, 10, 30, 11, 0, 0) - chrono::Duration::days(n as i64);
                project_in(harness, name, cwd, last)
            })
            .collect::<Vec<_>>()
    };
    send(app, snapshot_of(CLAUDE, projects(CLAUDE, claude)));
    send(app, snapshot_of(OPENCODE, projects(OPENCODE, opencode)));
}

// -- startup preselection

#[test]
fn startup_preselects_the_project_of_the_nearest_ancestor_of_the_working_directory() {
    // `/w/bri` is a string prefix of the working directory but not a path
    // ancestor; `/w` is an ancestor but not the nearest.
    let mut app = empty_app_in(&[CLAUDE], theme("default"), "/w/brio/src");
    let projects = vec![
        project_in(CLAUDE, "bri", "/w/bri", at(2026, 10, 30, 11, 0, 0)),
        project_in(CLAUDE, "w", "/w", at(2026, 10, 29, 11, 0, 0)),
        project_in(CLAUDE, "brio", "/w/brio", at(2026, 10, 28, 11, 0, 0)),
        project_in(CLAUDE, "other", "/w/other", at(2026, 10, 27, 11, 0, 0)),
    ];
    send(&mut app, snapshot_of(CLAUDE, projects));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(dir(&app), Some("/w/brio"));
    assert_eq!(project_rows(&app).selected, Some(2));
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(app.application().focus(), Some(&Id::ProjectList));
    assert_eq!(app.selected_session(), None);
    let sessions = session_rows(&app);
    assert_eq!(row_ids(&sessions), ["brio-1"]);
    assert_eq!(sessions.selected, None);
    assert_eq!(cwd(&app), "/w/brio");
}

#[test]
fn the_working_directory_itself_matches_its_project() {
    let mut app = empty_app_in(&[CLAUDE], theme("default"), "/w/brio");
    let projects = vec![
        project_in(CLAUDE, "w", "/w", at(2026, 10, 29, 11, 0, 0)),
        project_in(CLAUDE, "brio", "/w/brio", at(2026, 10, 28, 11, 0, 0)),
    ];
    send(&mut app, snapshot_of(CLAUDE, projects));
    assert_eq!(dir(&app), Some("/w/brio"));
}

#[test]
fn startup_preselects_the_harness_that_owns_the_deepest_match_whatever_arrives_first() {
    let claude = [("w", "/w"), ("x", "/x")];
    let opencode = [("other", "/o"), ("brio", "/w/brio")];
    // Claude only has the ancestor `/w`; OpenCode owns `/w/brio`.
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/w/brio/src");
    two_apps_data(&mut app, &claude, &opencode);
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(dir(&app), Some("/w/brio"));
    assert_eq!(project_rows(&app).selected, Some(1));
    assert_eq!(app.selected_session(), None);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert!(tabs(&app).tabs.iter().all(|t| t.available));
    assert_eq!(tabs(&app).selected, Some(OPENCODE));

    // The other way round: OpenCode reports first.
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/w/brio/src");
    let projects = |harness, list: &[(&str, &str)]| {
        list.iter()
            .map(|(name, cwd)| project_in(harness, name, cwd, at(2026, 10, 1, 0, 0, 0)))
            .collect::<Vec<_>>()
    };
    send(
        &mut app,
        snapshot_of(OPENCODE, projects(OPENCODE, &opencode)),
    );
    send(&mut app, snapshot_of(CLAUDE, projects(CLAUDE, &claude)));
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(dir(&app), Some("/w/brio"));
}

#[test]
fn equally_deep_matches_go_to_the_lowest_harness_number() {
    // OpenCode reports first, Claude (number 1) wins when it arrives.
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/w/brio/src");
    let projects = |harness| {
        vec![project_in(
            harness,
            "brio",
            "/w/brio",
            at(2026, 10, 1, 0, 0, 0),
        )]
    };
    send(&mut app, snapshot_of(OPENCODE, projects(OPENCODE)));
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    send(&mut app, snapshot_of(CLAUDE, projects(CLAUDE)));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(dir(&app), Some("/w/brio"));
}

#[test]
fn an_unavailable_harness_takes_no_part_in_the_preselection() {
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/w/brio");
    let claude = vec![project_in(
        CLAUDE,
        "brio",
        "/w/brio",
        at(2026, 10, 1, 0, 0, 0),
    )];
    // Claude says it is not available but still lists a matching project.
    let IndexEventParts(projects, sessions) = parts(snapshot_of(CLAUDE, claude));
    send(
        &mut app,
        fixtures::snapshot_event(CLAUDE, false, projects, sessions),
    );
    let opencode = vec![project_in(OPENCODE, "w", "/w", at(2026, 10, 1, 0, 0, 0))];
    send(&mut app, snapshot_of(OPENCODE, opencode));
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(dir(&app), Some("/w"));
}

struct IndexEventParts(Vec<crate::model::Project>, SessionMap);

fn parts(event: crate::index::IndexEvent) -> IndexEventParts {
    let crate::index::IndexEvent::Snapshot {
        projects, sessions, ..
    } = event
    else {
        unreachable!()
    };
    IndexEventParts((*projects).clone(), (*sessions).clone())
}

#[test]
fn without_a_match_startup_selects_the_first_available_harness_and_its_first_project() {
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/elsewhere");
    send(
        &mut app,
        fixtures::snapshot_event(CLAUDE, false, Vec::new(), SessionMap::new()),
    );
    send(&mut app, fixtures::fixture_snapshot(OPENCODE));
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(selected_project_name(&app), "Agentmon");
    assert_eq!(project_rows(&app).selected, Some(0));
    assert_eq!(app.selected_session(), None);
    assert_eq!(app.focused_pane(), Pane::Projects);

    // A sibling with the same name prefix is no match either.
    let mut app = empty_app_in(
        &[CLAUDE],
        theme("default"),
        "/home/jacob/Development/projects/Agentmon2",
    );
    send(&mut app, fixtures::fixture_snapshot(CLAUDE));
    assert_eq!(selected_project_name(&app), "Agentmon");
}

#[test]
fn a_later_snapshot_does_not_move_a_selection_the_user_made() {
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/w/brio/src");
    send(
        &mut app,
        snapshot_of(
            CLAUDE,
            vec![project_in(CLAUDE, "a", "/a", at(2026, 10, 1, 0, 0, 0))],
        ),
    );
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    press(&mut app, Key::Tab);
    press(&mut app, Key::BackTab);
    send(
        &mut app,
        snapshot_of(
            OPENCODE,
            vec![project_in(
                OPENCODE,
                "brio",
                "/w/brio",
                at(2026, 10, 1, 0, 0, 0),
            )],
        ),
    );
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(dir(&app), Some("/a"));
}

#[test]
fn a_late_snapshot_after_every_harness_reported_does_not_move_the_preselection() {
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/w/brio/src");
    let claude = vec![project_in(CLAUDE, "a", "/a", at(2026, 10, 1, 0, 0, 0))];
    send(&mut app, snapshot_of(CLAUDE, claude));
    send(&mut app, snapshot_of(OPENCODE, Vec::new()));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    // OpenCode gets a better match later on (a live update).
    let opencode = vec![project_in(
        OPENCODE,
        "brio",
        "/w/brio",
        at(2026, 10, 2, 0, 0, 0),
    )];
    send(&mut app, snapshot_of(OPENCODE, opencode));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
}

// -- keeping and clearing the session selection

#[test]
fn j_and_k_in_projects_clear_the_session_selection_and_sessions_selects_the_newest_again() {
    let mut app = loaded();
    focus_sessions(&mut app);
    press(&mut app, Key::Char('j'));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s2"));
    press(&mut app, Key::BackTab);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s2"));

    press(&mut app, Key::Char('j'));
    assert_eq!(selected_project_name(&app), "BRIO");
    assert_eq!(app.selected_session(), None);
    assert_eq!(session_rows(&app).selected, None);
    press(&mut app, Key::Tab);
    assert_eq!(
        app.selected_session().map(SessionId::as_str),
        Some("other1")
    );

    press(&mut app, Key::BackTab);
    press(&mut app, Key::Char('k'));
    assert_eq!(selected_project_name(&app), "Agentmon");
    assert_eq!(app.selected_session(), None);
    assert_eq!(session_rows(&app).selected, None);
    focus_sessions(&mut app);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s1"));
    assert_eq!(session_rows(&app).selected, Some(0));
}

#[test]
fn a_live_snapshot_leaves_nothing_selected_and_focusing_sessions_selects_the_new_newest() {
    let mut app = loaded();
    assert_eq!(app.selected_session(), None);
    send(
        &mut app,
        edited_snapshot(|projects, sessions| {
            let newest = fixtures::session(
                &projects[0],
                "s0",
                "Brand new",
                at(2026, 10, 30, 11, 55, 0),
                at(2026, 10, 30, 11, 59, 55),
            );
            sessions.get_mut(&projects[0].id).unwrap().insert(0, newest);
        }),
    );
    assert_eq!(app.selected_session(), None);
    let sessions = session_rows(&app);
    assert_eq!(row_ids(&sessions), ["s0", "s1", "s2", "s3", "s4", "s5"]);
    assert_eq!(sessions.selected, None);
    assert_eq!(app.focused_pane(), Pane::Projects);

    // Another one arrives before the user moves: still nothing selected.
    send(
        &mut app,
        edited_snapshot(|projects, sessions| {
            let newest = fixtures::session(
                &projects[0],
                "s00",
                "Newer still",
                at(2026, 10, 30, 11, 58, 0),
                at(2026, 10, 30, 11, 59, 58),
            );
            sessions.get_mut(&projects[0].id).unwrap().insert(0, newest);
        }),
    );
    assert_eq!(app.selected_session(), None);
    focus_sessions(&mut app);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s00"));
    assert_eq!(session_rows(&app).selected, Some(0));
}

// -- Enter

#[test]
fn enter_in_projects_focuses_sessions_and_selects_the_newest_session() {
    let mut app = loaded();
    assert_eq!(app.selected_session(), None);
    let msg = key_msg(&mut app, Key::Enter);
    assert_eq!(msg, Some(Msg::Selector(SelectorMsg::Focus(Pane::Sessions))));
    app.update(msg.unwrap());
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(app.application().focus(), Some(&Id::SessionList));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s1"));
    assert_eq!(session_rows(&app).selected, Some(0));
}

#[test]
fn enter_in_sessions_asks_to_open_the_selected_session_and_changes_nothing_else() {
    let mut app = loaded();
    press(&mut app, Key::Enter);
    press(&mut app, Key::Down);
    press(&mut app, Key::Down);
    let msg = key_msg(&mut app, Key::Enter);
    assert_eq!(msg, Some(Msg::OpenSession(CLAUDE, SessionId::from("s3"))));
    // Until the browser exists the app ignores the request.
    app.update(msg.unwrap());
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("s3"));
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert!(!app.should_quit());
}

#[test]
fn enter_asks_for_the_session_of_the_harness_shown() {
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/nowhere");
    send(&mut app, fixtures::fixture_snapshot(CLAUDE));
    send(&mut app, fixtures::fixture_snapshot(OPENCODE));
    press(&mut app, Key::Char('2'));
    press(&mut app, Key::Enter);
    assert_eq!(
        key_msg(&mut app, Key::Enter),
        Some(Msg::OpenSession(OPENCODE, SessionId::from("s1")))
    );
}

// -- Esc

#[test]
fn esc_quits_at_startup() {
    let mut app = loaded();
    let msg = key_msg(&mut app, Key::Esc);
    assert_eq!(msg, Some(Msg::Quit));
    app.update(msg.unwrap());
    assert!(app.should_quit());
}

#[test]
fn esc_in_sessions_quits_at_startup_too() {
    let mut app = loaded();
    press(&mut app, Key::Enter);
    assert_eq!(key_msg(&mut app, Key::Esc), Some(Msg::Quit));
}

#[test]
fn esc_closes_the_selector_when_a_session_is_open_behind_it() {
    for pane_keys in [&[][..], &[Key::Enter][..]] {
        let mut app = loaded();
        app.set_session_open(true);
        for key in pane_keys {
            press(&mut app, *key);
        }
        let msg = key_msg(&mut app, Key::Esc);
        assert_eq!(msg, Some(Msg::CloseSelector));
        app.update(msg.unwrap());
        assert!(!app.should_quit());
    }
    let mut app = loaded();
    app.set_session_open(true);
    app.set_session_open(false);
    assert_eq!(key_msg(&mut app, Key::Esc), Some(Msg::Quit));
}

// -- empty state

fn empty_harness_app() -> App {
    let mut app = empty_app_in(&[CLAUDE], theme("default"), "/nowhere");
    send(
        &mut app,
        fixtures::snapshot_event(CLAUDE, true, Vec::new(), SessionMap::new()),
    );
    app
}

fn no_harness_app() -> App {
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/nowhere");
    for harness in [CLAUDE, OPENCODE] {
        send(
            &mut app,
            fixtures::snapshot_event(harness, false, Vec::new(), SessionMap::new()),
        );
    }
    app
}

#[test]
fn a_harness_without_sessions_has_empty_panes_and_an_empty_working_directory() {
    let app = empty_harness_app();
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(dir(&app), None);
    let projects: ProjectRows = query(&app, Id::ProjectList, PROJECT_ROWS_ATTR);
    assert!(projects.rows.is_empty());
    assert!(session_rows(&app).rows.is_empty());
    assert_eq!(cwd(&app), "");
}

#[test]
fn a_machine_without_an_available_harness_selects_nothing_and_mutes_the_tab() {
    let mut app = no_harness_app();
    assert_eq!(app.selected_harness(), None);
    assert_eq!(dir(&app), None);
    let bar = tabs(&app);
    assert_eq!(bar.selected, None);
    assert!(bar.tabs.iter().all(|t| !t.available));
    assert_eq!(cwd(&app), "");
    // Tab labels are muted, none is underlined.
    let theme = theme("default");
    let buffer = screen(&mut app, 120, 40);
    assert_eq!(cell(&buffer, 5, 3).symbol(), "[");
    assert_eq!(cell(&buffer, 5, 3).fg, theme.role(Role::MutedForeground));
    assert_eq!(cell(&buffer, 6, 3).fg, theme.role(Role::MutedForeground));
    assert_eq!(cell(&buffer, 5, 4).symbol(), " ");
}

#[test]
fn keys_do_not_break_the_empty_state() {
    let mut app = no_harness_app();
    for key in [
        Key::Char('j'),
        Key::Char('k'),
        Key::Tab,
        Key::Enter,
        Key::Char('1'),
        Key::BackTab,
    ] {
        press(&mut app, key);
    }
    assert_eq!(app.selected_harness(), None);
    assert_eq!(app.selected_session(), None);
    // Nothing is selected in Sessions, so there is nothing to open.
    press(&mut app, Key::Tab);
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(key_msg(&mut app, Key::Enter), None);
}

#[test]
fn an_empty_project_in_a_harness_with_sessions_does_not_show_the_message() {
    let mut app = loaded();
    send(
        &mut app,
        edited_snapshot(|projects, sessions| {
            sessions.insert(projects[1].id.clone(), Vec::new());
        }),
    );
    press(&mut app, Key::Char('j'));
    assert_eq!(selected_project_name(&app), "BRIO");
    assert!(session_rows(&app).rows.is_empty());
    assert!(!session_rows(&app).harness_empty);
    let shown = text(&mut app, 120, 40);
    assert!(shown.contains("Sessions in BRIO"), "{shown}");
    assert!(!shown.contains("No sessions for this harness"), "{shown}");
}

#[test]
fn the_message_waits_until_every_harness_has_reported() {
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/nowhere");
    assert!(!session_rows(&app).harness_empty);
    let none = || fixtures::snapshot_event(CLAUDE, false, Vec::new(), SessionMap::new());
    send(&mut app, none());
    assert!(!session_rows(&app).harness_empty, "OpenCode is loading");
    send(
        &mut app,
        fixtures::snapshot_event(OPENCODE, false, Vec::new(), SessionMap::new()),
    );
    assert!(session_rows(&app).harness_empty);
    let shown = text(&mut app, 120, 40);
    assert!(shown.contains("No sessions for this harness"), "{shown}");
}

#[test]
fn a_harness_that_empties_shows_the_empty_state_again() {
    let mut app = loaded();
    send(
        &mut app,
        fixtures::snapshot_event(CLAUDE, true, Vec::new(), SessionMap::new()),
    );
    let shown = text(&mut app, 120, 40);
    assert!(shown.contains("No sessions for this harness"), "{shown}");
}

#[test]
fn the_empty_message_is_in_the_sessions_pane_in_muted_foreground() {
    let mut app = empty_harness_app();
    let theme = theme("default");
    let buffer = screen(&mut app, 120, 40);
    let shown = crate::test_support::buffer_to_text(&buffer);
    let (row, line) = shown
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("No sessions for this harness"))
        .unwrap();
    // The Sessions pane starts at column 4 + 36 + 1 = 41 and is 71 wide.
    let column = line.chars().position(|c| c == 'N').unwrap();
    assert!((41..41 + 71).contains(&column), "{column}");
    let middle = 41 + 71 / 2;
    assert!(column + 14 >= middle && column <= middle, "{column}");
    let c = cell(&buffer, column as u16, row as u16);
    assert_eq!(c.fg, theme.role(Role::MutedForeground));
    // The overlay's surface shows behind it, not the screen background.
    assert_eq!(c.bg, theme.role(Role::Surface));
}

#[test]
fn empty_state_80x24_text() {
    insta::assert_snapshot!(text(&mut empty_harness_app(), 80, 24));
}

#[test]
fn empty_state_120x40_text() {
    insta::assert_snapshot!(text(&mut empty_harness_app(), 120, 40));
}

#[test]
fn empty_state_120x40_styled() {
    insta::assert_snapshot!(styled(&mut empty_harness_app(), 120, 40));
}

#[test]
fn no_available_harness_120x40_text() {
    insta::assert_snapshot!(text(&mut no_harness_app(), 120, 40));
}

#[test]
fn no_available_harness_120x40_styled() {
    insta::assert_snapshot!(styled(&mut no_harness_app(), 120, 40));
}

// -- startup screens

fn twelve_app(working_dir: &str) -> App {
    let mut app = empty_app_in(&[CLAUDE], theme("default"), working_dir);
    send(
        &mut app,
        snapshot_of(CLAUDE, twelve_projects(HarnessId::Claude)),
    );
    app
}

#[test]
fn the_preselected_project_is_scrolled_into_view() {
    let mut app = twelve_app("/home/jacob/Development/projects/Quill/src");
    assert_eq!(selected_project_name(&app), "Quill");
    assert_eq!(project_rows(&app).selected, Some(8));
    let shown = text(&mut app, 80, 24);
    assert!(shown.contains("Quill"), "{shown}");
    assert!(!shown.contains("Agentmon"), "{shown}");
}

#[test]
fn preselection_before_the_first_resize_is_scrolled_too() {
    // The snapshot arrives before the terminal size is known.
    let mut app = super::App::new(
        super::selector_tests::init(&[CLAUDE], theme("default")),
        tuirealm::listener::EventListenerCfg::default(),
        "/home/jacob/Development/projects/Quill".into(),
    )
    .unwrap();
    app.set_clock(fixtures::now);
    send(
        &mut app,
        snapshot_of(CLAUDE, twelve_projects(HarnessId::Claude)),
    );
    let shown = text(&mut app, 80, 24);
    assert!(shown.contains("Quill"), "{shown}");
}

#[test]
fn startup_with_the_9th_project_preselected_80x24_text() {
    let mut app = twelve_app("/home/jacob/Development/projects/Quill/src");
    insta::assert_snapshot!(text(&mut app, 80, 24));
}

#[test]
fn startup_with_the_9th_project_preselected_80x24_styled() {
    let mut app = twelve_app("/home/jacob/Development/projects/Quill/src");
    insta::assert_snapshot!(styled(&mut app, 80, 24));
}

#[test]
fn the_preselected_project_is_the_focused_band_and_no_session_has_a_marker() {
    let mut app = twelve_app("/home/jacob/Development/projects/Quill/src");
    let theme = theme("default");
    let buffer = screen(&mut app, 80, 24);
    let shown = crate::test_support::buffer_to_text(&buffer);
    let (row, line) = shown
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("> Quill"))
        .unwrap();
    let marker = line.chars().position(|c| c == '>').unwrap();
    assert_eq!(
        cell(&buffer, marker as u16, row as u16).fg,
        theme.role(Role::Primary)
    );
    let band = cell(&buffer, marker as u16 + 2, row as u16);
    assert!(band.modifier.contains(ratatui::style::Modifier::REVERSED));
    // The one `>` is the project's: no session row carries a marker.
    assert_eq!(shown.matches('>').count(), 1, "{shown}");
}

#[test]
fn after_moving_to_the_next_project_no_session_has_a_marker_120x40_text() {
    let mut app = twelve_app("/home/jacob/Development/projects/Quill/src");
    press(&mut app, Key::Char('j'));
    assert_eq!(selected_project_name(&app), "Relay");
    assert_eq!(app.selected_session(), None);
    let shown = text(&mut app, 120, 40);
    assert_eq!(shown.matches('>').count(), 1, "{shown}");
    insta::assert_snapshot!(shown);
}
