//! The command line in the app (product §5.8): the filtered selector, a session
//! opened directly, and where the selector starts. Everything is resolved
//! with [`launch::resolve`] and started with the same `start_with` the
//! running application uses, which sends the start message.

use super::super::selector::{OPENCODE, empty_app_in, press, project_rows, send, styled, text};
use super::*;
use crate::app::Pane;
use crate::app::fixtures::fixture_snapshot;
use crate::app::runtime::{Startup, launch_warnings, prepare, resolve_startup, start_with};
use crate::app::tests::selector_start;
use crate::index::IndexEvent;
use crate::launch::{self, HarnessIndex, Preselect, Request, Resolution, Target};

const DIR: &str = "/home/user/projects/brio";

/// The three sessions the prefix `85a8` matches, in three projects, among
/// others it does not.
const FIRST: &str = "85a8c001-0000-4000-8000-000000000001";
const SECOND: &str = "85a8e003-0000-4000-8000-000000000003";
const THIRD: &str = "85a8d002-0000-4000-8000-000000000002";

fn harness_index(event: IndexEvent) -> HarnessIndex {
    match event {
        IndexEvent::Snapshot {
            harness,
            available,
            projects,
            sessions,
        } => HarnessIndex {
            harness,
            available,
            projects,
            sessions,
        },
        other => panic!("{other:?}"),
    }
}

/// The fixture projects with sessions `85a8…` in the first three.
fn prefixed_index() -> IndexEvent {
    let projects = fixtures::projects(CLAUDE);
    let mut sessions = SessionMap::new();
    let mut agentmon = fixtures::first_project_sessions(&projects[0]);
    agentmon[1].id = id(FIRST);
    agentmon[3].id = id(THIRD);
    sessions.insert(projects[0].id.clone(), agentmon);
    for (n, project) in projects.iter().enumerate().skip(1) {
        let name = if n == 1 {
            SECOND.to_owned()
        } else {
            format!("other{n}")
        };
        let one = session(
            project,
            &name,
            &format!("A session of {}", project.name),
            project.last_activity - chrono::Duration::minutes(30),
            project.last_activity,
        );
        sessions.insert(project.id.clone(), vec![one]);
    }
    snapshot_event(CLAUDE, true, projects, sessions)
}

fn request(target: &str) -> Request {
    Request {
        target: Some(target.to_owned()),
        ..Request::default()
    }
}

/// An app of 120x40 started from the command line `request` against
/// `event`.
fn started(request: &Request, event: IndexEvent) -> App {
    let index = vec![harness_index(event)];
    let resolution =
        launch::resolve(request, std::path::Path::new("/nowhere"), &[CLAUDE], &index).unwrap();
    let mut app = empty_app_in(&[CLAUDE], theme("default"), "/nowhere");
    let open = match &resolution.target {
        Target::Open(harness, summary) => Some((*harness, summary.clone())),
        _ => None,
    };
    start_with(
        &mut app,
        Startup {
            resolution,
            index,
            open,
        },
    );
    app
}

fn filtered() -> App {
    started(&request("85a8"), prefixed_index())
}

fn ids_of(app: &App) -> Vec<&str> {
    app.filtered_sessions()
        .iter()
        .map(SessionId::as_str)
        .collect()
}

// -- the filtered selector

#[test]
fn the_filtered_selector_lists_exactly_the_matches_with_the_first_selected() {
    let mut app = filtered();
    assert_eq!(app.filter_prefix(), Some("85a8"));
    assert_eq!(ids_of(&app), [FIRST, SECOND, THIRD]);
    let rows = session_rows(&app);
    assert_eq!(rows.search.as_deref(), Some("85a8"));
    assert_eq!(rows.selected, Some(0));
    let projects: Vec<Option<&str>> = rows.rows.iter().map(|r| r.project.as_deref()).collect();
    assert_eq!(projects, [Some("Agentmon"), Some("BRIO"), Some("Agentmon")]);
    // Sessions has the focus, no prompt is open.
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(app.application().focus(), Some(&Id::SessionList));
    assert!(!app.application().mounted(&Id::Prompt));
    assert!(text(&mut app, 120, 40).contains("Sessions matching \"85a8\""));
}

#[test]
fn the_projects_pane_keeps_its_normal_list_without_focus() {
    let app = filtered();
    let names: Vec<String> = project_rows(&app)
        .rows
        .into_iter()
        .map(|r| r.name)
        .collect();
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
    let focus = app
        .application()
        .query(&Id::ProjectList, tuirealm::props::Attribute::Focus)
        .unwrap()
        .unwrap()
        .into_attr();
    assert_eq!(focus, AttrValue::Flag(false));
}

#[test]
fn enter_opens_the_selected_match_and_the_arrows_move_the_selection() {
    let mut app = filtered();
    assert_eq!(
        super::super::selector::key_msg(&mut app, Key::Enter),
        Some(Msg::OpenSession(CLAUDE, id(FIRST)))
    );
    press(&mut app, Key::Down);
    press(&mut app, Key::Down);
    assert_eq!(
        super::super::selector::key_msg(&mut app, Key::Enter),
        Some(Msg::OpenSession(CLAUDE, id(THIRD)))
    );
    press(&mut app, Key::Up);
    assert_eq!(session_rows(&app).selected, Some(1));
    // The selection of the matches is not the selection of a project.
    assert_eq!(app.filter_prefix(), Some("85a8"));
}

#[test]
fn esc_leaves_the_filter_instead_of_quitting() {
    let mut app = filtered();
    press(&mut app, Key::Esc);
    assert!(!app.should_quit());
    assert_eq!(app.filter_prefix(), None);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(app.application().focus(), Some(&Id::ProjectList));
    assert_eq!(app.selected_session(), None);
    let rows = session_rows(&app);
    assert_eq!(rows.search, None);
    assert_eq!(rows.selected, None);
    assert_eq!(rows.rows.len(), 5, "the sessions of the first project");
    // A second Esc is the usual one in the selector.
    assert_eq!(
        super::super::selector::key_msg(&mut app, Key::Esc),
        Some(Msg::Quit)
    );
}

#[test]
fn after_esc_the_screen_is_the_normal_startup_screen() {
    let mut normal = empty_app_in(&[CLAUDE], theme("default"), "/nowhere");
    send(&mut normal, prefixed_index());
    let startup = text(&mut normal, 120, 40);
    let mut app = filtered();
    press(&mut app, Key::Esc);
    let after = text(&mut app, 120, 40);
    assert_eq!(after, startup);
    insta::assert_snapshot!(after);
}

#[test]
fn leaving_the_filter_goes_back_to_the_project_the_command_line_chose() {
    let request = Request {
        project: Some("calira".to_owned()),
        ..request("85a8")
    };
    let mut app = started(&request, prefixed_index());
    assert_eq!(app.filter_prefix(), Some("85a8"));
    press(&mut app, Key::Esc);
    assert_eq!(
        app.selected_project()
            .map(|p| p.as_str().rsplit('/').next().unwrap()),
        Some("Calira")
    );
}

#[test]
fn a_filter_shows_the_project_the_command_line_chose_in_the_projects_pane() {
    // The filter is shown after the lists are seeded, so the Projects pane
    // has the project `--project` names, as at a start without the prefix.
    let request = Request {
        project: Some("calira".to_owned()),
        ..request("85a8")
    };
    let app = started(&request, prefixed_index());
    assert_eq!(app.filter_prefix(), Some("85a8"));
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(
        app.selected_project()
            .map(|p| p.as_str().rsplit('/').next().unwrap()),
        Some("Calira")
    );
}

#[test]
fn moving_to_the_projects_pane_ends_the_filter_too() {
    let mut app = filtered();
    press(&mut app, Key::Tab);
    assert_eq!(app.filter_prefix(), None);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(session_rows(&app).search, None);
}

#[test]
fn the_search_key_ends_the_filter_and_searches() {
    let mut app = filtered();
    press(&mut app, Key::Char('/'));
    assert_eq!(app.filter_prefix(), None);
    assert_eq!(app.search_query(), Some(""));
    assert!(app.application().mounted(&Id::Prompt));
}

#[test]
fn a_new_snapshot_keeps_the_filter_and_the_selection() {
    let mut app = filtered();
    press(&mut app, Key::Down);
    send(&mut app, prefixed_index());
    assert_eq!(app.filter_prefix(), Some("85a8"));
    assert_eq!(ids_of(&app), [FIRST, SECOND, THIRD]);
    assert_eq!(session_rows(&app).selected, Some(1));
    assert_eq!(app.focused_pane(), Pane::Sessions);
}

#[test]
fn stats_reach_the_rows_of_other_projects() {
    let mut app = filtered();
    send(&mut app, fixtures::stats_event(CLAUDE, SECOND, 7, 600));
    let rows = session_rows(&app);
    assert!(rows.rows[0].stats.is_none());
    assert_eq!(rows.rows[1].stats.map(|s| s.turns), Some(7));
}

#[test]
fn a_filter_with_matches_in_another_harness_starts_on_that_harness() {
    let projects = fixtures::projects(OPENCODE);
    let mut sessions = SessionMap::new();
    for (n, project) in projects.iter().enumerate() {
        let name = format!("85a8op{n}");
        let one = session(
            project,
            &name,
            "An OpenCode session",
            project.last_activity - chrono::Duration::minutes(30),
            project.last_activity,
        );
        sessions.insert(project.id.clone(), vec![one]);
    }
    let opencode = snapshot_event(OPENCODE, true, projects, sessions);
    let index = vec![harness_index(prefixed_index()), harness_index(opencode)];
    let resolution = launch::resolve(
        &request("85a8"),
        std::path::Path::new("/nowhere"),
        &[CLAUDE, OPENCODE],
        &index,
    )
    .unwrap();
    let Target::Filtered { matches, .. } = &resolution.target else {
        panic!("{:?}", resolution.target);
    };
    // Claude's three come first; the tab is Claude's.
    assert_eq!(matches.len(), 9);
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/nowhere");
    start_with(
        &mut app,
        Startup {
            resolution,
            index,
            open: None,
        },
    );
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(app.filtered_sessions().len(), 3);
    // The other tab lists the other matches.
    press(&mut app, Key::Char('2'));
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(app.filtered_sessions().len(), 6);
    assert_eq!(app.filter_prefix(), Some("85a8"));
}

#[test]
fn the_filtered_selector_120x40_text() {
    let mut app = filtered();
    insta::assert_snapshot!(text(&mut app, 120, 40));
}

#[test]
fn the_filtered_selector_120x40_styled() {
    let mut app = filtered();
    send(&mut app, fixtures::stats_event(CLAUDE, FIRST, 12, 720));
    send(&mut app, fixtures::stats_event(CLAUDE, SECOND, 3, 95));
    insta::assert_snapshot!(styled(&mut app, 120, 40));
}

/// Claude's and OpenCode's sessions `85a8…`, three and two.
fn both_harnesses_index() -> Vec<HarnessIndex> {
    let projects = fixtures::projects(OPENCODE);
    let mut sessions = SessionMap::new();
    for (n, project) in projects.iter().take(2).enumerate() {
        let name = format!("85a8op{n}");
        let one = session(
            project,
            &name,
            "An OpenCode session",
            project.last_activity - chrono::Duration::minutes(30),
            project.last_activity,
        );
        sessions.insert(project.id.clone(), vec![one]);
    }
    let opencode = snapshot_event(OPENCODE, true, projects, sessions);
    vec![harness_index(prefixed_index()), harness_index(opencode)]
}

fn started_in_both(request: &Request) -> App {
    let index = both_harnesses_index();
    let resolution = launch::resolve(
        request,
        std::path::Path::new("/nowhere"),
        &[CLAUDE, OPENCODE],
        &index,
    )
    .unwrap();
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/nowhere");
    start_with(
        &mut app,
        Startup {
            resolution,
            index,
            open: None,
        },
    );
    app
}

#[test]
fn harness_keeps_restricting_the_filter_after_a_tab_switch() {
    let request = Request {
        harness: Some(OPENCODE),
        ..request("85a8")
    };
    let mut app = started_in_both(&request);
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(app.filtered_sessions().len(), 2);
    // Claude has `85a8…` sessions too, but the lookup did not include them.
    press(&mut app, Key::Char('1'));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert!(app.filtered_sessions().is_empty());
    assert_eq!(session_rows(&app).rows.len(), 0);
    assert_eq!(app.filter_prefix(), Some("85a8"));
    press(&mut app, Key::Char('2'));
    assert_eq!(app.filtered_sessions().len(), 2);
}

#[test]
fn without_harness_both_tabs_list_their_matches() {
    let mut app = started_in_both(&request("85a8"));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(app.filtered_sessions().len(), 3);
    press(&mut app, Key::Char('2'));
    assert_eq!(app.filtered_sessions().len(), 2);
}

/// The projects BRIO (the newest match) and Agentmon (the preselected
/// project, an older match).
fn brio_first_index() -> IndexEvent {
    let projects = fixtures::projects(CLAUDE);
    let mut sessions = SessionMap::new();
    let mut agentmon = fixtures::first_project_sessions(&projects[0]);
    agentmon[3].id = id(THIRD);
    sessions.insert(projects[0].id.clone(), agentmon);
    for (n, project) in projects.iter().enumerate().skip(1) {
        let name = if n == 1 {
            SECOND.to_owned()
        } else {
            format!("other{n}")
        };
        let one = session(
            project,
            &name,
            &format!("A session of {}", project.name),
            project.last_activity - chrono::Duration::minutes(30),
            project.last_activity,
        );
        sessions.insert(project.id.clone(), vec![one]);
    }
    snapshot_event(CLAUDE, true, projects, sessions)
}

#[test]
fn the_projects_pane_keeps_the_startup_selection_whatever_project_the_match_is_in() {
    let mut normal = empty_app_in(&[CLAUDE], theme("default"), "/nowhere");
    send(&mut normal, brio_first_index());
    let mut app = started(&request("85a8"), brio_first_index());
    // The first match is in BRIO; the startup project is Agentmon.
    assert_eq!(ids_of(&app), [SECOND, THIRD]);
    let expect = project_rows(&normal).selected;
    assert_eq!(project_rows(&app).selected, expect);
    assert_eq!(app.selected_project(), normal.selected_project());
    assert_eq!(
        super::super::selector::cwd(&app),
        super::super::selector::cwd(&normal)
    );
    // Moving among the matches changes neither.
    press(&mut app, Key::Down);
    assert_eq!(project_rows(&app).selected, expect);
    assert_eq!(
        super::super::selector::cwd(&app),
        super::super::selector::cwd(&normal)
    );
    // The Projects and Working Directory boxes read as at startup.
    let screen = text(&mut app, 120, 40);
    let startup = text(&mut normal, 120, 40);
    let left = |text: &str| -> Vec<String> {
        text.lines()
            .map(|l| l.chars().take(38).collect::<String>())
            .collect()
    };
    assert_eq!(left(&screen), left(&startup));
    assert!(screen.contains("/home/jacob/Development/projects/Agentmon"));
}

#[test]
fn the_filtered_selector_hints_say_esc_shows_all_sessions() {
    let mut app = filtered();
    assert!(text(&mut app, 120, 40).contains("ESC to show all sessions"));
    press(&mut app, Key::Esc);
    let after = text(&mut app, 120, 40);
    assert!(after.contains("ESC to Cancel"), "{after}");
    assert!(!after.contains("show all sessions"));
}

// -- opening directly

fn direct_rig() -> Rig {
    let rig = rig();
    rig.harness.script(
        BRIO_SESSION,
        vec![
            ModelUpdate::Appended(turns()),
            ModelUpdate::Progress(1.0),
            ModelUpdate::ParseWarnings(0),
            ModelUpdate::Loaded,
        ],
    );
    rig
}

/// Starts `rig`'s app from the command line `request`, as `run_app` does.
fn start(rig: &mut Rig, request: &Request) {
    let index = vec![harness_index(super::index())];
    let resolution =
        launch::resolve(request, std::path::Path::new(DIR), &[CLAUDE], &index).unwrap();
    let open = match &resolution.target {
        Target::Open(harness, summary) => Some((*harness, summary.clone())),
        _ => None,
    };
    start_with(
        &mut rig.app,
        Startup {
            resolution,
            index,
            open,
        },
    );
}

#[test]
fn an_exact_id_opens_in_the_browser() {
    let mut rig = direct_rig();
    start(&mut rig, &request(BRIO_SESSION));
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert_eq!(rig.app.open_session_id(), Some((CLAUDE, &id(BRIO_SESSION))));
    rig.pump_until(App::session_loaded);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION]);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(rig.turn_count() > 0);
}

#[test]
fn a_prefix_opens_in_the_browser() {
    let mut rig = direct_rig();
    start(&mut rig, &request("85a8665"));
    assert_eq!(rig.app.open_session_id(), Some((CLAUDE, &id(BRIO_SESSION))));
}

#[test]
fn latest_opens_the_newest_session_of_the_working_directory_project() {
    let mut rig = direct_rig();
    let latest = Request {
        latest: true,
        ..Request::default()
    };
    start(&mut rig, &latest);
    assert_eq!(
        rig.app.open_session_id(),
        Some((CLAUDE, &id(NEWER_SESSION)))
    );
}

#[test]
fn the_selector_opens_over_a_session_opened_directly() {
    let mut rig = direct_rig();
    start(&mut rig, &request(BRIO_SESSION));
    rig.pump_until(App::session_loaded);
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    assert_eq!(rig.app.selected_session(), Some(&id(BRIO_SESSION)));
    assert_eq!(rig.app.selected_harness(), Some(CLAUDE));
    rig.press(Key::Esc);
    rig.pump_until(|app| app.screen() == Screen::Browser);
    assert_eq!(rig.app.open_session_id(), Some((CLAUDE, &id(BRIO_SESSION))));
}

#[test]
fn a_session_opened_directly_knows_its_project() {
    let mut rig = direct_rig();
    start(&mut rig, &request(BRIO_SESSION));
    rig.pump_until(App::session_loaded);
    assert_eq!(rig.app.open_project_name().as_deref(), Some("BRIO"));
}

#[test]
fn opening_directly_without_an_index_shows_the_project_by_its_id() {
    // A transcript given by its path: the index does not list it (yet).
    let mut rig = direct_rig();
    let summary = {
        let brio = brio();
        session(
            &brio,
            BRIO_SESSION,
            "Investigate the camera profiles",
            at(2026, 10, 21, 8, 35, 15),
            at(2026, 10, 21, 9, 35, 15),
        )
    };
    let path = PathBuf::from(format!("/somewhere/{BRIO_SESSION}.jsonl"));
    start_with(
        &mut rig.app,
        Startup {
            resolution: Resolution {
                target: Target::File(path),
                preselect: Preselect::default(),
                warnings: Vec::new(),
            },
            index: Vec::new(),
            open: Some((CLAUDE, summary)),
        },
    );
    rig.pump_until(App::session_loaded);
    assert_eq!(rig.app.open_session_id(), Some((CLAUDE, &id(BRIO_SESSION))));
}

#[test]
fn a_direct_open_80x24_text() {
    let mut rig = direct_rig();
    start(&mut rig, &request(BRIO_SESSION));
    rig.pump_until(App::session_loaded);
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

// -- where the selector starts

#[test]
fn a_project_by_path_is_preselected() {
    let request = Request {
        project: Some("/home/jacob/Development/projects/Nayra".to_owned()),
        ..Request::default()
    };
    let app = started(&request, fixture_snapshot(CLAUDE));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(
        app.selected_project().map(|p| p.as_str()),
        Some("/home/jacob/Development/projects/Nayra")
    );
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(app.selected_session(), None);
}

#[test]
fn a_project_by_name_in_any_case_is_preselected() {
    let request = Request {
        project: Some("CAMERAsettings".to_owned()),
        ..Request::default()
    };
    let app = started(&request, fixture_snapshot(CLAUDE));
    assert_eq!(
        app.selected_project().map(|p| p.as_str()),
        Some("/home/jacob/Development/projects/CameraSettings")
    );
}

#[test]
fn the_project_stays_preselected_when_the_index_worker_reports() {
    let request = Request {
        project: Some("Nayra".to_owned()),
        ..Request::default()
    };
    let mut app = started(&request, fixture_snapshot(CLAUDE));
    // The worker's snapshot of the same data, as it arrives later.
    send(&mut app, fixture_snapshot(CLAUDE));
    assert_eq!(
        app.selected_project().map(|p| p.as_str()),
        Some("/home/jacob/Development/projects/Nayra")
    );
}

#[test]
fn an_unknown_project_warns_and_the_normal_preselection_is_used() {
    let request = Request {
        project: Some("nope".to_owned()),
        ..Request::default()
    };
    let index = vec![harness_index(fixture_snapshot(CLAUDE))];
    let resolution = launch::resolve(
        &request,
        std::path::Path::new("/nowhere"),
        &[CLAUDE],
        &index,
    )
    .unwrap();
    let mut init = init(&[CLAUDE], theme("default"));
    init.warnings.extend(launch_warnings(&resolution));
    let mut app = App::new(init, EventListenerCfg::default(), PathBuf::from("/nowhere")).unwrap();
    start_with(
        &mut app,
        Startup {
            resolution,
            index,
            open: None,
        },
    );
    assert_eq!(
        app.toast_text(),
        Some("--project: no project matches \"nope\"")
    );
    // The first (most recently active) project, as without arguments.
    assert_eq!(
        app.selected_project().map(|p| p.as_str()),
        Some("/home/jacob/Development/projects/Agentmon")
    );
}

#[test]
fn the_startup_warnings_show_before_the_session_opens() {
    // No harness or session channel is set, so the session cannot open: its
    // toast comes after the warnings' and replaces it.
    let mut init = init(&[CLAUDE], theme("default"));
    init.warnings.push(crate::config::ConfigWarning {
        key: "k".to_owned(),
        message: "unknown key \"x\"".to_owned(),
    });
    let mut app = App::new(init, EventListenerCfg::default(), PathBuf::from("/nowhere")).unwrap();
    assert_eq!(app.toast_text(), None, "nothing is shown before the start");
    let summary = session(
        &brio(),
        BRIO_SESSION,
        "Investigate the camera profiles",
        at(2026, 10, 21, 8, 35, 15),
        at(2026, 10, 21, 9, 35, 15),
    );
    let mut startup = selector_start(Preselect::default(), Vec::new());
    startup.open = Some((CLAUDE, summary));
    start_with(&mut app, startup);
    assert_eq!(app.toast_text(), Some("Cannot open the session"));
}

#[test]
fn the_working_directory_rule_is_used_without_a_project() {
    let mut app = empty_app_in(
        &[CLAUDE],
        theme("default"),
        "/home/jacob/Development/projects/BRIO/src",
    );
    start_with(
        &mut app,
        selector_start(
            Preselect::default(),
            vec![harness_index(fixture_snapshot(CLAUDE))],
        ),
    );
    assert_eq!(
        app.selected_project().map(|p| p.as_str()),
        Some("/home/jacob/Development/projects/BRIO")
    );
}

#[test]
fn a_harness_preselects_its_tab_when_it_reports_after_another() {
    // The working directory is a project of Claude's, but --harness opencode
    // wants the OpenCode tab, whichever snapshot arrives first.
    let dir = "/home/jacob/Development/projects/BRIO";
    for opencode_first in [false, true] {
        let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), dir);
        let preselect = Preselect {
            harness: Some(OPENCODE),
            project: None,
        };
        start_with(&mut app, selector_start(preselect, Vec::new()));
        let mut snapshots = [fixture_snapshot(CLAUDE), fixture_snapshot(OPENCODE)];
        if opencode_first {
            snapshots.reverse();
        }
        for snapshot in snapshots {
            send(&mut app, snapshot);
        }
        assert_eq!(app.selected_harness(), Some(OPENCODE), "{opencode_first}");
        assert_eq!(
            app.selected_project().map(|p| p.as_str()),
            Some("/home/jacob/Development/projects/BRIO"),
            "the working directory still picks the project"
        );
    }
}

#[test]
fn an_unavailable_harness_is_not_preselected() {
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/nowhere");
    let preselect = Preselect {
        harness: Some(OPENCODE),
        project: None,
    };
    start_with(&mut app, selector_start(preselect, Vec::new()));
    send(&mut app, fixture_snapshot(CLAUDE));
    send(
        &mut app,
        snapshot_event(OPENCODE, false, Vec::new(), SessionMap::new()),
    );
    assert_eq!(app.selected_harness(), Some(CLAUDE));
}

// -- --config and --theme

#[test]
fn choosing_a_theme_saves_it_to_the_config_file_given_with_config() {
    use crate::app::AppInit;
    use crate::paths::Paths;
    use crate::theme::TerminalEnv;

    let dir = tempfile::tempdir().unwrap();
    let default_config = dir.path().join("config/chronoscope/config.toml");
    let other = dir.path().join("mine.toml");
    std::fs::write(&other, "utc = true\n").unwrap();
    let paths = Paths::from_dirs(
        &dir.path().join("config"),
        &dir.path().join("cache"),
        None,
        dir.path(),
    )
    .with_config_file(other.clone());
    let env = TerminalEnv {
        colorterm: Some("truecolor".to_owned()),
        no_color: false,
    };
    let init = AppInit::load(Some(&paths), &env);
    let mut rig = build_rig_from(init, 120, 40, false);
    rig.open_themes();
    rig.app.update(Msg::SelectTheme("nord".to_owned()));
    let saved = std::fs::read_to_string(&other).unwrap();
    assert!(saved.contains("theme = \"nord\""), "{saved}");
    assert!(saved.contains("utc = true"), "the rest is kept: {saved}");
    assert!(!default_config.exists(), "the default file is not touched");
}

#[test]
fn a_theme_given_for_the_run_is_applied_without_writing() {
    use crate::app::AppInit;
    use crate::paths::Paths;
    use crate::theme::TerminalEnv;

    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("c.toml");
    std::fs::write(&config, "theme = \"nord\"\n").unwrap();
    let paths = Paths::from_dirs(
        &dir.path().join("config"),
        &dir.path().join("cache"),
        None,
        dir.path(),
    )
    .with_config_file(config.clone());
    let env = TerminalEnv {
        colorterm: Some("truecolor".to_owned()),
        no_color: false,
    };
    let init = AppInit::load_with_theme(Some(&paths), &env, Some("gruvbox"));
    let mut rig = build_rig_from(init, 120, 40, false);
    assert_eq!(rig.app.theme_id(), "gruvbox");
    assert_eq!(rig.app.theme().name(), "Gruvbox");
    assert_eq!(
        std::fs::read_to_string(&config).unwrap(),
        "theme = \"nord\"\n",
        "starting with --theme writes nothing"
    );
    // Choosing a theme is still saved, to the file in use.
    rig.open_themes();
    rig.app.update(Msg::SelectTheme("solarized".to_owned()));
    let saved = std::fs::read_to_string(&config).unwrap();
    assert!(saved.contains("theme = \"solarized\""), "{saved}");
}

// -- the options of the command line reach the app

fn truecolor_env() -> crate::theme::TerminalEnv {
    crate::theme::TerminalEnv {
        colorterm: Some("truecolor".to_owned()),
        no_color: false,
    }
}

/// Detected locations in `dir`, as `Paths::detect` would give them.
fn detected_paths(dir: &std::path::Path) -> crate::paths::Paths {
    crate::paths::Paths::from_dirs(&dir.join("config"), &dir.join("cache"), None, dir)
}

fn cli(args: &[&str]) -> crate::cli::Cli {
    crate::cli::parse_args(std::iter::once("chronoscope").chain(args.iter().copied())).unwrap()
}

#[test]
fn the_config_option_makes_the_app_load_and_save_that_file() {
    let dir = tempfile::tempdir().unwrap();
    let mine = dir.path().join("mine.toml");
    std::fs::write(&mine, "theme = \"nord\"\n").unwrap();
    let paths = detected_paths(dir.path());
    let args = cli(&["--config", mine.to_str().unwrap()]);
    let prepared = prepare(&args, Some(&paths), &truecolor_env(), dir.path()).unwrap();
    assert_eq!(prepared.init.theme_id, "nord");
    assert_eq!(prepared.init.config_path.as_deref(), Some(mine.as_path()));
    // Without the option the default location is used.
    let prepared = prepare(&cli(&[]), Some(&paths), &truecolor_env(), dir.path()).unwrap();
    assert_eq!(prepared.init.theme_id, "default");
    assert_eq!(
        prepared.init.config_path.as_deref(),
        Some(paths.config_file.as_path())
    );
}

#[test]
fn the_theme_option_makes_the_app_show_that_theme_and_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mine = dir.path().join("mine.toml");
    std::fs::write(&mine, "theme = \"nord\"\n").unwrap();
    let paths = detected_paths(dir.path());
    let args = cli(&["--config", mine.to_str().unwrap(), "--theme", "gruvbox"]);
    let prepared = prepare(&args, Some(&paths), &truecolor_env(), dir.path()).unwrap();
    assert_eq!(prepared.init.theme_id, "gruvbox");
    assert_eq!(prepared.init.theme.name(), "Gruvbox");
    assert!(prepared.init.warnings.is_empty());
    assert_eq!(
        std::fs::read_to_string(&mine).unwrap(),
        "theme = \"nord\"\n"
    );
    assert!(!paths.config_file.exists());
    // An unknown theme warns.
    let args = cli(&["--theme", "nope"]);
    let prepared = prepare(&args, Some(&paths), &truecolor_env(), dir.path()).unwrap();
    assert_eq!(prepared.init.theme_id, "default");
    assert_eq!(prepared.init.warnings.len(), 1);
}

#[test]
fn the_config_file_decides_which_harnesses_a_path_may_use() {
    let dir = tempfile::tempdir().unwrap();
    let mine = dir.path().join("mine.toml");
    std::fs::write(&mine, "[harness.claude]\nenabled = false\n").unwrap();
    let file = dir.path().join("1234.jsonl");
    std::fs::write(&file, LOOSE_LINE).unwrap();
    let paths = detected_paths(dir.path());
    let args = cli(&["--config", mine.to_str().unwrap(), file.to_str().unwrap()]);
    let err = prepare(&args, Some(&paths), &truecolor_env(), dir.path())
        .err()
        .expect("an error");
    assert_eq!(
        err.to_string(),
        "the Claude harness is disabled in the config; cannot open a .jsonl file"
    );
}

#[test]
fn the_config_option_without_a_home_directory_warns() {
    let dir = tempfile::tempdir().unwrap();
    let args = cli(&["--config", dir.path().join("x.toml").to_str().unwrap()]);
    let prepared = prepare(&args, None, &truecolor_env(), dir.path()).unwrap();
    let messages: Vec<&str> = prepared
        .init
        .warnings
        .iter()
        .map(|w| w.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|m| m.starts_with("--config:")),
        "{messages:?}"
    );
}

// -- a path

const LOOSE_LINE: &str = concat!(
    r#"{"type":"user","cwd":"/home/user/loose","sessionId":"0a0b0c0d","uuid":"u1","#,
    r#""timestamp":"2026-04-01T10:00:00.000Z","message":{"role":"user","content":"Hello there"}}"#,
    "\n"
);

/// A Claude harness over a data directory that holds one session of the
/// project `/home/user/loose` (shown as "Loose Alias"), and the path of
/// another transcript of that directory, outside the data directory.
fn loose_harnesses(
    dir: &tempfile::TempDir,
) -> (Vec<std::sync::Arc<dyn Harness>>, std::path::PathBuf) {
    use crate::harness::claude::ClaudeHarness;

    let data = dir.path().join("data/-home-user-loose");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("11111111.jsonl"), LOOSE_LINE).unwrap();
    let file = dir.path().join("elsewhere/0a0b0c0d.jsonl");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, LOOSE_LINE).unwrap();
    let aliases = [(
        std::path::PathBuf::from("/home/user/loose"),
        "Loose Alias".to_owned(),
    )]
    .into_iter()
    .collect();
    let harness = ClaudeHarness::with_data_dir(dir.path().join("data"), aliases);
    (vec![std::sync::Arc::new(harness)], file)
}

#[test]
fn a_path_outside_the_data_directory_gives_a_claude_session_to_open() {
    let dir = tempfile::tempdir().unwrap();
    let (harnesses, file) = loose_harnesses(&dir);
    let startup = resolve_startup(
        &request(file.to_str().unwrap()),
        std::path::Path::new("/nowhere"),
        &crate::config::Config::default(),
        &harnesses,
    )
    .unwrap();
    let (harness, summary) = startup.open.expect("a session to open");
    assert_eq!(harness, CLAUDE);
    assert_eq!(summary.id, id("0a0b0c0d"));
    assert_eq!(summary.project.as_str(), "/home/user/loose");
    assert_eq!(
        summary.cwd.as_deref(),
        Some(std::path::Path::new("/home/user/loose"))
    );
    assert_eq!(summary.title, "Hello there");
    assert_eq!(summary.path, file);
    // The index is read for a path too, so the header can name the project.
    assert_eq!(startup.index.len(), 1);
    assert_eq!(startup.index[0].projects[0].name, "Loose Alias");
}

#[test]
fn a_session_opened_by_path_shows_the_projects_display_name() {
    let dir = tempfile::tempdir().unwrap();
    let (harnesses, file) = loose_harnesses(&dir);
    let startup = resolve_startup(
        &request(file.to_str().unwrap()),
        std::path::Path::new("/nowhere"),
        &crate::config::Config::default(),
        &harnesses,
    )
    .unwrap();
    let mut rig = rig();
    start_with(&mut rig.app, startup);
    assert_eq!(rig.app.open_session_id(), Some((CLAUDE, &id("0a0b0c0d"))));
    assert_eq!(rig.app.open_project_name().as_deref(), Some("Loose Alias"));
}

#[test]
fn errors_of_the_command_line_are_reported_before_anything_starts() {
    let harnesses: Vec<std::sync::Arc<dyn Harness>> = Vec::new();
    let err = resolve_startup(
        &request("/no/such/file.jsonl"),
        std::path::Path::new("/nowhere"),
        &crate::config::Config::default(),
        &harnesses,
    )
    .expect_err("an error");
    assert_eq!(err.to_string(), "no such file: /no/such/file.jsonl");
    let err = resolve_startup(
        &request("zzz"),
        std::path::Path::new("/nowhere"),
        &crate::config::Config::default(),
        &harnesses,
    )
    .expect_err("an error");
    assert_eq!(err.to_string(), "no session matches \"zzz\"");
}
