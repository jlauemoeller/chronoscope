//! Local commands (`/effort`, `/clear`, ...) are injected `local_command`
//! entries and never start a turn (SPEC cycle 3 §2), against the fixtures
//! `local_commands.jsonl` and `local_commands_mixed.jsonl` and constructed
//! lines.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use chronoscope::components::{EntryKind, TurnRows};
use chronoscope::harness::Harness;
use chronoscope::harness::claude::{Assembler, ClaudeHarness};
use chronoscope::model::{
    AssistantTurn, ByteSource, InjectedEntry, Item, ModelUpdate, SessionSummary, Turn, TurnSources,
    UserTurn,
};

// ---------------------------------------------------------------- helpers

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude")
}

fn fixture_bytes(name: &str) -> Arc<[u8]> {
    Arc::from(fs::read(fixtures_dir().join(name)).unwrap())
}

fn whole(name: &str) -> Vec<Arc<Turn>> {
    Assembler::from_memory(fixture_bytes(name)).into_turns()
}

fn user(turn: &Turn) -> &UserTurn {
    match turn {
        Turn::User(t) => t,
        other => panic!("expected a User turn, got {other:?}"),
    }
}

fn assistant(turn: &Turn) -> &AssistantTurn {
    match turn {
        Turn::Assistant(t) => t,
        other => panic!("expected an Assistant turn, got {other:?}"),
    }
}

fn shape(turns: &[Arc<Turn>]) -> String {
    turns
        .iter()
        .map(|t| match &**t {
            Turn::User(_) => 'U',
            Turn::Assistant(_) => 'A',
            Turn::Compaction(_) => 'C',
        })
        .collect()
}

fn injected(item: &Item) -> &[InjectedEntry] {
    match item {
        Item::Injected { entries, .. } => entries,
        other => panic!("expected an Injected item, got {other:?}"),
    }
}

/// `(kind, text)` of each entry.
fn pairs(entries: &[InjectedEntry]) -> Vec<(&str, &str)> {
    entries
        .iter()
        .map(|e| (e.kind.as_str(), e.text.as_str()))
        .collect()
}

fn item_names(items: &[Item]) -> Vec<&'static str> {
    items
        .iter()
        .map(|i| match i {
            Item::Text { .. } => "text",
            Item::Injected { .. } => "injected",
            Item::Thinking { .. } => "thinking",
            Item::ToolCall(_) => "tool",
            Item::ModelChange { .. } => "model",
            Item::Notice { .. } => "notice",
        })
        .collect()
}

fn lines_with_ranges(bytes: &[u8]) -> Vec<(&[u8], std::ops::Range<u64>)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            out.push((&bytes[start..i], start as u64..i as u64));
            start = i + 1;
        }
    }
    out
}

/// A user line without `origin`, as local commands are recorded.
fn plain_user(ts: &str, text: &str) -> String {
    format!(
        r#"{{"type":"user","cwd":"/home/user/project","timestamp":"{ts}","message":{{"role":"user","content":{}}}}}"#,
        serde_json::to_string(text).unwrap()
    )
}

fn human_user(ts: &str, text: &str) -> String {
    format!(
        r#"{{"type":"user","cwd":"/home/user/project","timestamp":"{ts}","origin":{{"kind":"human"}},"message":{{"role":"user","content":{}}}}}"#,
        serde_json::to_string(text).unwrap()
    )
}

fn reply(ts: &str, msg: &str) -> String {
    format!(
        r#"{{"type":"assistant","timestamp":"{ts}","message":{{"id":"{msg}","model":"claude-sonnet-5-5","role":"assistant","content":[{{"type":"text","text":"ok"}}]}}}}"#
    )
}

fn assemble_lines(lines: &[String]) -> Vec<Arc<Turn>> {
    let mut text = lines.join("\n");
    text.push('\n');
    Assembler::from_memory(Arc::from(text.as_bytes())).into_turns()
}

const ECHO_EFFORT: &str = "<command-name>/effort</command-name>\n            <command-message>effort</command-message>\n            <command-args></command-args>";
const OUT_CANCELLED: &str = "<local-command-stdout>Cancelled</local-command-stdout>";

// ------------------------------------------------- the §2.5 fixture sequence

#[test]
fn the_fixture_sequence_gives_one_user_turn_and_one_assistant_turn() {
    let turns = whole("local_commands.jsonl");
    assert_eq!(shape(&turns), "UA");
    let prompt = user(&turns[0]);
    assert!(
        prompt
            .text
            .starts_with("<command-message>factory</command-message>")
    );
    assert_eq!(item_names(&prompt.items), ["injected"]);
    assert_eq!(
        pairs(injected(&prompt.items[0])),
        [
            (
                "meta",
                "<local-command-caveat>The command below was run locally.</local-command-caveat>"
            ),
            ("local_command", "/effort"),
            ("local_command", "Cancelled"),
        ]
    );
    // The echo and its output are separate entries with their own times.
    let entries = injected(&prompt.items[0]);
    assert_eq!(
        entries[1].timestamp.to_rfc3339(),
        "2026-10-21T08:35:11+00:00"
    );
    assert_eq!(
        entries[2].timestamp.to_rfc3339(),
        "2026-10-21T08:35:12+00:00"
    );
    assert_eq!(assistant(&turns[1]).items.len(), 1);
}

// ------------------------------------------------------------- placement

#[test]
fn local_commands_never_add_a_turn() {
    let turns = whole("local_commands_mixed.jsonl");
    // First prompt and its Assistant turn (the /model and /clear in it), the
    // second prompt (the /effort before it) and its reply, the three prompts
    // that stay prompts, the older-format `<command-message>` prompt and the
    // two `!` shell entries.
    assert_eq!(shape(&turns), "UAUAUAUAUAUUA");
    let prompts: Vec<&str> = turns
        .iter()
        .filter_map(|t| match &**t {
            Turn::User(u) => Some(u.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(prompts.len(), 7);
    assert_eq!(prompts[0], "First prompt");
    assert_eq!(prompts[1], "Second prompt");
}

#[test]
fn a_local_command_before_a_prompt_lands_in_that_users_turn() {
    let turns = whole("local_commands_mixed.jsonl");
    let second = user(&turns[2]);
    assert_eq!(second.text, "Second prompt");
    assert_eq!(item_names(&second.items), ["injected"]);
    assert_eq!(
        pairs(injected(&second.items[0])),
        [("local_command", "/effort"), ("local_command", "Cancelled")]
    );
    // It is not in the Assistant turn before.
    let before = assistant(&turns[1]);
    assert!(
        !before
            .items
            .iter()
            .any(|i| injected_pairs(i).contains(&("local_command", "/effort")))
    );
}

fn injected_pairs(item: &Item) -> Vec<(&str, &str)> {
    match item {
        Item::Injected { entries, .. } => pairs(entries),
        _ => Vec::new(),
    }
}

#[test]
fn assistant_entries_after_a_local_command_continue_the_same_turn() {
    let turns = whole("local_commands_mixed.jsonl");
    let first = assistant(&turns[1]);
    assert_eq!(
        item_names(&first.items),
        ["text", "injected", "text", "injected"]
    );
    assert_eq!(
        pairs(injected(&first.items[1])),
        [
            ("local_command", "/model opus"),
            ("local_command", "Set model to opus")
        ]
    );
    // Two Assistant messages in one turn.
    let stats = first.stats.as_ref().expect("the footer");
    assert_eq!(stats.message_count, 2);
    // The footer runs from the prompt (09:00:00) to the last turn_duration
    // (09:00:30), and not from the entry after the local command.
    assert_eq!(stats.duration, Duration::from_secs(30));
}

#[test]
fn a_local_command_at_the_end_of_the_input_lands_in_the_current_turn() {
    let turns = whole("local_commands_mixed.jsonl");
    let last = assistant(turns.last().unwrap());
    let tail = last.items.last().unwrap();
    assert_eq!(
        pairs(injected(tail)),
        [("local_command", "/exit"), ("local_command", "Bye!")]
    );
}

#[test]
fn an_echo_and_a_system_local_command_for_the_same_command_both_stay() {
    let turns = whole("local_commands_mixed.jsonl");
    let first = assistant(&turns[1]);
    // `/clear` from the echo, and `/clear` with its output from the `system`
    // entry (unchanged by this task): deliberately both.
    assert_eq!(
        pairs(injected(&first.items[3])),
        [
            ("local_command", "/clear"),
            ("local_command", "/clear\nAll clear")
        ]
    );
}

#[test]
fn a_local_command_in_a_session_that_starts_with_one_opens_the_first_user_turn() {
    let turns = assemble_lines(&[
        plain_user("2026-10-24T08:00:00.000Z", ECHO_EFFORT),
        plain_user("2026-10-24T08:00:01.000Z", OUT_CANCELLED),
        human_user("2026-10-24T08:00:02.000Z", "Real prompt"),
        reply("2026-10-24T08:00:03.000Z", "m1"),
    ]);
    assert_eq!(shape(&turns), "UA");
    assert_eq!(user(&turns[0]).text, "Real prompt");
    assert_eq!(
        pairs(injected(&user(&turns[0]).items[0])),
        [("local_command", "/effort"), ("local_command", "Cancelled")]
    );
}

#[test]
fn a_local_command_between_two_prompts_adds_no_sidebar_row() {
    let prompt1 = human_user("2026-10-24T08:00:00.000Z", "one");
    let reply1 = reply("2026-10-24T08:00:01.000Z", "m1");
    let prompt2 = human_user("2026-10-24T08:00:10.000Z", "two");
    let reply2 = reply("2026-10-24T08:00:11.000Z", "m2");
    let with = assemble_lines(&[
        prompt1.clone(),
        reply1.clone(),
        plain_user("2026-10-24T08:00:05.000Z", ECHO_EFFORT),
        plain_user("2026-10-24T08:00:06.000Z", OUT_CANCELLED),
        prompt2.clone(),
        reply2.clone(),
    ]);
    let without = assemble_lines(&[prompt1, reply1, prompt2, reply2]);
    let kinds = |turns: &[Arc<Turn>]| {
        let rows = TurnRows::from_turns(turns.iter().map(|t| &**t));
        (0..rows.entry_count())
            .map(|i| rows.entry(i).unwrap().1.kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        kinds(&with),
        [
            EntryKind::User,
            EntryKind::Assistant,
            EntryKind::User,
            EntryKind::Assistant
        ]
    );
    assert_eq!(kinds(&with), kinds(&without));
    assert_eq!(shape(&with), shape(&without));
}

// ------------------------------------------------------- what stays a prompt

#[test]
fn prompts_that_look_like_local_commands_stay_prompts() {
    let turns = whole("local_commands_mixed.jsonl");
    let texts: Vec<&str> = turns
        .iter()
        .filter_map(|t| match &**t {
            Turn::User(u) => Some(u.text.as_str()),
            _ => None,
        })
        .collect();
    // ` /clear` with a leading space and `origin.kind` human.
    assert!(texts.contains(&" /clear"));
    // `<command-name>` text with `origin.kind` human.
    assert!(texts.contains(&"<command-name>/clear</command-name>"));
    // `<command-message>` without any `origin`: the older-format rule.
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("<command-message>doctor"))
    );
    // The `!` shell commands keep their rows.
    assert!(texts.contains(&"<bash-input>ls</bash-input>"));
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("<bash-stdout>Cargo.toml</bash-stdout>"))
    );
    // The only User turn holding local commands is the second prompt's.
    let holders: Vec<&str> = turns
        .iter()
        .filter_map(|t| match &**t {
            Turn::User(u) if u.items.iter().any(|i| !injected_pairs(i).is_empty()) => {
                Some(u.text.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(holders, ["Second prompt"]);
}

#[test]
fn a_bash_stderr_entry_and_a_command_message_without_origin_are_prompts() {
    let turns = assemble_lines(&[
        plain_user(
            "2026-10-24T08:00:00.000Z",
            "<bash-stderr>oops</bash-stderr>",
        ),
        plain_user(
            "2026-10-24T08:00:01.000Z",
            "<command-message>doctor</command-message>\n<command-name>/doctor</command-name>",
        ),
    ]);
    assert_eq!(shape(&turns), "UU");
}

// ------------------------------------------------------ batch independence

/// A turn without its injected items and without `sources`, which record the
/// held lines.
fn without_injected(turn: &Turn) -> Turn {
    let mut turn = turn.clone();
    match &mut turn {
        Turn::User(t) => {
            t.items.retain(|i| !matches!(i, Item::Injected { .. }));
            t.sources = TurnSources::new();
        }
        Turn::Assistant(t) => {
            t.items.retain(|i| !matches!(i, Item::Injected { .. }));
            t.sources = TurnSources::new();
        }
        Turn::Compaction(t) => t.sources = TurnSources::new(),
    }
    turn
}

/// The `local_command` entries in turn and item order.
fn local_commands(turns: &[Arc<Turn>]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for turn in turns {
        let items: &[Item] = match &**turn {
            Turn::User(t) => &t.items,
            Turn::Assistant(t) => &t.items,
            Turn::Compaction(_) => &[],
        };
        for item in items {
            if let Item::Injected { entries, .. } = item {
                out.extend(
                    entries
                        .iter()
                        .filter(|e| e.kind == "local_command")
                        .map(|e| (e.timestamp.to_rfc3339(), e.text.clone())),
                );
            }
        }
    }
    out
}

fn streamed_line_by_line(bytes: &Arc<[u8]>) -> Vec<Arc<Turn>> {
    let mut asm = Assembler::new(ByteSource::Memory(Arc::clone(bytes)));
    let mut replayed: Vec<Arc<Turn>> = Vec::new();
    for (line, range) in lines_with_ranges(bytes) {
        for update in asm.push_line(line, range) {
            update.apply(&mut replayed);
        }
        // What the live path does at the end of every batch.
        for update in asm.flush_held() {
            update.apply(&mut replayed);
        }
    }
    replayed
}

#[test]
fn loading_at_once_and_streaming_split_at_every_line_give_the_same_turns_and_entries() {
    for name in ["local_commands.jsonl", "local_commands_mixed.jsonl"] {
        let bytes = fixture_bytes(name);
        let at_once = whole(name);
        let streamed = streamed_line_by_line(&bytes);
        assert_eq!(
            streamed
                .iter()
                .map(|t| without_injected(t))
                .collect::<Vec<_>>(),
            at_once
                .iter()
                .map(|t| without_injected(t))
                .collect::<Vec<_>>(),
            "{name}: turns"
        );
        let expected = local_commands(&at_once);
        assert!(!expected.is_empty());
        assert_eq!(local_commands(&streamed), expected, "{name}: entries");
    }
}

#[test]
fn a_batch_ending_between_a_local_command_and_the_next_prompt_keeps_it_in_the_current_turn() {
    let bytes = fixture_bytes("local_commands_mixed.jsonl");
    let streamed = streamed_line_by_line(&bytes);
    // Reloading places the /effort in the next turn; live it stays in the
    // current one (SPEC §2.2). The turns themselves are the same.
    let before = assistant(&streamed[1]);
    assert!(
        before
            .items
            .iter()
            .any(|i| injected_pairs(i).contains(&("local_command", "/effort")))
    );
    assert!(
        user(&streamed[2])
            .items
            .iter()
            .all(|i| injected_pairs(i).is_empty())
    );
}

// -------------------------------------------- the selector against the sidebar

fn jsonl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            jsonl_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            out.push(path);
        }
    }
}

fn loaded_turns(harness: &dyn Harness, summary: &SessionSummary) -> Vec<Arc<Turn>> {
    let (tx, rx) = mpsc::channel();
    let _handle = harness.load(&summary.id, tx).unwrap();
    let mut turns = Vec::new();
    loop {
        let update = rx
            .recv_timeout(Duration::from_secs(30))
            .expect("the load finishes");
        let done = update == ModelUpdate::Loaded;
        update.apply(&mut turns);
        if done {
            return turns;
        }
    }
}

#[test]
fn for_every_claude_fixture_the_selectors_turn_count_is_the_number_of_user_rows() {
    let mut files = Vec::new();
    jsonl_files(&fixtures_dir(), &mut files);
    assert!(files.len() >= 10, "{files:?}");
    let data = tempfile::tempdir().unwrap();
    let harness = ClaudeHarness::with_data_dir(data.path().to_owned(), Default::default());
    for path in files {
        let summary = harness.open_file(&path).unwrap();
        let turns = loaded_turns(&harness, &summary);
        let rows = TurnRows::from_turns(turns.iter().map(|t| &**t));
        let user_rows = (0..rows.entry_count())
            .filter(|i| rows.entry(*i).unwrap().1.kind == EntryKind::User)
            .count();
        let stats = harness.stats(&summary).unwrap();
        assert_eq!(stats.turns, user_rows, "{}", path.display());
    }
}

// ------------------------------------------------------------ discovery

fn summary_of(text: &str) -> SessionSummary {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s1.jsonl");
    fs::write(&path, text).unwrap();
    let data = tempfile::tempdir().unwrap();
    ClaudeHarness::with_data_dir(data.path().to_owned(), Default::default())
        .open_file(&path)
        .unwrap()
}

fn joined(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

#[test]
fn a_session_that_began_with_a_local_command_is_titled_from_its_next_prompt() {
    let summary = summary_of(&joined(&[
        plain_user("2026-10-24T08:00:00.000Z", ECHO_EFFORT),
        plain_user("2026-10-24T08:00:01.000Z", OUT_CANCELLED),
        human_user("2026-10-24T08:00:02.000Z", "Fix the build\nplease"),
    ]));
    assert_eq!(summary.title, "Fix the build");
    // Search over the first prompt follows.
    assert_eq!(
        summary.first_prompt.as_deref(),
        Some("Fix the build\nplease")
    );
}

#[test]
fn a_session_with_only_local_commands_is_untitled_and_has_no_first_prompt() {
    let summary = summary_of(&joined(&[
        plain_user("2026-10-24T08:00:00.000Z", ECHO_EFFORT),
        plain_user("2026-10-24T08:00:01.000Z", OUT_CANCELLED),
    ]));
    assert_eq!(summary.title, "(untitled)");
    assert_eq!(summary.first_prompt, None);
}

#[test]
fn an_ai_title_still_wins_over_the_prompt() {
    let summary = summary_of(&joined(&[
        plain_user("2026-10-24T08:00:00.000Z", ECHO_EFFORT),
        r#"{"type":"ai-title","aiTitle":"Titled by the model"}"#.to_owned(),
    ]));
    assert_eq!(summary.title, "Titled by the model");
}

fn turn_duration(ts: &str, ms: u64) -> String {
    format!(
        r#"{{"type":"system","subtype":"turn_duration","timestamp":"{ts}","durationMs":{ms},"messageCount":1}}"#
    )
}

#[test]
fn a_local_command_between_a_prompt_and_its_first_reply_leaves_the_footer_start_alone() {
    let turns = assemble_lines(&[
        human_user("2026-10-24T08:00:00.000Z", "go"),
        plain_user("2026-10-24T08:00:10.000Z", ECHO_EFFORT),
        plain_user("2026-10-24T08:00:11.000Z", OUT_CANCELLED),
        reply("2026-10-24T08:00:20.000Z", "m1"),
        // `durationMs` differs from the span, which is the one that counts.
        turn_duration("2026-10-24T08:00:30.000Z", 7000),
    ]);
    assert_eq!(shape(&turns), "UA");
    // From the prompt (08:00:00), not from the local command or the reply.
    let stats = assistant(&turns[1]).stats.as_ref().expect("the footer");
    assert_eq!(stats.duration, Duration::from_secs(30));
    // The entries were placed in the open User turn, and the Assistant turn
    // continued after them.
    let entries: Vec<_> = turns
        .iter()
        .flat_map(|t| match &**t {
            Turn::User(u) => u.items.clone(),
            Turn::Assistant(a) => a.items.clone(),
            Turn::Compaction(_) => Vec::new(),
        })
        .flat_map(|i| {
            injected_pairs(&i)
                .into_iter()
                .map(|(k, t)| (k.to_owned(), t.to_owned()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        entries,
        [
            ("local_command".to_owned(), "/effort".to_owned()),
            ("local_command".to_owned(), "Cancelled".to_owned())
        ]
    );
}

#[test]
fn a_local_command_at_the_end_of_the_input_after_a_prompt_lands_in_that_user_turn() {
    let turns = assemble_lines(&[
        human_user("2026-10-24T08:00:00.000Z", "go"),
        plain_user(
            "2026-10-24T08:00:10.000Z",
            "<command-name>/exit</command-name>",
        ),
        plain_user(
            "2026-10-24T08:00:11.000Z",
            "<local-command-stdout>Bye!</local-command-stdout>",
        ),
    ]);
    assert_eq!(shape(&turns), "U");
    let prompt = user(&turns[0]);
    assert_eq!(item_names(&prompt.items), ["injected"]);
    assert_eq!(
        pairs(injected(&prompt.items[0])),
        [("local_command", "/exit"), ("local_command", "Bye!")]
    );
}
