//! Subagent transcripts (product §4.4, task 027): `a` opens one over the
//! session, `Backspace` returns, through the app as the running application
//! drives it; and the screens that show them.

use serde_json::json;
use tuirealm::state::State;

use super::theme_selector::{assert_all_have, mounted_themes};
use super::*;
use crate::app::browser::NO_SUBAGENT_TRANSCRIPT;
use crate::components::transcript::Presentation;
use crate::components::transcript::fixtures::{arc, prompt, reply, with_id};
use crate::components::turn_block::fixtures as blocks;
use crate::components::{
    BREADCRUMB_ATTR, FoldState, LIVE_ATTR, PRESENTATION_ATTR, SESSION_ID_ATTR, SectionKey,
};
use crate::model::{Item, ItemId, ToolKind, ToolStatus};

const SUB1: &str = "85a86657-11a7-4318-a21a-0f4476babd5c/subagents/agent-r2";
const SUB2: &str = "85a86657-11a7-4318-a21a-0f4476babd5c/subagents/agent-n3";
const SUB3: &str = "85a86657-11a7-4318-a21a-0f4476babd5c/subagents/agent-q4";

/// A subagent call whose agent type is `agent`.
fn subagent_call(call: &str, agent: &str, prompt: &str, result: &str) -> Item {
    blocks::tool_of(
        ToolKind::Subagent,
        call,
        &format!("Ask {agent}"),
        json!({"subagent_type": agent, "prompt": prompt}),
        Some((result, false)),
        ToolStatus::Ok,
    )
}

/// An assistant turn `id` with the `items`.
fn assistant_of(id: &str, items: Vec<Item>) -> Arc<Turn> {
    arc(with_id(blocks::assistant(None, items, None), id))
}

/// The session: the fixture turns and a last one that asked the
/// `story-writer`.
fn parent_turns() -> Vec<Arc<Turn>> {
    let mut all = turns();
    all.push(assistant_of(
        "s9",
        vec![
            blocks::text("x1", "I will ask the story writer."),
            subagent_call(
                "call-a",
                "story-writer",
                "Write a short story",
                "Once upon a time.",
            ),
        ],
    ));
    all
}

/// The `story-writer`'s transcript: its prompt, and a call of an `Explore`
/// agent.
fn story_turns() -> Vec<Arc<Turn>> {
    vec![
        arc(prompt("su1", "Write a short story")),
        assistant_of(
            "sa1",
            vec![
                blocks::text("y1", "I need some facts first."),
                subagent_call(
                    "call-b",
                    "Explore",
                    "Look around the repository",
                    "The repository has two crates.",
                ),
            ],
        ),
    ]
}

/// The `Explore` agent's transcript.
fn explore_turns() -> Vec<Arc<Turn>> {
    vec![
        arc(prompt("eu1", "Look around the repository")),
        arc(reply("ea1", "The repository has two crates.")),
    ]
}

fn loaded(turns: Vec<Arc<Turn>>, active_ago: chrono::Duration) -> Vec<ModelUpdate> {
    vec![
        ModelUpdate::Appended(turns),
        ModelUpdate::Progress(1.0),
        ModelUpdate::ParseWarnings(0),
        ModelUpdate::Loaded,
        ModelUpdate::Activity {
            last_modified: std::time::SystemTime::from(now() - active_ago),
            idle: None,
        },
    ]
}

fn query(app: &App, id: Id, attr: tuirealm::props::Attribute) -> AttrValue {
    app.application()
        .query(&id, attr)
        .unwrap()
        .unwrap()
        .into_attr()
}

fn presentation(app: &App, id: Id) -> Presentation {
    payload_from_value(&query(app, id, PRESENTATION_ATTR)).unwrap()
}

fn state_of(app: &App, id: Id) -> State {
    app.application().state(&id).unwrap()
}

fn breadcrumb(app: &App) -> Vec<String> {
    payload_from_value(&query(app, Id::BrowserHeader, BREADCRUMB_ATTR)).unwrap()
}

fn header_session(app: &App) -> String {
    match query(app, Id::BrowserHeader, SESSION_ID_ATTR) {
        AttrValue::String(id) => id,
        other => panic!("{other:?}"),
    }
}

fn header_warnings(app: &App) -> usize {
    match query(app, Id::BrowserHeader, PARSE_WARNINGS_ATTR) {
        AttrValue::Length(n) => n,
        other => panic!("{other:?}"),
    }
}

fn turns_in(app: &App, id: Id) -> usize {
    match query(app, id, TURN_COUNT_ATTR) {
        AttrValue::Length(n) => n,
        other => panic!("{other:?}"),
    }
}

fn mounted(app: &App, id: Id) -> bool {
    app.application().mounted(&id)
}

impl Rig {
    /// The session of `parent_turns` is open, loaded, with the subagent
    /// section selected and unfolded; every agent resolves.
    fn open_with_subagent(&mut self) {
        self.open_with_subagent_after(Vec::new());
    }

    /// Like [`open_with_subagent`](Self::open_with_subagent), with the turns
    /// `before` ahead of those of `parent_turns`.
    fn open_with_subagent_after(&mut self, before: Vec<Arc<Turn>>) {
        self.harness.link(BRIO_SESSION, "call-a", SUB1);
        self.harness.link(SUB1, "call-b", SUB2);
        self.harness.script(
            BRIO_SESSION,
            loaded(
                before.into_iter().chain(parent_turns()).collect(),
                chrono::Duration::seconds(10),
            ),
        );
        self.harness
            .script(SUB1, loaded(story_turns(), chrono::Duration::seconds(5)));
        self.harness
            .script(SUB2, loaded(explore_turns(), chrono::Duration::seconds(5)));
        self.open(BRIO_SESSION);
        self.settle();
        self.unfold_last();
    }

    /// Selects the last section, a subagent call, unfolds it and scrolls
    /// to the end.
    fn unfold_last(&mut self) {
        self.press(Key::Char('G'));
        self.press(Key::Char('l'));
        self.press(Key::Char('G'));
        self.settle();
    }

    /// `a`, and the wait until the subagent at `depth` is loaded.
    pub(super) fn open_subagent(&mut self, depth: usize) {
        self.press(Key::Char('a'));
        self.pump_until(|app| app.subagent_depth() == depth && app.session_loaded());
        self.settle();
    }

    /// Backspace, and the wait until the transcript at `depth` is on top.
    pub(super) fn back_to(&mut self, depth: usize) {
        self.press(Key::Backspace);
        self.pump_until(|app| app.subagent_depth() == depth);
        self.settle();
    }

    fn top(&self) -> Id {
        match self.app.subagent_depth() {
            0 => Id::Transcript,
            depth => Id::Subagent(depth),
        }
    }
}

/// Like [`subagent_rig`], with the turns `before` ahead of the parent's.
pub(super) fn subagent_rig_after(before: Vec<Arc<Turn>>, width: u16, height: u16) -> Rig {
    let mut rig = rig_with(theme("default"), width, height);
    rig.open_with_subagent_after(before);
    rig
}

pub(super) fn subagent_rig(width: u16, height: u16) -> Rig {
    let mut rig = rig_with(theme("default"), width, height);
    rig.open_with_subagent();
    rig
}

// -- opening

#[test]
fn a_on_a_subagent_section_mounts_a_transcript_at_depth_1_and_focuses_it() {
    let mut rig = subagent_rig(120, 40);
    assert_eq!(rig.app.subagent_depth(), 0);
    assert!(!mounted(&rig.app, Id::Subagent(1)));
    rig.open_subagent(1);
    assert_eq!(rig.app.subagent_depth(), 1);
    assert!(mounted(&rig.app, Id::Subagent(1)));
    assert!(
        mounted(&rig.app, Id::Transcript),
        "the parent stays mounted"
    );
    assert_eq!(rig.focus(), Some(Id::Subagent(1)));
    // The harness was asked about the call of the session, and the load of
    // the subagent started.
    let asks = rig.harness.subagent_asks.lock().unwrap().clone();
    assert_eq!(asks, [(id(BRIO_SESSION), ItemId::from("call-a"))]);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION, SUB1]);
    assert!(!rig.harness.stopped(1));
    assert_eq!(rig.app.top_session_id().map(|s| s.as_str()), Some(SUB1));
    assert_eq!(rig.app.open_session_id().unwrap().1.as_str(), BRIO_SESSION);
    assert_eq!(rig.app.session_generation(), 2);
    assert_eq!(turns_in(&rig.app, Id::Subagent(1)), 2);
}

#[test]
fn the_header_shows_the_breadcrumb_and_the_subagent_id() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    assert_eq!(breadcrumb(&rig.app), ["story-writer"]);
    assert_eq!(header_session(&rig.app), SUB1);
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    assert!(
        rows[0].starts_with("╭─ Project BRIO › Agent: story-writer ─"),
        "{}",
        rows[0]
    );
    assert!(rows[0].ends_with(&format!(" {SUB1} ─╮")), "{}", rows[0]);
    assert!(
        rows[2].starts_with("│ /home/user/projects/brio "),
        "the cwd row is the session's: {}",
        rows[2]
    );
}

#[test]
fn only_the_subagent_is_drawn() {
    let mut rig = subagent_rig(120, 40);
    let parent = screen_text(&mut rig, 120, 40);
    assert!(parent.contains("Once upon a time."));
    rig.open_subagent(1);
    let child = screen_text(&mut rig, 120, 40);
    assert!(child.contains("Write a short story"), "{child}");
    assert!(child.contains("I need some facts first."), "{child}");
    assert!(!child.contains("Once upon a time."), "{child}");
    assert!(!child.contains("Logitech BRIO"), "{child}");
}

#[test]
fn a_subagent_without_a_transcript_shows_a_toast_and_mounts_nothing() {
    let mut rig = rig_with(theme("default"), 120, 40);
    rig.open_with_subagent();
    // Nothing links the call any more.
    rig.harness.subagents.lock().unwrap().clear();
    rig.press(Key::Char('a'));
    rig.pump_until(|app| app.toast_text().is_some());
    assert_eq!(
        rig.app.toast_text(),
        Some("No transcript for this subagent")
    );
    assert_eq!(rig.app.subagent_depth(), 0);
    assert!(!mounted(&rig.app, Id::Subagent(1)));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.harness.loads(), [BRIO_SESSION], "no load started");
    assert_eq!(breadcrumb(&rig.app), Vec::<String>::new());
}

#[test]
fn a_load_that_fails_shows_a_toast_and_mounts_nothing() {
    let mut rig = rig_with(theme("default"), 120, 40);
    rig.open_with_subagent();
    rig.harness.fail.store(true, Ordering::Relaxed);
    rig.press(Key::Char('a'));
    rig.pump_until(|app| app.toast_text().is_some());
    assert!(
        rig.app
            .toast_text()
            .is_some_and(|t| t.contains("the file is gone")),
        "{:?}",
        rig.app.toast_text()
    );
    assert_eq!(rig.app.subagent_depth(), 0);
    assert!(!mounted(&rig.app, Id::Subagent(1)));
}

#[test]
fn the_lookup_runs_on_a_worker_and_nothing_changes_until_it_answers() {
    let mut rig = subagent_rig(120, 40);
    let before = presentation(&rig.app, Id::Transcript);
    let screen = screen_text(&mut rig, 120, 40);
    rig.harness.hold_subagents();
    rig.press(Key::Char('a'));
    rig.settle();
    // The question is asked off the UI thread, which goes on while it waits.
    assert_eq!(rig.harness.subagent_ask_count(), 1);
    let threads = rig.harness.subagent_threads.lock().unwrap().clone();
    assert_eq!(threads, [Some("subagent-lookup".to_owned())]);
    assert_eq!(rig.app.subagent_depth(), 0);
    assert!(!mounted(&rig.app, Id::Subagent(1)));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.harness.loads(), [BRIO_SESSION], "no load yet");
    assert_eq!(presentation(&rig.app, Id::Transcript), before);
    assert_eq!(screen_text(&mut rig, 120, 40), screen);
    rig.harness.release_subagents();
    rig.pump_until(|app| app.subagent_depth() == 1 && app.session_loaded());
    assert_eq!(rig.focus(), Some(Id::Subagent(1)));
    assert_eq!(breadcrumb(&rig.app), ["story-writer"]);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION, SUB1]);
}

#[test]
fn a_lookup_that_fails_leaves_the_parent_as_it_was() {
    let mut rig = rig_with(theme("default"), 120, 40);
    rig.open_with_subagent();
    rig.harness.subagents.lock().unwrap().clear();
    let before = presentation(&rig.app, Id::Transcript);
    assert!(before.offset > 0 && before.selected.is_some(), "{before:?}");
    let state = state_of(&rig.app, Id::Transcript);
    let sidebar = state_of(&rig.app, Id::TurnSidebar);
    let header: Vec<String> = screen_text(&mut rig, 120, 40)
        .lines()
        .take(5)
        .map(str::to_owned)
        .collect();
    rig.harness.hold_subagents();
    rig.press(Key::Char('a'));
    rig.settle();
    assert_eq!(rig.app.toast_text(), None, "no answer yet");
    rig.harness.release_subagents();
    rig.pump_until(|app| app.toast_text().is_some());
    assert_eq!(rig.app.toast_text(), Some(NO_SUBAGENT_TRANSCRIPT));
    assert_eq!(rig.app.subagent_depth(), 0);
    assert!(!mounted(&rig.app, Id::Subagent(1)));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(presentation(&rig.app, Id::Transcript), before);
    assert_eq!(state_of(&rig.app, Id::Transcript), state);
    assert_eq!(state_of(&rig.app, Id::TurnSidebar), sidebar);
    assert_eq!(breadcrumb(&rig.app), Vec::<String>::new());
    assert_eq!(header_session(&rig.app), BRIO_SESSION);
    let after: Vec<String> = screen_text(&mut rig, 120, 40)
        .lines()
        .take(5)
        .map(str::to_owned)
        .collect();
    assert_eq!(after, header);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION], "no load started");
}

#[test]
fn a_pressed_twice_while_the_lookup_is_pending_opens_one_level() {
    let mut rig = subagent_rig(120, 40);
    rig.harness.hold_subagents();
    rig.press(Key::Char('a'));
    rig.press(Key::Char('a'));
    let seen = rig.settle();
    let presses = seen
        .iter()
        .filter(|msg| matches!(msg, Msg::OpenSubagent { .. }))
        .count();
    assert_eq!(presses, 2, "both reached the app: {seen:?}");
    assert_eq!(rig.harness.subagent_ask_count(), 1, "one lookup started");
    assert_eq!(rig.app.subagent_depth(), 0);
    rig.harness.release_subagents();
    rig.pump_until(|app| app.subagent_depth() == 1 && app.session_loaded());
    rig.settle();
    assert_eq!(rig.app.subagent_depth(), 1);
    assert_eq!(breadcrumb(&rig.app), ["story-writer"]);
    assert!(!mounted(&rig.app, Id::Subagent(2)));
    assert_eq!(rig.harness.subagent_ask_count(), 1);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION, SUB1]);
    assert_eq!(rig.focus(), Some(Id::Subagent(1)));
}

/// The answers the app saw to a subagent lookup.
fn lookup_answers(seen: &[Msg]) -> usize {
    seen.iter()
        .filter(|msg| matches!(msg, Msg::SubagentFound { .. }))
        .count()
}

/// The rig with the story writer open at depth 1, its `Explore` call
/// selected, and `a` pressed on it with the lookup held.
fn nested_lookup_pending() -> Rig {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.unfold_last();
    rig.harness.hold_subagents();
    rig.press(Key::Char('a'));
    let asked = Instant::now() + Duration::from_secs(2);
    while rig.harness.subagent_ask_count() < 2 && Instant::now() < asked {
        rig.settle();
    }
    assert_eq!(rig.harness.subagent_ask_count(), 2);
    assert_eq!(rig.app.subagent_depth(), 1);
    rig
}

#[test]
fn a_lookup_overtaken_by_going_back_is_dropped() {
    let mut rig = nested_lookup_pending();
    rig.back_to(0);
    rig.harness.release_subagents();
    let seen = rig.settle();
    assert_eq!(lookup_answers(&seen), 1, "the answer came: {seen:?}");
    assert_eq!(rig.app.subagent_depth(), 0);
    assert!(!mounted(&rig.app, Id::Subagent(1)));
    assert!(!mounted(&rig.app, Id::Subagent(2)));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(breadcrumb(&rig.app), Vec::<String>::new());
    assert_eq!(header_session(&rig.app), BRIO_SESSION);
    assert_eq!(rig.app.toast_text(), None);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION, SUB1], "no load started");
}

#[test]
fn a_lookup_overtaken_by_opening_another_session_is_dropped() {
    let mut rig = nested_lookup_pending();
    rig.app.update(Msg::OpenSession(CLAUDE, id(NEWER_SESSION)));
    rig.harness.release_subagents();
    let seen = rig.settle();
    assert_eq!(lookup_answers(&seen), 1, "the answer came: {seen:?}");
    assert_eq!(rig.app.subagent_depth(), 0);
    assert!(!mounted(&rig.app, Id::Subagent(1)));
    assert!(!mounted(&rig.app, Id::Subagent(2)));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(header_session(&rig.app), NEWER_SESSION);
    assert_eq!(rig.app.toast_text(), None);
    assert_eq!(
        rig.harness.loads(),
        [BRIO_SESSION, SUB1, NEWER_SESSION],
        "no load of the subagent started"
    );
}

#[test]
fn a_lookup_is_dropped_even_when_the_other_session_fails_to_open() {
    let mut rig = nested_lookup_pending();
    rig.harness.fail.store(true, Ordering::Relaxed);
    rig.app.update(Msg::OpenSession(CLAUDE, id(NEWER_SESSION)));
    assert!(
        rig.app
            .toast_text()
            .is_some_and(|t| t.starts_with("Could not open the session")),
        "{:?}",
        rig.app.toast_text()
    );
    rig.harness.release_subagents();
    let seen = rig.settle();
    assert_eq!(lookup_answers(&seen), 1, "the answer came: {seen:?}");
    // The browser that stays shows the story writer, with nothing over it.
    assert_eq!(rig.app.subagent_depth(), 1);
    assert!(!mounted(&rig.app, Id::Subagent(2)));
    assert_eq!(breadcrumb(&rig.app), ["story-writer"]);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION, SUB1]);
}

#[test]
fn an_answer_that_comes_while_the_selector_is_open_is_dropped() {
    let mut rig = subagent_rig(120, 40);
    rig.harness.hold_subagents();
    rig.press(Key::Char('a'));
    rig.settle();
    rig.app.update(Msg::OpenSelector);
    let focus = rig.focus();
    assert_ne!(focus, Some(Id::Transcript));
    rig.harness.release_subagents();
    let seen = rig.settle();
    assert_eq!(lookup_answers(&seen), 1, "the answer came: {seen:?}");
    assert_eq!(rig.focus(), focus, "the selector keeps the focus");
    assert!(!mounted(&rig.app, Id::Subagent(1)));
    assert_eq!(rig.harness.loads(), [BRIO_SESSION]);
    rig.app.update(Msg::CloseSelector);
    assert_eq!(rig.app.subagent_depth(), 0);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // `a` works again.
    rig.open_subagent(1);
    assert_eq!(breadcrumb(&rig.app), ["story-writer"]);
}

#[test]
fn an_answer_to_a_lookup_nobody_waits_for_is_dropped() {
    let mut rig = subagent_rig(120, 40);
    let screen = screen_text(&mut rig, 120, 40);
    for found in [Some(id(SUB1)), None] {
        rig.app.update(Msg::SubagentFound {
            generation: rig.app.session_generation() + 1,
            found,
        });
    }
    assert_eq!(rig.app.subagent_depth(), 0);
    assert!(!mounted(&rig.app, Id::Subagent(1)));
    assert_eq!(rig.app.toast_text(), None);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION]);
    assert_eq!(screen_text(&mut rig, 120, 40), screen);
}

#[test]
fn a_on_any_other_section_does_nothing() {
    let mut rig = rig_with(theme("default"), 120, 40);
    rig.open_with_subagent();
    rig.press(Key::Char('g'));
    rig.settle();
    let before = presentation(&rig.app, Id::Transcript);
    rig.press(Key::Char('a'));
    rig.settle();
    assert_eq!(rig.app.subagent_depth(), 0);
    assert_eq!(rig.app.toast_text(), None);
    assert!(rig.harness.subagent_asks.lock().unwrap().is_empty());
    assert_eq!(presentation(&rig.app, Id::Transcript), before);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn a_is_ignored_while_the_selector_is_over_the_browser() {
    let mut rig = subagent_rig(120, 40);
    rig.app.update(Msg::OpenSelector);
    rig.app.update(Msg::OpenSubagent {
        call_id: ItemId::from("call-a"),
        label: "story-writer".to_owned(),
    });
    assert_eq!(rig.app.subagent_depth(), 0);
    assert_eq!(rig.harness.loads(), [BRIO_SESSION]);
}

// -- going back

#[test]
fn backspace_restores_the_parent_exactly_and_drops_the_watch() {
    let mut rig = subagent_rig(120, 40);
    // A parent that is not in its default state: scrolled, with a selection
    // and a section unfolded.
    let before = presentation(&rig.app, Id::Transcript);
    assert!(before.offset > 0, "{before:?}");
    assert!(before.selected.is_some());
    assert_ne!(before.folds, FoldState::new());
    let state = state_of(&rig.app, Id::Transcript);
    let screen = screen_text(&mut rig, 120, 40);
    rig.open_subagent(1);
    // Whatever happens in the subagent does not touch the parent.
    rig.press(Key::Char('g'));
    rig.press(Key::Char('l'));
    rig.settle();
    assert_ne!(presentation(&rig.app, Id::Subagent(1)).selected, None);
    assert_eq!(presentation(&rig.app, Id::Transcript), before);
    rig.back_to(0);
    assert_eq!(rig.app.subagent_depth(), 0);
    assert!(!mounted(&rig.app, Id::Subagent(1)));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(presentation(&rig.app, Id::Transcript), before);
    assert_eq!(state_of(&rig.app, Id::Transcript), state);
    assert!(rig.harness.stopped(1), "the subagent's watch was dropped");
    assert!(!rig.harness.stopped(0), "the parent's watch goes on");
    assert_eq!(
        rig.app.top_session_id().map(|s| s.as_str()),
        Some(BRIO_SESSION)
    );
    assert_eq!(screen_text(&mut rig, 120, 40), screen);
}

#[test]
fn backspace_restores_the_header() {
    let mut rig = subagent_rig(120, 40);
    let before = screen_text(&mut rig, 120, 40);
    rig.open_subagent(1);
    rig.back_to(0);
    assert_eq!(breadcrumb(&rig.app), Vec::<String>::new());
    assert_eq!(header_session(&rig.app), BRIO_SESSION);
    let header: Vec<&str> = before.lines().take(5).collect();
    let after = screen_text(&mut rig, 120, 40);
    assert_eq!(after.lines().take(5).collect::<Vec<_>>(), header);
}

#[test]
fn backspace_at_depth_0_does_nothing() {
    let mut rig = subagent_rig(120, 40);
    let before = presentation(&rig.app, Id::Transcript);
    let screen = screen_text(&mut rig, 120, 40);
    rig.press(Key::Backspace);
    let seen = rig.settle();
    assert!(!seen.contains(&Msg::CloseSubagent), "{seen:?}");
    // And the message itself is harmless there.
    rig.app.update(Msg::CloseSubagent);
    assert_eq!(rig.app.subagent_depth(), 0);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(presentation(&rig.app, Id::Transcript), before);
    assert_eq!(screen_text(&mut rig, 120, 40), screen);
    assert!(mounted(&rig.app, Id::Transcript));
}

#[test]
fn a_resize_while_covered_is_applied_to_the_parent_too() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.app.update(Msg::Resized(100, 30));
    rig.back_to(0);
    let text = screen_text(&mut rig, 100, 30);
    assert!(text.contains("Once upon a time."), "{text}");
}

// -- nesting

#[test]
fn nested_subagents_show_two_segments_and_pop_one_at_a_time() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.unfold_last();
    rig.open_subagent(2);
    assert_eq!(rig.app.subagent_depth(), 2);
    assert_eq!(breadcrumb(&rig.app), ["story-writer", "Explore"]);
    assert_eq!(header_session(&rig.app), SUB2);
    assert_eq!(rig.focus(), Some(Id::Subagent(2)));
    for depth in [Id::Transcript, Id::Subagent(1), Id::Subagent(2)] {
        assert!(mounted(&rig.app, depth), "{depth:?}");
    }
    let asks = rig.harness.subagent_asks.lock().unwrap().clone();
    assert_eq!(asks[1], (id(SUB1), ItemId::from("call-b")));
    assert_eq!(rig.harness.loads(), [BRIO_SESSION, SUB1, SUB2]);
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    assert!(
        rows[0].starts_with("╭─ Project BRIO › Agent: story-writer › Agent: Explore ─"),
        "{}",
        rows[0]
    );
    // One at a time.
    rig.back_to(1);
    assert_eq!(breadcrumb(&rig.app), ["story-writer"]);
    assert_eq!(header_session(&rig.app), SUB1);
    assert_eq!(rig.focus(), Some(Id::Subagent(1)));
    assert!(!mounted(&rig.app, Id::Subagent(2)));
    assert!(rig.harness.stopped(2));
    assert!(!rig.harness.stopped(1));
    rig.back_to(0);
    assert_eq!(breadcrumb(&rig.app), Vec::<String>::new());
    assert_eq!(header_session(&rig.app), BRIO_SESSION);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!mounted(&rig.app, Id::Subagent(1)));
    assert!(rig.harness.stopped(1));
}

// -- late updates

#[test]
fn late_updates_of_a_popped_subagent_are_ignored() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    let old = rig.app.session_generation();
    rig.back_to(0);
    let parent_turns = turns_in(&rig.app, Id::Transcript);
    // From the worker (it is stopped, but a batch can be on its way), and as
    // a message.
    rig.harness
        .send(1, ModelUpdate::Appended(vec![arc(reply("late", "Late."))]));
    rig.settle();
    rig.app.update(Msg::Session {
        generation: old,
        update: SessionUpdate::new(7, ModelUpdate::Appended(vec![arc(reply("late2", "Late."))])),
    });
    assert_eq!(turns_in(&rig.app, Id::Transcript), parent_turns);
    assert!(!screen_text(&mut rig, 120, 40).contains("Late."));
}

#[test]
fn a_popped_load_number_is_never_given_to_another_subagent() {
    let mut rig = subagent_rig(120, 40);
    rig.harness.link(BRIO_SESSION, "call-a", SUB3);
    rig.harness
        .script(SUB3, loaded(explore_turns(), chrono::Duration::seconds(5)));
    rig.open_subagent(1);
    let first = rig.app.session_generation();
    rig.back_to(0);
    rig.open_subagent(1);
    let second = rig.app.session_generation();
    assert!(second > first, "{second} > {first}");
    let before = turns_in(&rig.app, Id::Subagent(1));
    rig.app.update(Msg::Session {
        generation: first,
        update: SessionUpdate::new(9, ModelUpdate::Appended(vec![arc(reply("late", "Late."))])),
    });
    assert_eq!(turns_in(&rig.app, Id::Subagent(1)), before);
}

#[test]
fn a_session_opened_later_never_reuses_the_number_of_a_subagent() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    let subagent = rig.app.session_generation();
    rig.app.update(Msg::OpenSession(CLAUDE, id(NEWER_SESSION)));
    assert!(rig.app.session_generation() > subagent);
}

// -- the updates of the loads that are not on top

#[test]
fn a_covered_session_keeps_receiving_its_turns_and_the_header_stays_the_subagents() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    let parent = turns_in(&rig.app, Id::Transcript);
    let child = turns_in(&rig.app, Id::Subagent(1));
    rig.harness.send(
        0,
        ModelUpdate::Appended(vec![arc(reply("t20", "A late reply of the parent."))]),
    );
    rig.harness.send(0, ModelUpdate::ParseWarnings(2));
    rig.settle();
    assert_eq!(turns_in(&rig.app, Id::Transcript), parent + 1);
    assert_eq!(turns_in(&rig.app, Id::Subagent(1)), child);
    assert_eq!(header_warnings(&rig.app), 0, "the header is the subagent's");
    let folds = presentation(&rig.app, Id::Transcript).folds;
    rig.back_to(0);
    assert_eq!(presentation(&rig.app, Id::Transcript).folds, folds);
    assert_eq!(header_warnings(&rig.app), 2, "and the session's again");
}

#[test]
fn a_scrolled_up_parent_keeps_its_presentation_when_its_turns_change_while_covered() {
    let mut rig = subagent_rig(120, 40);
    // Away from the end, so that new turns do not move the view.
    rig.press(Key::Char('g'));
    rig.settle();
    let before = presentation(&rig.app, Id::Transcript);
    assert_eq!(before.offset, 0);
    rig.app.update(Msg::OpenSubagent {
        call_id: ItemId::from("call-a"),
        label: "story-writer".to_owned(),
    });
    rig.pump_until(|app| app.subagent_depth() == 1 && app.session_loaded());
    // The call gets a changed result, and a turn arrives.
    let changed = assistant_of(
        "s9",
        vec![
            blocks::text("x1", "I will ask the story writer."),
            subagent_call("call-a", "story-writer", "Write a short story", "The end."),
        ],
    );
    rig.harness.send(0, ModelUpdate::Updated(vec![changed]));
    rig.harness.send(
        0,
        ModelUpdate::Appended(vec![arc(reply("t30", "Another reply."))]),
    );
    rig.settle();
    assert_eq!(turns_in(&rig.app, Id::Transcript), parent_turns().len() + 1);
    rig.back_to(0);
    assert_eq!(presentation(&rig.app, Id::Transcript), before);
    assert_eq!(
        rig.app.new_sections(),
        1,
        "the new turn is counted for the parent"
    );
}

#[test]
fn the_header_warnings_and_live_marker_follow_the_transcript_on_top() {
    let mut rig = subagent_rig(120, 40);
    assert!(rig.app.session_is_live());
    rig.open_subagent(1);
    assert!(rig.app.session_is_live(), "the subagent was active 5 s ago");
    rig.harness.send(1, ModelUpdate::ParseWarnings(3));
    rig.settle();
    assert_eq!(header_warnings(&rig.app), 3);
    assert!(screen_text(&mut rig, 120, 40).contains("3 entries could not be parsed"));
    // Time passes: both go quiet.
    *rig.clock.lock().unwrap() = now() + chrono::Duration::minutes(10);
    rig.app.update(Msg::Tick);
    assert!(!rig.app.session_is_live());
    assert_eq!(
        query(&rig.app, Id::BrowserHeader, LIVE_ATTR),
        AttrValue::Flag(false)
    );
    // The session had no warnings.
    rig.back_to(0);
    assert_eq!(header_warnings(&rig.app), 0);
    assert!(!screen_text(&mut rig, 120, 40).contains("could not be parsed"));
    // Opening it again: its marker is its own, and the session's is back after.
    rig.open_subagent(1);
    rig.back_to(0);
    assert!(!rig.app.session_is_live());
}

#[test]
fn a_subagent_that_is_still_loading_shows_its_progress_and_the_parents_status_returns() {
    let mut rig = rig_with(theme("default"), 120, 40);
    rig.open_with_subagent();
    // The subagent never says it is loaded.
    rig.harness.script(
        SUB1,
        vec![
            ModelUpdate::Appended(story_turns()),
            ModelUpdate::Progress(0.4),
        ],
    );
    rig.press(Key::Char('a'));
    rig.pump_until(|app| app.subagent_depth() == 1);
    rig.settle();
    assert!(!rig.app.session_loaded());
    assert_eq!(status(&rig.app), ["Loading… 40%"]);
    rig.back_to(0);
    assert!(rig.app.session_loaded());
    assert!(status(&rig.app).is_empty());
}

// -- other sessions

#[test]
fn opening_another_session_clears_the_whole_stack() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.unfold_last();
    rig.open_subagent(2);
    let old = rig.app.session_generation();
    rig.app.update(Msg::OpenSession(CLAUDE, id(NEWER_SESSION)));
    assert_eq!(rig.app.subagent_depth(), 0);
    for depth in 1..=2 {
        assert!(!mounted(&rig.app, Id::Subagent(depth)), "{depth}");
    }
    assert!(rig.harness.stopped(1));
    assert!(rig.harness.stopped(2));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(breadcrumb(&rig.app), Vec::<String>::new());
    assert_eq!(header_session(&rig.app), NEWER_SESSION);
    // Backspace does nothing in the new session.
    rig.app.update(Msg::CloseSubagent);
    assert_eq!(header_session(&rig.app), NEWER_SESSION);
    // Late updates of the old subagent are dropped.
    rig.app.update(Msg::Session {
        generation: old,
        update: SessionUpdate::new(5, ModelUpdate::Appended(vec![arc(reply("late", "Late."))])),
    });
    assert_eq!(turns_in(&rig.app, Id::Transcript), 0);
}

#[test]
fn the_selector_opened_over_a_subagent_returns_to_it() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.app.update(Msg::OpenSelector);
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    rig.app.update(Msg::CloseSelector);
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert_eq!(rig.focus(), Some(Id::Subagent(1)));
    assert_eq!(rig.top(), Id::Subagent(1));
    assert!(screen_text(&mut rig, 120, 40).contains("Write a short story"));
}

// -- searching and saving in a subagent

#[test]
fn the_search_prompt_works_on_the_transcript_on_top() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.press(Key::Char('/'));
    rig.pump_until(App::transcript_prompt_open);
    for c in "facts".chars() {
        rig.press(Key::Char(c));
    }
    rig.press(Key::Enter);
    rig.pump_until(|app| app.transcript_search_position().is_some());
    assert_eq!(
        rig.app.transcript_search_position().map(|p| p.to_string()),
        Some("1/1".to_owned())
    );
    rig.back_to(0);
    assert_eq!(
        rig.app.transcript_search_position(),
        None,
        "the parent was not searched"
    );
}

#[test]
fn copying_in_a_subagent_copies_its_text() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.press(Key::Char('g'));
    rig.press(Key::Char('y'));
    let seen = rig.settle();
    let copied = seen.iter().find_map(|msg| match msg {
        Msg::CopyText(text) => Some(text.clone()),
        _ => None,
    });
    assert_eq!(copied.as_deref(), Some("Write a short story"));
}

// -- screens

#[test]
fn parent_with_a_subagent_selected_120x40_text() {
    let mut rig = subagent_rig(120, 40);
    let text = screen_text(&mut rig, 120, 40);
    for want in ["Prompt:", "Result:"] {
        assert!(text.contains(want), "{want}\n{text}");
    }
    // The hint is on the title row, not at the end of the body.
    let title = text
        .lines()
        .find(|l| l.contains("▼ Ask story-writer"))
        .unwrap();
    assert!(title.contains("─  a to open transcript │"), "{title}");
    assert_eq!(text.matches("to open transcript").count(), 1, "{text}");
    insta::assert_snapshot!(text);
}

#[test]
fn the_subagent_120x40_text() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn the_subagent_120x40_styled() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

#[test]
fn the_subagent_80x24_text() {
    let mut rig = subagent_rig(80, 24);
    rig.open_subagent(1);
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn nested_subagents_120x40_text() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.unfold_last();
    rig.open_subagent(2);
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn after_backspace_the_screen_is_the_parent_as_it_was_120x40_text() {
    let mut rig = subagent_rig(120, 40);
    let before = screen_text(&mut rig, 120, 40);
    rig.open_subagent(1);
    rig.back_to(0);
    let after = screen_text(&mut rig, 120, 40);
    assert_eq!(after, before);
    insta::assert_snapshot!(after);
}

#[test]
fn a_subagent_view_hints_at_the_back_key_and_the_session_does_not() {
    let mut rig = subagent_rig(120, 40);
    let last = |text: String| text.lines().last().unwrap().to_owned();
    let root = last(screen_text(&mut rig, 120, 40));
    assert!(!root.contains("go back"), "{root}");
    rig.open_subagent(1);
    for (width, height) in [(120, 40), (80, 24)] {
        let line = last(screen_text(&mut rig, width, height));
        assert!(line.contains("Backspace to go back"), "{line}");
        assert!(
            line.starts_with(" / to search | Backspace to go back"),
            "{line}"
        );
    }
    rig.back_to(0);
    assert_eq!(last(screen_text(&mut rig, 120, 40)), root);
}

#[test]
fn the_subagent_id_is_cut_from_the_start_at_80_columns() {
    let mut rig = subagent_rig(80, 24);
    rig.open_subagent(1);
    let rows = rows_of(&screen_buffer(&mut rig, 80, 24));
    assert!(
        rows[0].starts_with("╭─ Project BRIO › Agent: story-writer ─"),
        "{}",
        rows[0]
    );
    assert!(rows[0].ends_with("/subagents/agent-r2 ─╮"), "{}", rows[0]);
}

#[test]
fn the_selected_subagent_section_shows_the_hint() {
    let mut rig = subagent_rig(120, 40);
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    let title = rows
        .iter()
        .find(|r| r.contains("a to open transcript"))
        .unwrap();
    assert!(
        title.contains("▼ Ask story-writer (story-writer)  ─"),
        "{title}"
    );
    assert!(title.ends_with("a to open transcript │"), "{title}");
    // The selected block is the subagent call's.
    let selected = presentation(&rig.app, Id::Transcript).selected.unwrap();
    assert_eq!(selected.key, SectionKey::Item(ItemId::from("call-a")));
}

// -- the real Claude harness on the subagent fixture

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// An app whose harness is the real Claude one, over a copy of the subagent
/// fixture tree (`pppp0001` and its two subagents), with the session in the
/// index.
fn claude_rig(width: u16, height: u16) -> (Rig, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("projects");
    copy_dir(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude/subagents"),
        &data,
    );
    let mut rig = rig_with(theme("default"), width, height);
    let claude = crate::harness::claude::ClaudeHarness::with_data_dir(data, Default::default());
    rig.app
        .set_harnesses([Arc::new(claude) as Arc<dyn Harness>]);
    let loading = project_in(
        CLAUDE,
        "loading",
        "/home/user/loading",
        at(2026, 10, 28, 9, 30, 0),
    );
    let mut sessions = SessionMap::new();
    sessions.insert(
        loading.id.clone(),
        vec![session(
            &loading,
            "pppp0001",
            "Explore the repo and write the story",
            at(2026, 4, 1, 10, 0, 0),
            at(2026, 4, 1, 10, 4, 5),
        )],
    );
    rig.app.update(fixtures::message(snapshot_event(
        CLAUDE,
        true,
        vec![loading],
        sessions,
    )));
    (rig, tmp)
}

impl Rig {
    /// Moves up from the last section until the tool call `call` is
    /// selected.
    fn select_call(&mut self, call: &str) {
        self.press(Key::Char('G'));
        self.settle();
        for _ in 0..12 {
            let selected = presentation(&self.app, Id::Transcript).selected;
            if selected.is_some_and(|s| s.key == SectionKey::Item(ItemId::from(call))) {
                return;
            }
            self.press(Key::Up);
            self.settle();
        }
        panic!("never selected {call}");
    }
}

#[test]
fn with_the_claude_harness_the_subagent_opens_with_its_prompt_as_the_first_user_turn() {
    let (mut rig, _tmp) = claude_rig(120, 40);
    rig.app.update(Msg::OpenSession(CLAUDE, id("pppp0001")));
    rig.pump_until(App::session_loaded);
    rig.settle();
    // `toolu_res02` is linked through the result entry (the story-writer).
    rig.select_call("toolu_res02");
    rig.open_subagent(1);
    assert_eq!(rig.focus(), Some(Id::Subagent(1)));
    assert_eq!(breadcrumb(&rig.app), ["story-writer"]);
    assert_eq!(
        rig.app.top_session_id().map(|s| s.as_str()),
        Some("pppp0001/subagents/agent-r2")
    );
    let screen = screen_text(&mut rig, 120, 40);
    let rows: Vec<&str> = screen.lines().collect();
    assert!(
        rows[0].starts_with("╭─ Project loading › Agent: story-writer ─"),
        "{}",
        rows[0]
    );
    assert!(
        rows[0].ends_with(" pppp0001/subagents/agent-r2 ─╮"),
        "{}",
        rows[0]
    );
    // The first block under the header is a User turn holding the prompt.
    assert!(
        super::from_transcript(rows[5]).starts_with("╭─ User "),
        "{}",
        rows[5]
    );
    assert!(
        rows[6..9]
            .iter()
            .any(|row| row.contains("Write a short story")),
        "{screen}"
    );
    assert_eq!(
        turns_in(&rig.app, Id::Subagent(1)),
        2,
        "the user turn and the reply"
    );
}

#[test]
fn with_the_claude_harness_the_subagent_120x40_text() {
    let (mut rig, _tmp) = claude_rig(120, 40);
    rig.app.update(Msg::OpenSession(CLAUDE, id("pppp0001")));
    rig.pump_until(App::session_loaded);
    rig.settle();
    rig.select_call("toolu_res02");
    rig.open_subagent(1);
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn with_the_claude_harness_a_call_without_a_transcript_shows_the_toast() {
    let (mut rig, _tmp) = claude_rig(120, 40);
    rig.app.update(Msg::OpenSession(CLAUDE, id("pppp0001")));
    rig.pump_until(App::session_loaded);
    rig.settle();
    // `toolu_gone03` names an agent that has no transcript.
    rig.select_call("toolu_gone03");
    rig.press(Key::Char('a'));
    rig.pump_until(|app| app.toast_text().is_some());
    assert_eq!(
        rig.app.toast_text(),
        Some("No transcript for this subagent")
    );
    assert_eq!(rig.app.subagent_depth(), 0);
    assert!(!mounted(&rig.app, Id::Subagent(1)));
}

#[test]
fn a_preview_reaches_the_parent_transcript_behind_the_subagent_too() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    let ids: Vec<Id> = mounted_themes(&rig.app).iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&Id::Transcript) && ids.contains(&Id::Subagent(1)));
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    assert_all_have(&rig.app, &theme("terminal"), 5);
    let ids: Vec<Id> = mounted_themes(&rig.app).iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&Id::Transcript) && ids.contains(&Id::Subagent(1)));
    // Esc gives the old theme back to both, and Backspace shows the parent
    // in it.
    rig.close_themes();
    assert_all_have(&rig.app, &theme("default"), 5);
    assert_eq!(rig.focus(), Some(Id::Subagent(1)));
    rig.back_to(0);
    assert_all_have(&rig.app, &theme("default"), 4);
}

#[test]
fn a_kept_theme_reaches_the_parent_transcript_behind_the_subagent_too() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    rig.back_to(0);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_all_have(&rig.app, &theme("terminal"), 4);
}
