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

// ---- Task 019: per-tool bodies ----

use crate::model::ToolKind;

const LSUSB: &str = "/tmp/claude-1000/-home-jacob-Development-projects-brio/85a86657-11a7-4318-a21a-0f4476babd5c/scratchpad/lsusb.txt";

/// A turn holding one finished tool call of `kind`.
fn tool_turn(
    kind: ToolKind,
    description: &str,
    input: serde_json::Value,
    result: Option<&str>,
) -> crate::model::Turn {
    let status = if result.is_some() {
        ToolStatus::Ok
    } else {
        ToolStatus::Pending
    };
    assistant(
        Some("high"),
        vec![tool_of(
            kind,
            "c1",
            description,
            input,
            result.map(|text| (text, false)),
            status,
        )],
        None,
    )
}

fn unfolded(turn: &crate::model::Turn) -> Opts {
    Opts::default().unfold(item_id(turn, "c1")).dim(false)
}

fn shell_turn() -> crate::model::Turn {
    let command = format!(
        "sudo -n true 2>&1 && echo SUDO_OK; lsusb -v -d 046d:085e > {LSUSB} 2>&1; wc -l {LSUSB}; grep -E 'bInterfaceClass|bInterfaceSubClass|bInterfaceNumber|iInterface|bcdUVC|bcdDevice|iProduct|iSerial|guidExtensionCode' {LSUSB} | head -200"
    );
    let result = format!(
        "sudo: a password is required\n1799\n{LSUSB}\nCouldn't open device, some information will be missing\n  bcdDevice            0.17\n  iProduct                2 Logitech BRIO\n  iSerial                 3 8DF1E759\n      bInterfaceNumber        0\n      bInterfaceClass        14 Video"
    );
    tool_turn(
        ToolKind::Shell,
        "Dump full USB descriptors for the BRIO",
        json!({ "command": command }),
        Some(&result),
    )
}

#[test]
fn shell_unfolded_with_a_long_command_at_100_text() {
    let turn = shell_turn();
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn shell_unfolded_with_a_long_command_at_100_styled() {
    let turn = shell_turn();
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn read_unfolded_with_numbered_content_at_100_text() {
    let turn = tool_turn(
        ToolKind::Read,
        "Read the camera module",
        json!({"file_path": "/home/jacob/projects/brio/src/camera.rs", "offset": 8, "limit": 4}),
        Some(
            "     8→use std::fs::File;\n     9→\n    10→pub fn open(path: &str) -> File {\n    11→    File::open(path).expect(\"camera device\")\n",
        ),
    );
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn read_unfolded_with_numbered_content_at_100_styled() {
    let turn = tool_turn(
        ToolKind::Read,
        "Read the camera module",
        json!({"file_path": "/home/jacob/projects/brio/src/camera.rs"}),
        Some("     1→use std::fs::File;\n     2→\n     3→pub fn open() {}\n"),
    );
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn write_unfolded_with_rust_content_at_100_styled() {
    let turn = tool_turn(
        ToolKind::Write,
        "Create the entry point",
        json!({"file_path": "/home/jacob/projects/brio/src/main.rs", "content": "fn main() {\n    let name = \"brio\";\n    println!(\"hello {name}\"); // greet\n}\n"}),
        Some("File created successfully"),
    );
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &unfolded(&turn), 100)));
}

fn edit_turn() -> crate::model::Turn {
    tool_turn(
        ToolKind::Edit,
        "Greet by name",
        json!({
            "file_path": "/home/jacob/projects/brio/src/main.rs",
            "old_string": "fn main() {\n    let name = \"brio\";\n    println!(\"hello\");\n}\n",
            "new_string": "fn main() {\n    let name = \"brio\";\n    println!(\"hello {name}\");\n    println!(\"bye\");\n}\n",
        }),
        Some("The file has been updated."),
    )
}

#[test]
fn edit_unfolded_with_a_diff_at_100_text() {
    let turn = edit_turn();
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn edit_unfolded_with_a_diff_at_100_styled() {
    let turn = edit_turn();
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn grep_unfolded_at_100_text() {
    let turn = tool_turn(
        ToolKind::Grep,
        "Find the UVC extension unit code",
        json!({"pattern": "guidExtensionCode", "path": "/home/jacob/projects/brio", "glob": "*.txt"}),
        Some(
            "lsusb.txt:412:guidExtensionCode {2c8a1d9a-0b52-4a6e-a1e0-5a9b3f6d7c10}\nlsusb.txt:530:guidExtensionCode {8b3c7e11-4f90-4b0f-9a5d-1e2f3a4b5c6d}",
        ),
    );
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn glob_unfolded_at_100_text() {
    let turn = tool_turn(
        ToolKind::Glob,
        "List the Rust sources",
        json!({"pattern": "src/**/*.rs", "path": "/home/jacob/projects/brio"}),
        Some("src/main.rs\nsrc/camera.rs\nsrc/uvc/mod.rs"),
    );
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn web_fetch_unfolded_at_100_text() {
    let turn = tool_turn(
        ToolKind::WebFetch,
        "Read the UVC spec",
        json!({"url": "https://www.usb.org/document-library/video-class-v15-document-set", "prompt": "Which control selectors does the extension unit define?"}),
        Some("The extension unit defines vendor-specific control selectors; see section 3.7.2.6."),
    );
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn web_search_unfolded_at_100_text() {
    let turn = tool_turn(
        ToolKind::WebSearch,
        "Search for BRIO extension units",
        json!({"query": "logitech brio uvc extension unit guid"}),
        Some("1. Reverse engineering Logitech UVC extension units\n2. libwebcam: uvcdynctrl"),
    );
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn other_mcp_tool_unfolded_with_pretty_json_at_100_text() {
    let turn = tool_turn(
        ToolKind::Other("mcp__github__get_issue".into()),
        "Fetch the tracking issue",
        json!({"owner": "jacob", "repo": "brio", "number": 12, "labels": ["uvc", "camera"]}),
        Some("Title: Expose the extension unit controls\nState: open"),
    );
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 100)));
}

fn subagent_turn() -> crate::model::Turn {
    tool_turn(
        ToolKind::Subagent,
        "Explore the UVC descriptors",
        json!({
            "subagent_type": "Explore",
            "description": "Explore the UVC descriptors",
            "prompt": "Look through `lsusb.txt` and list:\n\n- every extension unit\n- its GUID",
        }),
        Some("Found **two** extension units:\n\n1. exposure\n2. pan and tilt"),
    )
}

#[test]
fn subagent_folded_at_100_text() {
    let turn = subagent_turn();
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 100)));
}

#[test]
fn subagent_unfolded_at_100_text() {
    let turn = subagent_turn();
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn failed_shell_call_at_100_styled() {
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
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn failed_edit_shows_its_error_below_the_diff_at_100_styled() {
    let turn = assistant(
        Some("high"),
        vec![tool_of(
            ToolKind::Edit,
            "c1",
            "Greet by name",
            json!({
                "file_path": "/home/jacob/projects/brio/src/main.rs",
                "old_string": "println!(\"hello\");\n",
                "new_string": "println!(\"hello {name}\");\n",
            }),
            Some(("String to replace not found in file.", true)),
            ToolStatus::Error,
        )],
        None,
    );
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &unfolded(&turn), 100)));
}

#[test]
fn failed_edit_with_a_truncated_diff_shows_its_error_first_at_100_text() {
    let old: String = (1..=10).map(|n| format!("let step_{n} = {n};\n")).collect();
    let turn = assistant(
        Some("high"),
        vec![tool_of(
            ToolKind::Edit,
            "c1",
            "Drop the setup lines",
            json!({
                "file_path": "/home/jacob/projects/brio/src/main.rs",
                "old_string": old,
                "new_string": "",
            }),
            Some(("String to replace not found in file.", true)),
            ToolStatus::Error,
        )],
        None,
    );
    let opts = unfolded(&turn).truncate(5);
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &opts, 100)));
}
