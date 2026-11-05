//! A session that holds only local commands (SPEC cycle 3 §2.3): the browser's
//! empty state lists them, a turn that arrives replaces it with the normal
//! view, and a subagent's transcript over it does not lose them.

use std::path::Path;
use std::sync::mpsc;

use tuirealm::props::Attribute;

use super::subagent::subagent_rig;
use super::*;
use crate::components::LOCAL_COMMANDS_ONLY;
use crate::harness::claude::{Assembler, ClaudeHarness};
use crate::model::Item;

const FIXTURE: &str = "tests/fixtures/claude/local_commands_only.jsonl";

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

/// What the real Claude loader sends for the local-commands-only fixture,
/// up to its `Loaded`.
fn fixture_updates() -> Vec<ModelUpdate> {
    let data = tempfile::tempdir().unwrap();
    let harness = ClaudeHarness::with_data_dir(data.path().to_owned(), Default::default());
    let summary = harness.open_file(&fixture_path()).unwrap();
    let (tx, rx) = mpsc::channel();
    let _handle = harness.load(&summary.id, tx).unwrap();
    let mut updates = Vec::new();
    loop {
        let update = rx.recv_timeout(Duration::from_secs(30)).unwrap();
        let done = update == ModelUpdate::Loaded;
        updates.push(update);
        if done {
            return updates;
        }
    }
}

fn commands(list: &[&str]) -> ModelUpdate {
    ModelUpdate::LocalCommands(list.iter().map(|c| (*c).to_owned()).collect())
}

fn loaded_with(list: ModelUpdate) -> Vec<ModelUpdate> {
    vec![
        list,
        ModelUpdate::Progress(1.0),
        ModelUpdate::ParseWarnings(0),
        ModelUpdate::Loaded,
    ]
}

/// The rig with the fixture's session open and loaded.
fn fixture_rig(theme: Theme, width: u16, height: u16) -> Rig {
    let mut rig = rig_with(theme, width, height);
    rig.harness.script(BRIO_SESSION, fixture_updates());
    rig.open(BRIO_SESSION);
    rig
}

/// The message the empty state shows.
fn message(app: &App) -> String {
    match app
        .application()
        .query(&Id::EmptySession, Attribute::Text)
        .unwrap()
        .unwrap()
        .into_attr()
    {
        AttrValue::String(message) => message,
        other => panic!("{other:?}"),
    }
}

const LISTED: &str = "No turns. Local commands only:\n/effort\n/clear";

#[test]
fn the_fixture_shows_its_commands_in_the_empty_state() {
    let rig = fixture_rig(theme("default"), 120, 40);
    assert!(rig.app.session_is_empty());
    assert_eq!(rig.turn_count(), 0);
    assert_eq!(message(&rig.app), LISTED);
}

#[test]
fn a_session_without_commands_keeps_the_plain_message() {
    let mut rig = rig();
    rig.harness.script(BRIO_SESSION, vec![ModelUpdate::Loaded]);
    rig.open(BRIO_SESSION);
    assert!(rig.app.session_is_empty());
    assert_eq!(message(&rig.app), "Session file is empty");
}

/// Held entries after a trailing compaction keep today's behaviour: the turns
/// are shown and no message appears.
#[test]
fn entries_held_after_a_trailing_compaction_show_the_turns_and_no_message() {
    let lines = [
        r#"{"type":"user","uuid":"u1","timestamp":"2026-10-23T10:00:00.000Z","origin":{"kind":"human"},"message":{"role":"user","content":"hello"}}"#,
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-10-23T10:00:05.000Z","message":{"id":"m1","model":"claude-sonnet-5-5","role":"assistant","content":[{"type":"text","text":"hi"}]}}"#,
        r#"{"type":"system","subtype":"compact_boundary","uuid":"k1","timestamp":"2026-10-23T10:00:10.000Z","content":"Conversation compacted"}"#,
        r#"{"type":"user","uuid":"s1","timestamp":"2026-10-23T10:00:11.000Z","isCompactSummary":true,"message":{"role":"user","content":"Summary."}}"#,
        r#"{"type":"user","uuid":"e1","timestamp":"2026-10-23T10:00:12.000Z","message":{"role":"user","content":"<command-name>/effort</command-name>"}}"#,
    ];
    let text = lines.join("\n") + "\n";
    let turns = Assembler::from_memory(Arc::from(text.as_bytes())).into_turns();
    assert_eq!(turns.len(), 3, "the entry is held, not in a turn");
    let mut rig = rig();
    let mut script = vec![ModelUpdate::Appended(turns)];
    script.extend(loaded_with(commands(&[])).into_iter().skip(1));
    rig.harness.script(BRIO_SESSION, script);
    rig.open(BRIO_SESSION);
    assert!(!rig.app.session_is_empty());
    assert_eq!(message(&rig.app), "Session file is empty");
    let screen = screen_text(&mut rig, 120, 40);
    assert!(!screen.contains("Local commands only"), "{screen}");
    assert!(screen.contains("[1] Turns"), "{screen}");
}

#[test]
fn the_message_waits_for_the_load_to_end() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    rig.app
        .update(session_msg(&rig.app, commands(&["/effort"])));
    assert!(!rig.app.session_is_empty(), "still loading");
    assert_eq!(message(&rig.app), "Session file is empty");
    rig.app.update(session_msg(&rig.app, ModelUpdate::Loaded));
    assert_eq!(message(&rig.app), format!("{LOCAL_COMMANDS_ONLY}\n/effort"));
}

#[test]
fn an_appended_command_extends_the_list_without_a_key_press() {
    let mut rig = fixture_rig(theme("default"), 120, 40);
    rig.harness
        .send(0, commands(&["/effort", "/clear", "/exit"]));
    rig.pump_until(|app| message(app).ends_with("/exit"));
    assert!(rig.app.session_is_empty());
    let screen = screen_text(&mut rig, 120, 40);
    assert!(screen.contains("/exit"), "{screen}");
}

#[test]
fn a_turn_that_arrives_replaces_the_state_with_the_normal_view() {
    let mut rig = fixture_rig(theme("default"), 120, 40);
    assert_eq!(rig.app.routing().area(Id::TurnSidebar), None);
    rig.harness
        .send(0, ModelUpdate::Appended(turns_after_the_commands()));
    rig.pump_until(|app| !app.session_is_empty());
    assert!(rig.app.routing().area(Id::TurnSidebar).is_some());
    assert_eq!(message(&rig.app), "Session file is empty");
    let screen = screen_text(&mut rig, 120, 40);
    assert!(!screen.contains("Local commands only"), "{screen}");
    assert!(screen.contains("[1] Turns"), "{screen}");
    assert!(screen.contains("5 injected items"), "{screen}");
}

/// The turns of the fixture followed by a prompt and a reply, assembled the
/// way a live session would hold them: the commands are in the first User
/// turn.
fn turns_after_the_commands() -> Vec<Arc<Turn>> {
    let mut bytes = std::fs::read(fixture_path()).unwrap();
    for line in [
        r#"{"type":"user","cwd":"/home/user/project","timestamp":"2026-10-23T10:01:00.000Z","origin":{"kind":"human"},"message":{"role":"user","content":"Now a real question"}}"#,
        r#"{"type":"assistant","timestamp":"2026-10-23T10:01:05.000Z","message":{"id":"msg_x","model":"claude-sonnet-5-5","role":"assistant","content":[{"type":"text","text":"A real answer"}]}}"#,
    ] {
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
    }
    Assembler::from_memory(Arc::from(bytes)).into_turns()
}

#[test]
fn the_commands_are_part_of_the_first_user_turn_once_it_arrives() {
    let turns = turns_after_the_commands();
    let Turn::User(first) = &*turns[0] else {
        panic!("{:?}", turns[0]);
    };
    assert_eq!(first.text, "Now a real question");
    let Some(Item::Injected { entries, .. }) = first.items.first() else {
        panic!("{:?}", first.items);
    };
    let local: Vec<&str> = entries
        .iter()
        .filter(|e| e.kind == "local_command")
        .map(|e| e.text.as_str())
        .collect();
    assert_eq!(local[..3], ["/effort", "Cancelled", "/clear"]);
}

#[test]
fn a_reset_goes_back_to_the_plain_message_and_the_new_content_shows() {
    let mut rig = fixture_rig(theme("default"), 120, 40);
    rig.harness.send(0, ModelUpdate::Reset);
    rig.harness.send(0, ModelUpdate::Loaded);
    rig.pump_until(|app| message(app) == "Session file is empty");
    assert!(rig.app.session_is_empty());
    // A truncated file that holds one other command.
    rig.harness.send(0, commands(&["/model opus"]));
    rig.harness.send(0, ModelUpdate::Loaded);
    rig.pump_until(|app| message(app).ends_with("/model opus"));
    assert_eq!(
        message(&rig.app),
        format!("{LOCAL_COMMANDS_ONLY}\n/model opus")
    );
}

/// Between a `Reset` and the end of the reload the list is gone.
#[test]
fn a_reset_alone_goes_back_to_the_plain_message() {
    let mut rig = fixture_rig(theme("default"), 120, 40);
    rig.harness.send(0, ModelUpdate::Reset);
    rig.settle();
    assert_eq!(message(&rig.app), "Session file is empty");
}

/// A `Reset` that reaches a view covered by a subagent clears that view's
/// list, so the session shows what it holds now when the subagent closes.
#[test]
fn a_reset_of_a_covered_session_clears_its_list() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.harness.send(0, ModelUpdate::Reset);
    for update in loaded_with(commands(&["/effort"])) {
        rig.harness.send(0, update);
    }
    rig.harness.send(0, ModelUpdate::Reset);
    rig.harness.send(0, ModelUpdate::Loaded);
    rig.settle();
    rig.back_to(0);
    assert!(rig.app.session_is_empty());
    assert_eq!(message(&rig.app), "Session file is empty");
}

#[test]
fn the_list_belongs_to_its_load() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    assert_eq!(rig.app.subagent_depth(), 1);
    // While the subagent covers it, the session is reloaded and now holds
    // only local commands.
    rig.harness.send(0, ModelUpdate::Reset);
    for update in loaded_with(commands(&["/effort", "/clear"])) {
        rig.harness.send(0, update);
    }
    rig.settle();
    assert!(!rig.app.session_is_empty(), "the subagent is on top");
    assert_eq!(message(&rig.app), "Session file is empty");
    rig.back_to(0);
    assert!(rig.app.session_is_empty());
    assert_eq!(message(&rig.app), LISTED);
    assert_eq!(rig.app.routing().area(Id::TurnSidebar), None);
}

#[test]
fn a_subagent_without_commands_over_a_session_with_commands_shows_its_own_message() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.harness.send(1, ModelUpdate::Reset);
    rig.harness.send(1, ModelUpdate::Loaded);
    rig.harness.send(0, ModelUpdate::Reset);
    for update in loaded_with(commands(&["/effort"])) {
        rig.harness.send(0, update);
    }
    rig.settle();
    assert!(rig.app.session_is_empty(), "the subagent has no turns");
    assert_eq!(message(&rig.app), "Session file is empty");
    rig.back_to(0);
    assert_eq!(message(&rig.app), format!("{LOCAL_COMMANDS_ONLY}\n/effort"));
}

// -- snapshots

#[test]
fn local_commands_only_120x40_text() {
    let mut rig = fixture_rig(theme("default"), 120, 40);
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn local_commands_only_80x24_text() {
    let mut rig = fixture_rig(theme("default"), 80, 24);
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn local_commands_only_120x40_styled() {
    let mut rig = fixture_rig(theme("default"), 120, 40);
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

#[test]
fn local_commands_only_120x40_styled_terminal_theme() {
    let mut rig = fixture_rig(theme("terminal"), 120, 40);
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

/// More commands than the body has rows at 80x24 (17).
#[test]
fn a_list_longer_than_the_body_80x24_text() {
    let mut rig = rig_with(theme("default"), 80, 24);
    let list: Vec<String> = (1..=25).map(|n| format!("/command-{n}")).collect();
    rig.harness
        .script(BRIO_SESSION, loaded_with(ModelUpdate::LocalCommands(list)));
    rig.open(BRIO_SESSION);
    let screen = screen_text(&mut rig, 80, 24);
    assert!(
        screen.contains("No turns. Local commands only:"),
        "{screen}"
    );
    assert!(screen.contains("/command-16"), "{screen}");
    assert!(!screen.contains("/command-17"), "{screen}");
    insta::assert_snapshot!(screen);
}

#[test]
fn the_first_frame_after_a_turn_arrives_120x40_text() {
    let mut rig = fixture_rig(theme("default"), 120, 40);
    rig.harness
        .send(0, ModelUpdate::Appended(turns_after_the_commands()));
    rig.pump_until(|app| !app.session_is_empty());
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}
