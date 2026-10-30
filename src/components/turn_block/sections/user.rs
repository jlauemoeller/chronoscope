//! The user prompt section.

use ratatui::text::Line;

use super::{SectionCtx, SectionRenderer, ellipsize, fold_marker, with_lead};
use crate::markdown::render_markdown;

/// How many lines of a folded prompt are shown.
const FOLDED_LINES: usize = 2;

/// A User turn's prompt, rendered as markdown in `foreground`.
///
/// Folded, it shows the first two wrapped lines, the second ending in `…`
/// when there is more.
#[derive(Debug, Clone, Copy)]
pub struct UserPromptSection<'a>(pub &'a str);

impl SectionRenderer for UserPromptSection<'_> {
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let mut lines = render_markdown(self.0, ctx.width, ctx.foreground(), ctx.theme);
        if ctx.folded && lines.len() > FOLDED_LINES {
            lines.truncate(FOLDED_LINES);
            if let Some(last) = lines.pop() {
                lines.push(ellipsize(last, usize::from(ctx.width), true));
            }
        }
        with_lead(lines, fold_marker(ctx.folded), ctx.foreground())
    }
}
