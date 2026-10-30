use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::components::transcript::fixtures::with_id;
use crate::components::turn_block::fixtures::*;
use crate::components::turn_block::{FoldState, SectionId, SectionKey};
use crate::model::{
    ByteSource, Item, ItemId, LineFormat, OutputSource, ToolCall, ToolKind, ToolOutput, ToolResult,
    ToolStatus, Turn,
};

const ROWS: usize = 120;

fn meta() -> ExportMeta {
    ExportMeta {
        title: "Investigate the camera profiles".to_owned(),
        project: "brio".to_owned(),
        session_id: "85a86657-11a7-4318-a21a-0f4476babd5c".to_owned(),
        cwd: "/home/user/projects/brio".to_owned(),
        started: ts("2026-10-21T08:35:15Z"),
        harness: "Claude".to_owned(),
    }
}

/// A session with every kind of turn: a prompt with a system item, the
/// assistant turn of the full snapshot (text, folded thinking, a Shell call
/// folded and one unfolded with a three-line result, a model change and the
/// stats), a short prompt, an assistant turn with a failing Read call whose
/// result is lazy, and a compaction.
/// A row of the lazy output, long enough for 120 of them to be lazy.
fn row(n: usize) -> String {
    format!("row {n} {}", "-".repeat(40))
}

fn session() -> Vec<Arc<Turn>> {
    let lazy = (1..=ROWS).map(row).collect::<Vec<_>>().join("\n");
    assert!(lazy.len() > crate::model::INLINE_OUTPUT_LIMIT);
    let line = format!(
        r#"{{"message":{{"content":[{{"type":"tool_result","tool_use_id":"r1","content":{}}}]}}}}"#,
        serde_json::to_string(&lazy).unwrap()
    );
    let output = ToolOutput::lazy(
        ROWS,
        lazy.len(),
        OutputSource {
            bytes: ByteSource::Memory(Arc::from(line.as_bytes())),
            range: 0..line.len() as u64,
            format: LineFormat::Claude,
            call_id: "r1".to_owned(),
        },
    );
    assert!(output.is_lazy());
    let read = Item::ToolCall(ToolCall {
        id: ItemId::from("r1"),
        tool: ToolKind::Read,
        description: "Read the <dump> & friends".to_owned(),
        input: json!({"file_path": "/tmp/lsusb.txt", "limit": 120}),
        result: Some(ToolResult {
            output,
            is_error: true,
        }),
        status: ToolStatus::Error,
        duration: None,
    });
    let fenced_output = tool(
        "c3",
        "Show a markdown file",
        json!({"command": "cat README.md"}),
        Some(("Use ```` ```sh ```` blocks.\n```\ncode\n```", false)),
        ToolStatus::Ok,
    );
    vec![
        Arc::new(with_id(
            user(
                LONG_PROMPT,
                vec![injected(
                    "i1",
                    &[("file", "/home/jacob/notes.md\nUse the GUIDs.")],
                )],
            ),
            "u1",
        )),
        Arc::new(with_id(full_assistant(), "a1")),
        Arc::new(with_id(
            user(
                "Short follow-up.\n\nWith a second paragraph.\n\nThird.",
                vec![],
            ),
            "u2",
        )),
        Arc::new(with_id(
            assistant(None, vec![read, fenced_output], None),
            "a2",
        )),
        Arc::new(with_id(
            compaction("The user is investigating a camera.\n\n- UVC dumped"),
            "c1",
        )),
    ]
}

fn export(turns: &[Arc<Turn>], folds: &FoldState, show_system: bool, full: bool) -> String {
    let theme = theme();
    let formatter = formatter();
    let settings = ExportSettings {
        theme: &theme,
        formatter: &formatter,
        folds,
        truncate: 2,
        open_key: "a",
        show_system,
        block_width: Some(80),
    };
    export_markdown(Some(&meta()), turns, &settings, full)
}

fn id(turn: &str, key: SectionKey) -> SectionId {
    SectionId {
        turn: crate::model::TurnId::from(turn),
        key,
    }
}

fn item(turn: &str, item: &str) -> SectionId {
    id(turn, SectionKey::Item(ItemId::from(item)))
}

/// The folds of the folded golden: the second prompt, the second tool call
/// of the assistant turn and the compaction are unfolded.
fn some_unfolded() -> FoldState {
    let mut folds = FoldState::new();
    folds.set_folded(id("u2", SectionKey::Prompt), false);
    folds.set_folded(item("a1", "c2"), false);
    folds.set_folded(id("c1", SectionKey::Summary), false);
    folds
}

#[test]
fn the_folded_export_is_a_golden_file() {
    let text = export(&session(), &some_unfolded(), false, false);
    insta::assert_snapshot!(text);
}

#[test]
fn the_folded_export_has_summaries_for_folded_and_content_for_unfolded_sections() {
    let text = export(&session(), &some_unfolded(), false, false);
    // Folded: the summary as a block quote.
    assert!(
        text.contains("> Find the BRIO on USB and check available tools\n"),
        "{text}"
    );
    assert!(text.contains("> Thinking: "), "{text}");
    assert!(text.contains("The user is investigating a camera.\n\n- UVC dumped"));
    // Unfolded: the prompt in full, the tool call with fenced bodies.
    assert!(text.contains("Short follow-up.\n\nWith a second paragraph.\n\nThird."));
    assert!(text.contains("**Dump full USB descriptors for the BRIO**"));
    assert!(text.contains("**Command:**\n\n```sh\n"));
    assert!(text.contains("**Result:**\n\n```\nsudo: a password is required"));
    // Not folded at all: the assistant's text.
    assert!(text.contains("- Check `lsusb -v` for the UVC descriptors"));
    // No details blocks in the folded export.
    assert!(!text.contains("<details>"));
}

#[test]
fn hidden_system_content_is_omitted_from_the_folded_export() {
    let hidden = export(&session(), &some_unfolded(), false, false);
    assert!(!hidden.contains("notes.md"), "{hidden}");
    assert!(!hidden.contains("injected"), "{hidden}");
    let shown = export(&session(), &some_unfolded(), true, false);
    assert!(shown.contains("> file: /home/jacob/notes.md"), "{shown}");
}

#[test]
fn a_truncated_output_of_the_folded_export_keeps_its_more_lines_row() {
    let mut folds = FoldState::new();
    folds.set_folded(item("a2", "r1"), false);
    let text = export(&session(), &folds, false, false);
    assert!(
        text.contains(&format!(
            "{}\n{}\n```\n\n_… 118 more lines_",
            row(1),
            row(2)
        )),
        "{text}"
    );
    assert!(!text.contains(&row(3)), "{text}");
    folds.set_expanded(item("a2", "r1"));
    let text = export(&session(), &folds, false, false);
    assert!(text.contains(&format!("{}\n```", row(ROWS))), "{text}");
    assert!(!text.contains("more lines"), "{text}");
}

#[test]
fn the_full_export_is_a_golden_file() {
    let text = export(&session(), &FoldState::new(), false, true);
    insta::assert_snapshot!(text);
}

#[test]
fn the_full_export_has_a_details_block_per_tool_call_with_fenced_input_and_output() {
    let text = export(&session(), &FoldState::new(), false, true);
    assert_eq!(text.matches("<details>").count(), 4, "{text}");
    assert_eq!(text.matches("</details>").count(), 4);
    assert!(text.contains(
        "<details><summary>Find the BRIO on USB and check available tools</summary>\n\n```sh\nlsusb | grep -i brio\n```\n\n```\nBus 001 Device 004: ID 046d:085e Logitech, Inc. BRIO\n```\n\n</details>"
    ), "{text}");
    // Another kind of tool: its input as JSON, the description escaped.
    assert!(text.contains(
        "<details><summary>Read the &lt;dump&gt; &amp; friends</summary>\n\n```json\n{\n  \"file_path\": \"/tmp/lsusb.txt\",\n  \"limit\": 120\n}\n```"
    ), "{text}");
    // The output of the failing call follows its input directly.
    assert!(
        text.contains(&format!("\n}}\n```\n\n```\n{}\n", row(1))),
        "{text}"
    );
}

#[test]
fn the_full_export_has_untruncated_outputs_including_lazy_ones() {
    let text = export(&session(), &FoldState::new(), false, true);
    // Truncation was 2 lines; the lazy output (120 rows) is all there.
    for n in 1..=ROWS {
        assert!(text.contains(&format!("\n{}\n", row(n))), "row {n}");
    }
    assert!(!text.contains("more lines"), "{text}");
}

#[test]
fn the_full_export_includes_system_content_whatever_the_toggle() {
    let text = export(&session(), &FoldState::new(), false, true);
    assert!(
        text.contains("**file**\n\n```\n/home/jacob/notes.md\nUse the GUIDs.\n```"),
        "{text}"
    );
}

#[test]
fn the_full_export_ignores_the_folds() {
    let all = export(&session(), &FoldState::new(), true, true);
    let some = export(&session(), &some_unfolded(), true, true);
    assert_eq!(all, some);
    // The whole first prompt and the whole thinking text, not their summaries.
    assert!(all.contains(LONG_PROMPT));
    assert!(all.contains("> The user wants the UVC extension units.\n> Start with lsusb."));
}

#[test]
fn headings_carry_the_role_the_model_the_effort_and_the_timestamp() {
    let text = export(&session(), &FoldState::new(), false, false);
    assert!(
        text.contains("\n## User — 2026-10-21 08:35:15Z\n"),
        "{text}"
    );
    assert!(
        text.contains("\n## Assistant (claude-opus-5.5 @ high) — 2026-10-21 08:35:18Z\n"),
        "{text}"
    );
    // No effort: no ` @ `.
    assert!(
        text.contains("\n## Assistant (claude-opus-5.5) — 2026-10-21 08:35:18Z\n"),
        "{text}"
    );
    assert!(
        text.contains("\n## Compaction — 2026-10-21 09:00:00Z\n"),
        "{text}"
    );
}

#[test]
fn the_document_starts_with_the_title_and_the_metadata() {
    let text = export(&session(), &FoldState::new(), false, false);
    assert!(
        text.starts_with(
            "# Investigate the camera profiles\n\n- Project: brio\n- Session: 85a86657-11a7-4318-a21a-0f4476babd5c\n- Working directory: /home/user/projects/brio\n- Started: 2026-10-21 08:35:15Z\n- Harness: Claude\n\n## User"
        ),
        "{text}"
    );
    assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
}

#[test]
fn without_metadata_the_document_starts_at_the_first_turn() {
    let theme = theme();
    let formatter = formatter();
    let folds = FoldState::new();
    let settings = ExportSettings {
        theme: &theme,
        formatter: &formatter,
        folds: &folds,
        truncate: 20,
        open_key: "a",
        show_system: true,
        block_width: None,
    };
    let text = export_markdown(None, &session()[4..], &settings, false);
    assert_eq!(
        text,
        "## Compaction — 2026-10-21 09:00:00Z\n\n> ── context compacted ──\n"
    );
}

// -- fences

#[test]
fn a_fence_is_three_backticks_when_the_content_has_none() {
    assert_eq!(fence_length("plain"), 3);
    assert_eq!(fence_length(""), 3);
    assert_eq!(fenced("sh", "ls"), "```sh\nls\n```");
}

#[test]
fn a_fence_is_longer_than_the_longest_run_of_backticks_in_its_content() {
    assert_eq!(fence_length("a ` b"), 3);
    assert_eq!(fence_length("a `` b"), 3);
    assert_eq!(fence_length("```"), 4);
    assert_eq!(fence_length("x ```` y ``` z"), 5);
    assert_eq!(fence_length("`````````"), 10);
    let content = "before\n````\ninner\n````\nafter";
    let block = fenced("", content);
    assert_eq!(block, format!("`````\n{content}\n`````"));
}

#[test]
fn a_fence_of_the_export_holds_the_backticks_of_the_output() {
    let text = export(&session(), &FoldState::new(), false, true);
    // The output holds a run of three and one of four backticks.
    assert!(
        text.contains("`````\nUse ```` ```sh ```` blocks.\n```\ncode\n```\n`````"),
        "{text}"
    );
}

#[test]
fn a_trailing_newline_of_the_content_is_not_doubled() {
    assert_eq!(fenced("", "a\nb\n"), "```\na\nb\n```");
}
