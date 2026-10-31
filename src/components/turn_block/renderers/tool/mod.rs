//! The tool call section: the description row, then one body per
//! [`ToolKind`] (SPEC §5.3).
//!
//! Each kind has its own [`ToolBodyRenderer`] type in its own module. A body
//! is a list of labelled blocks (`Command:`, `Result:`, ...); the section lays
//! them out with the task-017 geometry: the `▼ <description>` row, a blank
//! row, then for each block its label at column 4, a blank row, its content
//! at column 7 and a blank row before the next label.
//!
//! Long output is cut at `truncate_output_lines` logical lines. The
//! `… N more lines` row is returned apart from the section's lines
//! ([`Rendered::more`]) because it is a selection stop of its own.
//!
//! Output text is materialised (`ToolOutput::text()`) only by an unfolded
//! body; a folded call renders its description and nothing else.

mod edit;
mod other;
mod read;
mod search;
mod shell;
mod subagent;
mod web;
mod write;

#[cfg(test)]
mod testing;

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::{
    BODY_INDENT, Rendered, SectionCtx, SectionRenderer, ellipsize, first_line, fold_marker, indent,
    label, verbatim_lines, with_lead, word_wrap,
};
use crate::markdown::char_wrap;
use crate::model::{ToolCall, ToolKind, ToolStatus};
use crate::theme::Token;

pub use edit::EditBody;
pub use other::OtherBody;
pub use read::ReadBody;
pub use search::{GlobBody, GrepBody};
pub use shell::ShellBody;
pub use subagent::SubagentBody;
pub use web::{WebFetchBody, WebSearchBody};
pub use write::WriteBody;

/// The text shown for a call that has no result yet.
pub const PENDING: &str = "(pending)";

/// A label (`Command:`) and the content below it, from column 0 of the
/// content (the section indents it).
#[derive(Debug, Clone)]
pub struct Block {
    /// The label text, including its colon.
    pub label: &'static str,
    /// The content lines, already wrapped to the body width.
    pub lines: Vec<Line<'static>>,
}

/// What a [`ToolBodyRenderer`] produces.
#[derive(Debug, Clone, Default)]
pub struct Body {
    /// The labelled blocks, in order.
    pub blocks: Vec<Block>,
    /// How many logical lines were cut from the body's last block, which is
    /// always the truncatable one. The `… N more lines` row goes directly
    /// under it.
    pub remaining: Option<usize>,
    /// A row at the label column after the blocks, preceded by a blank row
    /// (the subagent's "open transcript" hint). It comes before the
    /// `… N more lines` row, if there is one.
    pub hint: Option<Line<'static>>,
}

/// Renders the body of one kind of tool call.
pub trait ToolBodyRenderer {
    /// The body of `call` for `ctx`. Called for unfolded sections only.
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body;
}

/// The renderer for `kind`.
pub fn body_renderer(kind: &ToolKind) -> &'static dyn ToolBodyRenderer {
    match kind {
        ToolKind::Shell => &ShellBody,
        ToolKind::Read => &ReadBody,
        ToolKind::Write => &WriteBody,
        ToolKind::Edit => &EditBody,
        ToolKind::Grep => &GrepBody,
        ToolKind::Glob => &GlobBody,
        ToolKind::WebFetch => &WebFetchBody,
        ToolKind::WebSearch => &WebSearchBody,
        ToolKind::Subagent => &SubagentBody,
        ToolKind::Other(_) => &OtherBody,
    }
}

/// A tool call.
///
/// Folded: `▶ <description>` (plus the agent type for a subagent). Unfolded:
/// the description, then the body of the call's kind.
#[derive(Debug, Clone, Copy)]
pub struct ToolCallSection<'a>(pub &'a ToolCall);

impl ToolCallSection<'_> {
    /// The description, or the tool's name when the description is blank.
    pub fn description_text(&self) -> String {
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

    /// Folded: one row, cut with `…` (the agent type of a subagent is kept).
    /// Unfolded: the description is prose, so it is word-wrapped.
    fn description(&self, ctx: &SectionCtx<'_>, style: Style) -> Vec<Line<'static>> {
        let text = self.description_text();
        let suffix = subagent::agent_type(self.0).map(|agent| format!(" ({agent})"));
        if !ctx.folded {
            let full = match &suffix {
                Some(suffix) => format!("{text}{suffix}"),
                None => text,
            };
            return word_wrap(&full, ctx.width, style);
        }
        let width = usize::from(ctx.width);
        let line = Line::from(Span::styled(first_line(&text), style));
        let Some(suffix) = suffix else {
            return vec![ellipsize(line, width, false)];
        };
        let suffix_width = unicode_width::UnicodeWidthStr::width(suffix.as_str());
        if suffix_width + 1 >= width {
            let all = Line::from(Span::styled(
                format!("{}{suffix}", first_line(&text)),
                style,
            ));
            return vec![ellipsize(all, width, false)];
        }
        let mut line = ellipsize(line, width - suffix_width, false);
        line.spans.push(Span::styled(suffix, style));
        vec![line]
    }

    /// The description rows, and the body when the section is unfolded. The
    /// styled rendering and the plain text are both made from these.
    fn parts(&self, ctx: &SectionCtx<'_>) -> (Vec<Line<'static>>, Option<Body>) {
        let style = ctx.theme.style(Token::ToolDescription);
        let description = self.description(ctx, style);
        let body = (!ctx.folded).then(|| body_renderer(&self.0.tool).body(self.0, ctx));
        (description, body)
    }

    /// The section's lines, with the truncation row apart.
    pub fn render(&self, ctx: &SectionCtx<'_>) -> Rendered {
        let style = ctx.theme.style(Token::ToolDescription);
        let (mut lines, body) = self.parts(ctx);
        let mut has_more = false;
        if let Some(body) = body {
            let label_style = ctx.theme.style(Token::Label);
            for block in body.blocks {
                lines.push(Line::default());
                lines.push(label(block.label, label_style));
                lines.push(Line::default());
                lines.extend(indent_like_text(block.lines));
            }
            if let Some(hint) = body.hint {
                lines.push(Line::default());
                lines.push(hint);
            }
            if let Some(remaining) = body.remaining {
                has_more = true;
                let row = Line::from(Span::styled(more_text(remaining), ctx.muted()));
                let row = ellipsize(row, usize::from(ctx.body_width()), false);
                lines.extend(indent_like_text(vec![row]));
            }
        }
        let mut lines = with_lead(lines, fold_marker(ctx.folded), style);
        let more = if has_more { lines.pop() } else { None };
        Rendered { lines, more }
    }

    /// The section as plain text, in parts: the description rows, then (when
    /// unfolded) each labelled block's rows without the column indent, the
    /// subagent's "open transcript" hint and the `… N more lines` row.
    pub fn plain_parts(&self, ctx: &SectionCtx<'_>) -> PlainTool {
        let (description, body) = self.parts(ctx);
        let description = description.iter().map(ToString::to_string).collect();
        let Some(body) = body else {
            return PlainTool {
                description,
                ..PlainTool::default()
            };
        };
        PlainTool {
            description,
            blocks: body
                .blocks
                .into_iter()
                .map(|block| PlainBlock {
                    label: block.label,
                    lines: block.lines.iter().map(ToString::to_string).collect(),
                })
                .collect(),
            hint: body.hint.map(|hint| hint.to_string()),
            more: body.remaining.map(more_text),
        }
    }

    /// The section as plain text rows: what [`render`](Self::render) shows,
    /// without the fold marker and the column indents. Blank rows and labels
    /// are kept, and so is the content's own indentation. The subagent's
    /// "open transcript" hint is left out unless `with_hint`.
    pub fn plain_lines(&self, ctx: &SectionCtx<'_>, with_hint: bool) -> Vec<String> {
        let parts = self.plain_parts(ctx);
        let mut out = parts.description;
        for block in parts.blocks {
            out.push(String::new());
            out.push(block.label.to_owned());
            out.push(String::new());
            out.extend(block.lines);
        }
        if let (true, Some(hint)) = (with_hint, parts.hint) {
            out.push(String::new());
            out.push(hint);
        }
        out.extend(parts.more);
        out
    }
}

/// A tool call as plain text, in parts (see [`ToolCallSection::plain_parts`]).
#[derive(Debug, Clone, Default)]
pub struct PlainTool {
    /// The description rows.
    pub description: Vec<String>,
    /// The labelled blocks of an unfolded call.
    pub blocks: Vec<PlainBlock>,
    /// The subagent's "open transcript" hint.
    pub hint: Option<String>,
    /// The `… N more lines` row of a truncated output.
    pub more: Option<String>,
}

/// A label and the rows below it.
#[derive(Debug, Clone)]
pub struct PlainBlock {
    /// The label, including its colon.
    pub label: &'static str,
    /// The content rows.
    pub lines: Vec<String>,
}

/// The text of the truncation row.
fn more_text(remaining: usize) -> String {
    format!(
        "… {} more {}",
        group_thousands(remaining),
        lines_noun(remaining)
    )
}

impl SectionRenderer for ToolCallSection<'_> {
    /// The section's lines with the truncation row, if any, at the end.
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let Rendered { mut lines, more } = self.render(ctx);
        lines.extend(more);
        lines
    }
}

/// Indents each non-blank line to the content column; the padding takes the
/// style of the line's text.
fn indent_like_text(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|line| {
            let style = line.spans.first().map(|s| s.style).unwrap_or_default();
            indent(vec![line], BODY_INDENT, style).remove(0)
        })
        .collect()
}

fn lines_noun(n: usize) -> &'static str {
    if n == 1 { "line" } else { "lines" }
}

/// `1234` as `1 234`.
pub fn group_thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

// ---- helpers shared by the body renderers ----

/// The number of lines a body shows: all when expanded.
pub(super) fn limit(ctx: &SectionCtx<'_>) -> usize {
    if ctx.expanded {
        usize::MAX
    } else {
        ctx.truncate
    }
}

/// The first `max` lines of `text` and how many lines were cut.
pub(super) fn truncate_lines(text: &str, max: usize) -> (&str, usize) {
    let mut end = 0;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        if i == max {
            return (&text[..end], text[end..].lines().count());
        }
        end += line.len();
    }
    (text, 0)
}

/// A string field of the tool input, the first of `keys` that is present.
pub(super) fn str_field<'a>(input: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| input.get(*key)?.as_str())
        .filter(|s| !s.trim().is_empty())
}

/// The path a file tool works on.
pub(super) fn path_of(input: &Value) -> Option<&str> {
    str_field(input, &["file_path", "path", "notebook_path", "filePath"])
}

/// The input as one line of JSON, for a call that lacks the field a renderer
/// looks for.
pub(super) fn compact_json(input: &Value) -> String {
    serde_json::to_string(input).unwrap_or_default()
}

/// A block of char-wrapped `text` in `style`. Empty text gives one empty line
/// so that the block is never empty.
pub(super) fn text_block(
    label: &'static str,
    text: &str,
    style: Style,
    ctx: &SectionCtx<'_>,
) -> Block {
    let mut lines = if ctx.verbatim {
        verbatim_lines(text, style)
    } else {
        char_wrap(text, ctx.body_width(), style)
    };
    if lines.is_empty() {
        lines.push(Line::default());
    }
    Block { label, lines }
}

/// A block with the single muted `text`.
pub(super) fn muted_block(label: &'static str, text: &str, ctx: &SectionCtx<'_>) -> Block {
    text_block(label, text, ctx.muted(), ctx)
}

/// The result of `call` under `label`, and how many lines were cut. A pending
/// call shows `(pending)`, an empty result `(no output)` and a result that
/// cannot be read `(output unavailable: ...)`; a failed call's text is in
/// `error_output`.
pub(super) fn result_block(
    label: &'static str,
    call: &ToolCall,
    ctx: &SectionCtx<'_>,
) -> (Block, Option<usize>) {
    let error = ctx.theme.style(Token::ErrorOutput);
    let style = if call.status == ToolStatus::Error {
        error
    } else {
        ctx.theme.style(Token::Output)
    };
    let Some(result) = call
        .result
        .as_ref()
        .filter(|_| call.status != ToolStatus::Pending)
    else {
        return (muted_block(label, PENDING, ctx), None);
    };
    let text = match result.output.text() {
        Ok(text) => text,
        Err(err) => return (unavailable_block(label, call, &err, ctx), None),
    };
    if text.trim().is_empty() {
        return (muted_block(label, "(no output)", ctx), None);
    }
    let (shown, cut) = truncate_lines(&text, limit(ctx));
    let block = text_block(label, shown, style, ctx);
    (block, (cut > 0).then_some(cut))
}

/// The block for an output that could not be materialised. Logged on every
/// layout of the section; the section is laid out only while unfolded.
pub(super) fn unavailable_block(
    label: &'static str,
    call: &ToolCall,
    err: &crate::model::ToolOutputError,
    ctx: &SectionCtx<'_>,
) -> Block {
    log::warn!("tool output of {} unavailable: {err}", call.id);
    let message = format!("(output unavailable: {err})");
    text_block(label, &message, ctx.theme.style(Token::ErrorOutput), ctx)
}

/// `body` (`Path:` first, then the content or diff) for a call that takes no
/// `Result:` when it succeeded (Write, Edit). A failed or pending call gets
/// one after all, so that its error or `(pending)` shows.
///
/// A body has one truncatable block, and it is the last. `content_cut` says
/// whether the content or diff exceeds `truncate_output_lines`; the caller
/// computes it from the configured limit, not from the current one, so that
/// expanding the section does not reorder the blocks.
/// - Cut content or diff: the `Result:` goes right after `Path:` and is shown
///   in full, so that the `… N more lines` row is directly under the lines it
///   counts.
/// - Otherwise the `Result:` comes last and is truncated like any other
///   result, and `body.remaining` is its cut. (`(pending)` is never cut.)
pub(super) fn with_status_result(
    mut body: Body,
    call: &ToolCall,
    ctx: &SectionCtx<'_>,
    content_cut: bool,
) -> Body {
    if !matches!(call.status, ToolStatus::Error | ToolStatus::Pending) {
        return body;
    }
    if content_cut {
        let full = SectionCtx {
            expanded: true,
            ..*ctx
        };
        let (block, _) = result_block("Result:", call, &full);
        body.blocks.insert(1.min(body.blocks.len()), block);
    } else {
        let (block, remaining) = result_block("Result:", call, ctx);
        body.blocks.push(block);
        body.remaining = remaining;
    }
    body
}

/// The usual body: `blocks` followed by the result block under `label`.
pub(super) fn with_result(
    mut blocks: Vec<Block>,
    label: &'static str,
    call: &ToolCall,
    ctx: &SectionCtx<'_>,
) -> Body {
    let (block, remaining) = result_block(label, call, ctx);
    blocks.push(block);
    Body {
        blocks,
        remaining,
        hint: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_are_grouped_with_a_space() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1000), "1 000");
        assert_eq!(group_thousands(1234), "1 234");
        assert_eq!(group_thousands(1_234_567), "1 234 567");
    }

    #[test]
    fn truncate_lines_cuts_at_a_logical_line() {
        assert_eq!(truncate_lines("a\nb\nc\n", 2), ("a\nb\n", 1));
        assert_eq!(truncate_lines("a\nb\nc", 2), ("a\nb\n", 1));
        assert_eq!(truncate_lines("a\nb\n", 2), ("a\nb\n", 0));
        assert_eq!(truncate_lines("a\nb", 5), ("a\nb", 0));
        assert_eq!(truncate_lines("a\nb", 0), ("", 2));
        assert_eq!(truncate_lines("", 3), ("", 0));
    }

    #[test]
    fn dispatch_picks_a_renderer_for_every_kind() {
        let kinds = [
            ToolKind::Shell,
            ToolKind::Read,
            ToolKind::Write,
            ToolKind::Edit,
            ToolKind::Grep,
            ToolKind::Glob,
            ToolKind::WebFetch,
            ToolKind::WebSearch,
            ToolKind::Subagent,
            ToolKind::Other("mcp__x__y".into()),
        ];
        let labels = [
            "Command:", "Path:", "Path:", "Path:", "Pattern:", "Pattern:", "URL:", "Query:",
            "Prompt:", "Input:",
        ];
        for (kind, first) in kinds.into_iter().zip(labels) {
            let call = testing::call(kind, serde_json::json!({}), Some("out"));
            let theme = crate::theme::builtin::default_theme();
            let body = body_renderer(&call.tool).body(&call, &SectionCtx::new(&theme, 60, false));
            assert_eq!(body.blocks[0].label, first, "{:?}", call.tool);
        }
    }
}
