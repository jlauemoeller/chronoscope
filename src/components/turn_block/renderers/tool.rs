//! The tool call section, with the generic body used for every tool kind.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::{
    BODY_INDENT, SectionCtx, SectionRenderer, ellipsize, first_line, fold_marker, indent, label,
    with_lead, word_wrap,
};
use crate::markdown::char_wrap;
use crate::model::{ToolCall, ToolKind, ToolStatus};
use crate::theme::Token;

/// The text shown for a call that has no result yet.
pub const PENDING: &str = "(pending)";

/// A tool call.
///
/// Folded: `▶ <description>`. Unfolded: the description, then `Input:` with
/// the pretty-printed input and `Result:` with the output.
#[derive(Debug, Clone, Copy)]
pub struct ToolCallSection<'a>(pub &'a ToolCall);

impl ToolCallSection<'_> {
    /// The description, or the tool's name when the description is blank.
    fn description_text(&self) -> String {
        if !self.0.description.trim().is_empty() {
            return self.0.description.trim().to_owned();
        }
        match &self.0.tool {
            ToolKind::Shell => "Shell",
            ToolKind::Read => "Read",
            ToolKind::Write => "Write",
            ToolKind::Edit => "Edit",
            ToolKind::Grep => "Grep",
            ToolKind::Glob => "Glob",
            ToolKind::WebFetch => "WebFetch",
            ToolKind::WebSearch => "WebSearch",
            ToolKind::Subagent => "Subagent",
            ToolKind::Other(name) => name,
        }
        .to_owned()
    }

    /// Folded: one row, cut with `…`. Unfolded: the description is prose, so
    /// it is word-wrapped.
    fn description(&self, ctx: &SectionCtx<'_>, style: Style) -> Vec<Line<'static>> {
        let text = self.description_text();
        if ctx.folded {
            let line = Line::from(Span::styled(first_line(&text), style));
            return vec![ellipsize(line, usize::from(ctx.width), false)];
        }
        word_wrap(&text, ctx.width, style)
    }

    fn input(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let style = ctx.theme.style(Token::Command);
        let json = serde_json::to_string_pretty(&self.0.input).unwrap_or_default();
        indent(
            char_wrap(&json, ctx.body_width(), style),
            BODY_INDENT,
            style,
        )
    }

    fn result(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let output = ctx.theme.style(Token::Output);
        let error = ctx.theme.style(Token::ErrorOutput);
        let is_error = self.0.status == ToolStatus::Error;
        let style = if is_error { error } else { output };
        let (text, style) = match (&self.0.result, self.0.status) {
            (_, ToolStatus::Pending) | (None, _) => (PENDING.to_owned(), ctx.muted()),
            (Some(result), _) => match result.output.text() {
                Ok(text) if text.trim().is_empty() => ("(no output)".to_owned(), ctx.muted()),
                Ok(text) => (text.into_owned(), style),
                Err(err) => (format!("({err})"), error),
            },
        };
        let mut lines = char_wrap(&text, ctx.body_width(), style);
        if lines.is_empty() {
            lines.push(Line::from(Span::styled(text, style)));
        }
        indent(lines, BODY_INDENT, style)
    }
}

impl SectionRenderer for ToolCallSection<'_> {
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let style = ctx.theme.style(Token::ToolDescription);
        let mut lines = self.description(ctx, style);
        if !ctx.folded {
            let label_style = ctx.theme.style(Token::Label);
            lines.push(Line::default());
            lines.push(label("Input:", label_style));
            lines.push(Line::default());
            lines.extend(self.input(ctx));
            lines.push(Line::default());
            lines.push(label("Result:", label_style));
            lines.push(Line::default());
            lines.extend(self.result(ctx));
        }
        with_lead(lines, fold_marker(ctx.folded), style)
    }
}
