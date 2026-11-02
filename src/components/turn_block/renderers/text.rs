//! Assistant text, the model-change note and notices: the sections that
//! cannot fold.

use ratatui::text::Line;

use super::{SectionCtx, SectionRenderer, with_lead, word_wrap};
use crate::markdown::render_markdown;
use crate::model::NoticeLevel;
use crate::theme::Token;

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
        with_lead(word_wrap(&note, ctx.width, ctx.muted()), "  ", ctx.muted())
    }
}

/// An error or warning about how the turn ended: plain prose (not markdown),
/// `error_output` for an error and muted for a warning, indented by two spaces.
#[derive(Debug, Clone, Copy)]
pub struct NoticeSection<'a>(pub NoticeLevel, pub &'a str);

impl SectionRenderer for NoticeSection<'_> {
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let style = match self.0 {
            NoticeLevel::Error => ctx.theme.style(Token::ErrorOutput),
            NoticeLevel::Warning => ctx.muted(),
        };
        with_lead(word_wrap(self.1, ctx.width, style), "  ", style)
    }
}
