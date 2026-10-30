//! One renderer per section kind (SPEC §5.2). Each is a small pure type that
//! turns its data into styled lines for a width.
//!
//! The lines start at column 2 of the block: the first two cells of the first
//! line hold the fold marker (`▶ `/`▼ `) or two spaces, the text starts at
//! column 4 and is [`SectionCtx::width`] cells wide.

mod compaction;
mod stats;
mod system;
mod text;
mod thinking;
mod tool;
mod user;

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::section::{Section, SectionContent};
use crate::theme::{Role, Theme};

pub use compaction::CompactionSection;
pub use stats::{StatsSection, format_duration, format_stats, format_tokens};
pub use system::SystemSection;
pub use text::{AssistantTextSection, ModelChangeSection};
pub use thinking::ThinkingSection;
pub use tool::ToolCallSection;
pub use user::UserPromptSection;

/// The columns taken by the block's border, padding and marker cells on the
/// left of the text: border, padding, and the two marker cells.
pub const TEXT_COLUMN: u16 = 4;
/// The width the block leaves for the text: 4 cells on the left and 4 on the
/// right of it (3 of padding and the border).
pub const CHROME: u16 = 8;
/// The extra indent of the content below a label (column 7).
pub const BODY_INDENT: u16 = 3;

/// The text width of a block of width `block_width`.
pub fn text_width(block_width: u16) -> u16 {
    block_width.saturating_sub(CHROME).max(1)
}

/// What a renderer needs to know besides its data.
#[derive(Debug, Clone, Copy)]
pub struct SectionCtx<'a> {
    /// The theme every style comes from.
    pub theme: &'a Theme,
    /// The text width (`W − 8`).
    pub width: u16,
    /// Whether the section is folded. Ignored by sections that cannot fold.
    pub folded: bool,
}

impl SectionCtx<'_> {
    /// The width of the content below a label (`W − 11`).
    pub fn body_width(&self) -> u16 {
        self.width.saturating_sub(BODY_INDENT).max(1)
    }

    /// The `muted_foreground` style.
    pub fn muted(&self) -> Style {
        Style::new().fg(self.theme.role(Role::MutedForeground))
    }

    /// The `foreground` style.
    pub fn foreground(&self) -> Style {
        Style::new().fg(self.theme.role(Role::Foreground))
    }
}

/// Renders one kind of section.
pub trait SectionRenderer {
    /// The lines of the section, from column 2, for `ctx`.
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>>;
}

/// The lines of `section`, folded or not, at `width` text cells.
pub fn render_section(
    section: &Section<'_>,
    theme: &Theme,
    width: u16,
    folded: bool,
) -> Vec<Line<'static>> {
    let ctx = SectionCtx {
        theme,
        width,
        folded,
    };
    match section.content {
        SectionContent::Prompt(text) => UserPromptSection(text).lines(&ctx),
        SectionContent::Text(text) => AssistantTextSection(text).lines(&ctx),
        SectionContent::Thinking(text) => ThinkingSection(text).lines(&ctx),
        SectionContent::Tool(call) => ToolCallSection(call).lines(&ctx),
        SectionContent::ModelChange(model) => ModelChangeSection(model).lines(&ctx),
        SectionContent::Stats(stats) => StatsSection(stats).lines(&ctx),
        SectionContent::Compaction(summary) => CompactionSection(summary).lines(&ctx),
        SectionContent::System(entries) => SystemSection(entries).lines(&ctx),
    }
}

/// The marker of a foldable section.
pub(crate) fn fold_marker(folded: bool) -> &'static str {
    if folded { "▶ " } else { "▼ " }
}

/// Puts `first` (a fold marker or two spaces) in front of the first line and
/// two spaces in front of the others; blank lines stay blank.
pub(crate) fn with_lead(
    lines: Vec<Line<'static>>,
    first: &str,
    style: Style,
) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            if i > 0 && line.spans.is_empty() {
                return line;
            }
            let lead = if i == 0 { first } else { "  " };
            let mut spans = vec![Span::styled(lead.to_owned(), style)];
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect()
}

/// Indents every non-blank line by `n` spaces.
pub(crate) fn indent(lines: Vec<Line<'static>>, n: u16, style: Style) -> Vec<Line<'static>> {
    let pad = " ".repeat(usize::from(n));
    lines
        .into_iter()
        .map(|line| {
            if line.spans.is_empty() {
                return line;
            }
            let mut spans = vec![Span::styled(pad.clone(), style)];
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect()
}

/// A label line (`Input:`, `Result:`).
pub(crate) fn label(text: &str, style: Style) -> Line<'static> {
    Line::from(Span::styled(text.to_owned(), style))
}

/// Cuts `line` to at most `width` cells, ending in `…` where it was cut. With
/// `always`, the `…` is added even to a line that fits (when there is room).
pub(crate) fn ellipsize(line: Line<'static>, width: usize, always: bool) -> Line<'static> {
    let full = line.width();
    if full <= width && !always {
        return line;
    }
    let keep = if full < width {
        full
    } else {
        width.saturating_sub(1)
    };
    let style = line.spans.last().map_or_else(Style::default, |s| s.style);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    'outer: for span in line.spans {
        let mut text = String::new();
        for g in span.content.graphemes(true) {
            if used + g.width() > keep {
                if !text.is_empty() {
                    spans.push(Span::styled(text, span.style));
                }
                break 'outer;
            }
            used += g.width();
            text.push_str(g);
        }
        spans.push(Span::styled(text, span.style));
    }
    spans.push(Span::styled("…", style));
    Line::from(spans)
}

/// The first non-blank line of `text`, trimmed, with control characters
/// replaced by spaces.
pub(crate) fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipsize_keeps_a_line_that_fits() {
        let line = Line::from("abc");
        assert_eq!(ellipsize(line, 3, false).to_string(), "abc");
    }

    #[test]
    fn ellipsize_cuts_to_the_width_including_the_ellipsis() {
        assert_eq!(
            ellipsize(Line::from("abcdef"), 4, false).to_string(),
            "abc…"
        );
    }

    #[test]
    fn ellipsize_always_appends_when_there_is_room() {
        assert_eq!(ellipsize(Line::from("abc"), 4, true).to_string(), "abc…");
        assert_eq!(ellipsize(Line::from("abcd"), 4, true).to_string(), "abc…");
    }

    #[test]
    fn ellipsize_never_splits_a_wide_character() {
        let cut = ellipsize(Line::from("日本語"), 4, false);
        assert_eq!(cut.to_string(), "日…");
        assert!(cut.width() <= 4);
    }

    #[test]
    fn ellipsize_spans_keep_their_own_styles() {
        let line = Line::from(vec![
            Span::styled("ab", Style::new().fg(ratatui::style::Color::Red)),
            Span::styled("cdef", Style::new().fg(ratatui::style::Color::Blue)),
        ]);
        let cut = ellipsize(line, 5, false);
        assert_eq!(cut.to_string(), "abcd…");
        assert_eq!(cut.spans[1].style.fg, Some(ratatui::style::Color::Blue));
    }

    #[test]
    fn first_line_skips_blank_lines_and_trims() {
        assert_eq!(first_line("\n  \n  hello \nworld"), "hello");
        assert_eq!(first_line(""), "");
    }
}
