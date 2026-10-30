//! A section as plain text, for the clipboard (and, later, save and export).
//!
//! [`displayed_text`] is the section as the block shows it, [`full_text`] the
//! whole of it whatever the fold state and truncation. Both are pure: they
//! need the turn, the section's id and the few settings that decide how it is
//! drawn.
//!
//! The text is made from the same renderers as the screen, laid out at a
//! width no line reaches ([`PLAIN_WIDTH`]), so that the only line breaks are
//! the logical ones. What is dropped is the chrome of the block: borders, the
//! fold marker, the selection marker and the column indents. What stays is
//! the content's own shape: code indents, list bullets, the `│ ` quote
//! prefix, labels (`Command:`, `Result:`) and the blank rows between parts.
//! Lines have no trailing spaces and the text no trailing newline.

use ratatui::text::Line;

use super::renderers::{SectionCtx, SystemSection, ToolCallSection, render_section};
use super::section::{FoldState, Section, SectionContent, SectionId, derive_sections};
use crate::model::Turn;
use crate::theme::Theme;

/// The width sections are rendered at for plain text: wide enough that
/// nothing wraps.
pub const PLAIN_WIDTH: u16 = u16::MAX;

/// A horizontal rule fills the width it is rendered at; in plain text it is
/// this many cells.
const RULE_CELLS: usize = 80;

/// The section `id` of `turn` as displayed: the folded summary when it is
/// folded, everything shown when it is not (a truncated output keeps its
/// `… N more lines` row). The truncation row's id stands for its tool call.
/// `None` when the turn has no such section.
pub fn displayed_text(
    turn: &Turn,
    id: &SectionId,
    folds: &FoldState,
    theme: &Theme,
    truncate: usize,
    open_key: &str,
) -> Option<String> {
    let section = find(turn, id)?;
    let ctx = SectionCtx {
        theme,
        width: PLAIN_WIDTH,
        folded: folds.is_folded(&section),
        truncate,
        expanded: folds.is_expanded(&section.id),
        open_key,
    };
    Some(plain_lines(&section, &ctx, true).join("\n"))
}

/// The section `id` of `turn` in full, whatever its fold state and
/// truncation: for a tool call the description, the full input or command and
/// the full result (read from the transcript when it is not held in memory),
/// with the labels of the unfolded body; for the other sections, their
/// unfolded text. `None` when the turn has no such section.
pub fn full_text(turn: &Turn, id: &SectionId, theme: &Theme, open_key: &str) -> Option<String> {
    let section = find(turn, id)?;
    let ctx = SectionCtx {
        theme,
        width: PLAIN_WIDTH,
        folded: false,
        truncate: usize::MAX,
        expanded: true,
        open_key,
    };
    Some(plain_lines(&section, &ctx, false).join("\n"))
}

/// The section `id` stands for, in `turn`.
fn find<'a>(turn: &'a Turn, id: &SectionId) -> Option<Section<'a>> {
    let id = id.more_parent().unwrap_or_else(|| id.clone());
    derive_sections(turn, true)
        .into_iter()
        .find(|section| section.id == id)
}

fn plain_lines(section: &Section<'_>, ctx: &SectionCtx<'_>, with_hint: bool) -> Vec<String> {
    match section.content {
        SectionContent::Tool(call) => ToolCallSection(call).plain_lines(ctx, with_hint),
        SectionContent::System(entries) => SystemSection(entries).plain_lines(ctx),
        _ => render_section(section, ctx)
            .lines
            .into_iter()
            .map(without_lead)
            .collect(),
    }
}

/// The row without the two-cell lead (the fold marker or two spaces) the
/// renderers put in front of every non-blank row, without trailing spaces.
fn without_lead(line: Line<'static>) -> String {
    let text = line
        .spans
        .iter()
        .skip(1)
        .map(|span| span.content.as_ref())
        .collect::<String>();
    cap_rule(text.trim_end())
}

/// Cuts a row that is a horizontal rule (possibly after a quote bar) to
/// [`RULE_CELLS`].
fn cap_rule(text: &str) -> String {
    let Some(start) = text.find('─') else {
        return text.to_owned();
    };
    let (prefix, rule) = text.split_at(start);
    if rule.chars().all(|c| c == '─') && rule.chars().count() > RULE_CELLS {
        format!("{prefix}{}", "─".repeat(RULE_CELLS))
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::super::fixtures::*;
    use super::super::section::SectionKey;
    use super::*;
    use crate::model::{
        ByteSource, Item, ItemId, LineFormat, OutputSource, Thinking, ToolCall, ToolKind,
        ToolOutput, ToolResult, ToolStatus, TurnStats,
    };
    use crate::theme::Theme;

    const KEY: &str = "a";

    fn shown(turn: &Turn, id: &SectionId, folds: &FoldState, truncate: usize) -> String {
        displayed_text(turn, id, folds, &theme(), truncate, KEY).unwrap()
    }

    fn full(turn: &Turn, id: &SectionId) -> String {
        full_text(turn, id, &theme(), KEY).unwrap()
    }

    fn unfolded(turn: &Turn, item: &str) -> FoldState {
        let mut folds = FoldState::new();
        folds.set_folded(item_id(turn, item), false);
        folds
    }

    /// An assistant turn with the Shell call `c1`.
    fn shell_turn(command: &str, result: &str) -> Turn {
        assistant(
            None,
            vec![tool(
                "c1",
                "Print the log",
                json!({ "command": command }),
                Some((result, false)),
                ToolStatus::Ok,
            )],
            None,
        )
    }

    fn numbered(n: usize) -> String {
        (1..=n)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_folded_tool_call_is_exactly_its_description() {
        let turn = full_assistant();
        let id = item_id(&turn, "c1");
        assert_eq!(
            shown(&turn, &id, &FoldState::new(), 20),
            "Find the BRIO on USB and check available tools"
        );
    }

    #[test]
    fn an_unfolded_shell_call_has_labels_command_and_result_without_chrome() {
        let turn = shell_turn("ls -la", "a\nb");
        let id = item_id(&turn, "c1");
        assert_eq!(
            shown(&turn, &id, &unfolded(&turn, "c1"), 20),
            "Print the log\n\nCommand:\n\nls -la\n\nResult:\n\na\nb"
        );
    }

    #[test]
    fn a_long_command_is_one_logical_line() {
        let command = format!("echo {}", "x".repeat(500));
        let turn = shell_turn(&command, "ok");
        let id = item_id(&turn, "c1");
        let text = shown(&turn, &id, &unfolded(&turn, "c1"), 20);
        assert_eq!(
            text,
            format!("Print the log\n\nCommand:\n\n{command}\n\nResult:\n\nok")
        );
    }

    #[test]
    fn a_truncated_output_keeps_its_more_lines_row() {
        let turn = shell_turn("cat log", &numbered(10));
        let id = item_id(&turn, "c1");
        assert_eq!(
            shown(&turn, &id, &unfolded(&turn, "c1"), 3),
            "Print the log\n\nCommand:\n\ncat log\n\nResult:\n\nline 1\nline 2\nline 3\n… 7 more lines"
        );
    }

    #[test]
    fn an_expanded_output_has_no_more_lines_row() {
        let turn = shell_turn("cat log", &numbered(10));
        let id = item_id(&turn, "c1");
        let mut folds = unfolded(&turn, "c1");
        folds.set_expanded(id.clone());
        let text = shown(&turn, &id, &folds, 3);
        assert!(text.ends_with("line 9\nline 10"), "{text}");
        assert!(!text.contains("more lines"));
    }

    #[test]
    fn the_truncation_row_stands_for_its_tool_call() {
        let turn = shell_turn("cat log", &numbered(10));
        let id = item_id(&turn, "c1");
        let folds = unfolded(&turn, "c1");
        let more = id.more_row().unwrap();
        assert_eq!(shown(&turn, &more, &folds, 3), shown(&turn, &id, &folds, 3));
        assert_eq!(full(&turn, &more), full(&turn, &id));
    }

    #[test]
    fn an_unknown_section_has_no_text() {
        let turn = shell_turn("ls", "a");
        let other = item_id(&turn, "nope");
        assert_eq!(
            displayed_text(&turn, &other, &FoldState::new(), &theme(), 20, KEY),
            None
        );
        assert_eq!(full_text(&turn, &other, &theme(), KEY), None);
    }

    #[test]
    fn a_folded_user_prompt_is_two_lines_and_an_ellipsis() {
        let turn = user("first line\n\nsecond line\n\nthird line\n\nfourth", vec![]);
        let id = key_id(&turn, SectionKey::Prompt);
        assert_eq!(
            shown(&turn, &id, &FoldState::new(), 20),
            "first line\nsecond line…"
        );
    }

    #[test]
    fn a_short_folded_user_prompt_has_no_ellipsis() {
        let turn = user("first line\n\nsecond line", vec![]);
        let id = key_id(&turn, SectionKey::Prompt);
        assert_eq!(
            shown(&turn, &id, &FoldState::new(), 20),
            "first line\nsecond line"
        );
    }

    #[test]
    fn an_unfolded_user_prompt_is_all_of_it() {
        let turn = user("first line\n\nsecond line\n\nthird line", vec![]);
        let id = key_id(&turn, SectionKey::Prompt);
        let mut folds = FoldState::new();
        folds.set_folded(id.clone(), false);
        assert_eq!(
            shown(&turn, &id, &folds, 20),
            "first line\n\nsecond line\n\nthird line"
        );
        assert_eq!(
            full(&turn, &id),
            "first line\n\nsecond line\n\nthird line",
            "Y ignores the fold"
        );
    }

    #[test]
    fn a_long_paragraph_is_not_broken_at_the_screen_width() {
        let turn = user(LONG_PROMPT, vec![]);
        let id = key_id(&turn, SectionKey::Prompt);
        assert_eq!(full(&turn, &id), LONG_PROMPT);
    }

    #[test]
    fn markdown_keeps_its_own_indentation_and_markers() {
        let turn = assistant(
            None,
            vec![text(
                "t1",
                "Steps:\n\n- one\n- two\n\n> quoted\n\n```\nfn main() {\n    run();\n}\n```",
            )],
            None,
        );
        let id = item_id(&turn, "t1");
        assert_eq!(
            shown(&turn, &id, &FoldState::new(), 20),
            "Steps:\n\n• one\n• two\n\n│ quoted\n\n  fn main() {\n      run();\n  }"
        );
    }

    #[test]
    fn a_horizontal_rule_is_not_as_wide_as_the_render_width() {
        let turn = assistant(None, vec![text("t1", "above\n\n---\n\nbelow")], None);
        let id = item_id(&turn, "t1");
        assert_eq!(
            shown(&turn, &id, &FoldState::new(), 20),
            format!("above\n\n{}\n\nbelow", "─".repeat(RULE_CELLS))
        );
    }

    #[test]
    fn folded_and_unfolded_thinking() {
        let turn = assistant(
            None,
            vec![thinking(
                "th1",
                Thinking::Text("**Plan**\n\nLook at lsusb.".into()),
            )],
            None,
        );
        let id = item_id(&turn, "th1");
        assert_eq!(shown(&turn, &id, &FoldState::new(), 20), "Thinking: Plan");
        assert_eq!(
            shown(&turn, &id, &unfolded(&turn, "th1"), 20),
            "Thinking\nPlan\n\nLook at lsusb."
        );
        assert_eq!(full(&turn, &id), "Thinking\nPlan\n\nLook at lsusb.");
    }

    #[test]
    fn compaction_folded_is_the_divider_and_unfolded_has_the_summary() {
        let turn = compaction("Summary text.");
        let id = key_id(&turn, SectionKey::Summary);
        assert_eq!(
            shown(&turn, &id, &FoldState::new(), 20),
            "── context compacted ──"
        );
        assert_eq!(full(&turn, &id), "── context compacted ──\nSummary text.");
    }

    #[test]
    fn system_sections_have_no_column_indent() {
        let turn = user(
            "hi",
            vec![injected("i1", &[("file", "/notes.md\nUse the GUIDs.")])],
        );
        let id = item_id(&turn, "i1");
        assert_eq!(shown(&turn, &id, &FoldState::new(), 20), "file: /notes.md");
        assert_eq!(
            full(&turn, &id),
            "file: /notes.md\nfile\n/notes.md\nUse the GUIDs."
        );
    }

    #[test]
    fn the_stats_footer_and_a_model_change_are_their_text() {
        let turn = assistant(
            None,
            vec![Item::ModelChange {
                id: ItemId::from("m1"),
                model: "claude-sonnet-5.5".to_owned(),
            }],
            Some(TurnStats { ..stats() }),
        );
        assert_eq!(
            shown(&turn, &item_id(&turn, "m1"), &FoldState::new(), 20),
            "model → claude-sonnet-5.5"
        );
        assert_eq!(
            shown(
                &turn,
                &key_id(&turn, SectionKey::Footer),
                &FoldState::new(),
                20
            ),
            "5m 02s · 112 messages · 45.2k tokens"
        );
    }

    #[test]
    fn full_content_of_a_folded_truncated_call_has_the_whole_output() {
        let result = numbered(500);
        let turn = shell_turn("cat log", &result);
        let id = item_id(&turn, "c1");
        // Folded, and truncated were it unfolded.
        assert_eq!(
            full(&turn, &id),
            format!("Print the log\n\nCommand:\n\ncat log\n\nResult:\n\n{result}")
        );
    }

    #[test]
    fn full_content_of_a_lazy_output_over_4_kib_is_read_from_the_transcript() {
        let result = numbered(1_000);
        assert!(result.len() > crate::model::INLINE_OUTPUT_LIMIT);
        let line = format!(
            r#"{{"message":{{"content":[{{"type":"tool_result","tool_use_id":"c1","content":{}}}]}}}}"#,
            serde_json::to_string(&result).unwrap()
        );
        let source = OutputSource {
            bytes: ByteSource::Memory(Arc::from(line.as_bytes())),
            range: 0..line.len() as u64,
            format: LineFormat::Claude,
            call_id: "c1".to_owned(),
        };
        let output = ToolOutput::lazy(1_000, result.len(), source);
        assert!(output.is_lazy());
        let turn = assistant(
            None,
            vec![Item::ToolCall(ToolCall {
                id: ItemId::from("c1"),
                tool: ToolKind::Shell,
                description: "Print the log".to_owned(),
                input: json!({"command": "cat log"}),
                result: Some(ToolResult {
                    output,
                    is_error: false,
                }),
                status: ToolStatus::Ok,
                duration: None,
            })],
            None,
        );
        let id = item_id(&turn, "c1");
        assert_eq!(
            full(&turn, &id),
            format!("Print the log\n\nCommand:\n\ncat log\n\nResult:\n\n{result}")
        );
        // As displayed it is cut like any other.
        let text = shown(&turn, &id, &unfolded(&turn, "c1"), 3);
        assert!(text.ends_with("line 3\n… 997 more lines"), "{text}");
    }

    #[test]
    fn full_content_of_a_folded_call_of_another_kind_uses_its_labels() {
        let turn = assistant(
            None,
            vec![tool_of(
                ToolKind::Other("mcp__x__y".into()),
                "c1",
                "Ask the server",
                json!({"n": 3}),
                Some(("answer", false)),
                ToolStatus::Ok,
            )],
            None,
        );
        assert_eq!(
            full(&turn, &item_id(&turn, "c1")),
            "Ask the server\n\nInput:\n\n{\n  \"n\": 3\n}\n\nResult:\n\nanswer"
        );
    }

    #[test]
    fn a_pending_call_shows_pending() {
        let turn = assistant(
            None,
            vec![tool(
                "c1",
                "Run it",
                json!({"command": "sleep 9"}),
                None,
                ToolStatus::Pending,
            )],
            None,
        );
        assert_eq!(
            full(&turn, &item_id(&turn, "c1")),
            "Run it\n\nCommand:\n\nsleep 9\n\nResult:\n\n(pending)"
        );
    }

    #[test]
    fn the_theme_does_not_change_the_text() {
        let turn = full_assistant();
        let id = item_id(&turn, "c2");
        let other: Theme = distinct_theme();
        assert_eq!(
            full_text(&turn, &id, &other, KEY),
            full_text(&turn, &id, &theme(), KEY)
        );
    }
}
