//! Char wrapping, for code, commands and tool output.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const TAB: &str = "    ";

/// Wraps `text` at exactly `width` display cells, wherever that falls.
///
/// Spaces are not special, so paths and long tokens are never hidden.
/// Existing newlines start new lines, and blank lines and leading spaces are
/// kept. Tabs become four spaces. A trailing newline does not add a line, and
/// empty `text` gives no lines. Wide characters count 2 cells; one that does
/// not fit in what is left of a line moves to the next. A grapheme is never
/// split, and one wider than `width` gets a line to itself. Control
/// characters (such as a lone `\r`, or the `ESC` of an ANSI sequence) are
/// dropped; escape sequences are not interpreted, so strip them first if the
/// text may hold any. A `width` of 0 gives no lines, as in `render_markdown`.
pub fn char_wrap(text: &str, width: u16, style: Style) -> Vec<Line<'static>> {
    if width == 0 {
        return Vec::new();
    }
    text.lines()
        .flat_map(|line| wrap_spans(&[Span::styled(line.to_string(), style)], usize::from(width)))
        .map(Line::from)
        .collect()
}

/// Appends `grapheme` to `line`, merging it into the last span when the style
/// matches.
fn push(line: &mut Vec<Span<'static>>, grapheme: &str, style: Style) {
    match line.last_mut() {
        Some(last) if last.style == style => last.content.to_mut().push_str(grapheme),
        _ => line.push(Span::styled(grapheme.to_string(), style)),
    }
}

/// Char-wraps one line of styled spans (which has no newline) at `width`
/// cells. Always returns at least one, possibly empty, line.
pub(super) fn wrap_spans(spans: &[Span<'static>], width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    let mut used = 0;
    for span in spans {
        let text = span.content.replace('\t', TAB);
        for g in text.graphemes(true) {
            if g.chars().all(char::is_control) {
                continue;
            }
            let w = g.width();
            if used + w > width && used > 0 {
                lines.push(Vec::new());
                used = 0;
            }
            if let Some(line) = lines.last_mut() {
                push(line, g, span.style);
            }
            used += w;
        }
    }
    lines
}
