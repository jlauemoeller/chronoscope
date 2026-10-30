//! The thinking section (readable thinking only; redacted has no section).

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::{SectionCtx, SectionRenderer, ellipsize, first_line, fold_marker, with_lead};
use crate::markdown::render_markdown;
use crate::theme::Token;

/// Readable thinking, in the `thinking` token.
///
/// Folded: `▶ Thinking: <first line>` on one row. Unfolded: `▼ Thinking`,
/// then the markdown text.
#[derive(Debug, Clone, Copy)]
pub struct ThinkingSection<'a>(pub &'a str);

impl SectionRenderer for ThinkingSection<'_> {
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let style = ctx.theme.style(Token::Thinking);
        if ctx.folded {
            let header = header(&format!("Thinking: {}", self.first_line(ctx)), style);
            let line = ellipsize(header, usize::from(ctx.width), false);
            return with_lead(vec![line], fold_marker(true), style);
        }
        let mut lines = vec![header("Thinking", style)];
        lines.extend(render_markdown(self.0, ctx.width, style, ctx.theme));
        with_lead(lines, fold_marker(false), style)
    }
}

/// The width the first paragraph is rendered at before it is cut to a row.
const FIRST_LINE_WIDTH: u16 = 512;

impl ThinkingSection<'_> {
    /// The first line of the rendered markdown, as plain text: `**Plan**`
    /// reads `Plan`. Only the first paragraph is rendered.
    fn first_line(&self, ctx: &SectionCtx<'_>) -> String {
        let start = self.0.trim_start();
        let paragraph = start.split("\n\n").next().unwrap_or(start);
        render_markdown(paragraph, FIRST_LINE_WIDTH, Style::new(), ctx.theme)
            .iter()
            .map(ToString::to_string)
            .find(|line| !line.trim().is_empty())
            .map(|line| first_line(&line))
            .unwrap_or_default()
    }
}

fn header(text: &str, style: Style) -> Line<'static> {
    Line::from(Span::styled(text.trim_end().to_owned(), style))
}
