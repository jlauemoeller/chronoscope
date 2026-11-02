//! The details overlay with what the harness reads from the source (SPEC
//! cycle 2 §5.2, §5.3), through `App::update` on a Claude transcript written
//! to a file. The overlay opens, reads the section's lines and shows their
//! fields; a source that cannot be read leaves one muted `unavailable` row
//! and the overlay open.

use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::details::{open, overlay_rows};
use super::*;
use crate::harness::claude::{Assembler, ClaudeHarness};

const CWD: &str = "/home/user/projects/brio";

fn prompt_line() -> Value {
    json!({
        "type": "user", "uuid": "u1", "origin": {"kind": "human"},
        "timestamp": "2026-10-21T08:35:15.800Z",
        "permissionMode": "auto", "promptSource": "typed",
        "gitBranch": "main", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli",
        "message": {"role": "user", "content": "Is the camera on USB?"}
    })
}

fn text_line() -> Value {
    json!({
        "type": "assistant", "uuid": "a1", "timestamp": "2026-10-21T08:35:18.271Z",
        "requestId": "req_01", "requestedModel": "claude-opus-5-5",
        "gitBranch": "main", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli",
        "message": {
            "id": "msg_01", "model": "claude-opus-5-5", "stop_reason": null,
            "content": [{"type": "text", "text": "I'll list the USB devices."}]
        }
    })
}

fn call_line() -> Value {
    json!({
        "type": "assistant", "uuid": "a2", "timestamp": "2026-10-21T08:35:19.000Z",
        "requestId": "req_01", "requestedModel": "claude-opus-5-5",
        "gitBranch": "main", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli",
        "message": {
            "id": "msg_01", "model": "claude-opus-5-5", "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 3, "output_tokens": 245,
                "service_tier": "standard", "speed": "standard", "inference_geo": "global",
                "cache_creation": {"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 13212},
                "server_tool_use": {"web_search_requests": 0, "web_fetch_requests": 0}
            },
            "content": [{"type": "tool_use", "id": "c1", "name": "Bash",
                         "input": {"command": "lsusb", "description": "List USB devices"}}]
        }
    })
}

fn result_line() -> Value {
    json!({
        "type": "user", "uuid": "r1", "timestamp": "2026-10-21T08:35:21.000Z",
        "permissionDecision": {"decision": "accept", "source": "config"},
        "sandboxed": false,
        "gitBranch": "main", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli",
        "toolUseResult": {"stdout": "Bus 001 Device 004", "isAsync": false},
        "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "c1", "content": "Bus 001 Device 004"}]}
    })
}

fn duration_line() -> Value {
    json!({
        "type": "system", "subtype": "turn_duration", "uuid": "d1",
        "timestamp": "2026-10-21T08:35:23.000Z", "durationMs": 5000,
        "gitBranch": "main", "cwd": CWD, "version": "2.1.100", "entrypoint": "cli"
    })
}

/// A Claude transcript on disk, the turns assembled from it and an app that
/// has them open, with the real Claude harness to read the details.
struct Claude {
    rig: Rig,
    path: PathBuf,
    _dir: tempfile::TempDir,
}

fn claude(width: u16, height: u16, lines: &[Value], harness: bool) -> Claude {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    fs::write(&path, text).unwrap();
    let turns = Assembler::from_file(&path)
        .unwrap()
        .into_turns()
        .into_iter()
        .collect();
    let mut rig = rig_with(theme("default"), width, height);
    rig.open_with(turns);
    if harness {
        let claude = ClaudeHarness::with_data_dir(dir.path().to_path_buf(), Default::default());
        rig.app
            .set_harnesses([Arc::new(claude) as Arc<dyn Harness>]);
    }
    Claude {
        rig,
        path,
        _dir: dir,
    }
}

fn full() -> Vec<Value> {
    vec![
        prompt_line(),
        text_line(),
        call_line(),
        result_line(),
        duration_line(),
    ]
}

/// [`full`] with the effort and the cache and thinking tokens a Claude
/// transcript can record, which fill the whole Turn group.
fn rich() -> Vec<Value> {
    let mut lines = full();
    lines[1]["effort"] = json!("high");
    lines[2]["message"]["usage"]["cache_read_input_tokens"] = json!(29361);
    lines[2]["message"]["usage"]["cache_creation_input_tokens"] = json!(13212);
    lines[2]["message"]["usage"]["output_tokens_details"] = json!({"thinking_tokens": 113});
    lines
}

impl Claude {
    /// Selects the section `n` below the prompt and opens its details.
    fn details_of(&mut self, n: usize) -> Vec<String> {
        self.rig.down(n);
        self.rig.settle();
        self.rig.open_details();
        let area = self.rig.app.routing().area(Id::DetailsOverlay).unwrap();
        let (width, height) = (120, 40);
        assert!(area.width <= width && area.height <= height);
        overlay_rows(&mut self.rig, width, height)
            .iter()
            .map(|row| {
                row.trim_matches(|c: char| c == '│' || c.is_whitespace())
                    .to_owned()
            })
            .collect()
    }
}

/// The position of the first row that starts with `text`.
fn at(rows: &[String], text: &str) -> usize {
    rows.iter()
        .position(|row| row.starts_with(text))
        .unwrap_or_else(|| panic!("no row starts with {text:?} in {rows:#?}"))
}

/// The value of the row `label` in the group headed `group`.
fn value_in(rows: &[String], group: &str, label: &str) -> String {
    let from = at(rows, group);
    let end = rows[from..]
        .iter()
        .skip(2)
        .position(|r| r.is_empty())
        .map_or(rows.len(), |n| from + 2 + n);
    rows[from..end]
        .iter()
        .find_map(|row| row.strip_prefix(label))
        .unwrap_or_else(|| panic!("no {label:?} row in {group}: {rows:#?}"))
        .trim()
        .to_owned()
}

fn has(rows: &[String], text: &str) -> bool {
    rows.iter().any(|row| row.starts_with(text))
}

/// Asserts that rows start with `labels`, one after the other, in this order.
fn assert_in_order(rows: &[String], labels: &[&str]) {
    let mut from = 0;
    for label in labels {
        match rows[from..].iter().position(|row| row.starts_with(label)) {
            Some(found) => from += found + 1,
            None => panic!("no {label:?} row after row {from} (of {labels:?}) in {rows:#?}"),
        }
    }
}

#[test]
fn a_text_section_shows_the_message_and_environment_rows_of_its_source() {
    let mut c = claude(120, 40, &rich(), true);
    // The prompt is selected first; the text is below it.
    let rows = c.details_of(1);
    // The Turn group is the model's: every row a Claude transcript fills.
    assert_in_order(
        &rows,
        &[
            "Turn",
            "model",
            "effort",
            "start",
            "duration",
            "messages",
            "input tokens",
            "output tokens",
            "thinking tokens",
            "cache read tokens",
            "cache write tokens",
            "Message",
            "message.id",
            "requestId",
            "timestamp",
            "requestedModel",
            "stop_reason",
            "service_tier",
            "speed",
            "inference_geo",
            "5m cache writes",
            "1h cache writes",
            "web searches",
            "web fetches",
            "Environment",
            "gitBranch",
            "cwd",
            "version",
            "entrypoint",
        ],
    );
    // The stop reason is on the later line of the message.
    assert_eq!(value_in(&rows, "Turn", "effort"), "high");
    assert_eq!(value_in(&rows, "Turn", "thinking tokens"), "113");
    assert_eq!(value_in(&rows, "Turn", "cache read tokens"), "29.4k");
    assert_eq!(value_in(&rows, "Message", "stop_reason"), "tool_use");
    assert_eq!(value_in(&rows, "Environment", "gitBranch"), "main");
    assert_eq!(value_in(&rows, "Environment", "cwd"), CWD);
    assert!(!rows.iter().any(|r| r.contains("cost")), "{rows:#?}");
    assert!(!has(&rows, "unavailable"), "{rows:#?}");
    assert_eq!(c.rig.focus(), Some(Id::DetailsOverlay));
}

#[test]
fn a_tool_call_shows_its_start_then_the_models_rows_then_the_rest_of_its_result() {
    let mut c = claude(120, 40, &full(), true);
    let rows = c.details_of(2);
    assert_in_order(
        &rows,
        &[
            "Turn",
            "Tool call",
            "start",
            "duration",
            "status",
            "permissionDecision",
            "sandboxed",
            "isAsync",
            "Environment",
        ],
    );
    assert!(!has(&rows, "Message"), "{rows:#?}");
    // `start` is from the call's line, `duration` from the model.
    assert_eq!(
        value_in(&rows, "Tool call", "start"),
        "2026-10-21 08:35:19Z"
    );
    assert_eq!(value_in(&rows, "Tool call", "duration"), "2s");
    assert_eq!(value_in(&rows, "Tool call", "status"), "ok");
    assert_eq!(
        value_in(&rows, "Tool call", "permissionDecision"),
        "decision=accept, source=config"
    );
    assert_eq!(value_in(&rows, "Tool call", "sandboxed"), "false");
    assert_eq!(value_in(&rows, "Tool call", "isAsync"), "false");
}

#[test]
fn a_prompt_shows_the_timestamp_of_its_line_once() {
    let mut c = claude(120, 40, &full(), true);
    let rows = c.details_of(0);
    assert_in_order(
        &rows,
        &[
            "Prompt",
            "timestamp",
            "permissionMode",
            "promptSource",
            "Environment",
        ],
    );
    assert_eq!(
        rows.iter().filter(|r| r.starts_with("timestamp")).count(),
        1
    );
    assert!(!has(&rows, "Turn"), "{rows:#?}");
}

#[test]
fn a_call_without_a_result_yet_opens_with_no_unavailable_row() {
    let lines = vec![prompt_line(), text_line(), call_line()];
    let mut c = claude(120, 40, &lines, true);
    let rows = c.details_of(2);
    assert!(has(&rows, "Tool call"), "{rows:#?}");
    assert_eq!(
        value_in(&rows, "Tool call", "start"),
        "2026-10-21 08:35:19Z"
    );
    assert_eq!(value_in(&rows, "Tool call", "status"), "pending");
    assert!(!has(&rows, "permissionDecision"), "{rows:#?}");
    assert!(!has(&rows, "unavailable"), "{rows:#?}");
}

// -- a source that cannot be read

fn unavailable_in_each_group(rows: &[String]) {
    for group in ["Message", "Environment"] {
        let next = rows[at(rows, group)..]
            .iter()
            .position(|r| r.starts_with("unavailable"))
            .unwrap_or_else(|| panic!("no unavailable row after {group} in {rows:#?}"));
        assert!(
            next <= 2,
            "{group}'s unavailable row is not in its group: {rows:#?}"
        );
    }
    assert_eq!(
        rows.iter().filter(|r| r.starts_with("unavailable")).count(),
        2,
        "{rows:#?}"
    );
}

/// Every `unavailable` on the screen is drawn in `muted_foreground`.
fn unavailable_is_muted(rig: &mut Rig) {
    let buffer = screen_buffer(rig, 120, 40);
    let muted = theme("default").role(Role::MutedForeground);
    let mut seen = 0;
    for (y, row) in rows_of(&buffer).iter().enumerate() {
        if let Some(at) = row.find("unavailable") {
            let x = row[..at].chars().count();
            for dx in 0.."unavailable".len() {
                let cell = &buffer[(u16::try_from(x + dx).unwrap(), u16::try_from(y).unwrap())];
                assert_eq!(cell.fg, muted, "column {} of row {y}", x + dx);
            }
            seen += 1;
        }
    }
    assert_eq!(seen, 2, "one per group");
}

#[test]
fn a_file_that_is_gone_leaves_an_unavailable_row_per_group_and_the_overlay_open() {
    let mut c = claude(120, 40, &full(), true);
    fs::remove_file(&c.path).unwrap();
    let rows = c.details_of(1);
    unavailable_in_each_group(&rows);
    unavailable_is_muted(&mut c.rig);
    assert!(
        has(&rows, "Turn") && has(&rows, "model"),
        "the model's rows stay"
    );
    assert!(open(&c.rig.app));
    assert_eq!(c.rig.focus(), Some(Id::DetailsOverlay));
    // It still closes and the transcript still answers.
    c.rig.close_details_by(Key::Esc);
    assert_eq!(c.rig.focus(), Some(Id::Transcript));
}

#[test]
fn a_range_past_the_end_of_the_file_leaves_an_unavailable_row_per_group() {
    let mut c = claude(120, 40, &full(), true);
    fs::write(&c.path, "{}\n").unwrap();
    let rows = c.details_of(1);
    unavailable_in_each_group(&rows);
    assert!(open(&c.rig.app));
}

#[test]
fn a_line_that_does_not_parse_leaves_an_unavailable_row_per_group() {
    let mut c = claude(120, 40, &full(), true);
    let text = fs::read_to_string(&c.path).unwrap().replace('{', "(");
    fs::write(&c.path, text).unwrap();
    let rows = c.details_of(1);
    unavailable_in_each_group(&rows);
    assert!(open(&c.rig.app));
}

#[test]
fn a_prompt_whose_source_is_gone_keeps_the_models_timestamp_then_unavailable() {
    let mut c = claude(120, 40, &full(), true);
    fs::remove_file(&c.path).unwrap();
    let rows = c.details_of(0);
    assert_in_order(
        &rows,
        &["Prompt", "timestamp", "unavailable", "Environment"],
    );
}

// -- a harness that provides nothing

#[test]
fn a_harness_that_provides_nothing_leaves_the_models_rows_and_no_unavailable_row() {
    // The fake harness inherits the default of `Harness::section_details`,
    // as the Codex and Pi adapters do.
    let mut c = claude(120, 40, &full(), false);
    let rows = c.details_of(1);
    assert_in_order(&rows, &["Turn", "model", "start"]);
    for absent in ["Message", "Environment", "unavailable", "message.id", "cwd"] {
        assert!(!has(&rows, absent), "{absent}: {rows:#?}");
    }
    assert!(open(&c.rig.app));
    // A tool call: the model's rows only.
    c.rig.close_details_by(Key::Esc);
    let rows = c.details_of(1);
    assert_in_order(&rows, &["Tool call", "duration", "status"]);
    assert!(!has(&rows, "unavailable"), "{rows:#?}");
    let tool = &rows[at(&rows, "Tool call")..];
    assert!(!tool.iter().any(|r| r.starts_with("start")), "{rows:#?}");
}

// -- screens

fn screen(width: u16, height: u16, down: usize, remove_file: bool) -> String {
    let mut c = claude(width, height, &full(), true);
    if remove_file {
        fs::remove_file(&c.path).unwrap();
    }
    c.rig.down(down);
    c.rig.settle();
    c.rig.open_details();
    screen_text(&mut c.rig, width, height)
}

#[test]
fn the_details_of_a_claude_tool_call_80x24_text() {
    insta::assert_snapshot!(screen(80, 24, 2, false));
}

#[test]
fn the_details_of_a_claude_tool_call_120x40_text() {
    insta::assert_snapshot!(screen(120, 40, 2, false));
}

#[test]
fn the_details_of_a_section_whose_source_is_gone_80x24_text() {
    insta::assert_snapshot!(screen(80, 24, 1, true));
}

#[test]
fn the_details_of_a_section_whose_source_is_gone_120x40_text() {
    insta::assert_snapshot!(screen(120, 40, 1, true));
}
