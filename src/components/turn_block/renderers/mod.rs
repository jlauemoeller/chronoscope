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
pub use stats::{StatsSection, format_stats, format_tokens};
pub use system::SystemSection;
pub use text::{AssistantTextSection, ModelChangeSection, NoticeSection};
pub use thinking::ThinkingSection;
pub use tool::{Block, Body, PENDING, PlainBlock, PlainTool, ToolBodyRenderer, ToolCallSection};
pub use user::UserPromptSection;

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
    /// `truncate_output_lines`: tool output longer than this many lines is cut.
    pub truncate: usize,
    /// Whether the section's truncated output was expanded.
    pub expanded: bool,
    /// The label of the key that opens a subagent transcript.
    pub open_key: &'a str,
    /// Whether tool text is kept exactly as it is: one row per line of the
    /// text, with no wrapping, no tab expansion and no dropped characters.
    /// Used for plain text that must be complete (the full copy).
    pub verbatim: bool,
}

/// The label of the default `open_subagent` key.
pub const DEFAULT_OPEN_KEY: &str = "a";

impl<'a> SectionCtx<'a> {
    /// A context with the default truncation, nothing expanded and the
    /// default `open_subagent` key.
    pub fn new(theme: &'a Theme, width: u16, folded: bool) -> Self {
        Self {
            theme,
            width,
            folded,
            truncate: crate::config::DEFAULT_TRUNCATE_OUTPUT_LINES,
            expanded: false,
            open_key: DEFAULT_OPEN_KEY,
            verbatim: false,
        }
    }
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

/// The lines of a section, with the `… N more lines` row of a truncated tool
/// call kept apart: it is a selection stop of its own.
#[derive(Debug, Clone, Default)]
pub struct Rendered {
    /// The lines of the section itself.
    pub lines: Vec<Line<'static>>,
    /// The truncation row, to be drawn directly below `lines`.
    pub more: Option<Line<'static>>,
}

/// The lines of `section`, folded or not, for `ctx`.
pub fn render_section(section: &Section<'_>, ctx: &SectionCtx<'_>) -> Rendered {
    let lines = match section.content {
        SectionContent::Prompt(text) => UserPromptSection(text).lines(ctx),
        SectionContent::Text(text) => AssistantTextSection(text).lines(ctx),
        SectionContent::Thinking(text) => ThinkingSection(text).lines(ctx),
        SectionContent::Tool(call) => {
            return ToolCallSection(call).render(ctx);
        }
        SectionContent::ModelChange(model) => ModelChangeSection(model).lines(ctx),
        SectionContent::Notice(level, text) => NoticeSection(level, text).lines(ctx),
        SectionContent::Stats(stats) => StatsSection(stats).lines(ctx),
        SectionContent::Compaction(summary) => CompactionSection(summary).lines(ctx),
        SectionContent::System(entries) => SystemSection(entries).lines(ctx),
    };
    Rendered { lines, more: None }
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

/// One row per line of `text`, as it is: nothing is wrapped, tabs and control
/// characters stay. Lines end at `\n`, so a `\r\n` ending becomes a plain one.
pub(crate) fn verbatim_lines(text: &str, style: Style) -> Vec<Line<'static>> {
    text.lines()
        .map(|line| Line::from(Span::styled(line.to_owned(), style)))
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

/// Word-wraps prose `text` at `width` cells; a word longer than a line is
/// broken. Control characters become spaces, and a blank `text` gives one
/// empty line.
pub(crate) fn word_wrap(text: &str, width: u16, style: Style) -> Vec<Line<'static>> {
    wrap_with(
        text,
        textwrap::Options::new(usize::from(width.max(1))),
        style,
    )
}

/// Like [`word_wrap`], but a word is never split at a hyphen, so that a
/// searched phrase such as `non-empty` stays on one row.
pub(crate) fn word_wrap_unhyphenated(text: &str, width: u16, style: Style) -> Vec<Line<'static>> {
    let options = textwrap::Options::new(usize::from(width.max(1)))
        .word_splitter(textwrap::WordSplitter::NoHyphenation);
    wrap_with(text, options, style)
}

fn wrap_with(text: &str, options: textwrap::Options<'_>, style: Style) -> Vec<Line<'static>> {
    let text = without_controls(text);
    textwrap::wrap(&text, options)
        .into_iter()
        .map(|line| Line::from(Span::styled(line.into_owned(), style)))
        .collect()
}

/// `text` with every control character other than a line feed replaced by a
/// space, as the rows draw it.
pub(crate) fn without_controls(text: &str) -> String {
    text.chars()
        .map(|c| if c != '\n' && c.is_control() { ' ' } else { c })
        .collect()
}

/// Whether a rendered line has nothing to show.
pub(crate) fn is_blank(line: &Line<'_>) -> bool {
    line.spans.iter().all(|s| s.content.trim().is_empty())
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
