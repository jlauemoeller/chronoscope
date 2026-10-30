//! Assistant text and the model-change note: the sections that cannot fold.

use ratatui::text::{Line, Span};

use super::{SectionCtx, SectionRenderer, with_lead};
use crate::markdown::{char_wrap, render_markdown};

/// Markdown text of the assistant, in `foreground`, indented by two spaces.
#[derive(Debug, Clone, Copy)]
pub struct AssistantTextSection<'a>(pub &'a str);

impl SectionRenderer for AssistantTextSection<'_> {
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let lines = render_markdown(self.0, ctx.width, ctx.foreground(), ctx.theme);
        with_lead(lines, "  ", ctx.foreground())
    }
}

/// The muted `model → <model>` note.
#[derive(Debug, Clone, Copy)]
pub struct ModelChangeSection<'a>(pub &'a str);

impl SectionRenderer for ModelChangeSection<'_> {
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let note = format!("model → {}", self.0);
        let lines = char_wrap(&note, ctx.width, ctx.muted());
        let lines = if lines.is_empty() {
            vec![Line::from(Span::styled(note, ctx.muted()))]
        } else {
            lines
        };
        with_lead(lines, "  ", ctx.muted())
    }
}
