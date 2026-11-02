//! What the details overlay reads from a Claude transcript (cycle 2 SPEC
//! §5.2, §5.3), on constructed lines: the assembler keeps only byte ranges,
//! and the adapter reads the fields from the lines when asked.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chronoscope::components::section_source;
use chronoscope::components::turn_block::{SectionId, SectionKey};
use chronoscope::harness::Harness;
use chronoscope::harness::claude::{Assembler, ClaudeHarness};
use chronoscope::model::{
    AssistantTurn, ByteSource, Item, ItemId, SectionSource, SessionId, SourceGroup,
    SourceGroupKind, SourceKey, SourceKind, SourceRef, SourceValue, ToolCallId, ToolStatus, Turn,
};
use serde_json::{Value, json};

fn harness() -> ClaudeHarness {
    ClaudeHarness::with_data_dir(PathBuf::from("/nonexistent"), BTreeMap::new())
}

/// A transcript written as a file, and its assembled turns.
struct Transcript {
    _dir: tempfile::TempDir,
    path: PathBuf,
    turns: Vec<Arc<Turn>>,
}

fn transcript(lines: &[Value]) -> Transcript {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    write_lines(&path, lines);
    let turns = Assembler::from_file(&path).unwrap().into_turns();
    Transcript {
        _dir: dir,
        path,
        turns,
    }
}

fn write_lines(path: &Path, lines: &[Value]) {
    let mut text = String::new();
    for line in lines {
        text.push_str(&line.to_string());
        text.push('\n');
    }
    fs::write(path, text).unwrap();
}

impl Transcript {
    fn assistant(&self) -> &AssistantTurn {
        self.turns
            .iter()
            .find_map(|t| match &**t {
                Turn::Assistant(a) => Some(a),
                _ => None,
            })
            .expect("an Assistant turn")
    }

    /// The source of the item `id` of the Assistant turn.
    fn item(&self, id: &str, kind: SourceKind) -> SectionSource {
        let refs = self
            .assistant()
            .sources
            .get(&SourceKey::Item(ItemId::from(id)))
            .unwrap_or_else(|| panic!("no source for {id}"));
        SectionSource { kind, refs }
    }

    fn prompt(&self) -> SectionSource {
        let Turn::User(user) = &*self.turns[0] else {
            panic!("a User turn first")
        };
        SectionSource {
            kind: SourceKind::Prompt,
            refs: user.sources.get(&SourceKey::Own).expect("a prompt line"),
        }
    }
}

/// One group, as `label=value` strings.
#[derive(Debug, PartialEq)]
struct Rows {
    lead: Vec<String>,
    rows: Vec<String>,
    unavailable: bool,
}

fn value(value: &SourceValue) -> String {
    match value {
        SourceValue::Text(text) => text.clone(),
        SourceValue::Time(time) => time.to_rfc3339(),
        SourceValue::Tokens(n) => format!("{n} tokens"),
        SourceValue::Count(n) => n.to_string(),
    }
}

fn group(groups: &[SourceGroup], kind: SourceGroupKind) -> Option<Rows> {
    let group = groups.iter().find(|g| g.kind == kind)?;
    let rows = |rows: &[chronoscope::model::SourceRow]| {
        rows.iter()
            .map(|r| format!("{}={}", r.label, value(&r.value)))
            .collect()
    };
    Some(Rows {
        lead: rows(&group.lead),
        rows: rows(&group.rows),
        unavailable: group.unavailable,
    })
}

fn read(source: &SectionSource) -> Vec<SourceGroup> {
    harness().section_details(source)
}

// -- lines

const CWD: &str = "/home/user/project";

fn prompt_line() -> Value {
    json!({
        "type": "user", "uuid": "u1", "origin": {"kind": "human"},
        "timestamp": "2026-01-01T00:00:00Z",
        "permissionMode": "plan", "promptSource": "typed",
        "gitBranch": "main", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli",
        "message": {"role": "user", "content": "go"}
    })
}

/// A line of message `m1`: a text block, no stop reason yet.
fn first_line() -> Value {
    json!({
        "type": "assistant", "uuid": "a1", "timestamp": "2026-01-01T00:00:01Z",
        "requestId": "req_1", "requestedModel": "claude-opus-5-5",
        "gitBranch": "first", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli",
        "message": {
            "id": "m1", "model": "claude-opus-5-5", "stop_reason": null,
            "usage": {"output_tokens": 1},
            "content": [{"type": "text", "text": "Let me look."}]
        }
    })
}

/// A later line of `m1`: the tool call, with the stop reason and the usage.
fn call_line() -> Value {
    json!({
        "type": "assistant", "uuid": "a2", "timestamp": "2026-01-01T00:00:02Z",
        "requestId": "req_1", "requestedModel": "claude-opus-5-5",
        "gitBranch": "main", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli",
        "message": {
            "id": "m1", "model": "claude-opus-5-5", "stop_reason": "tool_use",
            "diagnostics": {"cache_miss_reason": {"type": "previous_message_not_found"}},
            "usage": {
                "input_tokens": 3, "output_tokens": 245,
                "service_tier": "standard", "speed": "standard", "inference_geo": "global",
                "cache_creation": {"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 13212},
                "server_tool_use": {"web_search_requests": 2, "web_fetch_requests": 1}
            },
            "content": [{"type": "tool_use", "id": "c1", "name": "Bash",
                         "input": {"command": "ls", "description": "List"}}]
        }
    })
}

fn result_line(tool_use_result: Value) -> Value {
    json!({
        "type": "user", "uuid": "r1", "timestamp": "2026-01-01T00:00:09Z",
        "permissionDecision": {"decision": "accept", "source": "config", "reasonType": "classifier"},
        "sandboxed": false,
        "gitBranch": "result-branch", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli",
        "toolUseResult": tool_use_result,
        "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "c1", "content": "file.txt"}
        ]}
    })
}

fn duration_line() -> Value {
    json!({
        "type": "system", "subtype": "turn_duration", "uuid": "d1",
        "timestamp": "2026-01-01T00:00:10Z", "durationMs": 9000,
        "gitBranch": "footer", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli"
    })
}

fn full() -> Transcript {
    transcript(&[
        prompt_line(),
        first_line(),
        call_line(),
        result_line(json!({"isAsync": true, "resolvedModel": "claude-sonnet-5-5", "stdout": "x"})),
        duration_line(),
    ])
}

fn environment(branch: &str) -> Vec<String> {
    vec![
        format!("gitBranch={branch}"),
        format!("cwd={CWD}"),
        "version=2.1.100".to_owned(),
        "entrypoint=cli".to_owned(),
    ]
}

// -- the groups

#[test]
fn the_message_group_lists_the_fields_of_the_message_in_order() {
    let t = full();
    let groups = read(&t.item("m1:0", SourceKind::Message));
    let message = group(&groups, SourceGroupKind::Message).unwrap();
    assert_eq!(
        message.rows,
        [
            "message.id=m1",
            "requestId=req_1",
            "timestamp=2026-01-01T00:00:02+00:00",
            "requestedModel=claude-opus-5-5",
            "stop_reason=tool_use",
            "service_tier=standard",
            "speed=standard",
            "inference_geo=global",
            "5m cache writes=0 tokens",
            "1h cache writes=13212 tokens",
            "web searches=2",
            "web fetches=1",
            "diagnostics.cache_miss_reason=type=previous_message_not_found",
        ]
    );
    assert!(message.lead.is_empty() && !message.unavailable);
}

#[test]
fn a_text_block_on_the_first_line_of_a_message_gets_the_stop_reason_of_a_later_line() {
    let t = full();
    // The text is on the first line, where `stop_reason` is null.
    let item = t.assistant().items[0].clone();
    assert!(matches!(item, Item::Text { .. }), "{item:?}");
    let groups = read(&t.item("m1:0", SourceKind::Message));
    let message = group(&groups, SourceGroupKind::Message).unwrap();
    assert!(message.rows.contains(&"stop_reason=tool_use".to_owned()));
    // And the usage, which is complete on the later line only.
    assert!(message.rows.contains(&"service_tier=standard".to_owned()));
    // The Environment of the section is read from that line too.
    let env = group(&groups, SourceGroupKind::Environment).unwrap();
    assert_eq!(env.rows, environment("main"));
}

#[test]
fn a_message_without_a_stop_reason_reads_its_latest_line() {
    let mut second = first_line();
    second["uuid"] = json!("a2");
    second["timestamp"] = json!("2026-01-01T00:00:05Z");
    second["message"]["content"] = json!([{"type": "text", "text": "More."}]);
    second["apiBlockIndex"] = json!(1);
    let mut first = first_line();
    first["apiBlockIndex"] = json!(0);
    let t = transcript(&[prompt_line(), first, second]);
    let groups = read(&t.item("m1:0", SourceKind::Message));
    let message = group(&groups, SourceGroupKind::Message).unwrap();
    assert!(
        message
            .rows
            .contains(&"timestamp=2026-01-01T00:00:05+00:00".to_owned()),
        "{message:?}"
    );
    assert!(!message.rows.iter().any(|r| r.starts_with("stop_reason")));
}

#[test]
fn a_tool_calls_start_is_from_its_call_and_the_rest_from_its_result() {
    let t = full();
    let groups = read(&t.item("c1", SourceKind::ToolCall));
    let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
    assert_eq!(call.lead, ["start=2026-01-01T00:00:02+00:00"]);
    assert_eq!(
        call.rows,
        [
            "permissionDecision=decision=accept, reasonType=classifier, source=config",
            "sandboxed=false",
            "resolvedModel=claude-sonnet-5-5",
            "isAsync=true",
        ]
    );
    assert!(!call.unavailable);
    // Where the call was recorded: the line of the call.
    let env = group(&groups, SourceGroupKind::Environment).unwrap();
    assert_eq!(env.rows, environment("main"));
    // The result's line is not the call's: its branch is another one.
    let result = t
        .assistant()
        .sources
        .get(&SourceKey::Item(ItemId::from("c1")))
        .unwrap()
        .result
        .unwrap();
    assert_ne!(
        result.range,
        t.item("c1", SourceKind::ToolCall).refs.line.range
    );
}

#[test]
fn the_tool_call_fields_are_read_from_the_top_of_the_result_line_first() {
    // `permissionDecision` as a plain string, `sandboxed` and `isAsync` in
    // `toolUseResult` only.
    let mut result = result_line(json!({"sandboxed": true, "isAsync": false}));
    result["permissionDecision"] = json!("allow");
    result.as_object_mut().unwrap().remove("sandboxed");
    let t = transcript(&[prompt_line(), first_line(), call_line(), result]);
    let groups = read(&t.item("c1", SourceKind::ToolCall));
    let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
    assert_eq!(
        call.rows,
        [
            "permissionDecision=allow",
            "sandboxed=true",
            "isAsync=false"
        ]
    );
}

#[test]
fn the_prompt_group_lists_its_fields() {
    let t = full();
    let groups = read(&t.prompt());
    let prompt = group(&groups, SourceGroupKind::Prompt).unwrap();
    assert_eq!(
        prompt.rows,
        [
            "timestamp=2026-01-01T00:00:00+00:00",
            "permissionMode=plan",
            "promptSource=typed",
        ]
    );
    assert_eq!(
        group(&groups, SourceGroupKind::Environment).unwrap().rows,
        environment("main")
    );
    assert!(group(&groups, SourceGroupKind::Message).is_none());
}

#[test]
fn the_footer_reads_its_turn_duration_line_for_the_environment_only() {
    let t = full();
    let refs = t
        .assistant()
        .sources
        .get(&SourceKey::Footer)
        .expect("the footer has a line");
    let groups = read(&SectionSource {
        kind: SourceKind::Other,
        refs,
    });
    assert_eq!(groups.len(), 1);
    assert_eq!(
        group(&groups, SourceGroupKind::Environment).unwrap().rows,
        environment("footer")
    );
}

#[test]
fn a_compaction_and_a_system_section_read_their_first_line_for_the_environment() {
    let boundary = json!({
        "type": "system", "subtype": "compact_boundary", "uuid": "cb1",
        "timestamp": "2026-01-01T00:05:00Z",
        "gitBranch": "boundary", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli"
    });
    let summary = json!({
        "type": "user", "uuid": "cs1", "isCompactSummary": true,
        "timestamp": "2026-01-01T00:05:01Z", "gitBranch": "summary",
        "message": {"role": "user", "content": "Summary"}
    });
    let t = transcript(&[prompt_line(), boundary, summary]);
    let Turn::Compaction(compaction) = &*t.turns[1] else {
        panic!("{:?}", t.turns)
    };
    let source = SectionSource {
        kind: SourceKind::Other,
        refs: compaction.sources.get(&SourceKey::Own).unwrap(),
    };
    let groups = read(&source);
    assert_eq!(
        group(&groups, SourceGroupKind::Environment).unwrap().rows,
        environment("boundary"),
        "the boundary line, not the summary's"
    );

    let hook = |uuid: &str, branch: &str| {
        json!({
            "type": "attachment", "uuid": uuid, "timestamp": "2026-01-01T00:00:03Z",
            "gitBranch": branch, "cwd": CWD,
            "attachment": {"type": "hook_system_message", "content": "x"}
        })
    };
    let t = transcript(&[
        prompt_line(),
        hook("h1", "first-hook"),
        hook("h2", "second-hook"),
        first_line(),
    ]);
    let Turn::User(user) = &*t.turns[0] else {
        panic!()
    };
    let Some(Item::Injected { id, entries }) = user.items.first() else {
        panic!("{:?}", user.items)
    };
    assert_eq!(entries.len(), 2);
    let source = SectionSource {
        kind: SourceKind::Other,
        refs: user.sources.get(&SourceKey::Item(id.clone())).unwrap(),
    };
    let env = group(&read(&source), SourceGroupKind::Environment).unwrap();
    assert_eq!(env.rows[0], "gitBranch=first-hook");
}

// -- the other kinds of section of an Assistant turn

fn hook(uuid: &str, branch: &str) -> Value {
    json!({
        "type": "attachment", "uuid": uuid, "timestamp": "2026-01-01T00:00:03Z",
        "gitBranch": branch, "cwd": CWD,
        "attachment": {"type": "hook_system_message", "content": "x"}
    })
}

/// The source of item `id` of the turn at `turn`, as the details key makes
/// it.
fn section(t: &Transcript, turn: usize, id: &str) -> SectionSource {
    let section = SectionId {
        turn: t.turns[turn].id().clone(),
        key: SectionKey::Item(ItemId::from(id)),
    };
    section_source(&t.turns[turn], &section).unwrap_or_else(|| panic!("no source for {id}"))
}

fn environment_branch(source: &SectionSource) -> String {
    group(&read(source), SourceGroupKind::Environment)
        .unwrap()
        .rows[0]
        .clone()
}

#[test]
fn a_notice_reads_the_message_group_and_environment_of_the_line_that_ended_the_message() {
    let mut ending = first_line();
    ending["uuid"] = json!("a2");
    ending["timestamp"] = json!("2026-01-01T00:00:03Z");
    ending["gitBranch"] = json!("ending");
    ending["message"]["stop_reason"] = json!("max_tokens");
    ending["message"]["content"] = json!([]);
    let t = transcript(&[prompt_line(), first_line(), ending]);
    let source = section(&t, 1, "notice:m1");
    assert_eq!(source.kind, SourceKind::Message);
    let groups = read(&source);
    let message = group(&groups, SourceGroupKind::Message).unwrap();
    assert!(message.rows.contains(&"stop_reason=max_tokens".to_owned()));
    assert!(
        message
            .rows
            .contains(&"timestamp=2026-01-01T00:00:03+00:00".to_owned()),
        "{message:?}"
    );
    assert_eq!(environment_branch(&source), "gitBranch=ending");
}

#[test]
fn an_api_error_notice_reads_the_message_group_and_environment() {
    let error = json!({
        "type": "assistant", "uuid": "a1", "timestamp": "2026-01-01T00:00:01Z",
        "isApiErrorMessage": true, "requestId": "req_err", "gitBranch": "error", "cwd": CWD,
        "message": {"id": "msg_e", "model": "<synthetic>", "content": [
            {"type": "text", "text": "API Error: 529 overloaded"}]}
    });
    let t = transcript(&[prompt_line(), error]);
    let source = section(&t, 1, "notice:msg_e");
    assert_eq!(source.kind, SourceKind::Message);
    let groups = read(&source);
    let message = group(&groups, SourceGroupKind::Message).unwrap();
    assert!(message.rows.contains(&"message.id=msg_e".to_owned()));
    assert!(message.rows.contains(&"requestId=req_err".to_owned()));
    assert_eq!(environment_branch(&source), "gitBranch=error");
}

#[test]
fn a_model_note_reads_the_environment_of_its_message_and_no_message_group() {
    let mut other = first_line();
    other["uuid"] = json!("a2");
    other["gitBranch"] = json!("other-model");
    other["message"]["id"] = json!("m2");
    other["message"]["model"] = json!("claude-sonnet-5-5");
    let t = transcript(&[prompt_line(), first_line(), other]);
    let source = section(&t, 1, "model:m2");
    assert_eq!(source.kind, SourceKind::Other);
    let groups = read(&source);
    assert!(group(&groups, SourceGroupKind::Message).is_none());
    assert_eq!(environment_branch(&source), "gitBranch=other-model");
}

#[test]
fn a_system_section_in_an_assistant_turn_reads_the_environment_of_its_first_line() {
    // Flushed into the open turn by the next assistant line.
    let mut next = first_line();
    next["uuid"] = json!("a2");
    next["message"]["id"] = json!("m2");
    let t = transcript(&[
        prompt_line(),
        first_line(),
        hook("h1", "mid-turn"),
        hook("h2", "second-hook"),
        next,
    ]);
    let source = section(&t, 1, "injected:h1");
    assert_eq!(source.kind, SourceKind::Other);
    assert_eq!(environment_branch(&source), "gitBranch=mid-turn");
}

#[test]
fn held_entries_that_open_an_assistant_turn_read_the_environment_of_their_first_line() {
    // After a compaction the entries stay held until the next assistant line
    // opens a turn.
    let boundary = json!({
        "type": "system", "subtype": "compact_boundary", "uuid": "cb1",
        "timestamp": "2026-01-01T00:00:01Z"
    });
    let summary = json!({
        "type": "user", "uuid": "cs1", "isCompactSummary": true,
        "timestamp": "2026-01-01T00:00:02Z", "message": {"role": "user", "content": "S"}
    });
    let t = transcript(&[
        prompt_line(),
        boundary,
        summary,
        hook("h1", "opening"),
        first_line(),
    ]);
    let turn = t.turns.len() - 1;
    assert!(matches!(&*t.turns[turn], Turn::Assistant(_)));
    let source = section(&t, turn, "injected:h1");
    assert_eq!(environment_branch(&source), "gitBranch=opening");
}

#[test]
fn an_empty_string_is_no_value() {
    let mut line = first_line();
    line["version"] = json!("");
    line["gitBranch"] = json!("");
    line["message"]["usage"]["speed"] = json!("");
    let t = transcript(&[prompt_line(), line]);
    let groups = read(&t.item("m1:0", SourceKind::Message));
    let env = group(&groups, SourceGroupKind::Environment).unwrap();
    assert_eq!(
        env.rows,
        [format!("cwd={CWD}"), "entrypoint=cli".to_owned()]
    );
    let message = group(&groups, SourceGroupKind::Message).unwrap();
    assert!(!message.rows.iter().any(|r| r.starts_with("speed")));
}

// -- assembly stays linear in the items of a turn

/// One Assistant turn of `n` one-line text messages, assembled; the time it
/// takes.
fn time_long_turn(n: usize) -> std::time::Duration {
    let mut text = prompt_line().to_string();
    for i in 0..n {
        let mut line = first_line();
        line["uuid"] = json!(format!("a{i}"));
        line["message"]["id"] = json!(format!("m{i}"));
        text.push('\n');
        text.push_str(&line.to_string());
    }
    let bytes: Arc<[u8]> = Arc::from(text.as_bytes());
    let start = std::time::Instant::now();
    let asm = Assembler::from_memory(bytes);
    let took = start.elapsed();
    assert_eq!(asm.turns().len(), 2);
    took
}

#[test]
fn assembling_a_long_turn_takes_time_in_proportion_to_its_items() {
    // Looking each new item up in the references would make this quadratic:
    // eight times the items would take sixty-four times as long.
    let best = |n| (0..2).map(|_| time_long_turn(n)).min().unwrap();
    let (small, large) = (best(3_000), best(24_000));
    assert!(
        large < small * 24,
        "3000 items took {small:?}, 24000 took {large:?}"
    );
}

// -- fields that are missing or of another type

#[test]
fn missing_fields_are_left_out_and_a_group_with_no_rows_is_empty() {
    let bare = json!({
        "type": "assistant", "uuid": "a1", "timestamp": "2026-01-01T00:00:01Z",
        "message": {"id": "m1", "model": "x", "content": [{"type": "text", "text": "hi"}]}
    });
    let t = transcript(&[
        json!({"type": "user", "uuid": "u1", "origin": {"kind": "human"},
               "message": {"content": "go"}}),
        bare,
    ]);
    let groups = read(&t.item("m1:0", SourceKind::Message));
    let message = group(&groups, SourceGroupKind::Message).unwrap();
    assert_eq!(
        message.rows,
        ["message.id=m1", "timestamp=2026-01-01T00:00:01+00:00"]
    );
    let env = group(&groups, SourceGroupKind::Environment).unwrap();
    assert!(env.rows.is_empty() && !env.unavailable, "{env:?}");
    let prompt = group(&read(&t.prompt()), SourceGroupKind::Prompt).unwrap();
    assert!(prompt.rows.is_empty() && !prompt.unavailable, "{prompt:?}");
}

#[test]
fn claude_records_no_cost_so_none_is_shown() {
    let t = full();
    for source in [
        t.item("m1:0", SourceKind::Message),
        t.item("c1", SourceKind::ToolCall),
        t.prompt(),
    ] {
        for group in read(&source) {
            for row in group.lead.iter().chain(&group.rows) {
                assert!(!row.label.contains("cost"), "{row:?}");
            }
        }
    }
}

#[test]
fn a_field_of_an_unexpected_type_counts_as_missing_and_does_not_fail_the_line() {
    let line = json!({
        "type": "assistant", "uuid": "a1", "timestamp": "2026-01-01T00:00:01Z",
        "requestId": 7, "requestedModel": ["x"], "gitBranch": {"a": 1}, "cwd": CWD,
        "version": true, "entrypoint": null, "diagnostics": "no",
        "message": {
            "id": "m1", "stop_reason": 5, "model": "x",
            "usage": {"service_tier": {"x": 1}, "speed": ["a"], "inference_geo": 3,
                      "cache_creation": "no", "server_tool_use": {"web_search_requests": "2",
                      "web_fetch_requests": -1}},
            "content": [{"type": "text", "text": "hi"}]
        }
    });
    let t = transcript(&[prompt_line(), line]);
    let groups = read(&t.item("m1:0", SourceKind::Message));
    let message = group(&groups, SourceGroupKind::Message).unwrap();
    assert_eq!(
        message.rows,
        ["message.id=m1", "timestamp=2026-01-01T00:00:01+00:00"]
    );
    assert!(!message.unavailable);
    let env = group(&groups, SourceGroupKind::Environment).unwrap();
    assert_eq!(env.rows, [format!("cwd={CWD}")]);
}

#[test]
fn a_tool_use_result_that_is_a_string_gives_the_top_level_fields_only() {
    let mut result = result_line(json!("Error: the command failed"));
    result["isAsync"] = json!(false);
    let t = transcript(&[prompt_line(), first_line(), call_line(), result]);
    let groups = read(&t.item("c1", SourceKind::ToolCall));
    let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
    assert!(!call.unavailable);
    assert_eq!(
        call.rows,
        [
            "permissionDecision=decision=accept, reasonType=classifier, source=config",
            "sandboxed=false",
            "isAsync=false",
        ]
    );
}

#[test]
fn a_very_large_tool_use_result_is_skipped_and_its_fields_are_read() {
    // 3 MB of stdout, and a nested object and array that must not be read.
    let stdout = "x".repeat(3 * 1024 * 1024);
    let result = result_line(json!({
        "stdout": stdout, "nested": {"isAsync": "not a bool", "deep": [[1, 2], {"a": null}]},
        "isAsync": true, "resolvedModel": "claude-haiku-5-5", "sandboxed": true
    }));
    let t = transcript(&[prompt_line(), first_line(), call_line(), result]);
    let groups = read(&t.item("c1", SourceKind::ToolCall));
    let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
    assert!(!call.unavailable);
    // The top of the line wins over `toolUseResult` for what both have.
    assert_eq!(
        call.rows[1..],
        [
            "sandboxed=false",
            "resolvedModel=claude-haiku-5-5",
            "isAsync=true"
        ]
    );
}

// -- a source that cannot be read

fn unavailable(groups: &[SourceGroup]) -> Vec<SourceGroupKind> {
    groups
        .iter()
        .filter(|g| g.unavailable)
        .map(|g| g.kind)
        .collect()
}

#[test]
fn a_file_that_is_gone_makes_every_group_of_the_section_unavailable() {
    let t = full();
    let source = t.item("m1:0", SourceKind::Message);
    fs::remove_file(&t.path).unwrap();
    let groups = read(&source);
    assert_eq!(
        unavailable(&groups),
        [SourceGroupKind::Message, SourceGroupKind::Environment]
    );
    for group in &groups {
        assert!(group.lead.is_empty() && group.rows.is_empty());
    }
    let groups = read(&t.prompt());
    assert_eq!(
        unavailable(&groups),
        [SourceGroupKind::Prompt, SourceGroupKind::Environment]
    );
    let groups = read(&t.item("c1", SourceKind::ToolCall));
    assert_eq!(
        unavailable(&groups),
        [SourceGroupKind::ToolCall, SourceGroupKind::Environment]
    );
}

#[test]
fn a_range_past_the_end_of_the_file_is_unavailable() {
    let t = full();
    let source = t.item("m1:0", SourceKind::Message);
    // The file was replaced by a shorter one.
    fs::write(&t.path, b"{}\n").unwrap();
    assert_eq!(
        unavailable(&read(&source)),
        [SourceGroupKind::Message, SourceGroupKind::Environment]
    );
    // Also for an in-memory transcript.
    let past = SectionSource {
        kind: SourceKind::Other,
        refs: chronoscope::model::SectionRefs {
            line: SourceRef {
                bytes: ByteSource::Memory(Arc::from(&b"{}"[..])),
                range: 10..20,
            },
            result: None,
        },
    };
    assert_eq!(unavailable(&read(&past)), [SourceGroupKind::Environment]);
}

#[test]
fn a_line_that_does_not_parse_is_unavailable() {
    let t = full();
    let source = t.item("m1:0", SourceKind::Message);
    // Same length, no longer JSON.
    let text = fs::read_to_string(&t.path).unwrap().replace('{', "(");
    fs::write(&t.path, text).unwrap();
    assert_eq!(
        unavailable(&read(&source)),
        [SourceGroupKind::Message, SourceGroupKind::Environment]
    );
    // A line that is JSON but not an object is not a transcript line.
    let array = SectionSource {
        kind: SourceKind::Prompt,
        refs: chronoscope::model::SectionRefs {
            line: SourceRef {
                bytes: ByteSource::Memory(Arc::from(&b"[1,2]"[..])),
                range: 0..5,
            },
            result: None,
        },
    };
    assert_eq!(
        unavailable(&read(&array)),
        [SourceGroupKind::Prompt, SourceGroupKind::Environment]
    );
}

#[test]
fn a_result_line_that_fails_leaves_start_and_marks_the_tool_call_unavailable() {
    let t = full();
    let source = t.item("c1", SourceKind::ToolCall);
    let result = source.refs.result.clone().expect("a result line");
    // Break only the result line.
    let mut bytes = fs::read(&t.path).unwrap();
    let range =
        usize::try_from(result.range.start).unwrap()..usize::try_from(result.range.end).unwrap();
    bytes[range.start] = b'(';
    fs::write(&t.path, bytes).unwrap();
    let groups = read(&source);
    let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
    assert_eq!(call.lead, ["start=2026-01-01T00:00:02+00:00"]);
    assert!(call.rows.is_empty() && call.unavailable);
    let env = group(&groups, SourceGroupKind::Environment).unwrap();
    assert!(!env.unavailable, "the call's line reads");
}

#[test]
fn a_call_without_a_result_yet_is_not_unavailable() {
    let t = transcript(&[prompt_line(), first_line(), call_line()]);
    let Item::ToolCall(call) = t.assistant().items.last().unwrap() else {
        panic!()
    };
    assert_eq!(call.status, ToolStatus::Pending);
    let source = t.item("c1", SourceKind::ToolCall);
    assert!(source.refs.result.is_none());
    let groups = read(&source);
    let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
    assert_eq!(call.lead, ["start=2026-01-01T00:00:02+00:00"]);
    assert!(call.rows.is_empty());
    assert!(unavailable(&groups).is_empty(), "{groups:?}");
}

#[test]
fn a_result_that_arrives_later_is_referenced_by_the_same_call() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live.jsonl");
    write_lines(&path, &[prompt_line(), first_line(), call_line()]);
    let mut asm = Assembler::new(ByteSource::File(Arc::new(path.clone())));
    let mut text = fs::read(&path).unwrap();
    asm.push_chunk(&text);
    let before = asm.turns()[1]
        .sources()
        .get(&SourceKey::Item(ItemId::from("c1")));
    assert!(before.unwrap().result.is_none());
    let line = result_line(json!({"isAsync": false})).to_string();
    text.extend_from_slice(line.as_bytes());
    text.push(b'\n');
    fs::write(&path, &text).unwrap();
    let updates = asm.push_chunk(&text[text.len() - line.len() - 1..]);
    assert!(!updates.is_empty());
    let after = asm.turns()[1]
        .sources()
        .get(&SourceKey::Item(ItemId::from("c1")))
        .unwrap();
    assert!(after.result.is_some());
}

#[test]
fn a_later_line_of_a_message_updates_the_turn_even_when_nothing_visible_changes() {
    // The first line has the text, the second only records how the message
    // ended: no new block, but the text section's references must move to it.
    let ending = json!({
        "type": "assistant", "uuid": "a2", "timestamp": "2026-01-01T00:00:03Z",
        "message": {"id": "m1", "model": "claude-opus-5-5", "stop_reason": "end_turn",
                    "usage": {"service_tier": "standard"}, "content": []}
    });
    let first = format!("{}\n{}\n", prompt_line(), first_line());
    let second = format!("{ending}\n");
    let bytes: Arc<[u8]> = Arc::from(format!("{first}{second}").as_bytes());
    let mut asm = Assembler::new(ByteSource::Memory(Arc::clone(&bytes)));
    asm.push_chunk(first.as_bytes());
    let text_range = |asm: &Assembler| {
        asm.turns()[1]
            .sources()
            .get(&SourceKey::Item(ItemId::from("m1:0")))
            .unwrap()
            .line
            .range
    };
    let before = text_range(&asm);
    let updates = asm.push_chunk(second.as_bytes());
    assert!(
        updates
            .iter()
            .any(|u| matches!(u, chronoscope::model::ModelUpdate::Updated(t) if t.len() == 1)),
        "{updates:?}"
    );
    let after = text_range(&asm);
    assert_ne!(before, after);
    assert_eq!(
        usize::try_from(after.start).unwrap(),
        first.len(),
        "the line that ends the message"
    );
    // A chunking of the same bytes ends with the same references.
    let whole = Assembler::from_memory(bytes);
    assert_eq!(whole.turns(), asm.turns());
}

// -- a subagent reads its own file

#[test]
fn a_subagent_transcript_reads_its_own_file() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("parent.jsonl");
    let child_dir = dir.path().join("parent").join("subagents");
    fs::create_dir_all(&child_dir).unwrap();
    let child = child_dir.join("agent-ab12.jsonl");
    let mut parent_call = call_line();
    parent_call["gitBranch"] = json!("parent-branch");
    let mut child_call = call_line();
    child_call["gitBranch"] = json!("child-branch");
    write_lines(&parent, &[prompt_line(), first_line(), parent_call]);
    write_lines(&child, &[prompt_line(), first_line(), child_call]);
    let parent_turns = Assembler::from_file(&parent).unwrap().into_turns();
    let child_turns = Assembler::from_file(&child).unwrap().into_turns();

    let source = |turns: &[Arc<Turn>]| {
        let Turn::Assistant(a) = &*turns[1] else {
            panic!()
        };
        SectionSource {
            kind: SourceKind::Message,
            refs: a
                .sources
                .get(&SourceKey::Item(ItemId::from("m1:0")))
                .unwrap(),
        }
    };
    let (p, c) = (source(&parent_turns), source(&child_turns));
    assert_eq!(
        p.refs.line.bytes,
        ByteSource::File(Arc::new(parent.clone()))
    );
    assert_eq!(c.refs.line.bytes, ByteSource::File(Arc::new(child.clone())));
    let branch = |source: &SectionSource| {
        group(&read(source), SourceGroupKind::Environment)
            .unwrap()
            .rows[0]
            .clone()
    };
    assert_eq!(branch(&p), "gitBranch=parent-branch");
    assert_eq!(branch(&c), "gitBranch=child-branch");
}

#[test]
fn a_loaded_subagent_names_its_own_file() {
    // Through the harness: the turns of a subagent session name the
    // subagent's file, not the parent's.
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude/subagents");
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixtures, dir.path());
    let harness = ClaudeHarness::with_data_dir(dir.path().to_path_buf(), BTreeMap::new());
    let file = dir.path().join("-home-user-loading/pppp0001.jsonl");
    let parent = harness.open_file(&file).unwrap();
    let agent = harness
        .subagent(&parent.id, &ToolCallId::from("toolu_meta01"))
        .expect("the subagent is found");
    let named = |id: &SessionId| {
        let turns = load(&harness, id);
        let bytes = turns
            .iter()
            .find_map(|t| match &**t {
                Turn::Assistant(a) => a
                    .items
                    .iter()
                    .find_map(|i| a.sources.get(&SourceKey::Item(i.id().clone()))),
                _ => None,
            })
            .expect("a section with a source")
            .line
            .bytes;
        match bytes {
            ByteSource::File(path) => path.as_ref().clone(),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(named(&parent.id), file);
    assert_eq!(
        named(&agent),
        dir.path()
            .join("-home-user-loading/pppp0001/subagents/agent-m1.jsonl")
    );
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn load(harness: &dyn Harness, id: &SessionId) -> Vec<Arc<Turn>> {
    use chronoscope::model::ModelUpdate;
    use std::sync::mpsc;
    use std::time::Duration;
    let (tx, rx) = mpsc::channel();
    let _handle = harness.load(id, tx).unwrap();
    let mut turns = Vec::new();
    loop {
        let update = rx.recv_timeout(Duration::from_secs(30)).expect("loads");
        update.apply(&mut turns);
        if update == ModelUpdate::Loaded {
            return turns;
        }
    }
}

// -- assembling does not read the detail fields

#[test]
fn assembling_a_line_with_detail_fields_of_unexpected_types_and_sizes_is_cheap() {
    let huge = "y".repeat(2 * 1024 * 1024);
    let odd = json!({
        "type": "assistant", "uuid": "a1", "timestamp": "2026-01-01T00:00:01Z",
        "requestId": {"a": [1, 2, 3]}, "requestedModel": 5, "gitBranch": [huge.clone()],
        "cwd": CWD, "version": false, "entrypoint": {"x": huge.clone()},
        "permissionDecision": 12, "sandboxed": "maybe", "resolvedModel": [], "isAsync": "yes",
        "diagnostics": [1], "promptSource": 1, "permissionMode": {},
        "message": {
            "id": "m1", "model": "x", "stop_reason": "end_turn",
            "diagnostics": huge.clone(),
            "usage": {"service_tier": [1], "speed": 7, "inference_geo": {},
                      "cache_creation": [huge.clone()], "server_tool_use": "none"},
            "content": [{"type": "text", "text": "hi"}]
        }
    });
    let odd_result = json!({
        "type": "user", "uuid": "r1", "timestamp": "2026-01-01T00:00:03Z",
        "permissionDecision": [huge.clone()], "sandboxed": 3,
        "toolUseResult": {"isAsync": false, "stdout": huge, "resolvedModel": {"a": 1},
                          "permissionDecision": 4, "sandboxed": "x"},
        "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "c1", "content": "ok"}]}
    });
    let tool = json!({
        "type": "assistant", "uuid": "a2", "timestamp": "2026-01-01T00:00:02Z",
        "message": {"id": "m2", "model": "x", "content": [
            {"type": "tool_use", "id": "c1", "name": "Bash", "input": {"command": "ls"}}]}
    });
    let text = [prompt_line(), odd, tool, odd_result]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let asm = Assembler::from_memory(Arc::from(text.as_bytes()));
    assert_eq!(asm.unparsed_count(), 0, "a line was not understood");
    let turns = asm.turns();
    assert_eq!(turns.len(), 2);
    let Turn::Assistant(turn) = &*turns[1] else {
        panic!()
    };
    assert_eq!(turn.items.len(), 2);
    // The turn holds a reference to the line, whatever its fields hold.
    assert!(
        turn.sources
            .get(&SourceKey::Item(ItemId::from("m1:0")))
            .is_some()
    );
}
