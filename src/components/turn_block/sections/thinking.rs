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
            let header = header(&format!("Thinking: {}", first_line(self.0)), style);
            let line = ellipsize(header, usize::from(ctx.width), false);
            return with_lead(vec![line], fold_marker(true), style);
        }
        let mut lines = vec![header("Thinking", style)];
        lines.extend(render_markdown(self.0, ctx.width, style, ctx.theme));
        with_lead(lines, fold_marker(false), style)
    }
}

fn header(text: &str, style: Style) -> Line<'static> {
    Line::from(Span::styled(text.trim_end().to_owned(), style))
}
