//! The Read body: `Path:` and the numbered `Content:`.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::{
    Block, Body, SectionCtx, ToolBodyRenderer, compact_json, limit, path_of, result_block,
    text_block, truncate_lines,
};
use crate::markdown::char_wrap;
use crate::model::{ToolCall, ToolStatus};
use crate::theme::Token;

/// `Path:` with the path (and the line range when the input has an offset or
/// limit), then `Content:` with the file in a line-number gutter.
///
/// The gutter is right-aligned and as wide as the largest number shown; a
/// number is in `muted_foreground`, followed by two spaces and the text in
/// `output`. If the result already carries line numbers (Claude's
/// `     1→text`, or `1<TAB>text`), they are parsed into the gutter instead
/// of numbering twice. Lines that follow numbered ones without a number
/// themselves (a `<system-reminder>` trailer, say) get an empty gutter.
/// Plain output is numbered from the input's `offset` (1 when absent).
#[derive(Debug, Clone, Copy)]
pub struct ReadBody;

/// The `(lines a–b)` note for the input's `offset` / `limit`.
fn range_note(input: &Value) -> Option<String> {
    let number = |key: &str| input.get(key).and_then(Value::as_u64).filter(|n| *n > 0);
    let offset = number("offset");
    let limit = number("limit");
    match (offset, limit) {
        (None, None) => None,
        (Some(a), Some(n)) => Some(format!(" (lines {a}–{})", a + n - 1)),
        (None, Some(n)) => Some(format!(" (lines 1–{n})")),
        (Some(a), None) => Some(format!(" (from line {a})")),
    }
}

/// `   12→text` or `12<TAB>text` as the number and the text.
fn parse_numbered(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start_matches(' ');
    let digits = trimmed.find(|c: char| !c.is_ascii_digit())?;
    if digits == 0 {
        return None;
    }
    let rest = &trimmed[digits..];
    let text = rest.strip_prefix('→').or_else(|| rest.strip_prefix('\t'))?;
    Some((trimmed[..digits].parse().ok()?, text))
}

/// The lines of `text` with their numbers.
fn numbered(text: &str, offset: usize) -> Vec<(Option<usize>, &str)> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.first().and_then(|l| parse_numbered(l)).is_some() {
        return lines
            .into_iter()
            .map(|line| match parse_numbered(line) {
                Some((n, text)) => (Some(n), text),
                None => (None, line),
            })
            .collect();
    }
    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| (Some(offset + i), line))
        .collect()
}

fn gutter_rows(
    lines: &[(Option<usize>, &str)],
    ctx: &SectionCtx<'_>,
    number_style: Style,
    text_style: Style,
) -> Vec<Line<'static>> {
    let digits = lines
        .iter()
        .filter_map(|(n, _)| *n)
        .max()
        .map_or(1, |max| max.to_string().len());
    let gutter = digits + 2;
    let text_width = usize::from(ctx.body_width()).saturating_sub(gutter).max(1);
    let mut rows = Vec::new();
    for (number, text) in lines {
        let mut wrapped = char_wrap(
            text,
            u16::try_from(text_width).unwrap_or(u16::MAX),
            text_style,
        );
        if wrapped.is_empty() {
            wrapped.push(Line::default());
        }
        for (i, line) in wrapped.into_iter().enumerate() {
            if number.is_none() && line.spans.is_empty() {
                rows.push(Line::default());
                continue;
            }
            let lead = match (i, number) {
                (0, Some(n)) => format!("{n:>digits$}  "),
                _ => " ".repeat(gutter),
            };
            let mut spans = vec![Span::styled(lead, number_style)];
            spans.extend(line.spans);
            rows.push(Line::from(spans));
        }
    }
    rows
}

impl ToolBodyRenderer for ReadBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        let command = ctx.theme.style(Token::Command);
        let mut path = match path_of(&call.input) {
            Some(path) => path.to_owned(),
            None => compact_json(&call.input),
        };
        path.push_str(&range_note(&call.input).unwrap_or_default());
        let path_block = text_block("Path:", &path, command, ctx);

        // Only a finished, successful read has file content to number. A
        // pending, failed, empty or unreadable result reads like any other.
        let text = call
            .result
            .as_ref()
            .filter(|r| call.status == ToolStatus::Ok && !r.is_error)
            .and_then(|r| r.output.text().ok())
            .filter(|text| !text.trim().is_empty());
        let Some(text) = text else {
            let (block, remaining) = result_block("Content:", call, ctx);
            return Body {
                blocks: vec![path_block, block],
                remaining,
                hint: None,
            };
        };
        let offset = call
            .input
            .get("offset")
            .and_then(Value::as_u64)
            .filter(|n| *n > 0)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(1);
        let (shown, cut) = truncate_lines(&text, limit(ctx));
        let lines = numbered(shown, offset);
        let rows = gutter_rows(&lines, ctx, ctx.muted(), ctx.theme.style(Token::Output));
        Body {
            blocks: vec![
                path_block,
                Block {
                    label: "Content:",
                    lines: rows,
                },
            ],
            remaining: (cut > 0).then_some(cut),
            hint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::testing::*;
    use super::super::{SectionRenderer, ToolCallSection, ToolKind};
    use super::*;
    use crate::theme::Role;

    #[test]
    fn claude_numbering_is_parsed_into_the_gutter_once() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"file_path": "/src/main.rs", "offset": 8, "limit": 3}),
            Some("     8→fn main() {\n     9→    run();\n    10→}\n"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(
            rows,
            [
                "▼ Do the thing",
                "",
                "  Path:",
                "",
                "     /src/main.rs (lines 8–10)",
                "",
                "  Content:",
                "",
                "      8  fn main() {",
                "      9      run();",
                "     10  }",
            ]
        );
        for row in &rows {
            assert_eq!(row.matches('→').count(), 0);
        }
    }

    #[test]
    fn tab_separated_numbering_is_parsed_too() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"file_path": "/a"}),
            Some("1\tone\n2\ttwo"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(&rows[8..], ["     1  one", "     2  two"]);
    }

    #[test]
    fn plain_output_is_numbered_from_the_input_offset() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"file_path": "/a.txt", "offset": 98}),
            Some("alpha\nbeta\ngamma"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(rows[4], "     /a.txt (from line 98)");
        assert_eq!(
            &rows[8..],
            ["      98  alpha", "      99  beta", "     100  gamma"]
        );
    }

    #[test]
    fn plain_output_without_an_offset_starts_at_one() {
        let theme = theme();
        let call = call(ToolKind::Read, json!({"file_path": "/a"}), Some("x\ny"));
        let rows = section(&call, &theme, 60);
        assert_eq!(rows[4], "     /a");
        assert_eq!(&rows[8..], ["     1  x", "     2  y"]);
    }

    #[test]
    fn unnumbered_trailer_lines_get_an_empty_gutter() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"file_path": "/a"}),
            Some("     1→x\n\n<system-reminder>careful</system-reminder>"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(
            &rows[8..],
            [
                "     1  x",
                "",
                "        <system-reminder>careful</system-reminder>"
            ]
        );
    }

    #[test]
    fn the_gutter_is_styled_and_long_lines_wrap_under_the_text() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"file_path": "/a"}),
            Some(&format!("     1→{}", "w".repeat(30))),
        );
        let body = body(&call, &theme, 20);
        let content = &body.blocks[1];
        assert_eq!(content.label, "Content:");
        let first = &content.lines[0];
        assert_eq!(first.spans[0].content, "1  ");
        assert_eq!(
            first.spans[0].style.fg,
            Some(color(&theme, Role::MutedForeground))
        );
        assert_eq!(
            first.spans[1].style.fg,
            Some(color(&theme, Role::Secondary))
        );
        // Body width 20 - 3 = 17; gutter 3; 14 cells of text per row.
        assert_eq!(content.lines.len(), 3);
        assert_eq!(
            content.lines[1].to_string(),
            format!("   {}", "w".repeat(14))
        );
        assert!(content.lines.iter().all(|l| l.width() <= 17));
    }

    #[test]
    fn labels_and_path_token() {
        let theme = theme();
        let call = call(ToolKind::Read, json!({"file_path": "/a"}), Some("x"));
        let ctx = SectionCtx::new(&theme, 60, false);
        let lines = ToolCallSection(&call).lines(&ctx);
        assert_eq!(
            fg_of(&lines[2], "Path:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(fg_of(&lines[4], "/a"), Some(color(&theme, Role::Accent)));
        assert_eq!(
            fg_of(&lines[6], "Content:"),
            Some(color(&theme, Role::Warning))
        );
    }

    #[test]
    fn a_failed_read_shows_the_error_without_a_gutter() {
        let theme = theme();
        let mut call = call(
            ToolKind::Read,
            json!({"file_path": "/nope"}),
            Some("no such file"),
        );
        call.status = crate::model::ToolStatus::Error;
        let body = body(&call, &theme, 60);
        let line = &body.blocks[1].lines[0];
        assert_eq!(line.to_string(), "no such file");
        assert_eq!(
            line.spans[0].style.fg,
            Some(color(&theme, Role::Destructive))
        );
    }

    #[test]
    fn only_the_first_lines_are_numbered_when_truncated() {
        let theme = theme();
        let text: String = (1..=10).map(|n| format!("{n:>6}→l{n}\n")).collect();
        let call = call(ToolKind::Read, json!({"file_path": "/a"}), Some(&text));
        let ctx = SectionCtx {
            truncate: 4,
            ..SectionCtx::new(&theme, 60, false)
        };
        let body = ReadBody.body(&call, &ctx);
        assert_eq!(body.blocks[1].lines.len(), 4);
        assert_eq!(body.remaining, Some(6));
    }
}
