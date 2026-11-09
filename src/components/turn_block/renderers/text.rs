//! Assistant text, the model-change note and notices: the sections that
//! cannot fold.

use ratatui::text::Line;

use super::{SectionCtx, SectionRenderer, with_lead, word_wrap, word_wrap_unhyphenated};
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
/// Words are not split at hyphens, and trailing whitespace is not drawn.
#[derive(Debug, Clone, Copy)]
pub struct NoticeSection<'a>(pub NoticeLevel, pub &'a str);

impl SectionRenderer for NoticeSection<'_> {
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let style = match self.0 {
            NoticeLevel::Error => ctx.theme.style(Token::ErrorOutput),
            NoticeLevel::Warning => ctx.muted(),
        };
        let text = self.1.trim_end();
        with_lead(word_wrap_unhyphenated(text, ctx.width, style), "  ", style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Role, Theme, builtin, emphasis};

    fn rows(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(ToString::to_string).collect()
    }

    fn lines_at(text: &str, theme: &Theme, width: u16) -> Vec<Line<'static>> {
        AssistantTextSection(text).lines(&SectionCtx::new(theme, width, false))
    }

    #[test]
    fn a_plain_paragraph_is_one_row_indented_by_two_spaces_in_foreground() {
        let theme = builtin::default_theme();
        let lines = lines_at("Hello there.", &theme, 40);
        assert_eq!(rows(&lines), ["  Hello there."]);
        let foreground = theme.role(Role::Foreground);
        for span in &lines[0].spans {
            assert_eq!(span.style.fg, Some(foreground), "{span:?}");
        }
    }

    #[test]
    fn text_wider_than_the_section_wraps_at_words_and_keeps_the_lead() {
        let theme = builtin::default_theme();
        let lines = lines_at("one two three four five", &theme, 10);
        assert_eq!(rows(&lines), ["  one two", "  three four", "  five"]);
        assert!(lines.iter().all(|line| line.width() <= 12));
    }

    #[test]
    fn markdown_emphasis_takes_the_styles_of_the_emphasis_module() {
        let theme = builtin::default_theme();
        let lines = lines_at("a **bold** and *slanted* ~~gone~~ word", &theme, 60);
        assert_eq!(rows(&lines), ["  a bold and slanted gone word"]);
        let style_of = |needle: &str| {
            lines[0]
                .spans
                .iter()
                .find(|span| span.content == needle)
                .map(|span| span.style)
                .unwrap_or_else(|| panic!("no span {needle:?} in {:?}", lines[0]))
        };
        let foreground = SectionCtx::new(&theme, 60, false).foreground();
        assert_eq!(style_of("bold"), emphasis::strong(foreground));
        assert_eq!(style_of("slanted"), emphasis::emphasis(foreground));
        assert_eq!(style_of("gone"), emphasis::strikethrough(foreground));
    }
}
