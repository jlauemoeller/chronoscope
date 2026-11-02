//! Claude turn assembly against fixture transcripts and constructed lines.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chronoscope::harness::claude::Assembler;
use chronoscope::model::{
    AssistantTurn, Item, ModelUpdate, Thinking, TokenUsage, ToolCall, ToolKind, ToolStatus, Turn,
    UserTurn,
};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude/session.jsonl")
}

fn fixture_bytes() -> Arc<[u8]> {
    Arc::from(std::fs::read(fixture_path()).unwrap())
}

fn fixture_turns() -> Vec<Arc<Turn>> {
    Assembler::from_memory(fixture_bytes()).into_turns()
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

fn call<'a>(turn: &'a AssistantTurn, id: &str) -> &'a ToolCall {
    turn.items
        .iter()
        .find_map(|item| match item {
            Item::ToolCall(c) if c.id.as_str() == id => Some(c),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no tool call {id}"))
}

fn item_ids(turn: &AssistantTurn) -> Vec<&str> {
    turn.items.iter().map(|i| i.id().as_str()).collect()
}

/// Lines of `bytes` with their absolute byte ranges (without newlines).
fn lines_with_ranges(bytes: &[u8]) -> Vec<(&[u8], std::ops::Range<u64>)> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let end = bytes[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |p| start + p);
        out.push((&bytes[start..end], start as u64..end as u64));
        start = end + 1;
    }
    out
}

fn assemble_str(text: &str) -> Vec<Arc<Turn>> {
    Assembler::from_memory(Arc::from(text.as_bytes())).into_turns()
}

/// A tiny deterministic generator (xorshift64); there is no `rand` dependency.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

// ---------------------------------------------------------------- turns

#[test]
fn fixture_alternates_user_and_assistant_turns() {
    let turns = fixture_turns();
    let kinds: Vec<&str> = turns
        .iter()
        .map(|t| match &**t {
            Turn::User(_) => "user",
            Turn::Assistant(_) => "assistant",
            Turn::Compaction(_) => "compaction",
        })
        .collect();
    assert_eq!(
        kinds,
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

#[test]
fn human_prompt_with_origin_starts_a_user_turn() {
    let turns = fixture_turns();
    let prompt = user(&turns[0]);
    assert_eq!(
        prompt.text,
        "Please look at the project and fix the failing test."
    );
    assert_eq!(prompt.id.as_str(), "00000000-0000-4000-8000-000000000001");
    assert_eq!(prompt.timestamp.to_rfc3339(), "2026-10-21T08:00:00+00:00");
}

#[test]
fn older_prompt_without_origin_starts_a_user_turn() {
    let turns = fixture_turns();
    assert_eq!(user(&turns[2]).text, "Thanks. Now summarise what changed.");
}

#[test]
fn prompt_given_as_text_blocks_joins_them() {
    let turns = fixture_turns();
    assert_eq!(user(&turns[4]).text, "One more thing:\nis it done?");
}

#[test]
fn task_notification_does_not_start_a_turn_and_stays_in_the_assistant_turn() {
    let turns = fixture_turns();
    // msg_D and msg_E (either side of the task-notification) share one turn.
    let second = assistant(&turns[3]);
    let ids = item_ids(second);
    assert_eq!(ids, ["msg_D:0", "msg_E:0"]);
    assert!(
        !turns
            .iter()
            .any(|t| matches!(&**t, Turn::User(u) if u.text.contains("task-notification")))
    );
}

#[test]
fn meta_and_tool_result_only_user_entries_are_not_prompts() {
    let text = concat!(
        r#"{"type":"user","uuid":"u1","timestamp":"2026-01-01T00:00:00Z","isMeta":true,"message":{"role":"user","content":"injected"}}"#,
        "\n",
        r#"{"type":"user","uuid":"u2","timestamp":"2026-01-01T00:00:01Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"x","content":"orphan"}]}}"#,
        "\n",
    );
    assert!(assemble_str(text).is_empty());
}

#[test]
fn each_turn_has_a_unique_stable_id() {
    let turns = fixture_turns();
    let mut ids: Vec<_> = turns.iter().map(|t| t.id().clone()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), turns.len());
    // The Assistant turn is identified by its first entry.
    assert_eq!(
        assistant(&turns[1]).id.as_str(),
        "00000000-0000-4000-8000-000000000010"
    );
}

// ------------------------------------------------------- message merging

#[test]
fn lines_sharing_a_message_id_merge_and_repeated_blocks_replace() {
    let turns = fixture_turns();
    let first = assistant(&turns[1]);
    let ids = item_ids(first);
    // msg_A: redacted thinking, text (sent twice, grown), Bash (sent twice).
    assert_eq!(&ids[..3], ["msg_A:0", "msg_A:1", "toolu_bash"]);
    assert_eq!(
        first.items[0],
        Item::Thinking {
            id: "msg_A:0".into(),
            thinking: Thinking::Redacted
        }
    );
    match &first.items[1] {
        Item::Text { text, .. } => assert_eq!(text, "I'll look around the repo first."),
        other => panic!("expected text, got {other:?}"),
    }
    assert_eq!(
        first
            .items
            .iter()
            .filter(|i| i.id().as_str() == "toolu_bash")
            .count(),
        1
    );
}

#[test]
fn distinct_blocks_of_the_same_kind_are_not_merged() {
    let turns = fixture_turns();
    let first = assistant(&turns[1]);
    let ids = item_ids(first);
    // msg_B has readable thinking at 0 and two redacted blocks at 1 and 2.
    assert_eq!(
        &ids[3..9],
        [
            "msg_B:0",
            "msg_B:1",
            "msg_B:2",
            "toolu_read",
            "toolu_grep",
            "model:msg_C"
        ]
    );
    assert_eq!(
        first.items[3],
        Item::Thinking {
            id: "msg_B:0".into(),
            thinking: Thinking::Text("The test probably lives in src/lib.rs.".into())
        }
    );
    for i in [4, 5] {
        assert!(matches!(
            &first.items[i],
            Item::Thinking {
                thinking: Thinking::Redacted,
                ..
            }
        ));
    }
}

#[test]
fn blocks_in_one_line_are_indexed_from_the_api_block_index() {
    let text = concat!(
        r#"{"type":"user","uuid":"u","origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"hi"}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:01Z","apiBlockIndex":2,"message":{"id":"m","model":"x","content":[{"type":"text","text":"one"},{"type":"text","text":"two"}]}}"#,
        "\n",
    );
    let turns = assemble_str(text);
    assert_eq!(item_ids(assistant(&turns[1])), ["m:2", "m:3"]);
}

/// One single-block `assistant` line without `apiBlockIndex` (older format).
fn old_line(n: u32, msg: &str, block: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"a{n}","timestamp":"2026-01-01T00:00:{n:02}Z","message":{{"id":"{msg}","model":"x","content":[{block}]}}}}"#
    )
}

#[test]
fn without_api_block_index_a_grown_text_replaces_the_earlier_one() {
    let text = [
        old_line(1, "m", r#"{"type":"text","text":"He"}"#),
        old_line(2, "m", r#"{"type":"text","text":"Hello"}"#),
        old_line(3, "m", r#"{"type":"text","text":"Hello"}"#),
    ]
    .join("\n");
    let turns = assemble_str(&text);
    let turn = assistant(&turns[0]);
    assert_eq!(turn.items.len(), 1);
    assert!(matches!(&turn.items[0], Item::Text { text, .. } if text == "Hello"));
}

fn older_format_message() -> String {
    [
        old_line(
            1,
            "m",
            r#"{"type":"thinking","thinking":"","signature":"s1"}"#,
        ),
        old_line(2, "m", r#"{"type":"text","text":"first"}"#),
        old_line(
            3,
            "m",
            r#"{"type":"tool_use","id":"t1","name":"Bash","input":{}}"#,
        ),
        old_line(4, "m", r#"{"type":"text","text":"second"}"#),
        old_line(
            5,
            "m",
            r#"{"type":"thinking","thinking":"","signature":"s2"}"#,
        ),
        old_line(
            6,
            "m",
            r#"{"type":"thinking","thinking":"","signature":"s2"}"#,
        ),
        old_line(7, "m", r#"{"type":"text","text":"second"}"#),
    ]
    .join("\n")
        + "\n"
}

#[test]
fn without_api_block_index_distinct_blocks_of_a_message_all_survive_in_order() {
    let turns = assemble_str(&older_format_message());
    let turn = assistant(&turns[0]);
    assert_eq!(item_ids(turn), ["m:0", "m:1", "t1", "m:2", "m:3", "m:4"]);
    assert!(matches!(
        &turn.items[0],
        Item::Thinking {
            thinking: Thinking::Redacted,
            ..
        }
    ));
    assert!(matches!(&turn.items[1], Item::Text { text, .. } if text == "first"));
    assert!(matches!(&turn.items[2], Item::ToolCall(_)));
    // "second" is sent again after a redacted block, so it is a new block.
    assert!(matches!(&turn.items[3], Item::Text { text, .. } if text == "second"));
    // A redacted block re-sent with the same signature is replaced, not duplicated.
    assert!(matches!(
        &turn.items[4],
        Item::Thinking {
            thinking: Thinking::Redacted,
            ..
        }
    ));
    assert!(matches!(&turn.items[5], Item::Text { text, .. } if text == "second"));
}

#[test]
fn without_api_block_index_a_readable_thinking_then_text_keeps_both() {
    let text = [
        old_line(
            1,
            "m",
            r#"{"type":"thinking","thinking":"hmm","signature":"s"}"#,
        ),
        old_line(2, "m", r#"{"type":"text","text":"answer"}"#),
    ]
    .join("\n");
    let turns = assemble_str(&text);
    assert_eq!(item_ids(assistant(&turns[0])), ["m:0", "m:1"]);
}

#[test]
fn older_format_assembles_identically_by_line_and_in_random_chunks() {
    let text = older_format_message();
    let bytes: Arc<[u8]> = Arc::from(text.as_bytes());
    let whole = Assembler::from_memory(Arc::clone(&bytes)).into_turns();
    assert_eq!(final_by_lines(&bytes), whole);
    for seed in 1..=40u64 {
        let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::clone(&bytes)));
        let mut replayed = Vec::new();
        let mut pos = 0;
        while pos < bytes.len() {
            let end = (pos + 1 + (rng.next() % 150) as usize).min(bytes.len());
            for u in asm.push_chunk(&bytes[pos..end]) {
                u.apply(&mut replayed);
            }
            pos = end;
        }
        for u in asm.finish() {
            u.apply(&mut replayed);
        }
        assert_eq!(replayed, whole, "seed {seed}");
        assert_eq!(asm.into_turns(), whole, "seed {seed}");
    }
}

#[test]
fn null_meta_flag_does_not_fail_the_line() {
    let text = r#"{"type":"user","uuid":"u","isMeta":null,"origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"hi"}}"#;
    assert_eq!(assemble_str(text).len(), 1);
}

// ---------------------------------------------------------- tool results

#[test]
fn tool_result_sets_status_duration_and_output() {
    let turns = fixture_turns();
    let bash = call(assistant(&turns[1]), "toolu_bash");
    assert_eq!(bash.tool, ToolKind::Shell);
    assert_eq!(bash.description, "List the project files");
    assert_eq!(bash.status, ToolStatus::Ok);
    // The call was written at :03.000 and the result at :05.000.
    assert_eq!(bash.duration, Some(Duration::from_secs(2)));
    let result = bash.result.as_ref().unwrap();
    assert!(!result.is_error);
    assert!(!result.output.is_lazy());
    assert_eq!(result.output.line_count(), 3);
    assert!(result.output.text().unwrap().starts_with("total 8\n"));
}

#[test]
fn api_request_blob_results_are_ignored() {
    let turns = fixture_turns();
    let bash = call(assistant(&turns[1]), "toolu_bash");
    let text = bash
        .result
        .as_ref()
        .unwrap()
        .output
        .text()
        .unwrap()
        .into_owned();
    assert!(!text.contains("BLOB"));
    // Duration comes from the real result, not the (earlier) blob.
    assert_eq!(bash.duration, Some(Duration::from_secs(2)));
}

#[test]
fn api_request_blob_before_the_real_result_never_attaches() {
    let text = concat!(
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:00Z","message":{"id":"m","model":"x","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{}}]}}"#,
        "\n",
        r#"{"type":"api-request-blob","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"blob"}]}}"#,
        "\n",
    );
    let turns = assemble_str(text);
    let c = call(assistant(&turns[0]), "t1");
    assert_eq!(c.status, ToolStatus::Pending);
    assert!(c.result.is_none());
    assert!(c.duration.is_none());
}

#[test]
fn error_result_has_error_status_and_joined_text_blocks() {
    let turns = fixture_turns();
    let grep = call(assistant(&turns[1]), "toolu_grep");
    assert_eq!(grep.tool, ToolKind::Grep);
    assert_eq!(grep.description, "Grep \"fn broken\" in src");
    assert_eq!(grep.status, ToolStatus::Error);
    let result = grep.result.as_ref().unwrap();
    assert!(result.is_error);
    assert_eq!(
        result.output.text().unwrap(),
        "grep: src: No such directory\nexit status 2"
    );
}

#[test]
fn call_without_result_is_pending() {
    let turns = fixture_turns();
    let mcp = call(assistant(&turns[1]), "toolu_mcp");
    assert_eq!(mcp.tool, ToolKind::Other("mcp__docs__search".into()));
    assert_eq!(mcp.description, "mcp__docs__search");
    assert_eq!(mcp.status, ToolStatus::Pending);
    assert!(mcp.result.is_none());
    assert!(mcp.duration.is_none());
}

#[test]
fn derived_description_uses_cwd_from_the_entries() {
    let turns = fixture_turns();
    let read = call(assistant(&turns[1]), "toolu_read");
    assert_eq!(read.tool, ToolKind::Read);
    assert_eq!(read.description, "Read src/lib.rs");
}

#[test]
fn server_tool_result_in_an_assistant_line_becomes_the_result() {
    // The real `advisor_tool_result` content is an encrypted payload without text.
    let text = concat!(
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:00Z","apiBlockIndex":0,"message":{"id":"m","model":"x","content":[{"type":"server_tool_use","id":"srv1","name":"advisor","input":{}}]}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a2","timestamp":"2026-01-01T00:00:04Z","apiBlockIndex":1,"message":{"id":"m","model":"x","content":[{"type":"advisor_tool_result","tool_use_id":"srv1","content":{"type":"advisor_redacted_result","encrypted_content":"AAAA"}}]}}"#,
        "\n",
    );
    let turns = assemble_str(text);
    let turn = assistant(&turns[0]);
    assert_eq!(turn.items.len(), 1, "the result block is not an item");
    let c = call(turn, "srv1");
    assert_eq!(c.tool, ToolKind::Other("advisor".into()));
    assert_eq!(c.status, ToolStatus::Ok);
    assert_eq!(c.duration, Some(Duration::from_secs(4)));
    let output = &c.result.as_ref().unwrap().output;
    assert_eq!(output.byte_len(), 0, "encrypted payload is not shown");
    assert_eq!(output.text().unwrap(), "");
}

#[test]
fn server_text_result_object_uses_its_text() {
    let text = concat!(
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:00Z","apiBlockIndex":0,"message":{"id":"m","model":"x","content":[{"type":"server_tool_use","id":"srv1","name":"advisor","input":{}}]}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a2","timestamp":"2026-01-01T00:00:04Z","apiBlockIndex":1,"message":{"id":"m","model":"x","content":[{"type":"advisor_tool_result","tool_use_id":"srv1","content":{"type":"advisor_result","text":"Use a map."}}]}}"#,
        "\n",
    );
    let turns = assemble_str(text);
    let c = call(assistant(&turns[0]), "srv1");
    assert_eq!(
        c.result.as_ref().unwrap().output.text().unwrap(),
        "Use a map."
    );
}

#[test]
fn web_search_results_render_as_title_and_url_lines() {
    let text = concat!(
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:00Z","apiBlockIndex":0,"message":{"id":"m","model":"x","content":[{"type":"server_tool_use","id":"srv1","name":"web_search","input":{"query":"q"}}]}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a2","timestamp":"2026-01-01T00:00:04Z","apiBlockIndex":1,"message":{"id":"m","model":"x","content":[{"type":"web_search_tool_result","tool_use_id":"srv1","content":[{"type":"web_search_result","title":"Ratatui","url":"https://ratatui.rs","encrypted_content":"zz"},{"type":"web_search_result","url":"https://example.com"}]}]}}"#,
        "\n",
    );
    let turns = assemble_str(text);
    let c = call(assistant(&turns[0]), "srv1");
    assert_eq!(
        c.result.as_ref().unwrap().output.text().unwrap(),
        "Ratatui — https://ratatui.rs\nhttps://example.com"
    );
}

#[test]
fn server_web_search_is_a_web_search_call() {
    let text = r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:00Z","message":{"id":"m","model":"x","content":[{"type":"server_tool_use","id":"srv2","name":"web_search","input":{"query":"ratatui"}}]}}"#;
    let turns = assemble_str(text);
    let c = call(assistant(&turns[0]), "srv2");
    assert_eq!(c.tool, ToolKind::WebSearch);
    assert_eq!(c.description, "WebSearch \"ratatui\"");
}

// ------------------------------------------------------- model and effort

#[test]
fn turn_model_is_that_of_its_first_message_and_a_change_is_an_inline_note() {
    let turns = fixture_turns();
    let first = assistant(&turns[1]);
    assert_eq!(first.model, "claude-opus-5-5");
    let ids = item_ids(first);
    let note = ids.iter().position(|id| *id == "model:msg_C").unwrap();
    assert_eq!(
        first.items[note],
        Item::ModelChange {
            id: "model:msg_C".into(),
            model: "claude-sonnet-5-5".into()
        }
    );
    // The note sits right before msg_C's first block.
    assert_eq!(ids[note + 1], "msg_C:0");
    // msg_A and msg_B (same model) produced no note.
    assert_eq!(
        first
            .items
            .iter()
            .filter(|i| matches!(i, Item::ModelChange { .. }))
            .count(),
        1
    );
}

#[test]
fn model_change_back_and_forth_gives_one_note_per_change() {
    let line = |n: u32, msg: &str, model: &str| {
        format!(
            r#"{{"type":"assistant","uuid":"a{n}","timestamp":"2026-01-01T00:00:0{n}Z","message":{{"id":"{msg}","model":"{model}","content":[{{"type":"text","text":"t{n}"}}]}}}}"#
        )
    };
    let text = [
        line(1, "m1", "A"),
        line(2, "m2", "B"),
        line(3, "m3", "B"),
        line(4, "m4", "A"),
    ]
    .join("\n");
    let turns = assemble_str(&text);
    let turn = assistant(&turns[0]);
    assert_eq!(turn.model, "A");
    assert_eq!(
        item_ids(turn),
        ["m1:0", "model:m2", "m2:0", "m3:0", "model:m4", "m4:0"]
    );
}

#[test]
fn synthetic_model_is_not_a_model_change() {
    let line = |n: u32, msg: &str, model: &str| {
        format!(
            r#"{{"type":"assistant","uuid":"a{n}","timestamp":"2026-01-01T00:00:0{n}Z","message":{{"id":"{msg}","model":"{model}","content":[{{"type":"text","text":"t{n}"}}]}}}}"#
        )
    };
    let text = [
        line(1, "m1", "A"),
        line(2, "m2", "<synthetic>"),
        line(3, "m3", "A"),
    ]
    .join("\n");
    let turns = assemble_str(&text);
    assert_eq!(item_ids(assistant(&turns[0])), ["m1:0", "m2:0", "m3:0"]);
}

#[test]
fn turn_starting_with_a_synthetic_message_takes_the_first_real_model_without_a_note() {
    let text = [
        old_line(1, "m1", r#"{"type":"text","text":"local"}"#)
            .replace(r#""model":"x""#, r#""model":"<synthetic>""#),
        old_line(2, "m2", r#"{"type":"text","text":"real"}"#)
            .replace(r#""model":"x""#, r#""model":"real-model""#),
        old_line(3, "m3", r#"{"type":"text","text":"more"}"#)
            .replace(r#""model":"x""#, r#""model":"other-model""#),
    ]
    .join("\n");
    let turns = assemble_str(&text);
    let turn = assistant(&turns[0]);
    assert_eq!(turn.model, "real-model");
    assert_eq!(item_ids(turn), ["m1:0", "m2:0", "model:m3", "m3:0"]);
}

#[test]
fn effort_comes_from_effort_then_per_turn_effort() {
    let turns = fixture_turns();
    assert_eq!(assistant(&turns[1]).effort.as_deref(), Some("high"));
    // Turn 2 only has `perTurnEffort`; a later message without effort keeps it.
    assert_eq!(assistant(&turns[3]).effort.as_deref(), Some("medium"));
    // Turn 3 has no effort at all.
    assert_eq!(assistant(&turns[5]).effort, None);
}

#[test]
fn effort_prefers_effort_over_per_turn_effort() {
    let text = r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:00Z","effort":"low","perTurnEffort":"high","message":{"id":"m","model":"x","content":[{"type":"text","text":"t"}]}}"#;
    let turns = assemble_str(text);
    assert_eq!(assistant(&turns[0]).effort.as_deref(), Some("low"));
}

// ----------------------------------------------------------------- stats

#[test]
fn turn_duration_becomes_the_stats_footer() {
    let turns = fixture_turns();
    let stats = assistant(&turns[1]).stats.as_ref().unwrap();
    // From the prompt (08:00:00) to the footer (08:00:30); the footer's
    // running total of 9 is not read, the turn has messages A, B and C.
    assert_eq!(stats.duration, Duration::from_secs(30));
    assert_eq!(stats.message_count, 3);
    assert_eq!(stats.cost, None);
    let stats = assistant(&turns[3]).stats.as_ref().unwrap();
    // Messages D and E; the footer's 3 is a running total.
    assert_eq!(stats.duration, Duration::from_secs(10));
    assert_eq!(stats.message_count, 2);
    assert!(assistant(&turns[5]).stats.is_none());
}

#[test]
fn tokens_count_each_message_once_with_cache_reads_and_writes() {
    let turns = fixture_turns();
    // Turn 1: msg_A (10+100, sent on 5 lines), msg_B (20+200, 5 lines) and
    // msg_C (30+50, 2 lines); every line carries 50k cache writes and 90k
    // cache reads, counted once per message (3 messages).
    assert_eq!(
        assistant(&turns[1]).stats.as_ref().unwrap().tokens,
        Some(TokenUsage {
            input: 60,
            output: 350,
            cache_read: 270_000,
            cache_write: 150_000,
            reasoning: 0,
        })
    );
    // Turn 2: msg_D (40+60) + msg_E (5+7).
    assert_eq!(
        assistant(&turns[3]).stats.as_ref().unwrap().tokens,
        Some(TokenUsage {
            input: 45,
            output: 67,
            cache_read: 180_000,
            cache_write: 100_000,
            reasoning: 0,
        })
    );
}

#[test]
fn turn_duration_without_an_assistant_turn_is_ignored() {
    let text = concat!(
        r#"{"type":"user","uuid":"u","origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"hi"}}"#,
        "\n",
        r#"{"type":"system","subtype":"turn_duration","durationMs":5,"messageCount":1,"timestamp":"2026-01-01T00:00:01Z"}"#,
        "\n",
    );
    let turns = assemble_str(text);
    assert_eq!(turns.len(), 1);
    assert!(matches!(&*turns[0], Turn::User(_)));
}

// ------------------------------------------------------------ lazy output

fn original_read_output() -> String {
    // Independent of the assembler: parse the fixture line directly.
    let bytes = std::fs::read(fixture_path()).unwrap();
    let line = String::from_utf8(bytes)
        .unwrap()
        .lines()
        .find(|l| l.contains(r#""tool_use_id":"toolu_read""#) && l.contains(r#""type":"user""#))
        .unwrap()
        .to_owned();
    let value: serde_json::Value = serde_json::from_str(&line).unwrap();
    value["message"]["content"][0]["content"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn large_result_is_lazy_with_correct_stats_and_exact_text_from_memory() {
    let expected = original_read_output();
    assert!(expected.len() > 4096);
    let turns = fixture_turns();
    let read = call(assistant(&turns[1]), "toolu_read");
    let output = &read.result.as_ref().unwrap().output;
    assert!(output.is_lazy());
    assert_eq!(output.line_count(), 120);
    assert_eq!(output.byte_len(), expected.len());
    assert_eq!(output.text().unwrap(), expected.as_str());
}

#[test]
fn large_result_is_lazy_with_exact_text_from_a_file() {
    let expected = original_read_output();
    let asm = Assembler::from_file(fixture_path()).unwrap();
    let turns = asm.into_turns();
    let read = call(assistant(&turns[1]), "toolu_read");
    let output = &read.result.as_ref().unwrap().output;
    assert!(output.is_lazy());
    assert_eq!(output.line_count(), 120);
    assert_eq!(output.byte_len(), expected.len());
    assert_eq!(output.text().unwrap(), expected.as_str());
    // The other results in the same file are inline.
    let bash = call(assistant(&turns[1]), "toolu_bash");
    assert!(!bash.result.as_ref().unwrap().output.is_lazy());
}

#[test]
fn result_of_exactly_4_kib_is_inline_and_one_byte_more_is_lazy() {
    let make = |len: usize| {
        let body = "x".repeat(len);
        format!(
            concat!(
                r#"{{"type":"assistant","uuid":"a","timestamp":"2026-01-01T00:00:00Z","message":{{"id":"m","model":"x","content":[{{"type":"tool_use","id":"t","name":"Bash","input":{{}}}}]}}}}"#,
                "\n",
                r#"{{"type":"user","uuid":"u","timestamp":"2026-01-01T00:00:01Z","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"t","content":"{}"}}]}}}}"#,
                "\n"
            ),
            body
        )
    };
    for (len, lazy) in [(4096, false), (4097, true)] {
        let turns = assemble_str(&make(len));
        let output = &call(assistant(&turns[0]), "t")
            .result
            .as_ref()
            .unwrap()
            .output;
        assert_eq!(output.is_lazy(), lazy, "len {len}");
        assert_eq!(output.byte_len(), len);
        assert_eq!(output.text().unwrap().len(), len);
    }
}

#[test]
fn lazy_text_reports_an_error_when_the_file_changed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    std::fs::copy(fixture_path(), &path).unwrap();
    let turns = Assembler::from_file(&path).unwrap().into_turns();
    std::fs::write(&path, b"{}\n").unwrap();
    let read = call(assistant(&turns[1]), "toolu_read");
    assert!(read.result.as_ref().unwrap().output.text().is_err());
}

// ----------------------------------------------------------- incremental

fn final_by_lines(bytes: &[u8]) -> Vec<Arc<Turn>> {
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(bytes)));
    for (line, range) in lines_with_ranges(bytes) {
        asm.push_line(line, range);
    }
    asm.into_turns()
}

#[test]
fn one_call_line_by_line_and_random_chunks_give_identical_turns() {
    let bytes = fixture_bytes();
    let whole = Assembler::from_memory(Arc::clone(&bytes)).into_turns();
    assert_eq!(whole.len(), 6);
    assert_eq!(final_by_lines(&bytes), whole);

    for seed in 1..=60u64 {
        let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::clone(&bytes)));
        let mut pos = 0;
        while pos < bytes.len() {
            let n = 1 + (rng.next() % 700) as usize;
            let end = (pos + n).min(bytes.len());
            asm.push_chunk(&bytes[pos..end]);
            pos = end;
        }
        asm.finish();
        assert_eq!(asm.into_turns(), whole, "seed {seed}");
    }
}

#[test]
fn replaying_emitted_updates_reproduces_the_final_turns() {
    let bytes = fixture_bytes();
    let whole = Assembler::from_memory(Arc::clone(&bytes)).into_turns();
    for chunk in [1usize, 7, 64, 500, 100_000] {
        let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::clone(&bytes)));
        let mut replayed: Vec<Arc<Turn>> = Vec::new();
        let apply_all = |updates: Vec<ModelUpdate>, replayed: &mut Vec<Arc<Turn>>| {
            for update in updates {
                update.apply(replayed);
            }
        };
        for part in bytes.chunks(chunk) {
            let updates = asm.push_chunk(part);
            apply_all(updates, &mut replayed);
        }
        let tail = asm.finish();
        apply_all(tail, &mut replayed);
        assert_eq!(replayed, whole, "chunk size {chunk}");
    }
}

#[test]
fn a_new_turn_is_never_both_appended_and_updated_in_one_call() {
    let bytes = fixture_bytes();
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::clone(&bytes)));
    for part in bytes.chunks(900) {
        let updates = asm.push_chunk(part);
        let mut appended = Vec::new();
        let mut updated = Vec::new();
        for update in &updates {
            match update {
                ModelUpdate::Appended(t) => appended.extend(t.iter().map(|t| t.id().clone())),
                ModelUpdate::Updated(t) => updated.extend(t.iter().map(|t| t.id().clone())),
                other => panic!("the assembler never sends {other:?}"),
            }
        }
        assert!(appended.iter().all(|id| !updated.contains(id)));
    }
}

#[test]
fn partial_trailing_line_is_buffered_until_its_newline() {
    let line = r#"{"type":"user","uuid":"u","origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"hello"}}"#;
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    let (head, tail) = line.split_at(40);
    assert!(asm.push_chunk(head.as_bytes()).is_empty());
    assert!(asm.push_chunk(tail.as_bytes()).is_empty());
    let updates = asm.push_chunk(b"\n");
    assert_eq!(updates.len(), 1);
    assert!(matches!(&updates[0], ModelUpdate::Appended(t) if t.len() == 1));
}

#[test]
fn finish_flushes_a_last_line_without_newline() {
    let line = r#"{"type":"user","uuid":"u","origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"hello"}}"#;
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    assert!(asm.push_chunk(line.as_bytes()).is_empty());
    let updates = asm.finish();
    assert_eq!(updates.len(), 1);
    assert_eq!(asm.turns().len(), 1);
}

fn live_lines() -> Vec<String> {
    vec![
        r#"{"type":"user","uuid":"u1","origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"go"}}"#.into(),
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:01Z","apiBlockIndex":0,"message":{"id":"m","model":"x","content":[{"type":"text","text":"Working"}]}}"#.into(),
        r#"{"type":"assistant","uuid":"a2","timestamp":"2026-01-01T00:00:02Z","apiBlockIndex":1,"message":{"id":"m","model":"x","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}]}}"#.into(),
        r#"{"type":"user","uuid":"r1","timestamp":"2026-01-01T00:00:05Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"ok"}]}}"#.into(),
        r#"{"type":"user","uuid":"u2","origin":{"kind":"human"},"timestamp":"2026-01-01T00:01:00Z","message":{"role":"user","content":"next"}}"#.into(),
    ]
}

fn push(asm: &mut Assembler, line: &str) -> Vec<ModelUpdate> {
    asm.push_line(line.as_bytes(), 0..line.len() as u64)
}

#[test]
fn extending_a_message_updates_the_turn_in_place_with_an_unchanged_id() {
    let lines = live_lines();
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    assert!(matches!(&push(&mut asm, &lines[0])[..], [ModelUpdate::Appended(t)] if t.len() == 1));
    let appended = push(&mut asm, &lines[1]);
    let id = match &appended[..] {
        [ModelUpdate::Appended(t)] => t[0].id().clone(),
        other => panic!("expected one Appended, got {other:?}"),
    };
    assert_eq!(id.as_str(), "a1");

    let updates = push(&mut asm, &lines[2]);
    match &updates[..] {
        [ModelUpdate::Updated(t)] => {
            assert_eq!(t.len(), 1);
            assert_eq!(t[0].id(), &id);
            assert_eq!(assistant(&t[0]).items.len(), 2);
        }
        other => panic!("expected one Updated, got {other:?}"),
    }
    assert_eq!(asm.turns().len(), 2);
}

#[test]
fn a_later_tool_result_updates_the_turn_owning_the_call() {
    let lines = live_lines();
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    for line in &lines[..3] {
        push(&mut asm, line);
    }
    // A new human prompt closes the turn before the result shows up.
    let prompt = push(&mut asm, &lines[4]);
    assert!(matches!(&prompt[..], [ModelUpdate::Appended(_)]));
    let before = asm.turns()[1].clone();
    assert_eq!(call(assistant(&before), "t1").status, ToolStatus::Pending);

    let updates = push(&mut asm, &lines[3]);
    match &updates[..] {
        [ModelUpdate::Updated(t)] => {
            assert_eq!(t.len(), 1);
            assert_eq!(t[0].id(), before.id());
            let c = call(assistant(&t[0]), "t1");
            assert_eq!(c.status, ToolStatus::Ok);
            assert_eq!(c.duration, Some(Duration::from_secs(3)));
        }
        other => panic!("expected one Updated, got {other:?}"),
    }
    // The earlier snapshot handed out is untouched.
    assert_eq!(call(assistant(&before), "t1").status, ToolStatus::Pending);
}

#[test]
fn a_repeated_identical_line_produces_no_update() {
    let lines = live_lines();
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    for line in &lines[..2] {
        push(&mut asm, line);
    }
    assert!(push(&mut asm, &lines[1]).is_empty());
}

#[test]
fn turn_duration_after_the_turn_is_an_update() {
    let lines = live_lines();
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    for line in &lines[..3] {
        push(&mut asm, line);
    }
    let footer = r#"{"type":"system","subtype":"turn_duration","durationMs":4000,"messageCount":2,"timestamp":"2026-01-01T00:00:06Z"}"#;
    let updates = push(&mut asm, footer);
    match &updates[..] {
        [ModelUpdate::Updated(t)] => {
            let stats = assistant(&t[0]).stats.as_ref().unwrap();
            // From the prompt at 00:00:00 to the footer at 00:00:06, not the
            // footer's `durationMs` of 4000.
            assert_eq!(stats.duration, Duration::from_secs(6));
            assert_eq!(stats.message_count, 1);
            assert_eq!(stats.tokens, None, "no usage recorded in these lines");
        }
        other => panic!("expected one Updated, got {other:?}"),
    }
}

// ------------------------------------------------------------- robustness

#[test]
fn malformed_lines_are_skipped_without_aborting() {
    let good = r#"{"type":"user","uuid":"u","origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"ok"}}"#;
    let mut bytes = Vec::new();
    for junk in [
        &b"not json"[..],
        b"{\"type\":\"assistant\",",
        b"",
        b"[1,2,3]",
        b"null",
        b"{\"type\":\"user\",\"message\":{\"content\":42}}",
        b"{\"type\":\"assistant\",\"message\":\"text\"}",
        &[0xff, 0xfe, b'{'],
    ] {
        bytes.extend_from_slice(junk);
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(good.as_bytes());
    bytes.push(b'\n');
    bytes.extend_from_slice(b"{\"type\":\"user\"\n");
    let turns = Assembler::from_memory(Arc::from(bytes)).into_turns();
    assert_eq!(turns.len(), 1);
    assert_eq!(user(&turns[0]).text, "ok");
}

#[test]
fn unhandled_entry_types_are_ignored() {
    let text = concat!(
        r#"{"type":"attachment","uuid":"x","attachment":{"type":"queued_command"}}"#,
        "\n",
        r#"{"type":"ai-title","aiTitle":"t"}"#,
        "\n",
        r#"{"type":"system","subtype":"away_summary","content":"s"}"#,
        "\n",
        r#"{"type":"cost-state"}"#,
        "\n",
        r#"{"type":"something-new","message":{"content":"x"}}"#,
        "\n",
    );
    assert!(assemble_str(text).is_empty());
}

#[test]
fn model_update_apply_replaces_by_turn_id() {
    let lines = live_lines();
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    let mut turns = Vec::new();
    for line in &lines[..3] {
        for update in push(&mut asm, line) {
            update.apply(&mut turns);
        }
    }
    assert_eq!(turns.len(), 2);
    assert_eq!(assistant(&turns[1]).items.len(), 2);
    ModelUpdate::Updated(Vec::new()).apply(&mut turns);
    assert_eq!(turns.len(), 2);
}

// ---- Token usage ----

const USER_LINE: &str = r#"{"type":"user","uuid":"u1","origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"go"}}"#;
const FOOTER_LINE: &str = r#"{"type":"system","subtype":"turn_duration","durationMs":4000,"messageCount":2,"timestamp":"2026-01-01T00:00:06Z"}"#;

/// An assistant line of message `msg` with the given raw `usage` JSON.
fn assistant_line(msg: &str, n: u32, usage: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"{msg}-{n}","timestamp":"2026-01-01T00:00:0{n}Z","apiBlockIndex":{n},"message":{{"id":"{msg}","model":"x","content":[{{"type":"text","text":"line {n}"}}],"usage":{usage}}}}}"#
    )
}

/// Feeds `lines` and returns the assembler.
fn assembled(lines: &[String]) -> Assembler {
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    for line in lines {
        push(&mut asm, line);
    }
    asm.finish();
    asm
}

fn footer_tokens(lines: &[String]) -> (Option<TokenUsage>, usize, usize) {
    let mut all = vec![USER_LINE.to_owned()];
    all.extend_from_slice(lines);
    all.push(FOOTER_LINE.to_owned());
    let asm = assembled(&all);
    let unparsed = asm.unparsed_count();
    let turns = asm.turns();
    let a = assistant(&turns[1]);
    let text_items = a
        .items
        .iter()
        .filter(|i| matches!(i, Item::Text { .. }))
        .count();
    (a.stats.as_ref().unwrap().tokens, unparsed, text_items)
}

#[test]
fn cache_reads_and_writes_are_summed_over_messages_each_counted_once() {
    // msg_a is sent on two lines; the later line carries the final usage.
    let lines = [
        assistant_line(
            "msg_a",
            1,
            r#"{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":1000,"cache_creation_input_tokens":500}"#,
        ),
        assistant_line(
            "msg_a",
            2,
            r#"{"input_tokens":10,"output_tokens":100,"cache_read_input_tokens":1000,"cache_creation_input_tokens":500}"#,
        ),
        assistant_line(
            "msg_b",
            3,
            r#"{"input_tokens":1,"output_tokens":2,"cache_read_input_tokens":3,"cache_creation_input_tokens":4}"#,
        ),
    ];
    let (tokens, unparsed, _) = footer_tokens(&lines);
    assert_eq!(unparsed, 0);
    assert_eq!(
        tokens,
        Some(TokenUsage {
            input: 11,
            output: 102,
            cache_read: 1003,
            cache_write: 504,
            reasoning: 0,
        })
    );
    assert_eq!(tokens.unwrap().total(), 11 + 102 + 504);
}

#[test]
fn claude_thinking_tokens_are_not_added_to_output() {
    let lines = [assistant_line(
        "msg_a",
        1,
        r#"{"input_tokens":10,"output_tokens":100,"output_tokens_details":{"thinking_tokens":60}}"#,
    )];
    let (tokens, unparsed, _) = footer_tokens(&lines);
    assert_eq!(unparsed, 0);
    let tokens = tokens.unwrap();
    assert_eq!(tokens.output, 100, "thinking tokens are part of output");
    assert_eq!(tokens.reasoning, 60);
    assert_eq!(tokens.total(), 110, "reasoning is never added to a total");
}

#[test]
fn usage_applies_to_a_footer_that_already_exists() {
    // The footer comes first; a later line of the same turn updates it.
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    for line in [
        USER_LINE.to_owned(),
        assistant_line("msg_a", 1, r#"{"input_tokens":1,"output_tokens":2}"#),
        FOOTER_LINE.to_owned(),
        assistant_line(
            "msg_a",
            2,
            r#"{"input_tokens":1,"output_tokens":2,"cache_read_input_tokens":9}"#,
        ),
    ] {
        push(&mut asm, &line);
    }
    asm.finish();
    let tokens = assistant(&asm.turns()[1]).stats.as_ref().unwrap().tokens;
    assert_eq!(tokens.unwrap().cache_read, 9);
}

#[test]
fn usage_fields_of_an_unexpected_type_count_as_missing_and_keep_the_line_parsed() {
    let lines = [assistant_line(
        "msg_a",
        1,
        r#"{"input_tokens":"many","output_tokens":7,"cache_read_input_tokens":[1],"cache_creation_input_tokens":{"a":1},"output_tokens_details":"bad"}"#,
    )];
    let (tokens, unparsed, text_items) = footer_tokens(&lines);
    assert_eq!(unparsed, 0, "the line still parses");
    assert_eq!(text_items, 1, "the content of the line is kept");
    assert_eq!(
        tokens,
        Some(TokenUsage {
            output: 7,
            ..TokenUsage::default()
        })
    );

    let lines = [assistant_line(
        "msg_a",
        1,
        r#"{"input_tokens":3,"output_tokens":7,"output_tokens_details":{"thinking_tokens":"lots"}}"#,
    )];
    let (tokens, unparsed, _) = footer_tokens(&lines);
    assert_eq!(unparsed, 0);
    assert_eq!(tokens.unwrap().reasoning, 0);

    // A `usage` that is not an object is missing as a whole.
    for usage in [r#""oops""#, "[1,2]", "7", "null"] {
        let lines = [assistant_line("msg_a", 1, usage)];
        let (tokens, unparsed, text_items) = footer_tokens(&lines);
        assert_eq!(unparsed, 0, "usage {usage}");
        assert_eq!(text_items, 1, "usage {usage}");
        assert_eq!(tokens, None, "usage {usage}");
    }
}

#[test]
fn a_turn_without_any_usage_has_no_token_figures() {
    // An explicit `null`, and a line with no `usage` key at all.
    let absent = assistant_line("msg_b", 2, "null").replace(r#","usage":null"#, "");
    assert!(!absent.contains("usage"), "{absent}");
    let lines = [assistant_line("msg_a", 1, "null"), absent];
    let (tokens, unparsed, text_items) = footer_tokens(&lines);
    assert_eq!(unparsed, 0);
    assert_eq!(text_items, 2);
    assert_eq!(tokens, None);
}

#[test]
fn an_out_of_range_number_in_usage_keeps_the_line_parsed() {
    // `1e400` does not fit an `f64`; as a new field and as an unknown field
    // nested in `usage` it counts as missing and fails nothing.
    for usage in [
        r#"{"output_tokens":7,"cache_read_input_tokens":1e400}"#,
        r#"{"output_tokens":7,"server_tool_use":{"web_search_requests":1e400}}"#,
        r#"{"output_tokens":7,"output_tokens_details":{"thinking_tokens":1e400}}"#,
    ] {
        let (tokens, unparsed, text_items) = footer_tokens(&[assistant_line("msg_a", 1, usage)]);
        assert_eq!(unparsed, 0, "usage {usage}");
        assert_eq!(text_items, 1, "usage {usage}");
        assert_eq!(
            tokens,
            Some(TokenUsage {
                output: 7,
                ..TokenUsage::default()
            }),
            "usage {usage}"
        );
    }
}

// ---- Invalid UTF-8 in a line ----

/// An assistant line (message `msg_a`, one text block with `text`) with the
/// raw `usage` bytes and `top_level` fields (each ending in a comma) before
/// `message`.
fn raw_assistant_line(text: &[u8], usage: &[u8], top_level: &[u8]) -> Vec<u8> {
    let mut line =
        br#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:01Z","#.to_vec();
    line.extend_from_slice(top_level);
    line.extend_from_slice(
        br#""message":{"id":"msg_a","model":"x","content":[{"type":"text","text":""#,
    );
    line.extend_from_slice(text);
    line.extend_from_slice(br#""}],"usage":"#);
    line.extend_from_slice(usage);
    line.extend_from_slice(b"}}");
    line
}

/// Feeds the user line, then `line` (as bytes), then the footer; returns
/// `unparsed_count`, the number of text items and the turn's tokens.
fn feed_raw(line: &[u8]) -> (usize, usize, Option<TokenUsage>) {
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    push(&mut asm, USER_LINE);
    asm.push_line(line, 0..line.len() as u64);
    push(&mut asm, FOOTER_LINE);
    asm.finish();
    let unparsed = asm.unparsed_count();
    let turns = asm.turns();
    match turns.get(1).map(|t| assistant(t)) {
        Some(a) => (
            unparsed,
            a.items
                .iter()
                .filter(|i| matches!(i, Item::Text { .. }))
                .count(),
            a.stats.as_ref().and_then(|s| s.tokens),
        ),
        None => (unparsed, 0, None),
    }
}

fn only_output(n: u64) -> Option<TokenUsage> {
    Some(TokenUsage {
        output: n,
        ..TokenUsage::default()
    })
}

#[test]
fn invalid_utf8_inside_usage_keeps_the_line_parsed() {
    for usage in [
        &b"{\"output_tokens\":7,\"service_tier\":\"\xff\"}"[..],
        b"{\"output_tokens\":7,\"x\":{\"y\":[\"a\xc3\"]}}",
        b"{\"output_tokens\":7,\"cache_read_input_tokens\":\"\xff\"}",
        b"{\"output_tokens\":7,\"output_tokens_details\":{\"x\":\"\xc3\"}}",
        b"{\"output_tokens\":7,\"\xff\":1}",
    ] {
        let line = raw_assistant_line(b"hi", usage, b"");
        let (unparsed, texts, tokens) = feed_raw(&line);
        let shown = String::from_utf8_lossy(usage).into_owned();
        assert_eq!(unparsed, 0, "usage {shown}");
        assert_eq!(texts, 1, "usage {shown}");
        assert_eq!(tokens.map(|t| t.output), Some(7), "usage {shown}");
    }
}

#[test]
fn invalid_utf8_outside_usage_keeps_its_outcome() {
    let clean: &[u8] = b"{\"output_tokens\":7}";
    // In the text of the block: unparsed.
    let line = raw_assistant_line(b"a\xffb", clean, b"");
    assert_eq!(feed_raw(&line), (1, 0, None));
    // In a known top-level string: unparsed.
    let line = raw_assistant_line(b"hi", clean, b"\"cwd\":\"\xff\",");
    assert_eq!(feed_raw(&line), (1, 0, None));
    // In an unknown top-level field: parsed, as before.
    let line = raw_assistant_line(b"hi", clean, b"\"zzz\":\"\xff\",");
    assert_eq!(feed_raw(&line), (0, 1, only_output(7)));
    // In the text and in `usage` together: unparsed.
    let line = raw_assistant_line(b"a\xffb", b"{\"service_tier\":\"\xff\"}", b"");
    assert_eq!(feed_raw(&line), (1, 0, None));
}

// ------------------------------------------------- message count, duration

fn human(ts: &str) -> String {
    format!(
        r#"{{"type":"user","uuid":"u-{ts}","origin":{{"kind":"human"}},"timestamp":"{ts}","message":{{"role":"user","content":"go"}}}}"#
    )
}

fn reply(id: &str, ts: &str, model: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"a-{id}","timestamp":"{ts}","message":{{"id":"{id}","model":"{model}","content":[{{"type":"text","text":"text {id}"}}]}}}}"#
    )
}

fn footer(ts: Option<&str>, duration_ms: Option<u64>, count: &str) -> String {
    let ts = ts.map_or(String::new(), |t| format!(r#","timestamp":"{t}""#));
    let ms = duration_ms.map_or(String::new(), |m| format!(r#","durationMs":{m}"#));
    format!(r#"{{"type":"system","subtype":"turn_duration"{ms},"messageCount":{count}{ts}}}"#)
}

fn queued(uuid: &str, ts: &str, origin: &str) -> String {
    format!(
        r#"{{"type":"attachment","uuid":"{uuid}","timestamp":"{ts}","attachment":{{"type":"queued_command","prompt":"queued text"{origin}}}}}"#
    )
}

const COMPACT_BOUNDARY: &str = r#"{"type":"system","subtype":"compact_boundary","uuid":"b1","timestamp":"2026-01-01T00:05:00Z"}"#;
const COMPACT_SUMMARY: &str = r#"{"type":"user","uuid":"s1","timestamp":"2026-01-01T00:05:01Z","isCompactSummary":true,"message":{"role":"user","content":"summary"}}"#;

/// The stats of the last Assistant turn of `lines`.
fn last_stats(lines: &[String]) -> chronoscope::model::TurnStats {
    let turns = assembled(lines).turns().to_vec();
    let turn = turns
        .iter()
        .rev()
        .find_map(|t| match &**t {
            Turn::Assistant(a) => Some(a.clone()),
            _ => None,
        })
        .expect("an Assistant turn");
    turn.stats.expect("a footer")
}

#[test]
fn several_footers_give_wall_clock_time_and_the_count_is_per_turn() {
    let stats = last_stats(&[
        human("2026-01-01T00:00:00Z"),
        reply("m1", "2026-01-01T00:00:01Z", "x"),
        footer(Some("2026-01-01T00:00:05Z"), Some(4000), "4124"),
        reply("m2", "2026-01-01T00:00:10Z", "x"),
        footer(Some("2026-01-01T00:00:20Z"), Some(2000), "4126"),
    ]);
    assert_eq!(stats.duration, Duration::from_secs(20), "not 4 s + 2 s");
    assert_eq!(stats.message_count, 2, "not 4126");
}

#[test]
fn the_spec_example_runs_six_minutes_and_fifty_one_seconds() {
    let stats = last_stats(&[
        human("2026-01-01T07:18:45Z"),
        reply("m1", "2026-01-01T07:18:50Z", "x"),
        footer(Some("2026-01-01T07:19:10Z"), Some(20_000), "10"),
        reply("m2", "2026-01-01T07:25:00Z", "x"),
        footer(Some("2026-01-01T07:25:36Z"), Some(16_000), "12"),
    ]);
    assert_eq!(stats.duration, Duration::from_secs(6 * 60 + 51));
}

#[test]
fn a_turn_after_a_compaction_starts_at_its_first_assistant_entry() {
    let stats = last_stats(&[
        human("2026-01-01T00:00:00Z"),
        reply("m0", "2026-01-01T00:00:01Z", "x"),
        COMPACT_BOUNDARY.to_owned(),
        COMPACT_SUMMARY.to_owned(),
        reply("m1", "2026-01-01T00:10:00Z", "x"),
        footer(Some("2026-01-01T00:10:30Z"), Some(1000), "7"),
    ]);
    assert_eq!(stats.duration, Duration::from_secs(30));
    assert_eq!(stats.message_count, 1);
}

#[test]
fn a_compaction_ends_the_association_with_the_prompt() {
    // The prompt is followed by a compaction before any assistant entry.
    let stats = last_stats(&[
        human("2026-01-01T00:00:00Z"),
        COMPACT_BOUNDARY.to_owned(),
        COMPACT_SUMMARY.to_owned(),
        reply("m1", "2026-01-01T00:10:00Z", "x"),
        footer(Some("2026-01-01T00:10:30Z"), Some(1000), "7"),
    ]);
    assert_eq!(stats.duration, Duration::from_secs(30));
}

#[test]
fn a_compaction_summary_without_a_boundary_ends_the_association_with_the_prompt() {
    let stats = last_stats(&[
        human("2026-01-01T00:00:00Z"),
        COMPACT_SUMMARY.to_owned(),
        reply("m1", "2026-01-01T00:10:00Z", "x"),
        footer(Some("2026-01-01T00:10:30Z"), Some(1000), "7"),
    ]);
    assert_eq!(stats.duration, Duration::from_secs(30));
}

#[test]
fn a_transcript_starting_with_an_assistant_entry_starts_at_that_entry() {
    let stats = last_stats(&[
        reply("m1", "2026-01-01T00:00:10Z", "x"),
        reply("m2", "2026-01-01T00:00:12Z", "x"),
        footer(Some("2026-01-01T00:00:40Z"), Some(1000), "7"),
    ]);
    assert_eq!(stats.duration, Duration::from_secs(30));
    assert_eq!(stats.message_count, 2);
}

#[test]
fn a_prompt_without_a_usable_timestamp_falls_back_to_the_first_assistant_entry() {
    let prompt = r#"{"type":"user","uuid":"u","origin":{"kind":"human"},"timestamp":"not a time","message":{"role":"user","content":"go"}}"#;
    let stats = last_stats(&[
        prompt.to_owned(),
        reply("m1", "2026-01-01T00:00:10Z", "x"),
        footer(Some("2026-01-01T00:00:40Z"), Some(1000), "7"),
    ]);
    assert_eq!(stats.duration, Duration::from_secs(30));
}

#[test]
fn a_footer_without_a_usable_timestamp_uses_its_duration_ms() {
    for ts in [None, Some("garbage")] {
        let stats = last_stats(&[
            human("2026-01-01T00:00:00Z"),
            reply("m1", "2026-01-01T00:00:01Z", "x"),
            footer(ts, Some(16_000), "7"),
        ]);
        assert_eq!(stats.duration, Duration::from_secs(16), "{ts:?}");
    }
}

#[test]
fn a_start_without_a_usable_timestamp_uses_the_duration_ms() {
    let no_ts_prompt = r#"{"type":"user","uuid":"u","origin":{"kind":"human"},"message":{"role":"user","content":"go"}}"#;
    let no_ts_reply = r#"{"type":"assistant","uuid":"a1","message":{"id":"m1","model":"x","content":[{"type":"text","text":"t"}]}}"#;
    let stats = last_stats(&[
        no_ts_prompt.to_owned(),
        no_ts_reply.to_owned(),
        footer(Some("2026-01-01T00:00:40Z"), Some(16_000), "7"),
    ]);
    assert_eq!(stats.duration, Duration::from_secs(16));
}

#[test]
fn an_end_before_the_start_falls_back_to_the_duration_ms() {
    let stats = last_stats(&[
        human("2026-01-01T00:01:00Z"),
        reply("m1", "2026-01-01T00:01:01Z", "x"),
        footer(Some("2026-01-01T00:00:30Z"), Some(9000), "7"),
    ]);
    assert_eq!(stats.duration, Duration::from_secs(9));
}

#[test]
fn a_footer_with_neither_a_span_nor_a_duration_ms_sets_no_stats() {
    let asm = assembled(&[
        human("2026-01-01T00:00:00Z"),
        reply("m1", "2026-01-01T00:00:01Z", "x"),
        footer(None, None, "7"),
    ]);
    assert!(assistant(&asm.turns()[1]).stats.is_none());
}

#[test]
fn a_footer_needs_no_duration_ms_when_the_timestamps_give_a_span() {
    let stats = last_stats(&[
        human("2026-01-01T00:00:00Z"),
        reply("m1", "2026-01-01T00:00:01Z", "x"),
        footer(Some("2026-01-01T00:00:08Z"), None, "7"),
    ]);
    assert_eq!(stats.duration, Duration::from_secs(8));
}

#[test]
fn there_is_no_footer_before_a_turn_duration_entry() {
    let asm = assembled(&[
        human("2026-01-01T00:00:00Z"),
        reply("m1", "2026-01-01T00:00:01Z", "x"),
        reply("m2", "2026-01-01T00:00:02Z", "x"),
    ]);
    assert!(assistant(&asm.turns()[1]).stats.is_none());
}

#[test]
fn a_non_human_queued_command_neither_starts_a_turn_nor_resets_the_start() {
    for origin in [r#","origin":{"kind":"task-notification"}"#, ""] {
        let asm = assembled(&[
            human("2026-01-01T00:00:00Z"),
            reply("m1", "2026-01-01T00:00:01Z", "x"),
            queued("q1", "2026-01-01T00:03:00Z", origin),
            reply("m2", "2026-01-01T00:03:05Z", "x"),
            footer(Some("2026-01-01T00:04:00Z"), Some(16_000), "50"),
        ]);
        let turns = asm.turns();
        assert_eq!(
            turns.len(),
            2,
            "origin {origin:?}: still one Assistant turn"
        );
        let stats = assistant(&turns[1]).stats.as_ref().unwrap();
        assert_eq!(
            stats.duration,
            Duration::from_secs(240),
            "origin {origin:?}"
        );
        assert_eq!(stats.message_count, 2);
    }
}

#[test]
fn a_human_queued_command_is_the_start_of_the_assistant_turn_after_it() {
    let asm = assembled(&[
        human("2026-01-01T00:00:00Z"),
        reply("m1", "2026-01-01T00:00:01Z", "x"),
        footer(Some("2026-01-01T00:00:10Z"), Some(1000), "5"),
        queued(
            "q1",
            "2026-01-01T00:02:00Z",
            r#","origin":{"kind":"human"}"#,
        ),
        reply("m2", "2026-01-01T00:02:03Z", "x"),
        footer(Some("2026-01-01T00:02:30Z"), Some(1000), "6"),
    ]);
    let turns = asm.turns();
    assert_eq!(turns.len(), 4);
    let first = assistant(&turns[1]).stats.as_ref().unwrap();
    assert_eq!(first.duration, Duration::from_secs(10));
    let second = assistant(&turns[3]).stats.as_ref().unwrap();
    assert_eq!(second.duration, Duration::from_secs(30));
    assert_eq!(second.message_count, 1);
}

#[test]
fn synthetic_messages_are_left_out_of_the_count() {
    let stats = last_stats(&[
        human("2026-01-01T00:00:00Z"),
        reply("m1", "2026-01-01T00:00:01Z", "x"),
        reply("m2", "2026-01-01T00:00:02Z", "<synthetic>"),
        reply("m3", "2026-01-01T00:00:03Z", "x"),
        footer(Some("2026-01-01T00:00:05Z"), Some(1000), "3"),
    ]);
    assert_eq!(stats.message_count, 2);
}

#[test]
fn a_turn_of_only_synthetic_messages_has_a_count_of_zero() {
    let stats = last_stats(&[
        human("2026-01-01T00:00:00Z"),
        reply("m1", "2026-01-01T00:00:01Z", "<synthetic>"),
        footer(Some("2026-01-01T00:00:05Z"), Some(1000), "9"),
    ]);
    assert_eq!(stats.message_count, 0);
}

#[test]
fn a_message_sent_in_several_lines_counts_once() {
    let stats = last_stats(&[
        human("2026-01-01T00:00:00Z"),
        assistant_line("msg_a", 1, "{}"),
        assistant_line("msg_a", 2, "{}"),
        footer(Some("2026-01-01T00:00:05Z"), Some(1000), "9"),
    ]);
    assert_eq!(stats.message_count, 1);
}

#[test]
fn messages_without_an_id_are_counted_by_uuid_then_line() {
    let no_id = |uuid: &str| {
        format!(
            r#"{{"type":"assistant","uuid":"{uuid}","timestamp":"2026-01-01T00:00:01Z","message":{{"model":"x","content":[{{"type":"text","text":"t"}}]}}}}"#
        )
    };
    let stats = last_stats(&[
        human("2026-01-01T00:00:00Z"),
        no_id("a1"),
        no_id("a1"),
        no_id("a2"),
        footer(Some("2026-01-01T00:00:05Z"), Some(1000), "9"),
    ]);
    assert_eq!(stats.message_count, 2);
}

#[test]
fn messages_without_an_id_or_uuid_are_counted_by_line_offset() {
    let bare = r#"{"type":"assistant","timestamp":"2026-01-01T00:00:01Z","message":{"model":"x","content":[{"type":"text","text":"t"}]}}"#;
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    let mut offset = 0;
    for line in [
        human("2026-01-01T00:00:00Z"),
        bare.to_owned(),
        bare.to_owned(),
        footer(Some("2026-01-01T00:00:05Z"), Some(1000), "9"),
    ] {
        let end = offset + line.len() as u64;
        asm.push_line(line.as_bytes(), offset..end);
        offset = end + 1;
    }
    // The same text at two offsets is two messages.
    let stats = assistant(&asm.turns()[1]).stats.clone().unwrap();
    assert_eq!(stats.message_count, 2);
}

fn late_message_lines() -> Vec<String> {
    vec![
        human("2026-01-01T00:00:00Z"),
        reply("m1", "2026-01-01T00:00:01Z", "x"),
        footer(Some("2026-01-01T00:00:05Z"), Some(4000), "100"),
        reply("m2", "2026-01-01T00:00:06Z", "x"),
    ]
}

#[test]
fn a_message_after_a_footer_updates_the_count() {
    let lines = late_message_lines();
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    for line in &lines[..3] {
        push(&mut asm, line);
    }
    assert_eq!(
        assistant(&asm.turns()[1])
            .stats
            .as_ref()
            .unwrap()
            .message_count,
        1
    );
    let updates = push(&mut asm, &lines[3]);
    match &updates[..] {
        [ModelUpdate::Updated(t)] => {
            let stats = assistant(&t[0]).stats.as_ref().unwrap();
            assert_eq!(stats.message_count, 2);
            assert_eq!(stats.duration, Duration::from_secs(5));
        }
        other => panic!("expected one Updated, got {other:?}"),
    }
}

#[test]
fn a_late_message_that_adds_no_item_still_updates_the_count_and_sends_updated() {
    let lines = late_message_lines();
    let mut asm = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(&b""[..])));
    for line in &lines[..3] {
        push(&mut asm, line);
    }
    let empty = r#"{"type":"assistant","uuid":"a-m2","timestamp":"2026-01-01T00:00:06Z","message":{"id":"m2","model":"x","content":[]}}"#;
    match &push(&mut asm, empty)[..] {
        [ModelUpdate::Updated(t)] => {
            assert_eq!(assistant(&t[0]).stats.as_ref().unwrap().message_count, 2);
        }
        other => panic!("expected one Updated, got {other:?}"),
    }
}

#[test]
fn a_synthetic_message_after_a_footer_leaves_the_count_alone() {
    let mut lines = late_message_lines();
    lines.pop();
    lines.push(reply("m2", "2026-01-01T00:00:06Z", "<synthetic>"));
    assert_eq!(last_stats(&lines).message_count, 1);
}

#[test]
fn a_late_message_gives_the_same_turns_by_chunk_and_line_by_line() {
    let lines = late_message_lines();
    let text = lines.join("\n") + "\n";

    let mut by_chunk = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(
        text.as_bytes(),
    )));
    let first_len = lines[..3].iter().map(|l| l.len() + 1).sum::<usize>();
    by_chunk.push_chunk(&text.as_bytes()[..first_len]);
    let updates = by_chunk.push_chunk(&text.as_bytes()[first_len..]);
    assert!(
        matches!(&updates[..], [ModelUpdate::Updated(t)] if
            assistant(&t[0]).stats.as_ref().unwrap().message_count == 2),
        "{updates:?}"
    );

    let mut by_line = Assembler::new(chronoscope::model::ByteSource::Memory(Arc::from(
        text.as_bytes(),
    )));
    let mut offset = 0;
    let mut last = Vec::new();
    for line in &lines {
        let end = offset + line.len() as u64;
        last = by_line.push_line(line.as_bytes(), offset..end);
        offset = end + 1;
    }
    assert!(
        matches!(&last[..], [ModelUpdate::Updated(t)] if
            assistant(&t[0]).stats.as_ref().unwrap().message_count == 2),
        "{last:?}"
    );

    let whole = assemble_str(&text);
    for turns in [by_chunk.turns(), by_line.turns()] {
        assert_eq!(turns.len(), whole.len());
        for (a, b) in turns.iter().zip(&whole) {
            assert_eq!(a, b);
        }
    }
    assert_eq!(
        assistant(&whole[1]).stats.as_ref().unwrap().message_count,
        2
    );
}

#[test]
fn a_message_count_of_the_wrong_type_does_not_fail_the_line() {
    for count in [r#""many""#, "[1]", r#"{"n":1}"#, "-3", "1.5", "null"] {
        let asm = assembled(&[
            human("2026-01-01T00:00:00Z"),
            reply("m1", "2026-01-01T00:00:01Z", "x"),
            footer(Some("2026-01-01T00:00:05Z"), Some(4000), count),
        ]);
        assert_eq!(asm.unparsed_count(), 0, "messageCount {count}");
        let stats = assistant(&asm.turns()[1]).stats.as_ref().unwrap();
        assert_eq!(stats.duration, Duration::from_secs(5));
        assert_eq!(stats.message_count, 1);
    }
}
