//! The Read body: `Path:` and the numbered `Content:`.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::{
    Block, Body, SectionCtx, ToolBodyRenderer, compact_json, limit, muted_block, path_of,
    result_block, text_block, unavailable_block,
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

/// The `(lines a–b)` note for the input's `offset` / `limit`. `last` is the
/// last line number the result holds, when it is known: the range does not
/// run past it. Nothing is said about a plain read from line 1.
fn range_note(input: &Value, last: Option<usize>) -> Option<String> {
    let number = |key: &str| input.get(key).and_then(Value::as_u64).filter(|n| *n > 0);
    let (offset, limit) = (number("offset"), number("limit"));
    if limit.is_none() && offset.is_none_or(|a| a == 1) {
        return None;
    }
    let start = offset.unwrap_or(1);
    let requested = limit.map(|n| start.saturating_add(n).saturating_sub(1));
    let end = match (requested, last.and_then(|l| u64::try_from(l).ok())) {
        (Some(r), Some(l)) => Some(r.min(l)),
        (r, l) => r.or(l),
    }
    // Numbers that disagree with the input must not give a reversed range.
    .filter(|end| *end >= start);
    Some(match end {
        Some(end) => format!(" (lines {start}–{end})"),
        None => format!(" (from line {start})"),
    })
}

/// `   12→text`, `12<TAB>text` or OpenCode's `12: text` (an empty line is
/// `12:`) as the number and the text.
fn parse_numbered(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start_matches(' ');
    let digits = trimmed.find(|c: char| !c.is_ascii_digit())?;
    if digits == 0 {
        return None;
    }
    let rest = &trimmed[digits..];
    let text = rest
        .strip_prefix('→')
        .or_else(|| rest.strip_prefix('\t'))
        .or_else(|| rest.strip_prefix(": "))
        .or_else(|| (rest == ":").then_some(""))?;
    Some((trimmed[..digits].parse().ok()?, text))
}

/// Whether `line` is the framing line OpenCode starts a read result with:
/// `Read file <path>, lines 1-127` or `Read directory <path>, entries 1-3`.
fn is_opencode_header(line: &str) -> bool {
    line.starts_with("Read file ") || line.starts_with("Read directory ")
}

/// The lines of `text` with their numbers. OpenCode's leading
/// `Read file ...` header line is not file content and is left out.
fn numbered(text: &str, offset: usize) -> Vec<(Option<usize>, &str)> {
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.first().is_some_and(|l| is_opencode_header(l)) {
        lines.remove(0);
    }
    // The first non-blank line decides whether the output carries numbers.
    let first = lines.iter().find(|l| !l.trim().is_empty());
    if first.and_then(|l| parse_numbered(l)).is_some() {
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
        .map(|(i, line)| (Some(offset.saturating_add(i)), line))
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
        let mut wrapped = if ctx.verbatim {
            super::super::verbatim_lines(text, text_style)
        } else {
            char_wrap(
                text,
                u16::try_from(text_width).unwrap_or(u16::MAX),
                text_style,
            )
        };
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

/// The `Path:` block: the path (or the input as JSON when it has none) and
/// the range note for `last`.
fn path_block(call: &ToolCall, last: Option<usize>, ctx: &SectionCtx<'_>) -> Block {
    let path = match path_of(&call.input) {
        Some(path) => path.to_owned(),
        None => compact_json(&call.input),
    };
    let path = format!(
        "{path}{}",
        range_note(&call.input, last).unwrap_or_default()
    );
    text_block("Path:", &path, ctx.theme.style(Token::Command), ctx)
}

/// The line the input's `offset` starts at: 1 when it is absent or 0.
fn first_line(input: &Value) -> usize {
    input
        .get("offset")
        .and_then(Value::as_u64)
        .filter(|n| *n > 0)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(1)
}

/// The body of a read whose result is `text`, which is not blank: `Path:`
/// with the range up to the last line shown, and the numbered `Content:`
/// block cut at the limit.
fn numbered_body(call: &ToolCall, text: &str, ctx: &SectionCtx<'_>) -> Body {
    let all = numbered(text, first_line(&call.input));
    let last = all.iter().rev().find_map(|(n, _)| *n);
    let max = limit(ctx);
    let cut = all.len().saturating_sub(max);
    let rows = gutter_rows(
        &all[..all.len().min(max)],
        ctx,
        ctx.muted(),
        ctx.theme.style(Token::Output),
    );
    let content = Block {
        label: "Content:",
        lines: rows,
    };
    Body {
        blocks: vec![path_block(call, last, ctx), content],
        remaining: (cut > 0).then_some(cut),
    }
}

/// The body of a read with no file content to number: `Path:` and the
/// `Content:` block `content`, with `remaining` lines cut from it.
fn plain_body(
    call: &ToolCall,
    content: Block,
    remaining: Option<usize>,
    ctx: &SectionCtx<'_>,
) -> Body {
    Body {
        blocks: vec![path_block(call, None, ctx), content],
        remaining,
    }
}

impl ToolBodyRenderer for ReadBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        // Only a finished, successful read has file content to number. A
        // pending or failed one reads like any other result.
        let readable = call
            .result
            .as_ref()
            .filter(|r| call.status == ToolStatus::Ok && !r.is_error);
        let Some(result) = readable else {
            let (content, remaining) = result_block("Content:", call, ctx);
            return plain_body(call, content, remaining, ctx);
        };
        // The output is read once; its error or emptiness is shown as such.
        match result.output.text() {
            Ok(text) if !text.trim().is_empty() => numbered_body(call, &text, ctx),
            Ok(_) => {
                let empty = muted_block("Content:", "(no output)", ctx);
                plain_body(call, empty, None, ctx)
            }
            Err(err) => {
                let unavailable = unavailable_block("Content:", call, &err, ctx);
                plain_body(call, unavailable, None, ctx)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::fixtures::*;
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
    fn opencode_numbering_and_header_become_a_plain_gutter() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"path": "/src/main.rs"}),
            Some("Read file /src/main.rs, lines 1-3\n1: fn main() {\n2: \n3:     run();\n"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(
            &rows[6..],
            [
                "  Content:",
                "",
                "     1  fn main() {",
                "     2  ",
                "     3      run();"
            ]
        );
        let text = rows.join("\n");
        assert!(!text.contains("Read file"), "{text}");
        assert!(!text.contains("1:"), "{text}");
    }

    #[test]
    fn opencode_numbering_keeps_the_numbers_of_a_later_range() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"path": "/a", "offset": 40, "limit": 2}),
            Some("Read file /a, lines 40-41\n40: x\n41: y"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(rows[4], "     /a (lines 40–41)");
        assert_eq!(&rows[8..], ["     40  x", "     41  y"]);
    }

    #[test]
    fn an_opencode_directory_listing_loses_its_header_line() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"path": "/p"}),
            Some("Read directory /p, entries 1-2\n.opencode/\nfactory.md"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(&rows[8..], ["     1  .opencode/", "     2  factory.md"]);
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
        assert_eq!(rows[4], "     /a.txt (lines 98–100)");
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

    #[test]
    fn huge_offsets_and_limits_do_not_overflow() {
        let theme = theme();
        let both = call(
            ToolKind::Read,
            json!({"file_path": "/a", "offset": u64::MAX, "limit": u64::MAX}),
            Some("x\ny"),
        );
        let rows = section(&both, &theme, 60);
        assert!(rows[4].starts_with("     /a (from line "), "{rows:?}");
        let only_offset = call(
            ToolKind::Read,
            json!({"file_path": "/a", "offset": u64::MAX}),
            Some("x\ny"),
        );
        assert_eq!(section(&only_offset, &theme, 60).len(), 10);
    }

    #[test]
    fn the_range_does_not_run_past_the_lines_returned() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"file_path": "/a", "limit": 2000}),
            Some("     1→a\n     2→b\n     3→c"),
        );
        assert_eq!(section(&call, &theme, 60)[4], "     /a (lines 1–3)");
    }

    #[test]
    fn a_read_from_line_one_without_a_limit_has_no_note() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"file_path": "/a", "offset": 1}),
            Some("x"),
        );
        assert_eq!(section(&call, &theme, 60)[4], "     /a");
    }

    #[test]
    fn numbering_is_detected_past_a_leading_unnumbered_line() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"file_path": "/a"}),
            Some("\n     5→x\n     6→y"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(&rows[8..], ["", "     5  x", "     6  y"]);
    }

    #[test]
    fn an_unreadable_output_is_read_once_and_reported() {
        use crate::model::{ByteSource, LineFormat, OutputSource, ToolOutput, ToolResult};
        let theme = theme();
        let mut call = call(ToolKind::Read, json!({"file_path": "/a"}), Some("x"));
        call.result = Some(ToolResult {
            output: ToolOutput::lazy(
                1,
                9,
                OutputSource {
                    bytes: ByteSource::Memory(std::sync::Arc::from(&b"{}"[..])),
                    range: 0..2,
                    format: LineFormat::Claude,
                    call_id: "c1".into(),
                },
            ),
            is_error: false,
        });
        let before = crate::model::text_calls();
        let body = body(&call, &theme, 60);
        assert_eq!(crate::model::text_calls() - before, 1);
        let line = &body.blocks[1].lines[0];
        assert!(line.to_string().starts_with("(output unavailable:"));
        assert_eq!(
            line.spans[0].style.fg,
            Some(color(&theme, Role::Destructive))
        );
    }

    #[test]
    fn numbered_body_numbers_from_the_offset_and_notes_the_last_line_shown() {
        let theme = theme();
        let call = call(
            ToolKind::Read,
            json!({"file_path": "/a", "offset": 9}),
            None,
        );
        let ctx = SectionCtx::new(&theme, 52, false);
        let body = numbered_body(&call, "x\ny", &ctx);
        assert_eq!(body.remaining, None);
        assert_eq!(body.blocks[0].label, "Path:");
        assert_eq!(texts(&body.blocks[0].lines), ["/a (lines 9–10)"]);
        assert_eq!(body.blocks[1].label, "Content:");
        assert_eq!(texts(&body.blocks[1].lines), [" 9  x", "10  y"]);
        let gutter = &body.blocks[1].lines[0].spans[0];
        assert_eq!(gutter.style.fg, Some(color(&theme, Role::MutedForeground)));
    }

    #[test]
    fn numbered_body_cuts_the_content_at_the_limit() {
        let theme = theme();
        let call = call(ToolKind::Read, json!({"file_path": "/a"}), None);
        let mut ctx = SectionCtx::new(&theme, 52, false);
        ctx.truncate = 2;
        let body = numbered_body(&call, "a\nb\nc\nd", &ctx);
        assert_eq!(texts(&body.blocks[1].lines), ["1  a", "2  b"]);
        assert_eq!(body.remaining, Some(2));
        // A plain read from line 1 has no range note.
        assert_eq!(texts(&body.blocks[0].lines), ["/a"]);
    }

    #[test]
    fn range_note_never_gives_a_reversed_range() {
        let input = json!({"offset": 98, "limit": 5});
        // Claude numbers that disagree with the offset.
        assert_eq!(range_note(&input, Some(3)), Some(" (from line 98)".into()));
        assert_eq!(
            range_note(&input, Some(200)),
            Some(" (lines 98–102)".into())
        );
        let huge = json!({"offset": u64::MAX, "limit": u64::MAX});
        let note = range_note(&huge, Some(3)).unwrap();
        assert!(note.starts_with(" (from line"), "{note}");
        let only_offset = json!({"offset": 98});
        assert_eq!(
            range_note(&only_offset, Some(3)),
            Some(" (from line 98)".into())
        );
    }
}
