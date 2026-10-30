//! Snapshots of every UI state of the turn block (task 017).

use serde_json::json;

use super::fixtures::*;
use super::*;
use crate::model::{Thinking, ToolStatus};
use crate::test_support::{buffer_to_styled, buffer_to_text};

fn prompt_id(turn: &crate::model::Turn) -> SectionId {
    key_id(turn, SectionKey::Prompt)
}

#[test]
fn user_prompt_folded_at_80() {
    let turn = user(LONG_PROMPT, vec![]);
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 80)));
}

#[test]
fn user_prompt_folded_at_120() {
    let turn = user(LONG_PROMPT, vec![]);
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 120)));
}

#[test]
fn user_prompt_unfolded_at_80() {
    let turn = user(LONG_PROMPT, vec![]);
    let opts = Opts::default().unfold(prompt_id(&turn));
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &opts, 80)));
}

#[test]
fn user_prompt_unfolded_and_selected_at_80_styled() {
    let turn = user(LONG_PROMPT, vec![]);
    let id = prompt_id(&turn);
    let opts = Opts::default().unfold(id.clone()).select(id);
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &opts, 80)));
}

#[test]
fn injected_item_before_the_prompt_at_80() {
    let turn = user(
        "Please look at the config.",
        vec![injected(
            "i1",
            &[(
                "file",
                "/home/jacob/.config/chronoscope/config.toml\nshow_system = true",
            )],
        )],
    );
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 80)));
}

fn full_opts() -> (crate::model::Turn, Opts) {
    let turn = full_assistant();
    let opts = Opts::default()
        .unfold(item_id(&turn, "c2"))
        .select(item_id(&turn, "c2"));
    (turn, opts)
}

#[test]
fn assistant_turn_at_120_text() {
    let (turn, opts) = full_opts();
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &opts, 120)));
}

#[test]
fn assistant_turn_at_120_selected_tool_dimmed_styled() {
    let (turn, opts) = full_opts();
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &opts, 120)));
}

#[test]
fn assistant_turn_at_120_selected_tool_not_dimmed_styled() {
    let (turn, opts) = full_opts();
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &opts.dim(false), 120)));
}

#[test]
fn assistant_turn_with_unfolded_thinking_at_80() {
    let turn = assistant(
        Some("high"),
        vec![
            text("t1", "Let me think first."),
            thinking(
                "th1",
                Thinking::Text(
                    "The camera is a **UVC** device.\n\n- extension units\n- processing unit controls\n\nI should check `lsusb -v` before anything else."
                        .into(),
                ),
            ),
        ],
        None,
    );
    let opts = Opts::default().unfold(item_id(&turn, "th1"));
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &opts, 80)));
}

#[test]
fn assistant_turn_with_unfolded_system_section_at_80() {
    let turn = assistant(
        Some("high"),
        vec![
            injected(
                "i1",
                &[
                    ("hook_system_message", "SessionStart hook says hello"),
                    ("file", "/home/jacob/notes.md\nline two of the file"),
                    ("local_command", "ls -la"),
                ],
            ),
            text("t1", "Done."),
        ],
        None,
    );
    let opts = Opts::default().unfold(item_id(&turn, "i1"));
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &opts, 80)));
}

#[test]
fn error_tool_call_unfolded_at_80_styled() {
    let turn = assistant(
        Some("high"),
        vec![tool(
            "c1",
            "Run the tests",
            json!({"command": "cargo test"}),
            Some((
                "error[E0432]: unresolved import `foo`\n --> src/main.rs:1:5",
                true,
            )),
            ToolStatus::Error,
        )],
        None,
    );
    let opts = Opts::default().unfold(item_id(&turn, "c1")).dim(false);
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &opts, 80)));
}

#[test]
fn pending_tool_call_unfolded_at_80_styled() {
    let turn = assistant(
        Some("high"),
        vec![tool(
            "c1",
            "Run the tests",
            json!({"command": "cargo test"}),
            None,
            ToolStatus::Pending,
        )],
        None,
    );
    let opts = Opts::default().unfold(item_id(&turn, "c1")).dim(false);
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &opts, 80)));
}

const SUMMARY: &str = "The user is investigating the **Logitech BRIO** camera.\n\n- UVC descriptors were dumped\n- No root access yet";

#[test]
fn compaction_folded_at_80() {
    let turn = compaction(SUMMARY);
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 80)));
}

#[test]
fn compaction_unfolded_at_80() {
    let turn = compaction(SUMMARY);
    let opts = Opts::default().unfold(key_id(&turn, SectionKey::Summary));
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &opts, 80)));
}

#[test]
fn redacted_thinking_only_is_an_empty_block_at_80() {
    let turn = assistant(
        Some("high"),
        vec![thinking("th1", Thinking::Redacted)],
        None,
    );
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 80)));
}

#[test]
fn assistant_title_without_effort_at_80() {
    let turn = assistant(None, vec![text("t1", "Hello.")], None);
    let mut turn = turn;
    if let crate::model::Turn::Assistant(a) = &mut turn {
        a.model = "claude-sonnet-5.5".into();
    }
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 80)));
}
