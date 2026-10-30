//! The compaction section.

use ratatui::text::{Line, Span};

use super::{SectionCtx, SectionRenderer, fold_marker, with_lead};
use crate::markdown::render_markdown;

/// The divider shown for a compaction.
pub const DIVIDER: &str = "── context compacted ──";

/// A compaction, in `muted_foreground`.
///
/// Folded: the divider. Unfolded: the divider, then the summary as markdown.
#[derive(Debug, Clone, Copy)]
pub struct CompactionSection<'a>(pub &'a str);

impl SectionRenderer for CompactionSection<'_> {
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let style = ctx.muted();
        let mut lines = vec![Line::from(Span::styled(DIVIDER, style))];
        if !ctx.folded {
            lines.extend(render_markdown(self.0, ctx.width, style, ctx.theme));
        }
        with_lead(lines, fold_marker(ctx.folded), style)
    }
}
