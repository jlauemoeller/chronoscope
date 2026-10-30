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

use super::renderers::{SectionCtx, SystemSection, ToolCallSection, render_section, text_width};
use super::section::{FoldState, Section, SectionContent, SectionId, derive_sections};
use crate::model::Turn;
use crate::theme::Theme;

/// The width sections are rendered at for plain text: wide enough that
/// nothing wraps.
const PLAIN_WIDTH: u16 = u16::MAX;

/// A horizontal rule fills the width it is rendered at; in plain text it is
/// this many cells.
const RULE_CELLS: usize = 80;

/// The section `id` of `turn` as displayed: the folded summary when it is
/// folded, everything shown when it is not (a truncated output keeps its
/// `… N more lines` row). The truncation row's id stands for its tool call.
/// `None` when the turn has no such section.
///
/// `block_width` is the width the turn's block is laid out at on screen, when
/// known. A folded summary is cut to the text width of that block (a folded
/// prompt keeps the two rows the screen shows, the second ending in `…`, with
/// rows that are soft-wrapped parts of one line joined with the whitespace the
/// source has there); without
/// it, summaries are cut at the logical lines. A folded tool call is always
/// its whole description. Unfolded sections are never cut at the width.
pub fn displayed_text(
    turn: &Turn,
    id: &SectionId,
    folds: &FoldState,
    theme: &Theme,
    truncate: usize,
    open_key: &str,
    block_width: Option<u16>,
) -> Option<String> {
    let section = find(turn, id)?;
    let folded = folds.is_folded(&section);
    let ctx = SectionCtx {
        theme,
        width: PLAIN_WIDTH,
        folded,
        truncate,
        expanded: folds.is_expanded(&section.id),
        open_key,
        verbatim: false,
    };
    let Some(block_width) = block_width.filter(|_| folded) else {
        return Some(plain_lines(&section, &ctx, true).join("\n"));
    };
    let narrow = SectionCtx {
        width: text_width(block_width),
        ..ctx
    };
    let lines = match section.content {
        SectionContent::Prompt(_) => {
            let rows = plain_lines(&section, &narrow, true);
            let whole = SectionCtx {
                folded: false,
                ..ctx
            };
            soft_joined(rows, &plain_lines(&section, &whole, true))
        }
        SectionContent::Tool(_) => plain_lines(&section, &ctx, true),
        _ => plain_lines(&section, &narrow, true),
    };
    Some(lines.join("\n"))
}

/// The section `id` of `turn` in full, whatever its fold state and
/// truncation: for a tool call the description, the full input or command and
/// the full result (read from the transcript when it is not held in memory),
/// with the labels of the unfolded body; for the other sections, their
/// unfolded text. Tool text is exactly as recorded: one row per line, with
/// tabs and long lines intact. `None` when the turn has no such section.
pub fn full_text(turn: &Turn, id: &SectionId, theme: &Theme, open_key: &str) -> Option<String> {
    let section = find(turn, id)?;
    let ctx = SectionCtx {
        theme,
        width: PLAIN_WIDTH,
        folded: false,
        truncate: usize::MAX,
        expanded: true,
        open_key,
        verbatim: true,
    };
    Some(plain_lines(&section, &ctx, false).join("\n"))
}

/// The rows of a folded summary with the ones that continue the line before
/// them joined to it. `logical` is the whole text as lines that are not
/// wrapped: a row continues the line before it when its text is what follows
/// the text that row ended at. The join uses the whitespace the source has
/// there (none where the screen broke a word), and the rows of a quote lose
/// their repeated `│ ` bar. A row that cannot be matched stays a line of its
/// own.
fn soft_joined(rows: Vec<String>, logical: &[String]) -> Vec<String> {
    let logical: Vec<&str> = logical
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();
    let mut out: Vec<String> = Vec::new();
    // The logical line the last row came from and how much of it was shown.
    let mut at: Option<(usize, usize)> = None;
    for row in rows {
        let continued = match (at, out.last_mut()) {
            (Some((line, from)), Some(last)) => {
                let source = logical[line];
                let prefix = quote_prefix(source);
                let body = row.trim_start();
                let body = body.strip_prefix(prefix).unwrap_or(body).trim_start();
                locate(source, from, body).map(|(end, gap)| (last, line, end, gap, body))
            }
            _ => None,
        };
        if let Some((last, line, end, gap, body)) = continued {
            last.push_str(gap);
            last.push_str(body);
            at = Some((line, end));
        } else {
            let line = at.map_or(0, |(line, _)| line + 1);
            at = logical
                .get(line)
                .and_then(|source| locate(source, 0, &row))
                .map(|(end, _)| (line, end));
            out.push(row);
        }
    }
    out
}

/// Where `row` (the last row of a cut summary ends in `…`, which is not text)
/// continues `source` after its first `from` bytes: the end of the match, and
/// the whitespace between.
fn locate<'a>(source: &'a str, from: usize, row: &str) -> Option<(usize, &'a str)> {
    let rest = &source[from..];
    let text = rest.trim_start();
    let gap = &rest[..rest.len() - text.len()];
    let row = row.trim();
    [row, row.strip_suffix('…').unwrap_or(row)]
        .into_iter()
        .find(|key| !key.is_empty() && text.starts_with(key))
        .map(|key| (from + gap.len() + key.len(), gap))
}

/// The quote bars (`│ `, repeated for nested quotes) a line starts with.
fn quote_prefix(line: &str) -> &str {
    let mut rest = line;
    while let Some(after) = rest.strip_prefix("│ ") {
        rest = after;
    }
    &line[..line.len() - rest.len()]
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
        displayed_text(turn, id, folds, &theme(), truncate, KEY, None).unwrap()
    }

    fn shown_at(turn: &Turn, id: &SectionId, width: u16) -> String {
        displayed_text(turn, id, &FoldState::new(), &theme(), 20, KEY, Some(width)).unwrap()
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
            displayed_text(&turn, &other, &FoldState::new(), &theme(), 20, KEY, None),
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

    // -- folded summaries at the width of the block

    /// The prompt of the 80x24 screen: block width 80, text width 72.
    const ROW_1: &str = "I have a Logitech BRIO web camera attached to this machine via USB. It";
    const ROW_2: &str = "works fine - both camera and microphone - but I don't have a way to…";

    #[test]
    fn a_folded_one_paragraph_prompt_is_what_the_fold_shows_as_one_line() {
        let turn = user(LONG_PROMPT, vec![]);
        let id = key_id(&turn, SectionKey::Prompt);
        let text = shown_at(&turn, &id, 80);
        assert_eq!(text, format!("{ROW_1} {ROW_2}"));
        assert!(text.ends_with('…') && !text.contains('\n'));
        // Unbounded, it is all of the paragraph.
        assert_eq!(shown(&turn, &id, &FoldState::new(), 20), LONG_PROMPT);
    }

    #[test]
    fn a_wider_block_shows_more_of_a_folded_prompt() {
        let turn = user(LONG_PROMPT, vec![]);
        let id = key_id(&turn, SectionKey::Prompt);
        let narrow = shown_at(&turn, &id, 60);
        let wide = shown_at(&turn, &id, 120);
        assert!(narrow.len() < wide.len());
        assert!(LONG_PROMPT.starts_with(narrow.trim_end_matches('…')));
        assert!(LONG_PROMPT.starts_with(wide.trim_end_matches('…')));
    }

    #[test]
    fn a_folded_prompt_of_several_paragraphs_keeps_its_two_rows() {
        let turn = user(
            "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.",
            vec![],
        );
        let id = key_id(&turn, SectionKey::Prompt);
        assert_eq!(
            shown_at(&turn, &id, 80),
            "First paragraph.\nSecond paragraph.…"
        );
    }

    #[test]
    fn a_folded_prompt_whose_first_paragraph_wraps_shows_only_what_the_fold_shows() {
        let turn = user(
            &format!("{LONG_PROMPT}\n\nA second paragraph that is never shown."),
            vec![],
        );
        let id = key_id(&turn, SectionKey::Prompt);
        assert_eq!(shown_at(&turn, &id, 80), format!("{ROW_1} {ROW_2}"));
    }

    #[test]
    fn a_wrapped_first_paragraph_and_a_second_one_are_two_lines() {
        let turn = user(
            &format!(
                "{} {}\n\nSecond paragraph.\n\nThird.",
                "word ".repeat(10).trim_end(),
                "more".repeat(5)
            ),
            vec![],
        );
        let id = key_id(&turn, SectionKey::Prompt);
        // At 80 columns the first paragraph fits one row.
        assert_eq!(
            shown_at(&turn, &id, 80),
            format!(
                "{} {}\nSecond paragraph.…",
                "word ".repeat(10).trim_end(),
                "more".repeat(5)
            )
        );
    }

    #[test]
    fn a_short_folded_prompt_has_no_ellipsis_at_any_width() {
        let turn = user("Short one.", vec![]);
        let id = key_id(&turn, SectionKey::Prompt);
        assert_eq!(shown_at(&turn, &id, 80), "Short one.");
    }

    #[test]
    fn a_folded_thinking_header_is_cut_like_the_screen() {
        let turn = assistant(
            None,
            vec![thinking("th1", Thinking::Text(LONG_PROMPT.into()))],
            None,
        );
        let id = item_id(&turn, "th1");
        let text = shown_at(&turn, &id, 60);
        assert!(text.starts_with("Thinking: I have a Logitech"), "{text}");
        assert!(text.ends_with('…') && text.chars().count() == 52, "{text}");
    }

    #[test]
    fn a_folded_tool_call_stays_its_whole_description_at_any_width() {
        let turn = full_assistant();
        let id = item_id(&turn, "c1");
        assert_eq!(
            shown_at(&turn, &id, 20),
            "Find the BRIO on USB and check available tools"
        );
    }

    #[test]
    fn soft_joined_matches_rows_to_the_unwrapped_lines() {
        let logical = ["alpha beta gamma delta".to_owned(), "second".to_owned()];
        assert_eq!(
            soft_joined(rows(&["alpha beta", "gamma…"]), &logical),
            ["alpha beta gamma…"]
        );
        assert_eq!(
            soft_joined(rows(&["alpha beta gamma delta", "second"]), &logical),
            ["alpha beta gamma delta", "second"]
        );
        assert_eq!(
            soft_joined(rows(&["│ alpha", "│ beta"]), &logical),
            ["│ alpha", "│ beta"],
            "rows that cannot be matched stay apart"
        );
    }

    fn rows(rows: &[&str]) -> Vec<String> {
        rows.iter().map(|r| (*r).to_owned()).collect()
    }

    #[test]
    fn soft_joined_joins_with_the_sources_own_whitespace() {
        let join = |r: &[&str], l: &str| soft_joined(rows(r), &[l.to_owned()]);
        assert_eq!(join(&["abc", "def"], "abcdef"), ["abcdef"]);
        assert_eq!(join(&["ab", "cd"], "ab cd"), ["ab cd"]);
        assert_eq!(join(&["abc", "de…"], "abcdefgh"), ["abcde…"]);
    }

    #[test]
    fn soft_joined_joins_the_rows_of_a_quote_under_one_bar() {
        let join = |r: &[&str], l: &str| soft_joined(rows(r), &[l.to_owned()]);
        assert_eq!(
            join(&["│ alpha", "│ beta"], "│ alpha beta"),
            ["│ alpha beta"]
        );
        assert_eq!(
            join(&["│ │ alpha", "│ │ beta…"], "│ │ alpha beta gamma"),
            ["│ │ alpha beta…"]
        );
    }

    // -- the full copy is verbatim

    #[test]
    fn full_content_keeps_tabs_and_control_free_lines_exactly() {
        let result = "a\tb\n\tindented\n  two spaces  ";
        let turn = shell_turn("printf 'x\\ty'\tdone", result);
        let id = item_id(&turn, "c1");
        assert_eq!(
            full(&turn, &id),
            format!("Print the log\n\nCommand:\n\nprintf 'x\\ty'\tdone\n\nResult:\n\n{result}")
        );
    }

    #[test]
    fn full_content_does_not_break_a_very_long_line() {
        let long = "x".repeat(70_000);
        let turn = shell_turn(&format!("echo {long}"), &format!("{long}\nend"));
        let id = item_id(&turn, "c1");
        let text = full(&turn, &id);
        assert_eq!(
            text,
            format!("Print the log\n\nCommand:\n\necho {long}\n\nResult:\n\n{long}\nend")
        );
        assert_eq!(text.lines().count(), 10);
    }

    #[test]
    fn the_displayed_copy_still_expands_tabs() {
        let turn = shell_turn("ls", "a\tb");
        let id = item_id(&turn, "c1");
        assert!(shown(&turn, &id, &unfolded(&turn, "c1"), 20).ends_with("a    b"));
    }

    #[test]
    fn full_content_of_a_write_keeps_the_file_as_written() {
        let content = "package main\n\nfunc main() {\n\tprintln(\"hi\")\n}\n";
        let turn = assistant(
            None,
            vec![tool_of(
                ToolKind::Write,
                "c1",
                "Write main.go",
                json!({"file_path": "/a/main.go", "content": content}),
                Some(("created", false)),
                ToolStatus::Ok,
            )],
            None,
        );
        assert_eq!(
            full(&turn, &item_id(&turn, "c1")),
            format!(
                "Write main.go\n\nPath:\n\n/a/main.go\n\nContent:\n\n{}",
                content.trim_end()
            )
        );
    }

    #[test]
    fn a_folded_single_token_prompt_is_copied_without_inserted_spaces() {
        let source = format!("https://example.com/{}", "abcdefghij".repeat(20));
        let turn = user(&source, vec![]);
        let text = shown_at(&turn, &key_id(&turn, SectionKey::Prompt), 80);
        assert!(!text.contains(' ') && !text.contains('\n'), "{text}");
        assert!(text.ends_with('…'), "{text}");
        assert!(source.starts_with(text.trim_end_matches('…')), "{text}");
    }

    #[test]
    fn a_folded_cjk_prompt_is_copied_without_inserted_spaces() {
        let source = "日本語のテキストはスペースなしで長く続きます。".repeat(8);
        let turn = user(&source, vec![]);
        let text = shown_at(&turn, &key_id(&turn, SectionKey::Prompt), 80);
        assert!(!text.contains(' ') && !text.contains('\n'), "{text}");
        assert!(text.ends_with('…'), "{text}");
        assert!(source.starts_with(text.trim_end_matches('…')), "{text}");
    }

    #[test]
    fn a_folded_quoted_prompt_is_one_line_with_one_bar() {
        let quote = "a quoted line that is long enough to wrap over more than one row at sixty or eighty columns wide, and then it goes on for a good while longer so that it needs more than two rows";
        let turn = user(&format!("> {quote}"), vec![]);
        let text = shown_at(&turn, &key_id(&turn, SectionKey::Prompt), 80);
        assert!(!text.contains('\n'), "{text}");
        assert_eq!(text.matches('│').count(), 1, "{text}");
        let body = text.strip_prefix("│ ").expect("starts with the bar");
        assert!(body.ends_with('…'), "{text}");
        assert!(quote.starts_with(body.trim_end_matches('…')), "{text}");
    }

    #[test]
    fn a_folded_code_block_line_wider_than_the_row_has_no_inserted_space() {
        let line = format!("let value = {};", "x".repeat(150));
        let turn = user(&format!("```\n{line}\n```"), vec![]);
        let text = shown_at(&turn, &key_id(&turn, SectionKey::Prompt), 80);
        assert!(!text.contains('\n'), "{text}");
        let body = text.trim_end_matches('…').trim_start();
        assert!(line.starts_with(body), "{text}");
        assert!(!body.contains("x x"), "{text}");
    }
}
