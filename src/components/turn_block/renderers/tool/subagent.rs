//! The Subagent body: the prompt, the final result and the hint that opens
//! the transcript.

use ratatui::text::{Line, Span};
use serde_json::Value;

use super::{
    Block, Body, PENDING, SectionCtx, ToolBodyRenderer, ellipsize, limit, muted_block, str_field,
    truncate_lines, unavailable_block,
};
use crate::markdown::render_markdown;
use crate::model::{ToolCall, ToolStatus};
use crate::theme::Token;

/// `Prompt:` with the prompt as markdown, `Result:` with the final result as
/// markdown (or `(pending)`), and a last row `<key> to open transcript`,
/// where `<key>` is the label of the `open_subagent` key.
///
/// The result is cut at `truncate_output_lines` source lines before the
/// markdown is laid out. The `… N more lines` row is the last row of the
/// section, so it comes after the hint.
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
    let mut lines = render_markdown(text, ctx.body_width(), style, ctx.theme);
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
        let hint = Line::from(Span::styled(
            format!("{} to open transcript", ctx.open_key),
            ctx.muted(),
        ));
        let hint = ellipsize(hint, usize::from(ctx.width), false);
        Body {
            blocks,
            remaining,
            hint: Some(hint),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::testing::*;
    use super::super::{SectionRenderer, ToolCallSection, ToolKind};
    use super::*;
    use crate::theme::Role;

    fn subagent(result: Option<&str>) -> ToolCall {
        call(
            ToolKind::Subagent,
            json!({"subagent_type": "Explore", "prompt": "Find the **USB** code", "description": "x"}),
            result,
        )
    }

    #[test]
    fn unfolded_shows_prompt_result_and_the_hint() {
        let theme = theme();
        let call = subagent(Some("Found it in `usb.rs`."));
        let rows = section(&call, &theme, 60);
        assert_eq!(
            rows,
            [
                "▼ Do the thing (Explore)",
                "",
                "  Prompt:",
                "",
                "     Find the USB code",
                "",
                "  Result:",
                "",
                "     Found it in usb.rs.",
                "",
                "  a to open transcript",
            ]
        );
        let ctx = SectionCtx::new(&theme, 60, false);
        let lines = ToolCallSection(&call).lines(&ctx);
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
            fg_of(&lines[10], "open transcript"),
            Some(color(&theme, Role::MutedForeground))
        );
    }

    #[test]
    fn folded_shows_the_agent_type_after_the_description() {
        let theme = theme();
        let call = subagent(None);
        let ctx = SectionCtx::new(&theme, 60, true);
        let lines = ToolCallSection(&call).lines(&ctx);
        assert_eq!(texts(&lines), ["▶ Do the thing (Explore)"]);
    }

    #[test]
    fn the_agent_type_survives_a_cut_description() {
        let theme = theme();
        let mut call = subagent(None);
        call.description = "d".repeat(80);
        let ctx = SectionCtx::new(&theme, 30, true);
        let lines = ToolCallSection(&call).lines(&ctx);
        let text = lines[0].to_string();
        assert!(text.ends_with("… (Explore)"), "{text}");
        assert_eq!(lines[0].width(), 32);
    }

    #[test]
    fn opencode_names_the_agent_in_agent_and_an_unknown_one_is_omitted() {
        let theme = theme();
        let ctx = SectionCtx::new(&theme, 60, true);
        let open = call(ToolKind::Subagent, json!({"agent": "general"}), None);
        assert_eq!(
            texts(&ToolCallSection(&open).lines(&ctx)),
            ["▶ Do the thing (general)"]
        );
        let unknown = call(ToolKind::Subagent, json!({"prompt": "p"}), None);
        assert_eq!(
            texts(&ToolCallSection(&unknown).lines(&ctx)),
            ["▶ Do the thing"]
        );
    }

    #[test]
    fn a_pending_subagent_shows_pending_and_the_hint_follows_the_key() {
        let theme = theme();
        let call = subagent(None);
        let ctx = SectionCtx {
            open_key: "Ctrl-o",
            ..SectionCtx::new(&theme, 60, false)
        };
        let rows = texts(&ToolCallSection(&call).lines(&ctx));
        assert_eq!(rows[8], "     (pending)");
        assert_eq!(rows.last().unwrap(), "  Ctrl-o to open transcript");
    }

    #[test]
    fn a_long_result_is_cut_and_expanding_shows_all_of_it() {
        let theme = theme();
        let text: String = (1..=10).map(|n| format!("r{n}\n\n")).collect();
        let call = subagent(Some(&text));
        let ctx = SectionCtx {
            truncate: 4,
            ..SectionCtx::new(&theme, 60, false)
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
    fn the_more_row_is_the_last_row_after_the_hint() {
        let theme = theme();
        let call = subagent(Some("a\nb\nc"));
        let ctx = SectionCtx {
            truncate: 1,
            ..SectionCtx::new(&theme, 60, false)
        };
        let section = ToolCallSection(&call).render(&ctx);
        let last = section.lines.last().unwrap().to_string();
        assert_eq!(last, "  a to open transcript");
        assert_eq!(section.more.unwrap().to_string().trim(), "… 2 more lines");
    }
}
