//! The Write body: `Path:` and the highlighted `Content:`.

use ratatui::text::Line;

use super::{
    Block, Body, SectionCtx, ToolBodyRenderer, compact_json, limit, muted_block, path_of,
    text_block, truncate_lines, with_status_result,
};
use crate::markdown::{highlight_code, wrap_styled_lines};
use crate::model::ToolCall;
use crate::theme::Token;

/// `Path:` with the path, then `Content:` with the file content highlighted by
/// the path's extension (`syntax.*` tokens) and char-wrapped. A failed or
/// pending call also shows its `Result:` (error text in `error_output`, or
/// `(pending)`).
#[derive(Debug, Clone, Copy)]
pub struct WriteBody;

impl ToolBodyRenderer for WriteBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        let path = path_of(&call.input);
        let shown_path = path.map_or_else(|| compact_json(&call.input), str::to_owned);
        let path_block = text_block("Path:", &shown_path, ctx.theme.style(Token::Command), ctx);
        let content = call.input.get("content").and_then(|c| c.as_str());
        let Some(content) = content.filter(|c| !c.is_empty()) else {
            let body = Body {
                blocks: vec![path_block, muted_block("Content:", "(no content)", ctx)],
                ..Body::default()
            };
            return with_status_result(body, call, ctx, false);
        };
        let extension = path
            .and_then(|p| std::path::Path::new(p).extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");
        // Whether the content is cut at the configured limit, expanded or not.
        let content_cut = truncate_lines(content, ctx.truncate).1 > 0;
        let (shown, cut) = truncate_lines(content, limit(ctx));
        let mut lines: Vec<Line<'static>> = if ctx.verbatim {
            super::super::verbatim_lines(shown, ctx.theme.style(Token::Output))
        } else {
            let highlighted = highlight_code(shown, extension, ctx.theme);
            wrap_styled_lines(&highlighted, ctx.body_width())
        };
        if lines.is_empty() {
            lines.push(Line::default());
        }
        let body = Body {
            blocks: vec![
                path_block,
                Block {
                    label: "Content:",
                    lines,
                },
            ],
            remaining: (cut > 0).then_some(cut),
        };
        with_status_result(body, call, ctx, content_cut)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::fixtures::*;
    use super::super::{SectionRenderer, ToolCallSection, ToolKind, ToolStatus};
    use super::*;
    use crate::theme::Role;

    #[test]
    fn rust_content_is_highlighted_by_extension() {
        let theme = theme();
        let call = call(
            ToolKind::Write,
            json!({"file_path": "/src/lib.rs", "content": "fn main() {}\n"}),
            Some("File created"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(
            rows,
            [
                "▼ Do the thing",
                "",
                "  Path:",
                "",
                "     /src/lib.rs",
                "",
                "  Content:",
                "",
                "     fn main() {}",
            ]
        );
        let body = body(&call, &theme, 60);
        let line = &body.blocks[1].lines[0];
        let fg_of_span = |needle: &str| {
            line.spans
                .iter()
                .find(|s| s.content.contains(needle))
                .and_then(|s| s.style.fg)
        };
        // `fn` is a keyword and `main` a function name; neither is drawn as
        // plain or `command` text.
        assert_eq!(fg_of_span("fn"), Some(KEYWORD));
        assert_eq!(
            fg_of_span("main"),
            theme.style(Token::SyntaxFunction).fg,
            "{line:?}"
        );
        assert_ne!(fg_of_span("main"), Some(KEYWORD));
        assert_ne!(fg_of_span("main"), Some(PLAIN));
        assert_ne!(fg_of_span("main"), Some(color(&theme, Role::Accent)));
    }

    #[test]
    fn labels_columns_and_path_token() {
        let theme = theme();
        let call = call(
            ToolKind::Write,
            json!({"file_path": "/notes.txt", "content": "hello"}),
            None,
        );
        let ctx = SectionCtx::new(&theme, 60, false);
        let lines = ToolCallSection(&call).lines(&ctx);
        assert_eq!(
            fg_of(&lines[2], "Path:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(
            fg_of(&lines[4], "/notes"),
            Some(color(&theme, Role::Accent))
        );
        assert_eq!(
            fg_of(&lines[6], "Content:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(lines[8].to_string(), "     hello");
    }

    #[test]
    fn long_lines_wrap_and_long_content_truncates() {
        let theme = theme();
        let content = format!("{}\n{}", "x".repeat(50), "y\n".repeat(9));
        let call = call(
            ToolKind::Write,
            json!({"file_path": "/a.txt", "content": content}),
            Some("created"),
        );
        let ctx = SectionCtx {
            truncate: 3,
            ..SectionCtx::new(&theme, 30, false)
        };
        let body = WriteBody.body(&call, &ctx);
        // 27 cells per row: the first logical line wraps onto two rows.
        let rows: Vec<String> = body.blocks[1]
            .lines
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            rows,
            ["x".repeat(27), "x".repeat(23), "y".into(), "y".into()]
        );
        assert_eq!(body.remaining, Some(7));
    }

    #[test]
    fn missing_content_is_noted() {
        let theme = theme();
        let call = call(ToolKind::Write, json!({"file_path": "/a"}), None);
        let body = body(&call, &theme, 60);
        assert_eq!(body.blocks[1].lines[0].to_string(), "(no content)");
    }

    #[test]
    fn a_failed_write_shows_its_error_and_a_pending_one_shows_pending() {
        let theme = theme();
        let mut failed = call(
            ToolKind::Write,
            json!({"file_path": "/a.txt", "content": "hello"}),
            Some("Permission denied"),
        );
        failed.status = ToolStatus::Error;
        let body = body(&failed, &theme, 60);
        assert_eq!(body.blocks.len(), 3);
        assert_eq!(body.blocks[2].label, "Result:");
        let line = &body.blocks[2].lines[0];
        assert_eq!(line.to_string(), "Permission denied");
        assert_eq!(
            line.spans[0].style.fg,
            Some(color(&theme, Role::Destructive))
        );

        let pending = call(
            ToolKind::Write,
            json!({"file_path": "/a.txt", "content": "hello"}),
            None,
        );
        let body = self::body(&pending, &theme, 60);
        let line = &body.blocks[2].lines[0];
        assert_eq!(line.to_string(), "(pending)");
        assert_eq!(
            line.spans[0].style.fg,
            Some(color(&theme, Role::MutedForeground))
        );
        // A successful write has no Result: block.
        let ok = call(
            ToolKind::Write,
            json!({"file_path": "/a.txt", "content": "hello"}),
            Some("created"),
        );
        assert_eq!(self::body(&ok, &theme, 60).blocks.len(), 2);
    }

    fn labels(body: &Body) -> Vec<&'static str> {
        body.blocks.iter().map(|b| b.label).collect()
    }

    fn at(truncate: usize, expanded: bool, theme: &crate::theme::Theme) -> SectionCtx<'_> {
        SectionCtx {
            truncate,
            expanded,
            ..SectionCtx::new(theme, 60, false)
        }
    }

    fn ten_lines(prefix: &str) -> String {
        (1..=10).map(|n| format!("{prefix}{n}\n")).collect()
    }

    fn write(content: &str, result: Option<&str>, status: ToolStatus) -> ToolCall {
        let mut call = call(
            ToolKind::Write,
            json!({"file_path": "/a.txt", "content": content}),
            result,
        );
        call.status = status;
        call
    }

    #[test]
    fn failed_short_content_long_error_puts_the_truncated_result_last() {
        let theme = theme();
        let call = write("a\nb\n", Some(&ten_lines("err")), ToolStatus::Error);
        let body = WriteBody.body(&call, &at(4, false, &theme));
        assert_eq!(labels(&body), ["Path:", "Content:", "Result:"]);
        assert_eq!(body.blocks[2].lines.len(), 4);
        assert_eq!(body.remaining, Some(6));
        let body = WriteBody.body(&call, &at(4, true, &theme));
        assert_eq!(labels(&body), ["Path:", "Content:", "Result:"]);
        assert_eq!(body.blocks[2].lines.len(), 10);
        assert_eq!(body.remaining, None);
    }

    #[test]
    fn failed_long_content_short_error_puts_the_result_before_the_content() {
        let theme = theme();
        let call = write(&ten_lines("c"), Some("denied"), ToolStatus::Error);
        for expanded in [false, true] {
            let body = WriteBody.body(&call, &at(4, expanded, &theme));
            assert_eq!(labels(&body), ["Path:", "Result:", "Content:"]);
            assert_eq!(body.blocks[1].lines[0].to_string(), "denied");
            assert_eq!(body.blocks[2].lines.len(), if expanded { 10 } else { 4 });
            assert_eq!(body.remaining, (!expanded).then_some(6));
        }
    }

    #[test]
    fn failed_long_content_and_long_error_show_the_error_in_full() {
        let theme = theme();
        let call = write(&ten_lines("c"), Some(&ten_lines("err")), ToolStatus::Error);
        for expanded in [false, true] {
            let body = WriteBody.body(&call, &at(4, expanded, &theme));
            assert_eq!(labels(&body), ["Path:", "Result:", "Content:"]);
            assert_eq!(body.blocks[1].lines.len(), 10);
            assert_eq!(body.remaining, (!expanded).then_some(6));
        }
    }

    #[test]
    fn pending_long_content_puts_pending_before_the_content() {
        let theme = theme();
        let call = write(&ten_lines("c"), None, ToolStatus::Pending);
        for expanded in [false, true] {
            let body = WriteBody.body(&call, &at(4, expanded, &theme));
            assert_eq!(labels(&body), ["Path:", "Result:", "Content:"]);
            assert_eq!(body.blocks[1].lines[0].to_string(), "(pending)");
            assert_eq!(body.remaining, (!expanded).then_some(6));
        }
    }

    #[test]
    fn a_successful_long_write_has_no_result_block() {
        let theme = theme();
        let call = write(&ten_lines("c"), Some("created"), ToolStatus::Ok);
        let body = WriteBody.body(&call, &at(4, false, &theme));
        assert_eq!(labels(&body), ["Path:", "Content:"]);
        assert_eq!(body.remaining, Some(6));
    }
}
