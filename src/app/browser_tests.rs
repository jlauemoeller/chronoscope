//! Update tests of the session browser (no terminal, a fake harness whose
//! `load` sends scripted updates) and the screen snapshots of `App::view`.
//!
//! Everything that depends on the subscriptions of `GlobalEvents` (`Ctrl-o`,
//! `q` in the search prompt, the session updates) is driven through
//! `App::tick`: keys and worker updates are delivered by ports, so they take
//! the same path as in the running application.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ratatui::layout::Size;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::listener::{EventListenerCfg, Poll, PortResult};
use tuirealm::props::AttrValue;

use super::fixtures::{self, at, now, project_in, session, snapshot_event};
use super::layout::Screen;
use super::selector_tests::{CLAUDE, init, session_rows, text, theme};
use super::session_port::{SESSION_MAX_PER_POLL, SessionPort};
use super::{App, Id, Msg, SessionUpdate, UserEvent, session_channel};
use crate::components::transcript::fixtures::session as fixture_turns;
use crate::components::{
    PARSE_WARNINGS_ATTR, STATUS_ATTR, TURN_COUNT_ATTR, VIEWPORT_ATTR, payload_from_value,
};
use crate::harness::{Harness, Result as HarnessResult, WatchHandle};
use crate::index::SessionMap;
use crate::model::{
    HarnessId, ModelUpdate, Project, ProjectId, SessionId, SessionStats, SessionSummary,
    ToolCallId, Turn,
};
use crate::test_support::{buffer_to_styled, buffer_to_text, render};
use crate::theme::{Role, Theme};

const BRIO_SESSION: &str = "85a86657-11a7-4318-a21a-0f4476babd5c";
const NEWER_SESSION: &str = "b2b2b2b2-0000-4000-8000-000000000002";
const OTHER_PROJECT_SESSION: &str = "c3c3c3c3-0000-4000-8000-000000000003";

// -- a fake harness

/// A harness whose `load` sends scripted updates at once. It keeps every
/// sender, so a test can send more updates later, as a live session would.
struct FakeHarness {
    scripts: Mutex<HashMap<SessionId, Vec<ModelUpdate>>>,
    loads: Mutex<Vec<SessionId>>,
    stops: Mutex<Vec<Arc<AtomicBool>>>,
    senders: Mutex<Vec<Sender<ModelUpdate>>>,
    fail: AtomicBool,
}

impl FakeHarness {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            scripts: Mutex::default(),
            loads: Mutex::default(),
            stops: Mutex::default(),
            senders: Mutex::default(),
            fail: AtomicBool::new(false),
        })
    }

    fn script(&self, session: &str, updates: Vec<ModelUpdate>) {
        self.scripts
            .lock()
            .unwrap()
            .insert(SessionId::from(session), updates);
    }

    fn loads(&self) -> Vec<String> {
        let loads = self.loads.lock().unwrap();
        loads.iter().map(|id| id.as_str().to_owned()).collect()
    }

    fn stopped(&self, load: usize) -> bool {
        self.stops.lock().unwrap()[load].load(Ordering::Relaxed)
    }

    /// Sends `update` as the worker of load number `load` (from 0).
    fn send(&self, load: usize, update: ModelUpdate) {
        self.senders.lock().unwrap()[load].send(update).unwrap();
    }
}

impl Harness for FakeHarness {
    fn id(&self) -> HarnessId {
        CLAUDE
    }

    fn available(&self) -> bool {
        true
    }

    fn refresh(&self) -> HarnessResult<()> {
        Ok(())
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    fn projects(&self) -> HarnessResult<Vec<Project>> {
        Ok(Vec::new())
    }

    fn sessions(&self, _project: &ProjectId) -> HarnessResult<Vec<SessionSummary>> {
        Ok(Vec::new())
    }

    fn stats(&self, _session: &SessionSummary) -> HarnessResult<SessionStats> {
        anyhow::bail!("not used")
    }

    fn stats_key(&self, _session: &SessionSummary) -> String {
        String::new()
    }

    fn load(&self, session: &SessionId, tx: Sender<ModelUpdate>) -> HarnessResult<WatchHandle> {
        if self.fail.load(Ordering::Relaxed) {
            anyhow::bail!("the file is gone");
        }
        self.loads.lock().unwrap().push(session.clone());
        let script = self.scripts.lock().unwrap().get(session).cloned();
        for update in script.unwrap_or_default() {
            tx.send(update).unwrap();
        }
        self.senders.lock().unwrap().push(tx);
        let stop = Arc::new(AtomicBool::new(false));
        self.stops.lock().unwrap().push(Arc::clone(&stop));
        Ok(WatchHandle::new(stop, std::thread::spawn(|| {})))
    }

    fn subagent(&self, _parent: &SessionId, _call: &ToolCallId) -> Option<SessionId> {
        None
    }

    fn resume_command(&self, _session: &SessionSummary) -> Option<std::process::Command> {
        None
    }
}

// -- a port that delivers the keys a test sends

struct KeyPort(Receiver<Event<UserEvent>>);

impl Poll<UserEvent> for KeyPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        Ok(self.0.try_recv().ok())
    }
}

/// The app, its fake harness and the way to type into it.
struct Rig {
    app: App,
    harness: Arc<FakeHarness>,
    keys: Sender<Event<UserEvent>>,
}

fn brio() -> Project {
    project_in(
        CLAUDE,
        "BRIO",
        "/home/user/projects/brio",
        at(2026, 10, 28, 9, 30, 0),
    )
}

fn agentmon() -> Project {
    project_in(
        CLAUDE,
        "Agentmon",
        "/home/user/projects/agentmon",
        at(2026, 10, 27, 9, 30, 0),
    )
}

/// The index: BRIO with two sessions (the newest first) and Agentmon with
/// one. Nothing is active at [`now`].
fn index() -> crate::index::IndexEvent {
    let brio = brio();
    let agentmon = agentmon();
    let mut sessions = SessionMap::new();
    sessions.insert(
        brio.id.clone(),
        vec![
            session(
                &brio,
                NEWER_SESSION,
                "Add the pan and tilt controls",
                at(2026, 10, 28, 9, 0, 0),
                at(2026, 10, 28, 9, 30, 0),
            ),
            session(
                &brio,
                BRIO_SESSION,
                "Investigate the camera profiles",
                at(2026, 10, 21, 8, 35, 15),
                at(2026, 10, 21, 9, 35, 15),
            ),
        ],
    );
    sessions.insert(
        agentmon.id.clone(),
        vec![session(
            &agentmon,
            OTHER_PROJECT_SESSION,
            "Tidy the watcher",
            at(2026, 10, 27, 9, 0, 0),
            at(2026, 10, 27, 9, 30, 0),
        )],
    );
    snapshot_event(CLAUDE, true, vec![brio, agentmon], sessions)
}

fn rig_with(theme: Theme, width: u16, height: u16) -> Rig {
    let (sink, events) = session_channel();
    let (keys, key_events) = channel();
    let listener = EventListenerCfg::default()
        .add_port(
            Box::new(SessionPort::new(events)),
            Duration::from_millis(2),
            SESSION_MAX_PER_POLL,
        )
        .add_port(Box::new(KeyPort(key_events)), Duration::from_millis(2), 4);
    let mut app = App::new(init(&[CLAUDE], theme), listener, PathBuf::from("/nowhere")).unwrap();
    app.set_clock(now);
    app.update(Msg::Resized(width, height));
    let harness = FakeHarness::new();
    app.set_harnesses([Arc::clone(&harness) as Arc<dyn Harness>]);
    app.set_session_sink(sink);
    app.update(fixtures::message(index()));
    Rig { app, harness, keys }
}

fn rig() -> Rig {
    rig_with(theme("default"), 120, 40)
}

fn id(session: &str) -> SessionId {
    SessionId::from(session)
}

fn turns() -> Vec<Arc<Turn>> {
    fixture_turns()
}

impl Rig {
    /// Ticks the app and applies the messages until `done` or two seconds
    /// pass. Returns every message seen.
    fn pump_until(&mut self, done: impl Fn(&App) -> bool) -> Vec<Msg> {
        let mut seen = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !done(&self.app) && Instant::now() < deadline {
            for msg in self.app.tick().unwrap() {
                seen.push(msg.clone());
                self.app.update(msg);
            }
        }
        assert!(done(&self.app), "the app did not get there; saw {seen:?}");
        seen
    }

    /// Ticks the app for a short time, so that whatever was sent has
    /// arrived.
    fn settle(&mut self) -> Vec<Msg> {
        let mut seen = Vec::new();
        let until = Instant::now() + Duration::from_millis(150);
        while Instant::now() < until {
            for msg in self.app.tick().unwrap() {
                seen.push(msg.clone());
                self.app.update(msg);
            }
        }
        seen
    }

    fn key(&self, code: Key, modifiers: KeyModifiers) {
        self.keys
            .send(Event::Keyboard(KeyEvent::new(code, modifiers)))
            .unwrap();
    }

    fn press(&self, code: Key) {
        self.key(code, KeyModifiers::NONE);
    }

    fn ctrl_o(&self) {
        self.key(Key::Char('o'), KeyModifiers::CONTROL);
    }

    /// Opens `session` and waits for its script to be loaded.
    fn open(&mut self, session: &str) {
        self.app.update(Msg::OpenSession(CLAUDE, id(session)));
        self.pump_until(App::session_loaded);
    }

    /// Opens the BRIO session with the fixture turns, fully loaded.
    fn open_loaded(&mut self) {
        self.harness.script(
            BRIO_SESSION,
            vec![
                ModelUpdate::Appended(turns()),
                ModelUpdate::Progress(1.0),
                ModelUpdate::ParseWarnings(0),
                ModelUpdate::Loaded,
            ],
        );
        self.open(BRIO_SESSION);
    }

    fn focus(&self) -> Option<Id> {
        self.app.application().focus().copied()
    }

    fn turn_count(&self) -> usize {
        turn_count(&self.app)
    }
}

fn turn_count(app: &App) -> usize {
    match app
        .application()
        .query(&Id::Transcript, TURN_COUNT_ATTR)
        .unwrap()
        .unwrap()
        .into_attr()
    {
        AttrValue::Length(n) => n,
        other => panic!("{other:?}"),
    }
}

fn status(app: &App) -> Vec<String> {
    let value = app
        .application()
        .query(&Id::BrowserHelpLine, STATUS_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

fn session_msg(app: &App, update: ModelUpdate) -> Msg {
    Msg::Session {
        generation: app.session_generation(),
        update: SessionUpdate::new(1, update),
    }
}

const SELECTOR_IDS: [Id; 6] = [
    Id::SelectorFrame,
    Id::HarnessTabs,
    Id::ProjectList,
    Id::SessionList,
    Id::WorkingDir,
    Id::HelpLine,
];
const BROWSER_IDS: [Id; 3] = [Id::BrowserHeader, Id::Transcript, Id::BrowserHelpLine];

// -- opening a session

#[test]
fn opening_a_session_mounts_the_browser_unmounts_the_selector_and_starts_a_load() {
    let mut rig = rig();
    for id in SELECTOR_IDS {
        assert!(rig.app.application().mounted(&id), "{id:?}");
    }
    for id in BROWSER_IDS {
        assert!(!rig.app.application().mounted(&id), "{id:?}");
    }
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    for id in BROWSER_IDS {
        assert!(rig.app.application().mounted(&id), "{id:?}");
    }
    for id in SELECTOR_IDS.into_iter().chain([Id::Prompt]) {
        assert!(!rig.app.application().mounted(&id), "{id:?}");
    }
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION]);
    assert_eq!(rig.app.session_generation(), 1);
    let open = rig.app.open_session_id().unwrap();
    assert_eq!((open.0, open.1.as_str()), (CLAUDE, BRIO_SESSION));
}

#[test]
fn a_session_the_index_does_not_list_is_ignored() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id("nope")));
    assert_eq!(rig.app.screen(), Screen::Selector);
    assert!(rig.harness.loads().is_empty());
    assert!(rig.app.application().mounted(&Id::SessionList));
    assert!(!rig.app.application().mounted(&Id::Transcript));
}

#[test]
fn a_load_that_fails_leaves_the_selector_and_shows_a_toast() {
    let mut rig = rig();
    rig.harness.fail.store(true, Ordering::Relaxed);
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    assert_eq!(rig.app.screen(), Screen::Selector);
    assert!(rig.app.application().mounted(&Id::SessionList));
    assert!(
        rig.app
            .toast_text()
            .is_some_and(|t| t.contains("the file is gone")),
        "{:?}",
        rig.app.toast_text()
    );
    assert_eq!(rig.app.session_generation(), 0);
}

#[test]
fn the_transcript_viewport_follows_the_terminal_size() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    let viewport = |app: &App| -> Size {
        let value = app
            .application()
            .query(&Id::Transcript, VIEWPORT_ATTR)
            .unwrap()
            .unwrap()
            .into_attr();
        payload_from_value(&value).unwrap()
    };
    assert_eq!(viewport(&rig.app), Size::new(120, 33));
    rig.app.update(Msg::Resized(80, 24));
    assert_eq!(viewport(&rig.app), Size::new(80, 17));
}

#[test]
fn the_transcript_gets_the_configured_settings() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    let attr = |attr| {
        rig.app
            .application()
            .query(&Id::Transcript, attr)
            .unwrap()
            .unwrap()
            .into_attr()
    };
    let config = rig.app.config().clone();
    assert_eq!(
        attr(crate::components::TRUNCATE_OUTPUT_LINES_ATTR),
        AttrValue::Length(config.truncate_output_lines)
    );
    assert_eq!(
        attr(crate::components::DIM_UNSELECTED_ATTR),
        AttrValue::Flag(config.dim_unselected)
    );
    assert_eq!(
        attr(crate::components::SHOW_SYSTEM_ATTR),
        AttrValue::Flag(config.show_system)
    );
}

// -- the updates of the load

#[test]
fn updates_travel_from_the_port_through_global_events_to_the_transcript() {
    let mut rig = rig();
    let first = turns()[..3].to_vec();
    let rest = turns()[3..].to_vec();
    rig.harness.script(
        BRIO_SESSION,
        vec![
            ModelUpdate::Appended(first),
            ModelUpdate::Progress(0.34),
            ModelUpdate::Appended(rest),
            ModelUpdate::Loaded,
        ],
    );
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    let seen = rig.pump_until(App::session_loaded);
    // Every update came as a Msg made by GlobalEvents from a user event.
    let updates: Vec<&ModelUpdate> = seen
        .iter()
        .filter_map(|m| match m {
            Msg::Session {
                generation: 1,
                update,
            } => Some(update.update()),
            _ => None,
        })
        .collect();
    assert_eq!(updates.len(), 4, "{seen:?}");
    assert!(matches!(updates[0], ModelUpdate::Appended(t) if t.len() == 3));
    assert!(matches!(updates[3], ModelUpdate::Loaded));
    // The transcript holds the turns; the app only knows how many through the
    // component.
    assert_eq!(rig.turn_count(), turns().len());
}

#[test]
fn the_app_keeps_no_copy_of_the_turns_it_forwards() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    let turn = turns()[0].clone();
    assert_eq!(Arc::strong_count(&turn), 1);
    let update = SessionUpdate::new(1, ModelUpdate::Appended(vec![Arc::clone(&turn)]));
    assert_eq!(Arc::strong_count(&turn), 2, "held by the update");
    let msg = Msg::Session {
        generation: rig.app.session_generation(),
        update,
    };
    rig.app.update(msg);
    // The message is gone. What holds the turn now is the transcript.
    assert_eq!(rig.turn_count(), 1);
    assert_eq!(
        Arc::strong_count(&turn),
        2,
        "this handle and the transcript: nothing in the app"
    );
}

#[test]
fn updated_and_reset_reach_the_transcript_too() {
    let mut rig = rig();
    rig.open_loaded();
    let mut changed = (*turns()[0]).clone();
    if let Turn::User(user) = &mut changed {
        user.text = "A changed prompt".to_owned();
    }
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Updated(vec![Arc::new(changed)]),
    ));
    assert_eq!(rig.turn_count(), turns().len());
    let text = text(&mut rig.app, 120, 40);
    assert!(text.contains("A changed prompt"), "{text}");
    rig.app.update(session_msg(&rig.app, ModelUpdate::Reset));
    assert_eq!(rig.turn_count(), 0);
}

#[test]
fn updates_of_an_earlier_load_are_ignored_after_another_session_is_opened() {
    let mut rig = rig();
    rig.harness.script(BRIO_SESSION, vec![ModelUpdate::Loaded]);
    rig.harness.script(
        NEWER_SESSION,
        vec![
            ModelUpdate::Appended(turns()[..2].to_vec()),
            ModelUpdate::Loaded,
        ],
    );
    rig.open(BRIO_SESSION);
    rig.app.update(Msg::OpenSelector);
    rig.app.update(Msg::OpenSession(CLAUDE, id(NEWER_SESSION)));
    assert_eq!(rig.app.session_generation(), 2);
    // The first load's worker is still running: it sends late.
    rig.harness
        .send(0, ModelUpdate::Appended(turns()[2..5].to_vec()));
    rig.harness.send(0, ModelUpdate::ParseWarnings(9));
    rig.pump_until(App::session_loaded);
    rig.settle();
    assert_eq!(rig.turn_count(), 2, "only the second load's turns");
    let header = rig
        .app
        .application()
        .query(&Id::BrowserHeader, PARSE_WARNINGS_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    assert_eq!(header, AttrValue::Length(0));
    // The same holds for a message that reaches `update` directly.
    rig.app.update(Msg::Session {
        generation: 1,
        update: SessionUpdate::new(99, ModelUpdate::Reset),
    });
    assert_eq!(rig.turn_count(), 2);
    // A control: the current generation is applied.
    rig.app.update(Msg::Session {
        generation: 2,
        update: SessionUpdate::new(100, ModelUpdate::Reset),
    });
    assert_eq!(rig.turn_count(), 0);
}

#[test]
fn opening_another_session_stops_the_first_load_and_starts_a_fresh_transcript() {
    let mut rig = rig();
    rig.open_loaded();
    assert_eq!(rig.turn_count(), 8);
    assert!(!rig.harness.stopped(0));
    rig.app.update(Msg::OpenSelector);
    rig.app.update(Msg::OpenSession(CLAUDE, id(NEWER_SESSION)));
    assert!(rig.harness.stopped(0), "the first worker is told to stop");
    assert!(!rig.harness.stopped(1));
    assert_eq!(rig.harness.loads(), [BRIO_SESSION, NEWER_SESSION]);
    assert_eq!(rig.turn_count(), 0, "a fresh transcript");
    assert!(!rig.app.session_loaded());
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.app.screen(), Screen::Browser);
}

#[test]
fn progress_shows_loading_in_the_help_line_and_loaded_removes_it() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    assert!(status(&rig.app).is_empty());
    rig.app
        .update(session_msg(&rig.app, ModelUpdate::Progress(0.34)));
    assert_eq!(status(&rig.app), ["Loading… 34%"]);
    rig.app
        .update(session_msg(&rig.app, ModelUpdate::Progress(0.5)));
    assert_eq!(status(&rig.app), ["Loading… 50%"]);
    rig.app.update(session_msg(&rig.app, ModelUpdate::Loaded));
    assert!(status(&rig.app).is_empty());
    // A late progress does not bring it back.
    rig.app
        .update(session_msg(&rig.app, ModelUpdate::Progress(0.9)));
    assert!(status(&rig.app).is_empty());
}

#[test]
fn the_percentage_is_rounded() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    for (fraction, shown) in [
        (0.0, "0"),
        (0.004, "0"),
        (0.005, "1"),
        (0.336, "34"),
        (1.0, "100"),
    ] {
        rig.app
            .update(session_msg(&rig.app, ModelUpdate::Progress(fraction)));
        assert_eq!(
            status(&rig.app),
            [format!("Loading… {shown}%")],
            "{fraction}"
        );
    }
}

#[test]
fn parse_warnings_reach_the_header() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    rig.app
        .update(session_msg(&rig.app, ModelUpdate::ParseWarnings(3)));
    let value = rig
        .app
        .application()
        .query(&Id::BrowserHeader, PARSE_WARNINGS_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    assert_eq!(value, AttrValue::Length(3));
    let text = text(&mut rig.app, 120, 40);
    assert!(text.contains("3 entries could not be parsed"), "{text}");
}

#[test]
fn a_session_without_turns_is_empty_once_loaded() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    assert!(!rig.app.session_is_empty(), "still loading");
    rig.app.update(session_msg(&rig.app, ModelUpdate::Loaded));
    assert!(rig.app.session_is_empty());
    let text = text(&mut rig.app, 80, 24);
    assert!(text.contains("Session file is empty"), "{text}");
}

#[test]
fn a_session_with_turns_is_not_empty() {
    let mut rig = rig();
    rig.open_loaded();
    assert!(!rig.app.session_is_empty());
    let text = text(&mut rig.app, 80, 24);
    assert!(!text.contains("Session file is empty"));
}

#[test]
fn turns_that_arrive_later_replace_the_empty_state() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    rig.app.update(session_msg(&rig.app, ModelUpdate::Loaded));
    assert!(rig.app.session_is_empty());
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(turns()[..1].to_vec()),
    ));
    assert!(!rig.app.session_is_empty());
    let text = text(&mut rig.app, 80, 24);
    assert!(!text.contains("Session file is empty"));
    assert!(text.contains("User"), "{text}");
}

// -- the selector over the browser

#[test]
fn ctrl_o_opens_the_selector_over_the_browser_on_the_open_session() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    let seen = rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    assert!(seen.contains(&Msg::OpenSelector), "{seen:?}");
    let app = &rig.app;
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!(
        app.selected_project().map(ProjectId::as_str),
        Some("/home/user/projects/brio")
    );
    assert_eq!(
        app.selected_session().map(SessionId::as_str),
        Some(BRIO_SESSION)
    );
    assert_eq!(app.focused_pane(), super::Pane::Sessions);
    assert_eq!(rig.focus(), Some(Id::SessionList));
    for id in SELECTOR_IDS.into_iter().chain(BROWSER_IDS) {
        assert!(app.application().mounted(&id), "{id:?}");
    }
    let rows = session_rows(app);
    assert_eq!(rows.selected, Some(1));
    assert_eq!(rows.rows[1].id.as_str(), BRIO_SESSION);
    // The turns were not touched.
    assert_eq!(rig.turn_count(), 8);
}

#[test]
fn the_selector_remembers_the_session_when_another_project_was_selected_before() {
    let mut rig = rig();
    // The user wandered to Agentmon in the selector before opening BRIO.
    rig.app
        .update(Msg::Selector(super::SelectorMsg::ProjectSelected(
            agentmon().id,
        )));
    rig.open_loaded();
    rig.app.update(Msg::OpenSelector);
    assert_eq!(
        rig.app.selected_project().map(ProjectId::as_str),
        Some("/home/user/projects/brio")
    );
    assert_eq!(
        rig.app.selected_session().map(SessionId::as_str),
        Some(BRIO_SESSION)
    );
}

#[test]
fn esc_closes_the_selector_and_returns_the_focus_to_the_transcript() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.press(Key::Esc);
    let seen = rig.pump_until(|app| app.screen() == Screen::Browser);
    assert!(seen.contains(&Msg::CloseSelector), "{seen:?}");
    assert_eq!(rig.focus(), Some(Id::Transcript));
    for id in SELECTOR_IDS.into_iter().chain([Id::Prompt]) {
        assert!(!rig.app.application().mounted(&id), "{id:?}");
    }
    assert!(!rig.app.should_quit());
    assert_eq!(rig.turn_count(), 8);
    // And the selector can be opened again.
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    assert_eq!(
        rig.app.selected_session().map(SessionId::as_str),
        Some(BRIO_SESSION)
    );
}

#[test]
fn enter_on_another_session_opens_it() {
    let mut rig = rig();
    rig.open_loaded();
    rig.harness.script(
        NEWER_SESSION,
        vec![
            ModelUpdate::Appended(turns()[..2].to_vec()),
            ModelUpdate::Loaded,
        ],
    );
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.press(Key::Up);
    rig.press(Key::Enter);
    rig.pump_until(|app| {
        app.open_session_id()
            .is_some_and(|(_, id)| id.as_str() == NEWER_SESSION)
    });
    rig.pump_until(App::session_loaded);
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert_eq!(rig.app.session_generation(), 2);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION, NEWER_SESSION]);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.turn_count(), 2);
    assert!(rig.harness.stopped(0));
}

#[test]
fn enter_on_the_open_session_just_closes_the_selector() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.press(Key::Enter);
    rig.pump_until(|app| app.screen() == Screen::Browser);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION], "no second load");
    assert_eq!(rig.app.session_generation(), 1);
    assert_eq!(rig.turn_count(), 8);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!rig.harness.stopped(0));
}

#[test]
fn q_typed_in_the_selectors_search_prompt_does_not_quit() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.press(Key::Char('/'));
    rig.pump_until(|app| app.application().mounted(&Id::Prompt));
    rig.press(Key::Char('q'));
    rig.pump_until(|app| app.search_query() == Some("q"));
    rig.settle();
    assert!(!rig.app.should_quit());
    assert_eq!(rig.app.search_query(), Some("q"));
    // Ctrl-o in the prompt is not a global key either.
    rig.ctrl_o();
    rig.settle();
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    // Control: once the prompt is closed, `q` quits again.
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::Prompt));
    assert_eq!(rig.focus(), Some(Id::SessionList));
    rig.press(Key::Char('q'));
    rig.pump_until(App::should_quit);
}

#[test]
fn opening_a_session_from_a_search_result_unmounts_the_prompt_too() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.press(Key::Char('/'));
    rig.pump_until(|app| app.application().mounted(&Id::Prompt));
    for c in "pan".chars() {
        rig.press(Key::Char(c));
    }
    rig.pump_until(|app| !app.search_results().is_empty());
    rig.press(Key::Enter);
    rig.pump_until(|app| {
        app.open_session_id()
            .is_some_and(|(_, id)| id.as_str() == NEWER_SESSION)
    });
    assert!(!rig.app.application().mounted(&Id::Prompt));
    assert_eq!(rig.app.search_query(), None);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.app.screen(), Screen::Browser);
}

#[test]
fn ctrl_o_does_nothing_on_the_startup_selector() {
    let mut rig = rig();
    rig.ctrl_o();
    rig.settle();
    assert_eq!(rig.app.screen(), Screen::Selector);
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    assert!(rig.app.application().mounted(&Id::SessionList));
}

#[test]
fn ctrl_o_with_the_selector_already_open_changes_nothing() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.press(Key::Up);
    rig.ctrl_o();
    rig.settle();
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    assert_eq!(rig.focus(), Some(Id::SessionList));
    assert_eq!(
        rig.app.selected_session().map(SessionId::as_str),
        Some(NEWER_SESSION),
        "the user's move is kept"
    );
    // One Esc is enough to close it.
    rig.press(Key::Esc);
    rig.pump_until(|app| app.screen() == Screen::Browser);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn index_updates_while_the_browser_is_shown_reach_the_selector_when_it_opens() {
    let mut rig = rig();
    rig.open_loaded();
    // A new session appears while the browser is on screen.
    let brio = brio();
    let agentmon = agentmon();
    let mut sessions = SessionMap::new();
    sessions.insert(
        brio.id.clone(),
        vec![
            session(
                &brio,
                "fresh",
                "Brand new",
                at(2026, 10, 29, 9, 0, 0),
                at(2026, 10, 29, 9, 30, 0),
            ),
            session(
                &brio,
                NEWER_SESSION,
                "Add the pan and tilt controls",
                at(2026, 10, 28, 9, 0, 0),
                at(2026, 10, 28, 9, 30, 0),
            ),
            session(
                &brio,
                BRIO_SESSION,
                "Investigate the camera profiles",
                at(2026, 10, 21, 8, 35, 15),
                at(2026, 10, 21, 9, 35, 15),
            ),
        ],
    );
    rig.app.update(fixtures::message(snapshot_event(
        CLAUDE,
        true,
        vec![brio, agentmon],
        sessions,
    )));
    rig.app.update(Msg::Tick);
    rig.app.update(Msg::OpenSelector);
    let rows = session_rows(&rig.app);
    let titles: Vec<&str> = rows.rows.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles[0], "Brand new");
    assert_eq!(rows.selected, Some(2));
    assert_eq!(
        rig.app.selected_session().map(SessionId::as_str),
        Some(BRIO_SESSION)
    );
}

#[test]
fn quit_still_works_in_the_selector_over_the_browser() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.press(Key::Char('q'));
    rig.pump_until(App::should_quit);
}

#[test]
fn shutdown_stops_the_load() {
    let mut rig = rig();
    rig.open_loaded();
    assert!(!rig.harness.stopped(0));
    rig.app.shutdown();
    assert!(rig.harness.stopped(0));
}

// -- screens

fn screen_text(rig: &mut Rig, width: u16, height: u16) -> String {
    text(&mut rig.app, width, height)
}

fn screen_buffer(rig: &mut Rig, width: u16, height: u16) -> ratatui::buffer::Buffer {
    rig.app.update(Msg::Resized(width, height));
    render(width, height, |frame| rig.app.view(frame))
}

fn rows_of(buffer: &ratatui::buffer::Buffer) -> Vec<String> {
    buffer_to_text(buffer).lines().map(str::to_owned).collect()
}

#[test]
fn the_browser_help_line_reads_as_the_spec_says() {
    let mut rig = rig();
    rig.open_loaded();
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    assert_eq!(
        rows[39].trim_end(),
        " / to search | ENTER to fold or unfold sections | y to copy current | ? for keybindings"
    );
    assert_eq!(rows[38].trim(), "", "the blank separator row");
}

#[test]
fn the_browser_layout_is_header_transcript_blank_row_and_help_line() {
    let mut rig = rig();
    rig.open_loaded();
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    assert!(rows[0].starts_with("╭─ Project BRIO "), "{}", rows[0]);
    assert!(rows[0].contains(BRIO_SESSION));
    assert!(rows[0].ends_with("─╮"));
    assert_eq!(rows[1].trim_matches('│').trim(), "");
    assert!(
        rows[2].starts_with("│ /home/user/projects/brio"),
        "{}",
        rows[2]
    );
    assert!(rows[4].starts_with("╰──") && rows[4].ends_with("──╯"));
    assert!(
        rows[5].starts_with("╭─ User "),
        "the first block: {}",
        rows[5]
    );
}

#[test]
fn every_cell_of_the_browser_is_painted_with_the_background() {
    for name in ["default", "nord"] {
        let theme = theme(name);
        let mut rig = rig_with(theme.clone(), 120, 40);
        rig.open_loaded();
        let buffer = screen_buffer(&mut rig, 120, 40);
        for y in 0..40 {
            for x in 0..120 {
                assert_eq!(
                    buffer[(x, y)].bg,
                    theme.role(Role::Background),
                    "{name}: cell {x},{y} {:?}",
                    buffer[(x, y)].symbol()
                );
            }
        }
    }
}

#[test]
fn a_transparent_theme_leaves_the_browser_unpainted() {
    let theme = theme("default").with_transparent_background(true);
    let mut rig = rig_with(theme, 120, 40);
    rig.open_loaded();
    let buffer = screen_buffer(&mut rig, 120, 40);
    for (x, y) in [(60, 1), (60, 6), (60, 38), (119, 39)] {
        assert_eq!(buffer[(x, y)].bg, ratatui::style::Color::Reset, "{x},{y}");
    }
}

#[test]
fn a_terminal_that_is_too_small_shows_only_the_message() {
    let mut rig = rig();
    rig.open_loaded();
    let text = screen_text(&mut rig, 59, 24);
    assert!(text.contains("Terminal too small"));
    assert!(!text.contains("Project"));
}

#[test]
fn loaded_browser_120x40_text() {
    let mut rig = rig();
    rig.open_loaded();
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn loaded_browser_80x24_text() {
    let mut rig = rig();
    rig.open_loaded();
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn loaded_browser_120x40_styled() {
    let mut rig = rig();
    rig.open_loaded();
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

#[test]
fn loading_browser_80x24_text() {
    let mut rig = rig();
    rig.harness.script(
        BRIO_SESSION,
        vec![
            ModelUpdate::Appended(turns()[..3].to_vec()),
            ModelUpdate::Progress(0.34),
        ],
    );
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    rig.pump_until(|app| turn_count(app) == 3 && !status(app).is_empty());
    assert_eq!(status(&rig.app), ["Loading… 34%"]);
    let text = screen_text(&mut rig, 80, 24);
    let help = text.lines().nth(23).unwrap();
    assert!(help.ends_with("Loading… 34% "), "{help:?}");
    insta::assert_snapshot!(text);
}

#[test]
fn empty_session_80x24_text() {
    let mut rig = rig();
    rig.harness.script(BRIO_SESSION, vec![ModelUpdate::Loaded]);
    rig.open(BRIO_SESSION);
    assert!(rig.app.session_is_empty());
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn parse_warnings_120x40_text() {
    let mut rig = rig();
    rig.harness.script(
        BRIO_SESSION,
        vec![
            ModelUpdate::Appended(turns()),
            ModelUpdate::ParseWarnings(3),
            ModelUpdate::Loaded,
        ],
    );
    rig.open(BRIO_SESSION);
    let text = screen_text(&mut rig, 120, 40);
    assert!(
        text.lines()
            .nth(2)
            .unwrap()
            .ends_with("3 entries could not be parsed │")
    );
    insta::assert_snapshot!(text);
}

#[test]
fn selector_over_the_browser_120x40_text() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    let text = screen_text(&mut rig, 120, 40);
    let rows: Vec<&str> = text.lines().collect();
    assert!(
        rows[0].starts_with("╭─ Project BRIO"),
        "browser header row 0: {}",
        rows[0]
    );
    assert!(
        rows[39].contains("to search"),
        "browser help line row h-1: {}",
        rows[39]
    );
    assert!(rows[1].starts_with("╭"), "the overlay: {}", rows[1]);
    insta::assert_snapshot!(text);
}

#[test]
fn selector_over_the_browser_200x50_text() {
    let mut rig = rig_with(theme("default"), 200, 50);
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    let text = screen_text(&mut rig, 200, 50);
    let rows: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
    // The browser shows in the columns beside the 160-column overlay: the
    // first block's border on row 5, and the header and help line in full.
    let left: String = rows[5][..20].iter().collect();
    assert!(
        left.starts_with("╭─ User "),
        "left of the overlay: {left:?}"
    );
    let right: String = rows[5][180..].iter().collect();
    assert!(
        right.contains("35:15Z") && right.ends_with("─╮"),
        "{right:?}"
    );
    assert_eq!(rows[0][0], '╭');
    assert_eq!(rows[0][199], '╮');
    assert!(rows[49].iter().collect::<String>().contains("to search"));
    assert!(
        rows[1][20] == '╭' && rows[1][179] == '╮',
        "the overlay is centred"
    );
    let cwd: String = rows[2][..20].iter().collect();
    assert_eq!(cwd, "│ /home/user/project", "the header beside the overlay");
    insta::assert_snapshot!(text);
}

#[test]
fn the_overlay_is_drawn_in_the_overlay_colours_and_the_browser_keeps_its_own() {
    let theme = theme("default");
    let mut rig = rig_with(theme.clone(), 120, 40);
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    let buffer = screen_buffer(&mut rig, 120, 40);
    // Row 0 and the last row are the browser's, on `background`.
    for x in [0, 60, 119] {
        for y in [0, 39] {
            assert_eq!(buffer[(x, y)].bg, theme.role(Role::Background), "{x},{y}");
        }
    }
    // The overlay is on `surface`, and its border is `ring`.
    assert_eq!(buffer[(60, 3)].bg, theme.role(Role::Surface));
    assert_eq!(buffer[(0, 1)].fg, theme.role(Role::Ring));
}

macro_rules! theme_snapshot {
    ($name:ident, $id:literal) => {
        #[test]
        fn $name() {
            let mut rig = rig_with(theme($id), 120, 40);
            rig.open_loaded();
            insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
        }
    };
}

theme_snapshot!(loaded_browser_theme_default, "default");
theme_snapshot!(loaded_browser_theme_terminal, "terminal");
theme_snapshot!(loaded_browser_theme_catppuccin, "catppuccin");
theme_snapshot!(loaded_browser_theme_gruvbox, "gruvbox");
theme_snapshot!(loaded_browser_theme_nord, "nord");
theme_snapshot!(loaded_browser_theme_tokyo_night, "tokyo-night");
theme_snapshot!(loaded_browser_theme_solarized, "solarized");
