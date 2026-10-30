//! The OpenCode harness through the app (task 028): the selector with both
//! tabs, and an OpenCode session in the browser. The real adapter reads the
//! fixture database (`tests/fixtures/opencode/fixture.sql`); the Claude side
//! is the selector's fixed index data.

use super::*;
use crate::components::transcript::Presentation;
use crate::components::{PRESENTATION_ATTR, SectionKey};
use crate::harness::opencode::OpenCodeHarness;
use crate::harness::opencode::testing::TempDb;
use crate::model::ItemId;

use super::super::selector_tests::{press as press_in_selector, project_rows, styled, tabs};

pub(super) struct OpenCodeRig {
    pub(super) rig: Rig,
    pub(super) harness: Arc<OpenCodeHarness>,
    _db: TempDb,
}

/// An app with a Claude tab (the fixed fixture index) and an OpenCode tab
/// (the fixture database, read by the real adapter), 120x40.
pub(super) fn two_tabs() -> OpenCodeRig {
    let db = TempDb::new();
    let harness = Arc::new(OpenCodeHarness::with_db_path(
        db.path.clone(),
        Default::default(),
    ));
    let (sink, events) = session_channel();
    let (keys, key_events) = channel();
    let listener = EventListenerCfg::default()
        .add_port(
            Box::new(SessionPort::new(events)),
            Duration::from_millis(2),
            SESSION_MAX_PER_POLL,
        )
        .add_port(Box::new(KeyPort(key_events)), Duration::from_millis(2), 4);
    let mut app = App::new(
        init(&[CLAUDE, HarnessId::OpenCode], theme("default")),
        listener,
        PathBuf::from("/nowhere"),
    )
    .unwrap();
    let clock = Arc::new(Mutex::new(now()));
    let shown = Arc::clone(&clock);
    app.set_clock(move || *shown.lock().unwrap());
    app.update(Msg::Resized(120, 40));
    let claude = FakeHarness::new();
    app.set_harnesses([
        Arc::clone(&claude) as Arc<dyn Harness>,
        Arc::clone(&harness) as Arc<dyn Harness>,
    ]);
    app.set_session_sink(sink);
    app.update(fixtures::message(fixtures::fixture_snapshot(CLAUDE)));
    app.update(fixtures::message(opencode_snapshot(&harness)));
    for event in opencode_stats(&harness) {
        app.update(fixtures::message(event));
    }
    OpenCodeRig {
        rig: Rig {
            app,
            harness: claude,
            keys,
            clock,
        },
        harness,
        _db: db,
    }
}

/// What the index worker would send for the OpenCode harness.
fn opencode_snapshot(harness: &OpenCodeHarness) -> crate::index::IndexEvent {
    let projects = harness.projects().unwrap();
    let mut sessions = SessionMap::new();
    for project in &projects {
        sessions.insert(project.id.clone(), harness.sessions(&project.id).unwrap());
    }
    snapshot_event(HarnessId::OpenCode, true, projects, sessions)
}

/// The stats events the index worker would send for the harness's sessions.
fn opencode_stats(harness: &OpenCodeHarness) -> Vec<crate::index::IndexEvent> {
    let mut events = Vec::new();
    for project in harness.projects().unwrap() {
        for session in harness.sessions(&project.id).unwrap() {
            events.push(crate::index::IndexEvent::Stats {
                harness: HarnessId::OpenCode,
                session: session.id.clone(),
                stats: harness.stats(&session).unwrap(),
            });
        }
    }
    events
}

fn presentation(app: &App) -> Presentation {
    let value = app
        .application()
        .query(&Id::Transcript, PRESENTATION_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

impl OpenCodeRig {
    /// Opens `session` and waits until it is loaded.
    fn open(&mut self, session: &str) {
        self.rig
            .app
            .update(Msg::OpenSession(HarnessId::OpenCode, id(session)));
        self.rig.pump_until(App::session_loaded);
        self.rig.settle();
    }

    /// Selects the section of tool call `call`, from the top.
    fn select_call(&mut self, call: &str) {
        self.rig.press(Key::Char('g'));
        self.rig.settle();
        for _ in 0..30 {
            let selected = presentation(&self.rig.app).selected;
            if selected.is_some_and(|s| s.key == SectionKey::Item(ItemId::from(call))) {
                return;
            }
            self.rig.press(Key::Down);
            self.rig.settle();
        }
        panic!("never selected {call}");
    }
}

// -- the selector

#[test]
fn both_tabs_show_and_two_switches_to_opencode_with_its_projects_and_sessions() {
    let mut rig = two_tabs();
    let app = &mut rig.rig.app;
    assert!(text(app, 120, 40).contains("[1] Claude  [2] OpenCode"));
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    press_in_selector(app, Key::Char('2'));
    assert_eq!(app.selected_harness(), Some(HarnessId::OpenCode));
    assert_eq!(tabs(app).selected, Some(HarnessId::OpenCode));
    let rows = project_rows(app);
    let names: Vec<&str> = rows.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Alpha App", "beta-tool"]);
    let sessions = session_rows(app);
    assert_eq!(sessions.project, "Alpha App");
    let titles: Vec<&str> = sessions.rows.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles, ["Add the settings screen", "Rename the config key"]);
    assert_eq!(rig.harness.projects().unwrap().len(), 2);
}

#[test]
fn the_stats_and_the_active_marker_come_from_the_database() {
    let mut rig = two_tabs();
    let app = &mut rig.rig.app;
    press_in_selector(app, Key::Char('2'));
    let rows = session_rows(app);
    let stats: Vec<_> = rows
        .rows
        .iter()
        .map(|r| r.stats.as_ref().map(|s| (s.turns, s.duration.as_secs())))
        .collect();
    assert_eq!(stats, [Some((2, 1800)), Some((1, 600))]);
    // `ses_main` and `ses_untitled` are idle and long past.
    assert!(rows.rows.iter().all(|r| !r.active));
    // In beta-tool, `ses_long` has no `time_idle`: the session is running.
    press_in_selector(app, Key::Char('j'));
    let rows = session_rows(app);
    let ids: Vec<&str> = rows.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["ses_bad", "ses_long", "ses_blank"]);
    let active: Vec<bool> = rows.rows.iter().map(|r| r.active).collect();
    assert_eq!(active, [false, true, false]);
}

#[test]
fn selector_with_opencode_selected_120x40_text() {
    let mut rig = two_tabs();
    press_in_selector(&mut rig.rig.app, Key::Char('2'));
    insta::assert_snapshot!(text(&mut rig.rig.app, 120, 40));
}

#[test]
fn selector_with_opencode_selected_120x40_styled() {
    let mut rig = two_tabs();
    press_in_selector(&mut rig.rig.app, Key::Char('2'));
    insta::assert_snapshot!(styled(&mut rig.rig.app, 120, 40));
}

/// Claude selected, OpenCode unavailable.
fn unavailable_opencode_app() -> App {
    let mut app = super::super::selector_tests::empty_app_in(
        &[CLAUDE, HarnessId::OpenCode],
        theme("default"),
        "/nowhere",
    );
    super::super::selector_tests::send(&mut app, fixtures::fixture_snapshot(CLAUDE));
    super::super::selector_tests::send(
        &mut app,
        snapshot_event(HarnessId::OpenCode, false, Vec::new(), SessionMap::new()),
    );
    app
}

#[test]
fn an_unavailable_opencode_is_muted_and_cannot_be_selected_80x24_text() {
    let mut app = unavailable_opencode_app();
    press_in_selector(&mut app, Key::Char('2'));
    assert_eq!(app.selected_harness(), Some(CLAUDE), "cannot be selected");
    let all = tabs(&app);
    assert_eq!(all.selected, Some(CLAUDE));
    assert!(
        all.tabs
            .iter()
            .any(|t| t.id == HarnessId::OpenCode && !t.available)
    );
    let screen = text(&mut app, 80, 24);
    assert!(screen.contains("[1] Claude  [2] OpenCode"), "{screen}");
    insta::assert_snapshot!(screen);
}

#[test]
fn an_unavailable_opencode_tab_is_muted_foreground_80x24_styled() {
    let mut app = unavailable_opencode_app();
    let buffer = super::super::selector_tests::screen(&mut app, 80, 24);
    let theme = theme("default");
    // Tabs at column 5 (row 3 text, row 4 underline); `[2] OpenCode` starts
    // two spaces after `[1] Claude` (10 cells).
    let muted = theme.role(Role::MutedForeground);
    for (i, ch) in "[2] OpenCode".chars().enumerate() {
        let cell = &buffer[(5 + 12 + i as u16, 3)];
        assert_eq!(cell.symbol(), ch.to_string());
        assert_eq!(cell.fg, muted, "cell {i}");
    }
    let primary = theme.role(Role::Primary);
    assert_eq!(buffer[(5, 3)].fg, primary, "the selected tab is primary");
    assert_eq!(buffer[(5, 4)].symbol(), "━");
    // No underline under the unavailable tab.
    assert_eq!(buffer[(5 + 12, 4)].symbol(), " ");
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}

// -- the browser

#[test]
fn an_opencode_session_opens_in_the_browser_and_renders_its_turns() {
    let mut rig = two_tabs();
    press_in_selector(&mut rig.rig.app, Key::Char('2'));
    rig.open("ses_main");
    assert_eq!(rig.rig.app.screen(), Screen::Browser);
    let open = rig.rig.app.open_session_id().unwrap();
    assert_eq!((open.0, open.1.as_str()), (HarnessId::OpenCode, "ses_main"));
    assert_eq!(turn_count(&rig.rig.app), 4);
    let screen = screen_text(&mut rig.rig, 120, 40);
    assert!(
        screen.contains("Add a settings screen with a dark mode toggle."),
        "{screen}"
    );
    assert!(screen.contains("Assistant"), "{screen}");
    // The Claude harness was never asked to load it.
    assert!(rig.rig.harness.loads().is_empty());
}

#[test]
fn a_non_default_variant_is_shown_in_the_title_and_default_is_not() {
    let mut rig = two_tabs();
    rig.open("ses_main");
    let screen = screen_text(&mut rig.rig, 120, 40);
    assert!(
        screen.contains("Assistant ── acme/coder-1 @ high"),
        "{screen}"
    );
    // The second Assistant turn has variant `default`.
    rig.rig.press(Key::Char('G'));
    rig.rig.settle();
    let screen = screen_text(&mut rig.rig, 120, 40);
    assert!(screen.contains("Assistant ── acme/coder-1 ─"), "{screen}");
    assert!(!screen.contains("@ default"), "{screen}");
}

#[test]
fn an_opencode_session_in_the_browser_with_a_shell_tool_unfolded_120x40_text() {
    let mut rig = two_tabs();
    rig.open("ses_main");
    rig.select_call("call-shell-1");
    rig.rig.press(Key::Enter);
    rig.rig.settle();
    let screen = screen_text(&mut rig.rig, 120, 40);
    assert!(screen.contains("main.rs"), "{screen}");
    assert!(screen.contains("acme/coder-1 @ high"), "{screen}");
    insta::assert_snapshot!(screen);
}

#[test]
fn an_unfolded_opencode_read_shows_the_file_in_a_gutter_without_the_framing() {
    let mut rig = two_tabs();
    rig.open("ses_main");
    rig.select_call("call-read-1");
    rig.rig.press(Key::Enter);
    rig.rig.settle();
    let screen = screen_text(&mut rig.rig, 120, 60);
    let content: Vec<&str> = screen
        .lines()
        .skip_while(|l| !l.contains("Content:"))
        .skip(2)
        .take(1)
        .collect();
    assert_eq!(content.len(), 1, "{screen}");
    // `1  fn main() {}`: the number is in the gutter, not repeated as `1:`,
    // and the `Read file ...` line of the result is not content.
    assert!(content[0].contains(" 1  fn main() {}"), "{}", content[0]);
    assert!(!screen.contains("Read file /home"), "{screen}");
    assert!(!screen.contains("1: fn main"), "{screen}");
}

#[test]
fn a_subagent_section_of_an_opencode_session_opens_the_child_transcript() {
    let mut rig = two_tabs();
    rig.open("ses_main");
    rig.select_call("call-sub-1");
    rig.rig.press(Key::Char('a'));
    rig.rig
        .pump_until(|app| app.subagent_depth() == 1 && app.session_loaded());
    rig.rig.settle();
    assert_eq!(
        rig.rig.app.top_session_id().map(|s| s.as_str()),
        Some("ses_child1")
    );
    assert_eq!(turn_count_at(&rig.rig.app, Id::Subagent(1)), 2);
    let screen = screen_text(&mut rig.rig, 120, 40);
    assert!(screen.contains("Look around the repository"), "{screen}");
    assert!(
        screen.contains("Two crates: a lib and a binary."),
        "{screen}"
    );
}

fn turn_count_at(app: &App, id: Id) -> usize {
    match app
        .application()
        .query(&id, TURN_COUNT_ATTR)
        .unwrap()
        .unwrap()
        .into_attr()
    {
        AttrValue::Length(n) => n,
        other => panic!("{other:?}"),
    }
}
