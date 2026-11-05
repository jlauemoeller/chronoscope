//! A session that holds nothing but local commands (SPEC cycle 3 §2.3): the
//! commands are collected by the `Assembler` and sent as
//! `ModelUpdate::LocalCommands` while the session has no turn to hold them.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use chronoscope::harness::claude::{Assembler, ClaudeHarness};
use chronoscope::harness::{Harness, WatchHandle};
use chronoscope::model::{Item, ModelUpdate, SessionId, Turn};
use tempfile::TempDir;

const WAIT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------- lines

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude/local_commands_only.jsonl")
}

fn ts(n: usize) -> String {
    format!("2026-05-01T10:{:02}:{:02}.000Z", (n / 60) % 60, n % 60)
}

fn echo_text(name: &str, args: &str) -> String {
    format!(
        "<command-name>/{name}</command-name>\n<command-message>{name}</command-message>\n<command-args>{args}</command-args>"
    )
}

/// An echo, as Claude Code writes it: a `user` entry without `origin`.
fn echo(n: usize, name: &str, args: &str) -> String {
    let content = serde_json::to_string(&echo_text(name, args)).unwrap();
    format!(
        r#"{{"type":"user","uuid":"e{n}","cwd":"/p","timestamp":"{}","message":{{"role":"user","content":{content}}}}}"#,
        ts(n)
    ) + "\n"
}

fn output(n: usize, text: &str) -> String {
    format!(
        r#"{{"type":"user","uuid":"o{n}","cwd":"/p","timestamp":"{}","message":{{"role":"user","content":"<local-command-stdout>{text}</local-command-stdout>"}}}}"#,
        ts(n)
    ) + "\n"
}

fn caveat(n: usize) -> String {
    format!(
        r#"{{"type":"user","uuid":"c{n}","cwd":"/p","timestamp":"{}","isMeta":true,"message":{{"role":"user","content":"<local-command-caveat>Run locally.</local-command-caveat>"}}}}"#,
        ts(n)
    ) + "\n"
}

/// A `system` `local_command` entry; `run` is the JSON of `commandRun`.
fn system(n: usize, run: Option<&str>) -> String {
    let run = run.map_or(String::new(), |r| format!(r#","commandRun":{r}"#));
    format!(
        r#"{{"type":"system","subtype":"local_command","uuid":"s{n}","cwd":"/p","timestamp":"{}","content":"<local-command-stdout>done</local-command-stdout>"{run}}}"#,
        ts(n)
    ) + "\n"
}

fn prompt(n: usize, text: &str) -> String {
    format!(
        r#"{{"type":"user","uuid":"u{n}","cwd":"/p","timestamp":"{}","origin":{{"kind":"human"}},"message":{{"role":"user","content":"{text}"}}}}"#,
        ts(n)
    ) + "\n"
}

fn reply(n: usize, msg: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"a{n}","cwd":"/p","timestamp":"{}","message":{{"id":"{msg}","model":"claude-sonnet-5-5","role":"assistant","content":[{{"type":"text","text":"ok"}}]}}}}"#,
        ts(n)
    ) + "\n"
}

fn compaction(n: usize) -> String {
    format!(
        r#"{{"type":"system","subtype":"compact_boundary","uuid":"k{n}","cwd":"/p","timestamp":"{}","content":"Conversation compacted","compactMetadata":{{"trigger":"auto"}}}}"#,
        ts(n)
    ) + "\n"
}

fn summary(n: usize) -> String {
    format!(
        r#"{{"type":"user","uuid":"m{n}","cwd":"/p","timestamp":"{}","isCompactSummary":true,"message":{{"role":"user","content":"Summary."}}}}"#,
        ts(n)
    ) + "\n"
}

fn attachment(n: usize) -> String {
    format!(
        r#"{{"type":"attachment","uuid":"f{n}","cwd":"/p","timestamp":"{}","attachment":{{"type":"file","filename":"/p/a.rs","content":"x"}}}}"#,
        ts(n)
    ) + "\n"
}

fn assembled(text: &str) -> Assembler {
    Assembler::from_memory(Arc::from(text.as_bytes()))
}

fn commands_of(text: &str) -> Vec<String> {
    assembled(text).local_commands().to_vec()
}

// ------------------------------------------------------- what is listed

#[test]
fn the_fixture_lists_its_commands_and_has_no_turn() {
    let asm = Assembler::from_file(fixture_path()).unwrap();
    assert!(asm.turns().is_empty());
    assert_eq!(asm.local_commands(), ["/effort", "/clear"]);
}

#[test]
fn a_command_with_an_echo_and_a_system_entry_is_listed_once() {
    let text = echo(1, "clear", "") + &system(2, Some(r#"{"command":"clear","args":""}"#));
    assert_eq!(commands_of(&text), ["/clear"]);
}

#[test]
fn the_arguments_of_an_echo_are_part_of_the_command() {
    let text = caveat(1) + &echo(2, "model", "opus") + &output(3, "Set model to opus");
    assert_eq!(commands_of(&text), ["/model opus"]);
}

#[test]
fn system_entries_without_an_echo_are_listed_by_their_commands() {
    let text = system(1, Some(r#"{"command":"model","args":"opus"}"#))
        + &system(2, Some(r#"{"command":"clear"}"#));
    assert_eq!(commands_of(&text), ["/model opus", "/clear"]);
}

#[test]
fn a_system_entry_without_a_command_run_is_not_listed() {
    assert!(commands_of(&system(1, None)).is_empty());
    assert!(commands_of(&system(1, Some(r#"{"command":""}"#))).is_empty());
    assert!(commands_of(&system(1, Some(r#"{"args":"x"}"#))).is_empty());
}

#[test]
fn the_first_line_of_a_system_command_is_listed() {
    let text = system(1, Some(r#"{"command":"say","args":"one\ntwo"}"#));
    assert_eq!(commands_of(&text), ["/say one"]);
}

#[test]
fn outputs_and_the_caveat_are_not_listed() {
    assert!(commands_of(&(caveat(1) + &output(2, "Cancelled"))).is_empty());
}

#[test]
fn an_echo_with_stray_text_is_listed_by_its_name_alone() {
    let stray = serde_json::to_string(
        "<command-name>/model</command-name>\n<command-args>opus</command-args>\nstray text",
    )
    .unwrap();
    let text = format!(
        r#"{{"type":"user","uuid":"x","timestamp":"{}","message":{{"role":"user","content":{stray}}}}}"#,
        ts(1)
    ) + "\n";
    assert_eq!(commands_of(&text), ["/model"]);
}

#[test]
fn tabs_in_the_arguments_are_listed_as_spaces() {
    let text =
        echo(1, "tabbed", "a\tb\tc") + &system(2, Some(r#"{"command":"sys","args":"x\ty"}"#));
    assert_eq!(commands_of(&text), ["/tabbed a b c"]);
    assert_eq!(
        commands_of(&system(1, Some(r#"{"command":"sys","args":"x\ty"}"#))),
        ["/sys x y"]
    );
}

#[test]
fn nothing_else_is_a_command() {
    for text in [
        String::new(),
        attachment(1),
        output(1, "x") + &attachment(2),
        prompt(1, "hello"),
    ] {
        assert!(commands_of(&text).is_empty(), "{text:?}");
    }
}

#[test]
fn the_list_follows_the_order_the_commands_ran() {
    let text = echo(1, "effort", "") + &echo(2, "model", "opus") + &echo(3, "effort", "");
    assert_eq!(commands_of(&text), ["/effort", "/model opus", "/effort"]);
}

// ------------------------------------------------------------- the stream

fn stream(path: &Path) -> (ClaudeHarness, SessionId, Vec<ModelUpdate>) {
    let data = tempfile::tempdir().unwrap();
    let harness = ClaudeHarness::with_data_dir(data.path().to_owned(), Default::default());
    let summary = harness.open_file(path).unwrap();
    let (tx, rx) = mpsc::channel();
    let _handle = harness.load(&summary.id, tx).unwrap();
    let mut updates = Vec::new();
    loop {
        let update = rx.recv_timeout(WAIT).expect("the load finishes");
        let done = update == ModelUpdate::Loaded;
        updates.push(update);
        if done {
            return (harness, summary.id, updates);
        }
    }
}

fn list(commands: &[&str]) -> ModelUpdate {
    ModelUpdate::LocalCommands(commands.iter().map(|c| (*c).to_owned()).collect())
}

fn temp_file(content: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    fs::write(&path, content).unwrap();
    (dir, path)
}

#[test]
fn the_fixture_streams_its_commands_before_loaded_and_no_turn() {
    let (_, _, updates) = stream(&fixture_path());
    assert!(
        !updates
            .iter()
            .any(|u| matches!(u, ModelUpdate::Appended(_) | ModelUpdate::Updated(_))),
        "{updates:?}"
    );
    let at = updates
        .iter()
        .position(|u| *u == list(&["/effort", "/clear"]))
        .unwrap_or_else(|| panic!("{updates:?}"));
    assert_eq!(
        updates[at + 1..].first(),
        Some(&ModelUpdate::ParseWarnings(0))
    );
    assert_eq!(updates.last(), Some(&ModelUpdate::Loaded));
    assert_eq!(
        updates
            .iter()
            .filter(|u| matches!(u, ModelUpdate::LocalCommands(_)))
            .count(),
        1
    );
}

#[test]
fn the_fixtures_stats_report_no_turn() {
    let data = tempfile::tempdir().unwrap();
    let harness = ClaudeHarness::with_data_dir(data.path().to_owned(), Default::default());
    let summary = harness.open_file(&fixture_path()).unwrap();
    assert_eq!(harness.stats(&summary).unwrap().turns, 0);
}

#[test]
fn a_file_with_nothing_else_streams_no_list() {
    for text in [
        String::new(),
        attachment(1),
        output(1, "x"),
        caveat(1),
        system(1, None),
        prompt(1, "hello") + &echo(2, "effort", ""),
    ] {
        let (_dir, path) = temp_file(&text);
        let (_, _, updates) = stream(&path);
        assert!(
            !updates
                .iter()
                .any(|u| matches!(u, ModelUpdate::LocalCommands(_))),
            "{text:?}: {updates:?}"
        );
    }
}

#[test]
fn held_entries_after_a_trailing_compaction_stay_held_and_are_not_listed() {
    let text = prompt(1, "hi")
        + &reply(2, "m1")
        + &compaction(3)
        + &summary(4)
        + &echo(5, "effort", "")
        + &output(6, "Cancelled");
    let (_dir, path) = temp_file(&text);
    let (_, _, updates) = stream(&path);
    assert!(
        !updates
            .iter()
            .any(|u| matches!(u, ModelUpdate::LocalCommands(_))),
        "{updates:?}"
    );
    let mut turns = Vec::new();
    for update in &updates {
        update.apply(&mut turns);
    }
    let shape: Vec<&str> = turns
        .iter()
        .map(|t| match &**t {
            Turn::User(_) => "U",
            Turn::Assistant(_) => "A",
            Turn::Compaction(_) => "C",
        })
        .collect();
    assert_eq!(shape, ["U", "A", "C"]);
    let injected = turns.iter().any(|t| match &**t {
        Turn::User(u) => u.items.iter().any(|i| matches!(i, Item::Injected { .. })),
        Turn::Assistant(a) => a.items.iter().any(|i| matches!(i, Item::Injected { .. })),
        Turn::Compaction(_) => false,
    });
    assert!(!injected, "the entries are still held: {turns:?}");

    // They appear once a later turn arrives.
    let more = text + &prompt(7, "later");
    let turns = assembled(&more).into_turns();
    let Turn::User(last) = &*turns[3] else {
        panic!("{:?}", turns[3]);
    };
    let Some(Item::Injected { entries, .. }) = last.items.first() else {
        panic!("{:?}", last.items);
    };
    let texts: Vec<&str> = entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, ["/effort", "Cancelled"]);
}

#[test]
fn the_commands_join_the_first_user_turn_when_it_arrives() {
    let text = caveat(1) + &echo(2, "effort", "") + &output(3, "Cancelled") + &prompt(4, "go");
    let turns = assembled(&text).into_turns();
    assert_eq!(turns.len(), 1);
    let Turn::User(first) = &*turns[0] else {
        panic!("{:?}", turns[0]);
    };
    let Some(Item::Injected { entries, .. }) = first.items.first() else {
        panic!("{:?}", first.items);
    };
    let texts: Vec<&str> = entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts[1..], ["/effort", "Cancelled"]);
}

// ------------------------------------------------------------------ live

struct Live {
    _data: TempDir,
    file: PathBuf,
    rx: Receiver<ModelUpdate>,
    _handle: WatchHandle,
    seen: Vec<ModelUpdate>,
}

impl Live {
    fn start(content: &str) -> Self {
        let data = tempfile::tempdir().unwrap();
        let project = data.path().join("projects").join("-p");
        fs::create_dir_all(&project).unwrap();
        let file = project.join("live.jsonl");
        fs::write(&file, content).unwrap();
        let harness =
            ClaudeHarness::with_data_dir(data.path().join("projects"), Default::default());
        let (tx, rx) = mpsc::channel();
        let handle = harness.load(&SessionId::from("live"), tx).unwrap();
        let mut live = Self {
            _data: data,
            file,
            rx,
            _handle: handle,
            seen: Vec::new(),
        };
        live.wait_for("Loaded and Activity", |seen| {
            seen.iter()
                .skip_while(|u| **u != ModelUpdate::Loaded)
                .any(|u| matches!(u, ModelUpdate::Activity { .. }))
        });
        live.seen.clear();
        live
    }

    fn append(&self, bytes: &str) {
        let mut file = OpenOptions::new().append(true).open(&self.file).unwrap();
        file.write_all(bytes.as_bytes()).unwrap();
    }

    fn wait_for(&mut self, what: &str, done: impl Fn(&[ModelUpdate]) -> bool) {
        let deadline = Instant::now() + WAIT;
        while !done(&self.seen) {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(update) => self.seen.push(update),
                Err(RecvTimeoutError::Timeout) => {
                    panic!("timed out waiting for {what}; saw {:#?}", self.seen)
                }
                Err(RecvTimeoutError::Disconnected) => panic!("disconnected waiting for {what}"),
            }
        }
    }
}

fn lists(updates: &[ModelUpdate]) -> Vec<&Vec<String>> {
    updates
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::LocalCommands(list) => Some(list),
            _ => None,
        })
        .collect()
}

#[test]
fn an_appended_command_sends_the_whole_list_again() {
    let mut live = Live::start(&(echo(1, "effort", "") + &output(2, "Cancelled")));
    live.append(&(echo(3, "model", "opus") + &output(4, "Set")));
    live.wait_for("the longer list", |seen| {
        seen.contains(&list(&["/effort", "/model opus"]))
    });
    assert_eq!(lists(&live.seen).len(), 1, "{:?}", live.seen);
    assert!(
        !live
            .seen
            .iter()
            .any(|u| matches!(u, ModelUpdate::Appended(_) | ModelUpdate::Updated(_)))
    );
}

#[test]
fn an_append_that_changes_no_command_sends_no_list() {
    let mut live = Live::start(&echo(1, "effort", ""));
    live.append(&output(2, "Cancelled"));
    live.wait_for("an activity", |seen| {
        seen.iter()
            .any(|u| matches!(u, ModelUpdate::Activity { .. }))
    });
    assert!(lists(&live.seen).is_empty(), "{:?}", live.seen);
}

#[test]
fn an_appended_prompt_brings_the_first_turn_with_the_commands_and_no_new_list() {
    let mut live = Live::start(&(echo(1, "effort", "") + &output(2, "Cancelled")));
    live.append(&(prompt(3, "go") + &reply(4, "m1")));
    live.wait_for("the turns", |seen| {
        seen.iter().any(|u| matches!(u, ModelUpdate::Appended(_)))
    });
    let mut turns = Vec::new();
    for update in &live.seen {
        update.apply(&mut turns);
    }
    let Turn::User(first) = &*turns[0] else {
        panic!("{:?}", turns[0]);
    };
    assert_eq!(first.text, "go");
    let Some(Item::Injected { entries, .. }) = first.items.first() else {
        panic!("{:?}", first.items);
    };
    let texts: Vec<&str> = entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, ["/effort", "Cancelled"]);
    assert!(lists(&live.seen).is_empty(), "{:?}", live.seen);
}

#[test]
fn a_command_after_the_first_turn_sends_no_list() {
    let mut live = Live::start(&(prompt(1, "go") + &reply(2, "m1")));
    live.append(&echo(3, "effort", ""));
    live.wait_for("an activity", |seen| {
        seen.iter()
            .any(|u| matches!(u, ModelUpdate::Activity { .. }))
    });
    assert!(lists(&live.seen).is_empty(), "{:?}", live.seen);
}

#[test]
fn reloading_a_truncated_file_resets_and_lists_what_the_new_content_holds() {
    let mut live = Live::start(&(echo(1, "effort", "") + &echo(2, "clear", "") + &output(3, "x")));
    fs::write(&live.file, echo(9, "model", "opus")).unwrap();
    live.wait_for("a reset and the new list", |seen| {
        seen.iter()
            .position(|u| *u == ModelUpdate::Reset)
            .is_some_and(|reset| {
                seen[reset..].contains(&list(&["/model opus"]))
                    && seen[reset..].contains(&ModelUpdate::Loaded)
            })
    });
}

#[test]
fn reloading_into_a_file_with_turns_sends_a_reset_and_no_list() {
    let mut live = Live::start(&(echo(1, "effort", "") + &echo(2, "clear", "") + &output(3, "x")));
    fs::write(&live.file, prompt(9, "hi")).unwrap();
    live.wait_for("a reset and Loaded", |seen| {
        seen.iter()
            .position(|u| *u == ModelUpdate::Reset)
            .is_some_and(|reset| seen[reset..].contains(&ModelUpdate::Loaded))
    });
    assert!(lists(&live.seen).is_empty(), "{:?}", live.seen);
}
