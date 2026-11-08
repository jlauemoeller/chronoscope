//! The Subagent body: the prompt and the final result. The hint that opens
//! the transcript is on the call's title row (see `row::with_open_hint`).

use ratatui::text::Line;
use serde_json::Value;

use super::{
    Block, Body, PENDING, SectionCtx, ToolBodyRenderer, limit, muted_block, str_field,
    truncate_lines, unavailable_block,
};
use crate::markdown::render_markdown;
use crate::model::{ToolCall, ToolStatus};
use crate::theme::Token;

/// `Prompt:` with the prompt as markdown and `Result:` with the final result
/// as markdown (or `(pending)`).
///
/// The result is cut at `truncate_output_lines` source lines before the
/// markdown is laid out. The `… N more lines` row follows the result and is
/// the last row of the section.
#[derive(Debug, Clone, Copy)]
pub struct SubagentBody;

/// The agent type of a subagent call: the input's `subagent_type` (Claude) or
/// `agent` (OpenCode); `None` for any other tool or when unknown.
pub(super) fn agent_type(call: &ToolCall) -> Option<String> {
    if call.tool != crate::model::ToolKind::Subagent {
        return None;
    }
    str_field(&call.input, &["subagent_type", "agent"]).map(|s| s.trim().to_owned())
}

fn markdown_block(
    label: &'static str,
    text: &str,
    style: ratatui::style::Style,
    ctx: &SectionCtx<'_>,
) -> Block {
    let mut lines = if ctx.verbatim {
        super::super::verbatim_lines(text, style)
    } else {
        render_markdown(text, ctx.body_width(), style, ctx.theme)
    };
    if lines.is_empty() {
        lines.push(Line::default());
    }
    Block { label, lines }
}

impl ToolBodyRenderer for SubagentBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        let command = ctx.theme.style(Token::Command);
        let prompt = match call.input.get("prompt").and_then(Value::as_str) {
            Some(prompt) if !prompt.trim().is_empty() => prompt,
            _ => "(no prompt)",
        };
        let mut blocks = vec![markdown_block("Prompt:", prompt, command, ctx)];
        let result = call
            .result
            .as_ref()
            .filter(|_| call.status != ToolStatus::Pending);
        let mut remaining = None;
        blocks.push(match result {
            None => muted_block("Result:", PENDING, ctx),
            Some(result) => match result.output.text() {
                Ok(text) if text.trim().is_empty() => muted_block("Result:", "(no output)", ctx),
                Ok(text) => {
                    // Cut by logical source lines, before the markdown is laid out.
                    let (shown, cut) = truncate_lines(&text, limit(ctx));
                    remaining = (cut > 0).then_some(cut);
                    let token = if call.status == ToolStatus::Error {
                        Token::ErrorOutput
                    } else {
                        Token::Output
                    };
                    markdown_block("Result:", shown, ctx.theme.style(token), ctx)
                }
                Err(err) => unavailable_block("Result:", call, &err, ctx),
            },
        });
        Body { blocks, remaining }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::style::Modifier;
    use serde_json::json;

    use super::super::fixtures::*;
    use super::super::{SectionRenderer, ToolCallSection, ToolKind};
    use super::*;
    use crate::components::turn_block::renderers::TitleChrome;
    use crate::theme::Role;

    const HINT: &str = "a to open transcript";

    fn subagent(result: Option<&str>) -> ToolCall {
        call(
            ToolKind::Subagent,
            json!({"subagent_type": "Explore", "prompt": "Find the **USB** code", "description": "x"}),
            result,
        )
    }

    /// A title row: `text` (without the lead), then the gap, a rule of
    /// `rule` cells, the gap and `hint`.
    fn title(lead: &str, text: &str, rule: usize, hint: &str) -> String {
        format!("{lead}{text}  {}  {hint}", "─".repeat(rule))
    }

    fn ctx_at<'a>(theme: &'a crate::theme::Theme, width: u16, folded: bool) -> SectionCtx<'a> {
        SectionCtx::new(theme, width, folded)
    }

    #[test]
    fn unfolded_shows_the_title_row_then_prompt_and_result() {
        let theme = theme();
        let call = subagent(Some("Found it in `usb.rs`."));
        let rows = section(&call, &theme, 60);
        // r = 62, d = 22: a rule of 62 - 22 - 4 - 20 = 16 cells.
        assert_eq!(
            rows,
            [
                title("▼ ", "Do the thing (Explore)", 16, HINT).as_str(),
                "",
                "  Prompt:",
                "",
                "     Find the USB code",
                "",
                "  Result:",
                "",
                "     Found it in usb.rs.",
            ]
        );
        let lines = ToolCallSection(&call).lines(&ctx_at(&theme, 60, false));
        assert_eq!(
            fg_of(&lines[2], "Prompt:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(fg_of(&lines[4], "Find"), Some(color(&theme, Role::Accent)));
        assert_eq!(
            fg_of(&lines[8], "Found"),
            Some(color(&theme, Role::Secondary))
        );
        assert_eq!(
            fg_of(&lines[0], "open transcript"),
            Some(color(&theme, Role::MutedForeground))
        );
        assert_eq!(fg_of(&lines[0], "─"), Some(color(&theme, Role::Border)));
    }

    #[test]
    fn the_body_ends_with_the_result_and_no_hint() {
        let theme = theme();
        let call = subagent(Some("Done."));
        let rows = section(&call, &theme, 60);
        assert_eq!(rows.last().unwrap(), "     Done.");
        assert!(rows[1..].iter().all(|r| !r.contains("to open transcript")));
    }

    #[test]
    fn folded_is_the_description_the_rule_and_the_hint() {
        let theme = theme();
        let call = subagent(None);
        let lines = ToolCallSection(&call).lines(&ctx_at(&theme, 60, true));
        assert_eq!(
            texts(&lines),
            [title("▶ ", "Do the thing (Explore)", 16, HINT)]
        );
        // Two cells wider than the text area: r = t + 2 after the lead.
        assert_eq!(lines[0].width(), 2 + 62);
    }

    #[test]
    fn the_agent_type_survives_a_cut_description_and_the_row_has_no_rule() {
        let theme = theme();
        let mut call = subagent(None);
        call.description = "d".repeat(80);
        let ctx = ctx_at(&theme, 30, true);
        let lines = ToolCallSection(&call).lines(&ctx);
        let text = lines[0].to_string();
        assert!(text.ends_with("… (Explore)"), "{text}");
        assert!(
            !text.contains('─') && !text.contains("transcript"),
            "{text}"
        );
        assert_eq!(lines[0].width(), 32);
        let render = ToolCallSection(&call).render(&ctx);
        assert_eq!(render.chrome, None);
    }

    #[test]
    fn a_row_with_a_duration_has_the_rule_after_it() {
        let theme = theme();
        let mut call = subagent(None);
        call.status = ToolStatus::Ok;
        call.duration = Some(Duration::from_secs(302));
        let rows = texts(&ToolCallSection(&call).lines(&ctx_at(&theme, 60, true)));
        let d = "Do the thing (Explore) · 5m 2s";
        assert_eq!(
            rows,
            [title("▶ ", d, 62 - d.chars().count() - 4 - 20, HINT)]
        );
    }

    #[test]
    fn opencode_names_the_agent_in_agent_and_an_unknown_one_is_omitted() {
        let theme = theme();
        let ctx = ctx_at(&theme, 60, true);
        let open = call(ToolKind::Subagent, json!({"agent": "general"}), None);
        assert_eq!(
            texts(&ToolCallSection(&open).lines(&ctx)),
            [title("▶ ", "Do the thing (general)", 16, HINT)]
        );
        // No agent type: the row still gets the rule and the hint.
        let unknown = call(ToolKind::Subagent, json!({"prompt": "p"}), None);
        assert_eq!(
            texts(&ToolCallSection(&unknown).lines(&ctx)),
            [title("▶ ", "Do the thing", 26, HINT)]
        );
    }

    #[test]
    fn other_calls_get_neither_rule_nor_hint() {
        let theme = theme();
        for folded in [true, false] {
            for kind in [ToolKind::Shell, ToolKind::Other("mcp__x__y".into())] {
                let call = call(kind, json!({"command": "ls"}), Some("out"));
                let render = ToolCallSection(&call).render(&ctx_at(&theme, 60, folded));
                assert_eq!(render.chrome, None);
                for line in &render.lines {
                    let text = line.to_string();
                    assert!(
                        !text.contains('─') && !text.contains("transcript"),
                        "{text}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_pending_subagent_shows_pending_and_the_hint_follows_the_key() {
        let theme = theme();
        let call = subagent(None);
        for key in ["o", "Ctrl-o"] {
            for folded in [true, false] {
                let ctx = SectionCtx {
                    open_key: key,
                    ..ctx_at(&theme, 60, folded)
                };
                let rows = texts(&ToolCallSection(&call).lines(&ctx));
                let hint = format!("{key} to open transcript");
                let rule = 62 - 22 - 4 - hint.len();
                let lead = if folded { "▶ " } else { "▼ " };
                assert_eq!(rows[0], title(lead, "Do the thing (Explore)", rule, &hint));
                if !folded {
                    assert_eq!(rows[8], "     (pending)");
                    assert_eq!(rows.len(), 9);
                }
            }
        }
    }

    #[test]
    fn a_description_over_two_rows_has_the_rule_and_hint_on_the_second_only() {
        let theme = theme();
        let mut call = subagent(Some("ok"));
        call.description =
            "Explore the repository, list every crate and report what each one is for".into();
        call.duration = Some(Duration::from_secs(302));
        let render = ToolCallSection(&call).render(&ctx_at(&theme, 60, false));
        let rows = texts(&render.lines);
        assert!(!rows[0].contains('─'), "{}", rows[0]);
        assert!(rows[1].contains("· 5m 2s  ─"), "{}", rows[1]);
        assert!(rows[1].ends_with(HINT), "{}", rows[1]);
        assert_eq!(render.chrome.unwrap().row, 1);
    }

    #[test]
    fn a_duration_on_a_row_of_its_own_has_the_rule_and_hint_after_it() {
        let theme = theme();
        let mut call = subagent(Some("ok"));
        call.description = "a".repeat(30);
        call.duration = Some(Duration::from_secs(302));
        let render = ToolCallSection(&call).render(&ctx_at(&theme, 40, false));
        let rows = texts(&render.lines);
        // 30 + " (Explore)" fills the row; the duration cannot follow it.
        assert_eq!(rows[0], format!("▼ {} (Explore)", "a".repeat(30)));
        assert!(!rows[0].contains('─'));
        assert!(rows[1].starts_with("  · 5m 2s  ─"), "{rows:?}");
        assert!(rows[1].ends_with(HINT));
        assert_eq!(render.chrome.unwrap().row, 1);
    }

    #[test]
    fn an_unfolded_hint_without_room_is_on_the_row_below_the_description() {
        let theme = theme();
        let mut call = subagent(Some("ok"));
        call.description = "d".repeat(28);
        // t = 40, d = 28 + 10 = 38: r = 42 < d + 5.
        let render = ToolCallSection(&call).render(&ctx_at(&theme, 40, false));
        let rows = texts(&render.lines);
        assert_eq!(rows[0], format!("▼ {} (Explore)", "d".repeat(28)));
        assert_eq!(rows[1], format!("  {}  {HINT}", "─".repeat(42 - 2 - 20)));
        assert_eq!(rows[2], "");
        assert_eq!(rows[3], "  Prompt:");
        assert_eq!(
            render.chrome,
            Some(TitleChrome {
                row: 1,
                span: 1,
                own_row: true,
                rule: Some(1)
            })
        );
        // Folded, the same row shows the hint nowhere.
        let folded = texts(&ToolCallSection(&call).lines(&ctx_at(&theme, 40, true)));
        assert_eq!(folded, [format!("▶ {} (Explore)", "d".repeat(28))]);
    }

    #[test]
    fn a_hint_row_narrower_than_the_hint_is_the_hint_cut() {
        let theme = theme();
        let call = subagent(Some("ok"));
        let render = ToolCallSection(&call).render(&ctx_at(&theme, 10, false));
        let rows = texts(&render.lines);
        // The description wraps; the hint is alone on its row, cut to r = 12.
        let at = rows.iter().position(|r| r.contains('…')).unwrap();
        assert_eq!(rows[at], "  a to open t…");
        assert_eq!(rows[at + 1], "");
    }

    #[test]
    fn the_rule_is_border_and_the_hint_muted_with_no_background() {
        let theme = theme();
        let call = subagent(None);
        let lines = ToolCallSection(&call).lines(&ctx_at(&theme, 60, true));
        let rule = lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains('─'))
            .unwrap();
        assert_eq!(rule.style.fg, Some(color(&theme, Role::Border)));
        assert!(!rule.style.add_modifier.contains(Modifier::BOLD));
        assert!(lines[0].spans.iter().all(|s| s.style.bg.is_none()));
    }

    #[test]
    fn a_long_result_is_cut_and_expanding_shows_all_of_it() {
        let theme = theme();
        let text: String = (1..=10).map(|n| format!("r{n}\n\n")).collect();
        let call = subagent(Some(&text));
        let ctx = SectionCtx {
            truncate: 4,
            ..ctx_at(&theme, 60, false)
        };
        let body = SubagentBody.body(&call, &ctx);
        // 4 source lines: r1, blank, r2, blank.
        assert_eq!(body.blocks[1].lines.len(), 3);
        assert_eq!(body.blocks[1].lines[0].to_string(), "r1");
        assert_eq!(body.remaining, Some(16));
        let expanded = SectionCtx {
            expanded: true,
            ..ctx
        };
        let body = SubagentBody.body(&call, &expanded);
        assert_eq!(body.remaining, None);
        assert!(body.blocks[1].lines.iter().any(|l| l.to_string() == "r10"));
    }

    #[test]
    fn the_more_row_directly_follows_the_result_and_is_the_last_row() {
        let theme = theme();
        let call = subagent(Some("a\nb\nc"));
        let ctx = SectionCtx {
            truncate: 1,
            ..ctx_at(&theme, 60, false)
        };
        let section = ToolCallSection(&call).render(&ctx);
        let last = section.lines.last().unwrap().to_string();
        assert_eq!(last, "     a");
        assert!(
            !section
                .lines
                .iter()
                .skip(1)
                .any(|l| l.to_string().contains("transcript"))
        );
        assert_eq!(section.more.unwrap().to_string().trim(), "… 2 more lines");
        // As a flat list of lines, the `more` row comes last.
        let all = texts(&ToolCallSection(&call).lines(&ctx));
        assert_eq!(all[all.len() - 2], "     a");
        assert_eq!(all.last().unwrap().trim(), "… 2 more lines");
    }
}
