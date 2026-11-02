//! Claude injected content, noise, compaction and parse warnings, against
//! fixture transcripts and constructed lines.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, Once};

use chronoscope::harness::claude::{Assembler, NEWEST_KNOWN_VERSION};
use chronoscope::model::{
    AssistantTurn, ByteSource, CompactionTurn, InjectedEntry, Item, ModelUpdate, Turn, UserTurn,
};
use log::{Level, LevelFilter, Log, Metadata, Record};

// ------------------------------------------------------------ test logger

thread_local! {
    static RECORDS: RefCell<Vec<(Level, String)>> = const { RefCell::new(Vec::new()) };
}

struct CaptureLogger;

impl Log for CaptureLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        RECORDS.with(|r| {
            r.borrow_mut()
                .push((record.level(), record.args().to_string()));
        });
    }

    fn flush(&self) {}
}

/// Installs the capturing logger. Records are kept per thread, so parallel
/// tests do not see each other's output.
fn capture_logs() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        log::set_logger(&CaptureLogger).expect("no other logger in this test binary");
        log::set_max_level(LevelFilter::Debug);
    });
    RECORDS.with(|r| r.borrow_mut().clear());
}

fn logged(level: Level) -> Vec<String> {
    RECORDS.with(|r| {
        r.borrow()
            .iter()
            .filter(|(l, _)| *l == level)
            .map(|(_, m)| m.clone())
            .collect()
    })
}

// ---------------------------------------------------------------- helpers

fn fixture_bytes(name: &str) -> Arc<[u8]> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude")
        .join(name);
    Arc::from(std::fs::read(path).unwrap())
}

fn assemble(text: &str) -> Assembler {
    Assembler::from_memory(Arc::from(text.as_bytes()))
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

fn compaction(turn: &Turn) -> &CompactionTurn {
    match turn {
        Turn::Compaction(t) => t,
        other => panic!("expected a Compaction turn, got {other:?}"),
    }
}

fn injected(item: &Item) -> &[InjectedEntry] {
    match item {
        Item::Injected { entries, .. } => entries,
        other => panic!("expected an Injected item, got {other:?}"),
    }
}

fn kinds(entries: &[InjectedEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.kind.as_str()).collect()
}

fn shape(turns: &[Arc<Turn>]) -> Vec<&'static str> {
    turns
        .iter()
        .map(|t| match &**t {
            Turn::User(_) => "user",
            Turn::Assistant(_) => "assistant",
            Turn::Compaction(_) => "compaction",
        })
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

/// A minimal human prompt line.
fn prompt(uuid: &str, text: &str) -> String {
    format!(
        r#"{{"type":"user","uuid":"{uuid}","timestamp":"2026-10-24T08:00:00.000Z","origin":{{"kind":"human"}},"message":{{"role":"user","content":"{text}"}}}}"#
    )
}

/// A minimal assistant text line.
fn reply(uuid: &str, msg: &str, text: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"{uuid}","timestamp":"2026-10-24T08:00:01.000Z","message":{{"id":"{msg}","model":"claude-sonnet-5-5","role":"assistant","content":[{{"type":"text","text":"{text}"}}]}}}}"#
    )
}

/// A minimal attachment line.
fn attachment(uuid: &str, ts: &str, body: &str) -> String {
    format!(r#"{{"type":"attachment","uuid":"{uuid}","timestamp":"{ts}","attachment":{body}}}"#)
}

fn lines(parts: &[String]) -> String {
    let mut out = parts.join("\n");
    out.push('\n');
    out
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

// ------------------------------------------------------------- placement

#[test]
fn injected_fixture_has_the_expected_turns() {
    let turns = assemble_fixture("injected.jsonl").into_turns();
    assert_eq!(
        shape(&turns),
        [
            "user",
            "assistant",
            "user",
            "assistant",
            "user",
            "assistant"
        ]
    );
}

fn assemble_fixture(name: &str) -> Assembler {
    Assembler::from_memory(fixture_bytes(name))
}

#[test]
fn entries_before_a_human_prompt_open_that_user_turn() {
    let turns = assemble_fixture("injected.jsonl").into_turns();
    let first = user(&turns[0]);
    assert_eq!(first.text, "Please fix the build.");
    assert_eq!(item_names(&first.items), ["injected"]);

    let entries = injected(&first.items[0]);
    assert_eq!(
        kinds(entries),
        ["opened_file_in_ide", "instructions", "file"]
    );
    // The first prompt is not preceded by an Assistant turn to hold them.
    assert_eq!(turns.len(), 6);
}

#[test]
fn three_entries_with_noise_between_collapse_into_one_item_with_kind_time_and_text() {
    let turns = assemble_fixture("injected.jsonl").into_turns();
    let entries = injected(&user(&turns[0]).items[0]);
    assert_eq!(entries.len(), 3);

    assert_eq!(entries[0].kind, "opened_file_in_ide");
    assert_eq!(
        entries[0].timestamp.to_rfc3339(),
        "2026-10-22T09:00:00+00:00"
    );
    assert_eq!(entries[0].text, "/home/user/project/src/lib.rs");

    assert_eq!(entries[1].kind, "instructions");
    assert_eq!(
        entries[1].timestamp.to_rfc3339(),
        "2026-10-22T09:00:00.200+00:00"
    );
    assert_eq!(
        entries[1].text,
        "/home/user/project/CLAUDE.md\n# Project rules\nBe brief.\n\n\
         /home/user/.claude/memory/MEMORY.md\n- no memories yet"
    );

    assert_eq!(entries[2].kind, "file");
    assert_eq!(
        entries[2].timestamp.to_rfc3339(),
        "2026-10-22T09:00:00.400+00:00"
    );
    assert_eq!(
        entries[2].text,
        "/home/user/project/notes.md\n# Notes\nremember the milk"
    );
}

#[test]
fn entries_in_the_middle_of_assistant_activity_stay_at_that_position() {
    let turns = assemble_fixture("injected.jsonl").into_turns();
    let turn = assistant(&turns[1]);
    assert_eq!(item_names(&turn.items), ["tool", "injected", "text"]);
    let entries = injected(&turn.items[1]);
    assert_eq!(kinds(entries), ["hook_system_message", "queued_command"]);
    assert_eq!(entries[0].text, "Tip: try the other profile.");
    assert_eq!(
        entries[1].text,
        "<task-notification>build finished</task-notification>"
    );
    assert!(turn.stats.is_some(), "the turn_duration footer still lands");
}

#[test]
fn system_and_meta_entries_use_their_kind_and_readable_text() {
    let turns = assemble_fixture("injected.jsonl").into_turns();
    let second = user(&turns[2]);
    assert_eq!(second.text, "Now run the tests.");
    let entries = injected(&second.items[0]);
    assert_eq!(
        kinds(entries),
        ["meta", "local_command", "informational", "away_summary"]
    );
    assert_eq!(
        entries[0].text,
        "<local-command-caveat>The command below was run locally.</local-command-caveat>"
    );
    assert_eq!(entries[1].text, "/clear\nAll clear");
    assert_eq!(entries[2].text, "Compacted while idle");
    assert_eq!(entries[3].text, "You were fixing the build; it passes.");
    assert_eq!(
        entries[3].timestamp.to_rfc3339(),
        "2026-10-22T09:01:00.300+00:00"
    );
}

#[test]
fn trailing_entries_collapse_into_the_previous_injected_item_of_the_turn() {
    let turns = assemble_fixture("injected.jsonl").into_turns();
    let turn = assistant(&turns[5]);
    // Text, then selected lines + peer message + (after the footer) a
    // directory listing and the plan mode entries: all consecutive, so one item.
    assert_eq!(item_names(&turn.items), ["text", "injected"]);
    let entries = injected(&turn.items[1]);
    assert_eq!(
        kinds(entries),
        [
            "selected_lines_in_ide",
            "queued_command",
            "directory",
            "plan_mode",
            "plan_mode_exit"
        ]
    );
    assert_eq!(
        entries[0].text,
        "/home/user/project/src/lib.rs:8-9\nfn main() {}"
    );
    assert_eq!(entries[1].text, "[Peer] status update");
    assert_eq!(entries[2].text, "/home/user/project/src\nlib.rs\nmain.rs");
    assert_eq!(entries[3].text, "/home/user/.claude/plans/tidy-plan.md");
    assert_eq!(entries[4].text, "/home/user/.claude/plans/tidy-plan.md");
    assert!(turn.stats.is_some());
}

#[test]
fn entries_between_a_prompt_and_the_first_reply_belong_to_the_prompt_turn() {
    let text = lines(&[
        prompt("u1", "hello"),
        attachment(
            "a1",
            "2026-10-24T08:00:00.500Z",
            r#"{"type":"hook_system_message","content":"from a hook"}"#,
        ),
        reply("r1", "msg_1", "hi"),
    ]);
    let turns = assemble(&text).into_turns();
    assert_eq!(shape(&turns), ["user", "assistant"]);
    assert_eq!(
        kinds(injected(&user(&turns[0]).items[0])),
        ["hook_system_message"]
    );
    assert_eq!(item_names(&assistant(&turns[1]).items), ["text"]);
}

#[test]
fn entries_before_the_very_first_turn_go_into_it() {
    let text = lines(&[
        attachment(
            "a1",
            "2026-10-24T08:00:00.000Z",
            r#"{"type":"opened_file_in_ide","filename":"/p/a.rs"}"#,
        ),
        reply("r1", "msg_1", "no prompt in this transcript"),
    ]);
    let turns = assemble(&text).into_turns();
    assert_eq!(shape(&turns), ["assistant"]);
    assert_eq!(
        item_names(&assistant(&turns[0]).items),
        ["injected", "text"]
    );
}

#[test]
fn entries_with_no_turn_to_hold_them_are_not_invented_into_one() {
    let text = lines(&[attachment(
        "a1",
        "2026-10-24T08:00:00.000Z",
        r#"{"type":"opened_file_in_ide","filename":"/p/a.rs"}"#,
    )]);
    assert!(assemble(&text).into_turns().is_empty());
}

#[test]
fn a_flush_at_the_end_of_input_puts_trailing_entries_in_the_current_turn() {
    let text = lines(&[
        prompt("u1", "hello"),
        reply("r1", "msg_1", "hi"),
        attachment(
            "a1",
            "2026-10-24T08:00:02.000Z",
            r#"{"type":"hook_system_message","content":"late"}"#,
        ),
    ]);
    let bytes: Arc<[u8]> = Arc::from(text.as_bytes());
    let mut asm = Assembler::new(ByteSource::Memory(Arc::clone(&bytes)));
    let mut replayed = Vec::new();
    for update in asm.push_chunk(&bytes) {
        update.apply(&mut replayed);
    }
    // Held: the next entry decides where it goes.
    assert_eq!(item_names(&assistant(&asm.turns()[1]).items), ["text"]);

    let updates = asm.finish();
    assert!(matches!(updates.as_slice(), [ModelUpdate::Updated(t)] if t.len() == 1));
    for update in updates {
        update.apply(&mut replayed);
    }
    assert_eq!(
        item_names(&assistant(&asm.turns()[1]).items),
        ["text", "injected"]
    );
    assert_eq!(replayed, asm.turns());
}

#[test]
fn flushing_held_entries_mid_stream_keeps_a_partial_line_for_later() {
    let head = lines(&[
        prompt("u1", "hello"),
        reply("r1", "msg_1", "hi"),
        attachment(
            "a1",
            "2026-10-24T08:00:02.000Z",
            r#"{"type":"hook_system_message","content":"late"}"#,
        ),
    ]);
    let next = reply("r2", "msg_2", "and more");
    let (half, rest) = next.split_at(30);
    let mut asm = Assembler::new(ByteSource::Memory(Arc::from(&b""[..])));
    let mut replayed = Vec::new();
    for u in asm.push_chunk(format!("{head}{half}").as_bytes()) {
        u.apply(&mut replayed);
    }
    let buffered = asm.buffered_from();
    assert_eq!(item_names(&assistant(&asm.turns()[1]).items), ["text"]);

    let updates = asm.flush_held();
    assert!(matches!(updates.as_slice(), [ModelUpdate::Updated(t)] if t.len() == 1));
    for u in updates {
        u.apply(&mut replayed);
    }
    assert_eq!(
        item_names(&assistant(&asm.turns()[1]).items),
        ["text", "injected"]
    );
    assert_eq!(asm.unparsed_count(), 0, "the partial line is not consumed");
    assert_eq!(asm.buffered_from(), buffered);
    assert!(asm.flush_held().is_empty(), "nothing left to flush");

    for u in asm.push_chunk(format!("{rest}\n").as_bytes()) {
        u.apply(&mut replayed);
    }
    assert_eq!(asm.unparsed_count(), 0);
    // The reply continues the turn; the flushed entries stay where they were.
    assert_eq!(
        item_names(&assistant(&asm.turns()[1]).items),
        ["text", "injected", "text"]
    );
    assert_eq!(replayed, asm.turns());
}

fn hook(uuid: &str, text: &str) -> String {
    attachment(
        uuid,
        "2026-10-24T08:00:00.500Z",
        &format!(r#"{{"type":"hook_system_message","content":"{text}"}}"#),
    )
}

#[test]
fn a_batch_flush_between_consecutive_entries_in_a_user_turn_still_collapses_them() {
    let first = lines(&[prompt("u1", "hello"), hook("a1", "A")]);
    let second = lines(&[hook("a2", "B"), reply("r1", "msg_1", "hi")]);
    let whole = assemble(&format!("{first}{second}")).into_turns();
    assert_eq!(item_names(&user(&whole[0]).items), ["injected"]);
    assert_eq!(injected(&user(&whole[0]).items[0]).len(), 2);

    // The turns carry the bytes their references name, so both read the same.
    let bytes: Arc<[u8]> = Arc::from(format!("{first}{second}").as_bytes());
    let mut asm = Assembler::new(ByteSource::Memory(bytes));
    let mut replayed = Vec::new();
    for batch in [&first, &second] {
        for u in asm.push_chunk(batch.as_bytes()) {
            u.apply(&mut replayed);
        }
        for u in asm.flush_held() {
            u.apply(&mut replayed);
        }
    }
    assert_eq!(asm.turns(), whole);
    assert_eq!(replayed, whole);
}

#[test]
fn a_footer_between_entries_in_a_user_turn_does_not_split_them() {
    let text = lines(&[
        prompt("u1", "hello"),
        hook("a1", "A"),
        r#"{"type":"system","subtype":"turn_duration","durationMs":5}"#.to_owned(),
        hook("a2", "B"),
        reply("r1", "msg_1", "hi"),
    ]);
    let turns = assemble(&text).into_turns();
    assert_eq!(injected(&user(&turns[0]).items[0]).len(), 2);
    assert_eq!(user(&turns[0]).items.len(), 1);
}

#[test]
fn entries_before_and_after_a_prompt_stay_in_separate_items() {
    let text = lines(&[
        hook("a0", "before"),
        prompt("u1", "hello"),
        hook("a1", "after"),
        reply("r1", "msg_1", "hi"),
    ]);
    let turns = assemble(&text).into_turns();
    let items = &user(&turns[0]).items;
    assert_eq!(item_names(items), ["injected", "injected"]);
    assert_eq!(injected(&items[0])[0].text, "before");
    assert_eq!(injected(&items[1])[0].text, "after");
}

#[test]
fn entries_before_and_after_a_later_prompt_stay_in_separate_items() {
    let text = lines(&[
        prompt("u1", "one"),
        hook("a1", "A"),
        reply("r1", "msg_1", "hi"),
        hook("x2", "X"),
        prompt("u2", "two"),
        hook("b2", "B"),
        reply("r2", "msg_2", "ho"),
    ]);
    let turns = assemble(&text).into_turns();
    let items = &user(&turns[2]).items;
    assert_eq!(item_names(items), ["injected", "injected"]);
    assert_eq!(injected(&items[0])[0].text, "X");
    assert_eq!(injected(&items[1])[0].text, "B");
}

#[test]
fn entries_before_and_after_a_later_prompt_stay_apart_across_a_live_flush() {
    let text = lines(&[
        prompt("u1", "one"),
        hook("a1", "A"),
        reply("r1", "msg_1", "hi"),
        hook("x2", "X"),
        prompt("u2", "two"),
        hook("b2", "B"),
    ]);
    let mut asm = Assembler::new(ByteSource::Memory(Arc::from(&b""[..])));
    asm.push_chunk(text.as_bytes());
    asm.flush_held();
    let turns = asm.turns();
    let items = &user(&turns[2]).items;
    assert_eq!(item_names(items), ["injected", "injected"]);
    assert_eq!(injected(&items[0])[0].text, "X");
    assert_eq!(injected(&items[1])[0].text, "B");
}

#[test]
fn entries_left_without_a_turn_are_logged_at_debug_level() {
    capture_logs();
    let text = lines(&[
        prompt("u1", "go"),
        r#"{"type":"system","subtype":"compact_boundary","uuid":"b1","timestamp":"2026-10-24T08:00:05.000Z"}"#.to_owned(),
        hook("a1", "trailing"),
    ]);
    let turns = assemble(&text).into_turns();
    assert_eq!(shape(&turns), ["user", "compaction"]);
    assert!(
        logged(Level::Debug)
            .iter()
            .any(|m| m.starts_with("1 injected entry has no turn"))
    );
}

#[test]
fn an_attachment_without_a_type_is_counted_under_a_readable_label() {
    let text = lines(&[r#"{"type":"attachment","attachment":{}}"#.to_owned()]);
    let asm = assemble(&text);
    assert_eq!(asm.unparsed_count(), 0);
    assert_eq!(asm.unknown_attachment_kinds()["<missing>"], 1);
}

#[test]
fn a_following_prompt_takes_held_entries_even_across_batches() {
    let first = lines(&[
        prompt("u1", "hello"),
        reply("r1", "msg_1", "hi"),
        attachment(
            "a1",
            "2026-10-24T08:00:02.000Z",
            r#"{"type":"hook_system_message","content":"for the next prompt"}"#,
        ),
    ]);
    let second = lines(&[prompt("u2", "again")]);
    let mut asm = Assembler::new(ByteSource::Memory(Arc::from(&b""[..])));
    asm.push_chunk(first.as_bytes());
    asm.push_chunk(second.as_bytes());
    asm.finish();
    let turns = asm.turns();
    assert_eq!(item_names(&assistant(&turns[1]).items), ["text"]);
    assert_eq!(item_names(&user(&turns[2]).items), ["injected"]);
}

// -------------------------------------------------------- queued_command

#[test]
fn human_queued_command_starts_a_user_turn_and_a_new_assistant_turn() {
    let turns = assemble_fixture("injected.jsonl").into_turns();
    let queued = user(&turns[4]);
    assert_eq!(queued.text, "Also run clippy.");
    assert_eq!(queued.id.as_str(), "00000000-0000-4000-8000-000000000103");
    assert_eq!(queued.timestamp.to_rfc3339(), "2026-10-22T09:01:03+00:00");
    assert_eq!(
        assistant(&turns[3]).items.len(),
        1,
        "work before it is closed"
    );
    match &assistant(&turns[5]).items[0] {
        Item::Text { text, .. } => assert_eq!(text, "Running clippy too."),
        other => panic!("{other:?}"),
    }
}

#[test]
fn human_queued_command_takes_held_entries_as_its_first_item() {
    let text = lines(&[
        prompt("u1", "go"),
        reply("r1", "msg_1", "working"),
        attachment(
            "a1",
            "2026-10-24T08:00:02.000Z",
            r#"{"type":"opened_file_in_ide","filename":"/p/a.rs"}"#,
        ),
        attachment(
            "q1",
            "2026-10-24T08:00:03.000Z",
            r#"{"type":"queued_command","prompt":"and more","origin":{"kind":"human"}}"#,
        ),
    ]);
    let turns = assemble(&text).into_turns();
    assert_eq!(shape(&turns), ["user", "assistant", "user"]);
    let queued = user(&turns[2]);
    assert_eq!(queued.text, "and more");
    assert_eq!(kinds(injected(&queued.items[0])), ["opened_file_in_ide"]);
}

#[test]
fn queued_command_without_a_human_origin_is_injected() {
    for origin in [
        r#","origin":{"kind":"task-notification"}"#,
        r#","origin":{"kind":"peer"}"#,
        "",
    ] {
        let text = lines(&[
            prompt("u1", "go"),
            reply("r1", "msg_1", "working"),
            attachment(
                "q1",
                "2026-10-24T08:00:03.000Z",
                &format!(r#"{{"type":"queued_command","prompt":"background news"{origin}}}"#),
            ),
            reply("r2", "msg_2", "noted"),
        ]);
        let turns = assemble(&text).into_turns();
        assert_eq!(shape(&turns), ["user", "assistant"], "origin {origin}");
        let turn = assistant(&turns[1]);
        assert_eq!(item_names(&turn.items), ["text", "injected", "text"]);
        let entries = injected(&turn.items[1]);
        assert_eq!(kinds(entries), ["queued_command"]);
        assert_eq!(entries[0].text, "background news");
    }
}

#[test]
fn human_queued_command_prompt_may_be_a_list_of_text_blocks() {
    let text = lines(&[attachment(
        "q1",
        "2026-10-24T08:00:03.000Z",
        r#"{"type":"queued_command","prompt":[{"type":"text","text":"one"},{"type":"text","text":"two"}],"origin":{"kind":"human"}}"#,
    )]);
    let turns = assemble(&text).into_turns();
    assert_eq!(user(&turns[0]).text, "one\ntwo");
}

// ------------------------------------------------------------------ noise

#[test]
fn noise_never_makes_an_item_and_is_not_counted() {
    let noise: Vec<(&str, String)> = vec![
        ("mode", r#"{"type":"mode","mode":"normal"}"#.into()),
        (
            "permission-mode",
            r#"{"type":"permission-mode","permissionMode":"auto"}"#.into(),
        ),
        ("atis-latch", r#"{"type":"atis-latch"}"#.into()),
        ("ai-title", r#"{"type":"ai-title","aiTitle":"t"}"#.into()),
        (
            "last-prompt",
            r#"{"type":"last-prompt","lastPrompt":"p"}"#.into(),
        ),
        ("cost-state", r#"{"type":"cost-state","costUsd":1}"#.into()),
        (
            "api-request",
            r#"{"type":"api-request","requestId":"r"}"#.into(),
        ),
        (
            "api-request-blob with a message",
            r#"{"type":"api-request-blob","message":{"role":"user","content":"x"}}"#.into(),
        ),
        (
            "api-request-shape",
            r#"{"type":"api-request-shape"}"#.into(),
        ),
        (
            "file-history-snapshot",
            r#"{"type":"file-history-snapshot","snapshot":{}}"#.into(),
        ),
        (
            "file-history-delta",
            r#"{"type":"file-history-delta"}"#.into(),
        ),
        (
            "system turn_duration without a turn",
            r#"{"type":"system","subtype":"turn_duration","durationMs":5}"#.into(),
        ),
        (
            "system other subtype",
            r#"{"type":"system","subtype":"agents_killed","content":"x"}"#.into(),
        ),
        (
            "system without subtype",
            r#"{"type":"system","content":"x"}"#.into(),
        ),
        (
            "user task-notification",
            r#"{"type":"user","origin":{"kind":"task-notification"},"message":{"role":"user","content":"<task-notification/>"}}"#.into(),
        ),
        (
            "user peer",
            r#"{"type":"user","origin":{"kind":"peer"},"message":{"role":"user","content":"hi"}}"#.into(),
        ),
        (
            "user without text or origin",
            r#"{"type":"user","message":{"role":"user","content":[{"type":"image","source":{}}]}}"#.into(),
        ),
        (
            "attachment without a type",
            r#"{"type":"attachment","attachment":{}}"#.into(),
        ),
    ];
    let mut attachments: Vec<(String, String)> = [
        "total_tokens_reminder",
        "deferred_tools_record",
        "deferred_tools_delta",
        "skill_listing",
        "credential_org",
        "batching_reminder_sent",
        "environment",
        "date",
        "model",
        "auto_mode",
        "some_future_kind",
    ]
    .iter()
    .map(|k| {
        (
            format!("attachment {k}"),
            format!(r#"{{"type":"attachment","attachment":{{"type":"{k}","x":1}}}}"#),
        )
    })
    .collect();
    attachments.extend(noise.into_iter().map(|(n, l)| (n.to_owned(), l)));

    for (name, line) in attachments {
        let text = lines(&[
            prompt("u1", "go"),
            line,
            reply("r1", "msg_1", "ok"),
            r#"{"type":"mode","mode":"x"}"#.to_owned(),
        ]);
        let asm = assemble(&text);
        assert_eq!(asm.unparsed_count(), 0, "{name}");
        let turns = asm.into_turns();
        assert_eq!(shape(&turns), ["user", "assistant"], "{name}");
        assert!(user(&turns[0]).items.is_empty(), "{name}");
        assert_eq!(item_names(&assistant(&turns[1]).items), ["text"], "{name}");
    }
}

#[test]
fn noise_does_not_break_a_run_of_injected_entries() {
    let text = lines(&[
        attachment(
            "a1",
            "2026-10-24T08:00:00.000Z",
            r#"{"type":"opened_file_in_ide","filename":"/p/a.rs"}"#,
        ),
        r#"{"type":"mode","mode":"x"}"#.into(),
        attachment("n1", "2026-10-24T08:00:00.100Z", r#"{"type":"date"}"#),
        attachment(
            "a2",
            "2026-10-24T08:00:00.200Z",
            r#"{"type":"opened_file_in_ide","filename":"/p/b.rs"}"#,
        ),
        r#"{"type":"user","origin":{"kind":"task-notification"},"message":{"role":"user","content":"x"}}"#.into(),
        attachment(
            "a3",
            "2026-10-24T08:00:00.300Z",
            r#"{"type":"opened_file_in_ide","filename":"/p/c.rs"}"#,
        ),
        prompt("u1", "go"),
    ]);
    let turns = assemble(&text).into_turns();
    assert_eq!(injected(&user(&turns[0]).items[0]).len(), 3);
}

#[test]
fn metadata_entries_are_skipped_without_counting() {
    capture_logs();
    let text = lines(&[
        prompt("u1", "go"),
        r#"{"type":"agent-name","agentName":"a"}"#.into(),
        r#"{"type":"queue-operation","operation":"enqueue"}"#.into(),
        r#"{"type":"continued-in","target":"t"}"#.into(),
        r#"{"type":"bridge-session","sessionId":"s"}"#.into(),
        r#"{"type":"tools-used","tools":["x"]}"#.into(),
        r#"{"type":"some-future-type","value":1}"#.into(),
        r#"{"type":"some-future-type","message":null}"#.into(),
        reply("r1", "msg_1", "ok"),
    ]);
    let asm = assemble(&text);
    assert_eq!(asm.unparsed_count(), 0);
    assert!(logged(Level::Warn).is_empty());
    assert_eq!(
        logged(Level::Debug).len(),
        7,
        "each is logged at debug level"
    );
    assert_eq!(shape(&asm.into_turns()), ["user", "assistant"]);
}

#[test]
fn blank_lines_are_not_counted() {
    let text = format!(
        "{}\n\n   \n{}\n",
        prompt("u1", "go"),
        reply("r1", "m", "ok")
    );
    let asm = assemble(&text);
    assert_eq!(asm.unparsed_count(), 0);
    assert_eq!(shape(&asm.into_turns()), ["user", "assistant"]);
}

// --------------------------------------------------------------- unparsed

#[test]
fn malformed_line_unknown_type_with_message_and_unknown_block_each_count_once() {
    capture_logs();
    let cases: Vec<(&str, String)> = vec![
        ("malformed line", "{ this is not json".to_owned()),
        ("json that is not an object", "42".to_owned()),
        (
            "unknown type with a message",
            r#"{"type":"future-entry","message":{"role":"user","content":"x"}}"#.to_owned(),
        ),
        (
            "unknown block in an assistant message",
            r#"{"type":"assistant","message":{"id":"m2","content":[{"type":"hologram"}]}}"#
                .to_owned(),
        ),
        (
            "unknown block in a user message",
            r#"{"type":"user","origin":{"kind":"human"},"message":{"role":"user","content":[{"type":"text","text":"hi"},{"type":"hologram"}]}}"#
                .to_owned(),
        ),
    ];
    for (name, line) in cases {
        capture_logs();
        let text = lines(&[prompt("u1", "before"), line, reply("r1", "msg_1", "after")]);
        let asm = assemble(&text);
        assert_eq!(asm.unparsed_count(), 1, "{name}");
        assert_eq!(logged(Level::Warn).len(), 1, "{name}");
        assert!(logged(Level::Warn)[0].contains("line 2"), "{name}");
        // Parsing continued after the skipped line.
        let turns = asm.into_turns();
        let last = turns.last().unwrap();
        assert!(
            matches!(&**last, Turn::Assistant(a) if matches!(a.items.last(), Some(Item::Text { text, .. }) if text == "after")),
            "{name}"
        );
    }
}

#[test]
fn every_unknown_block_counts_once() {
    let text = lines(&[
        r#"{"type":"assistant","message":{"id":"m","content":[{"type":"alpha"},{"type":"text","text":"ok"},{"type":"beta"},{"type":"gamma"}]}}"#.to_owned(),
    ]);
    assert_eq!(assemble(&text).unparsed_count(), 3);
}

#[test]
fn known_blocks_are_not_counted() {
    let text = lines(&[
        r#"{"type":"assistant","message":{"id":"m","content":[{"type":"text","text":"a"},{"type":"thinking","thinking":"t"},{"type":"redacted_thinking"},{"type":"tool_use","id":"t1","name":"Bash","input":{}},{"type":"server_tool_use","id":"t2","name":"web_search","input":{}},{"type":"web_search_tool_result","tool_use_id":"t2","content":[]}]}}"#.to_owned(),
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"x"},{"type":"image","source":{}}]}}"#.to_owned(),
    ]);
    assert_eq!(assemble(&text).unparsed_count(), 0);
}

#[test]
fn the_fixture_has_three_unparsed_entries() {
    let asm = assemble_fixture("injected.jsonl");
    // One malformed line, one unknown entry type with a message and one
    // unknown content block. Metadata, noise and the unknown attachment kind
    // are not counted.
    assert_eq!(asm.unparsed_count(), 3);
}

#[test]
fn an_entry_with_a_wrongly_typed_field_is_unparsed_not_fatal() {
    let text = lines(&[
        prompt("u1", "go"),
        r#"{"type":"user","uuid":7,"message":{"role":"user","content":"x"}}"#.to_owned(),
        reply("r1", "msg_1", "ok"),
    ]);
    let asm = assemble(&text);
    assert_eq!(asm.unparsed_count(), 1);
    assert_eq!(shape(&asm.into_turns()), ["user", "assistant"]);
}

// ------------------------------------------------- unknown attachment kind

#[test]
fn unknown_attachment_kinds_are_counted_and_logged_once_each() {
    capture_logs();
    let asm = assemble_fixture("injected.jsonl");
    assert_eq!(asm.unparsed_count(), 3, "not counted as unparsed");
    let kinds = asm.unknown_attachment_kinds();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds["brand_new_reminder"], 2);
    // `date` and `total_tokens_reminder` are known noise.
    drop(asm);
    let infos = logged(Level::Info);
    assert_eq!(
        infos,
        ["unknown attachment kind \"brand_new_reminder\" (2\u{d7})"]
    );
}

#[test]
fn unknown_attachment_kinds_are_logged_once_per_session_even_when_finished_twice() {
    capture_logs();
    let text = lines(&[
        attachment("a1", "2026-10-24T08:00:00.000Z", r#"{"type":"zzz_one"}"#),
        attachment("a2", "2026-10-24T08:00:00.000Z", r#"{"type":"zzz_one"}"#),
        attachment("a3", "2026-10-24T08:00:00.000Z", r#"{"type":"zzz_two"}"#),
    ]);
    let mut asm = assemble(&text);
    asm.finish();
    let turns_before = asm.turns().len();
    assert_eq!(turns_before, 0);
    let infos = logged(Level::Info);
    assert_eq!(infos.len(), 2);
    assert!(infos.contains(&"unknown attachment kind \"zzz_one\" (2\u{d7})".to_owned()));
    assert!(infos.contains(&"unknown attachment kind \"zzz_two\" (1\u{d7})".to_owned()));
}

// ------------------------------------------------------------- compaction

#[test]
fn compact_boundary_and_summary_make_one_compaction_turn() {
    let turns = assemble_fixture("compaction.jsonl").into_turns();
    assert_eq!(
        shape(&turns),
        ["user", "assistant", "compaction", "user", "assistant"]
    );
    let turn = compaction(&turns[2]);
    assert_eq!(turn.id.as_str(), "00000000-0000-4000-8000-000000000202");
    assert_eq!(turn.timestamp.to_rfc3339(), "2026-10-23T10:40:15.516+00:00");
    assert_eq!(
        turn.summary,
        "This session is being continued from a previous conversation that ran out of \
         context. Summary: the refactor of module A is done."
    );
    // The summary did not start a User turn.
    let user_texts: Vec<&str> = turns
        .iter()
        .filter_map(|t| match &**t {
            Turn::User(u) => Some(u.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        user_texts,
        ["Start the refactor.", "Continue with module B."]
    );
}

#[test]
fn entries_after_the_summary_go_into_the_next_turn() {
    let turns = assemble_fixture("compaction.jsonl").into_turns();
    let next = user(&turns[3]);
    assert_eq!(next.text, "Continue with module B.");
    assert_eq!(
        kinds(injected(&next.items[0])),
        ["compact_file_reference", "informational"]
    );
    assert_eq!(
        injected(&next.items[0])[0].text,
        "/home/user/project/src/a.rs"
    );
}

#[test]
fn entries_after_a_compaction_open_the_next_assistant_turn_when_no_prompt_follows() {
    let text = lines(&[
        r#"{"type":"system","subtype":"compact_boundary","uuid":"b1","timestamp":"2026-10-24T08:00:00.000Z","content":"Conversation compacted"}"#.to_owned(),
        r#"{"type":"user","uuid":"s1","timestamp":"2026-10-24T08:00:00.001Z","isCompactSummary":true,"message":{"role":"user","content":"the summary"}}"#.to_owned(),
        attachment(
            "a1",
            "2026-10-24T08:00:00.100Z",
            r#"{"type":"compact_file_reference","filename":"/p/a.rs"}"#,
        ),
        reply("r1", "msg_1", "carrying on"),
    ]);
    let turns = assemble(&text).into_turns();
    assert_eq!(shape(&turns), ["compaction", "assistant"]);
    assert_eq!(compaction(&turns[0]).summary, "the summary");
    assert_eq!(
        item_names(&assistant(&turns[1]).items),
        ["injected", "text"]
    );
}

#[test]
fn a_boundary_without_a_summary_has_an_empty_summary() {
    let text = lines(&[
        prompt("u1", "go"),
        r#"{"type":"system","subtype":"compact_boundary","uuid":"b1","timestamp":"2026-10-24T08:00:05.000Z","content":"Conversation compacted"}"#.to_owned(),
        prompt("u2", "next"),
    ]);
    let turns = assemble(&text).into_turns();
    assert_eq!(shape(&turns), ["user", "compaction", "user"]);
    assert_eq!(compaction(&turns[1]).summary, "");
    assert_eq!(user(&turns[2]).text, "next");
}

#[test]
fn a_summary_text_in_blocks_is_joined() {
    let text = lines(&[
        r#"{"type":"system","subtype":"compact_boundary","uuid":"b1","timestamp":"2026-10-24T08:00:05.000Z"}"#.to_owned(),
        r#"{"type":"user","isCompactSummary":true,"message":{"role":"user","content":[{"type":"text","text":"part one"},{"type":"text","text":"part two"}]}}"#.to_owned(),
    ]);
    let turns = assemble(&text).into_turns();
    assert_eq!(compaction(&turns[0]).summary, "part one\npart two");
}

#[test]
fn a_summary_arriving_in_a_later_batch_updates_the_compaction_turn() {
    let boundary = lines(&[
        r#"{"type":"system","subtype":"compact_boundary","uuid":"b1","timestamp":"2026-10-24T08:00:05.000Z"}"#.to_owned(),
    ]);
    let summary = lines(&[
        r#"{"type":"user","isCompactSummary":true,"message":{"role":"user","content":"late summary"}}"#.to_owned(),
    ]);
    let mut asm = Assembler::new(ByteSource::Memory(Arc::from(&b""[..])));
    let mut replayed = Vec::new();
    let first = asm.push_chunk(boundary.as_bytes());
    assert!(matches!(first.as_slice(), [ModelUpdate::Appended(t)] if t.len() == 1));
    for u in first {
        u.apply(&mut replayed);
    }
    let second = asm.push_chunk(summary.as_bytes());
    assert!(matches!(second.as_slice(), [ModelUpdate::Updated(t)] if t.len() == 1));
    for u in second {
        u.apply(&mut replayed);
    }
    assert_eq!(compaction(&replayed[0]).summary, "late summary");
    assert_eq!(replayed, asm.turns());
}

#[test]
fn a_summary_without_a_boundary_is_still_a_compaction_not_a_prompt() {
    let text = lines(&[
        r#"{"type":"user","uuid":"s1","timestamp":"2026-10-24T08:00:05.000Z","isCompactSummary":true,"message":{"role":"user","content":"orphan summary"}}"#.to_owned(),
    ]);
    let turns = assemble(&text).into_turns();
    assert_eq!(shape(&turns), ["compaction"]);
    assert_eq!(compaction(&turns[0]).summary, "orphan summary");
}

// ---------------------------------------------------------------- version

#[test]
fn a_newer_version_warns_exactly_once_per_session_and_still_parses() {
    capture_logs();
    let newer = "99.1.0";
    let text = lines(&[
        format!(
            r#"{{"type":"user","version":"{newer}","uuid":"u1","timestamp":"2026-10-24T08:00:00.000Z","origin":{{"kind":"human"}},"message":{{"role":"user","content":"hello"}}}}"#
        ),
        format!(
            r#"{{"type":"assistant","version":"{newer}","uuid":"r1","message":{{"id":"m1","model":"x","content":[{{"type":"text","text":"hi"}}]}}}}"#
        ),
        r#"{"type":"assistant","version":"99.2.0","uuid":"r2","message":{"id":"m2","model":"x","content":[{"type":"text","text":"again"}]}}"#.to_owned(),
        attachment("a1", "2026-10-24T08:00:00.000Z", r#"{"type":"zzz"}"#),
    ]);
    let asm = assemble(&text);
    let warnings = logged(Level::Warn);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains(newer));
    assert!(warnings[0].contains(NEWEST_KNOWN_VERSION));
    assert_eq!(asm.unparsed_count(), 0);
    let turns = asm.into_turns();
    assert_eq!(shape(&turns), ["user", "assistant"]);
    assert_eq!(item_names(&assistant(&turns[1]).items), ["text", "text"]);
}

#[test]
fn versions_up_to_the_newest_known_do_not_warn() {
    capture_logs();
    let text = lines(&[
        format!(r#"{{"type":"mode","version":"{NEWEST_KNOWN_VERSION}"}}"#),
        r#"{"type":"mode","version":"2.1.9"}"#.to_owned(),
        r#"{"type":"mode","version":"1.99.1000"}"#.to_owned(),
        r#"{"type":"mode","version":"2.1"}"#.to_owned(),
        r#"{"type":"mode"}"#.to_owned(),
    ]);
    assemble(&text);
    assert!(logged(Level::Warn).is_empty());
}

#[test]
fn the_newest_known_version_is_the_newest_in_the_fixtures() {
    let mut newest = (0u64, 0u64, 0u64);
    for name in ["session.jsonl", "injected.jsonl", "compaction.jsonl"] {
        let bytes = fixture_bytes(name);
        for line in String::from_utf8_lossy(&bytes).lines() {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if let Some(v) = value.get("version").and_then(|v| v.as_str()) {
                let p: Vec<u64> = v.split('.').map(|x| x.parse().unwrap()).collect();
                newest = newest.max((p[0], p[1], p[2]));
            }
        }
    }
    assert_eq!(
        format!("{}.{}.{}", newest.0, newest.1, newest.2),
        NEWEST_KNOWN_VERSION
    );
}

// ------------------------------------------------------------ incremental

fn chunked(bytes: &Arc<[u8]>, seed: u64, max: u64) -> Vec<Arc<Turn>> {
    let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut asm = Assembler::new(ByteSource::Memory(Arc::clone(bytes)));
    let mut replayed: Vec<Arc<Turn>> = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let end = (pos + 1 + (rng.next() % max) as usize).min(bytes.len());
        for update in asm.push_chunk(&bytes[pos..end]) {
            update.apply(&mut replayed);
        }
        pos = end;
    }
    for update in asm.finish() {
        update.apply(&mut replayed);
    }
    assert_eq!(
        replayed,
        asm.turns(),
        "seed {seed}: replayed updates differ"
    );
    asm.into_turns()
}

#[test]
fn random_chunks_give_identical_turns_with_injected_content_and_compaction() {
    for name in ["injected.jsonl", "compaction.jsonl"] {
        let bytes = fixture_bytes(name);
        let asm = Assembler::from_memory(Arc::clone(&bytes));
        let unparsed = asm.unparsed_count();
        let whole = asm.into_turns();
        assert!(!whole.is_empty());
        for seed in 1..=60 {
            assert_eq!(chunked(&bytes, seed, 400), whole, "{name} seed {seed}");
        }
        for seed in 1..=15 {
            assert_eq!(chunked(&bytes, seed, 12), whole, "{name} tiny seed {seed}");
        }

        // The unparsed count does not depend on chunking either.
        let mut asm = Assembler::new(ByteSource::Memory(Arc::clone(&bytes)));
        for part in bytes.chunks(37) {
            asm.push_chunk(part);
        }
        asm.finish();
        assert_eq!(asm.unparsed_count(), unparsed, "{name}");
    }
}

#[test]
fn line_by_line_equals_the_whole_transcript() {
    for name in ["injected.jsonl", "compaction.jsonl"] {
        let bytes = fixture_bytes(name);
        let whole = Assembler::from_memory(Arc::clone(&bytes)).into_turns();
        let mut asm = Assembler::new(ByteSource::Memory(Arc::clone(&bytes)));
        let mut start = 0usize;
        while start < bytes.len() {
            let end = bytes[start..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(bytes.len(), |p| start + p);
            asm.push_line(&bytes[start..end], start as u64..end as u64);
            start = end + 1;
        }
        assert_eq!(asm.into_turns(), whole, "{name}");
    }
}

#[test]
fn every_prefix_replays_to_the_matching_turns() {
    // Whatever the split point, the updates applied so far plus the flush at
    // the end reproduce the final turns.
    let bytes = fixture_bytes("injected.jsonl");
    let whole = Assembler::from_memory(Arc::clone(&bytes)).into_turns();
    for split in (1..bytes.len()).step_by(97) {
        let mut asm = Assembler::new(ByteSource::Memory(Arc::clone(&bytes)));
        let mut replayed = Vec::new();
        for part in [&bytes[..split], &bytes[split..]] {
            for u in asm.push_chunk(part) {
                u.apply(&mut replayed);
            }
        }
        for u in asm.finish() {
            u.apply(&mut replayed);
        }
        assert_eq!(replayed, whole, "split {split}");
    }
}
