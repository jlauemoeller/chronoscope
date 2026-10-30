//! The Write body: `Path:` and the highlighted `Content:`.

use ratatui::text::Line;

use super::{
    Block, Body, SectionCtx, ToolBodyRenderer, compact_json, limit, muted_block, path_of,
    text_block, truncate_lines,
};
use crate::markdown::{highlight_code, wrap_styled_lines};
use crate::model::ToolCall;
use crate::theme::Token;

/// `Path:` with the path, then `Content:` with the file content highlighted by
/// the path's extension (`syntax.*` tokens) and char-wrapped.
#[derive(Debug, Clone, Copy)]
pub struct WriteBody;

impl ToolBodyRenderer for WriteBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        let path = path_of(&call.input);
        let shown_path = path.map_or_else(|| compact_json(&call.input), str::to_owned);
        let path_block = text_block("Path:", &shown_path, ctx.theme.style(Token::Command), ctx);
        let content = call.input.get("content").and_then(|c| c.as_str());
        let Some(content) = content.filter(|c| !c.is_empty()) else {
            return Body {
                blocks: vec![path_block, muted_block("Content:", "(no content)", ctx)],
                ..Body::default()
            };
        };
        let extension = path
            .and_then(|p| std::path::Path::new(p).extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let (shown, cut) = truncate_lines(content, limit(ctx));
        let highlighted = highlight_code(shown, extension, ctx.theme);
        let mut lines: Vec<Line<'static>> = wrap_styled_lines(&highlighted, ctx.body_width());
        if lines.is_empty() {
            lines.push(Line::default());
        }
        Body {
            blocks: vec![
                path_block,
                Block {
                    label: "Content:",
                    lines,
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
            None,
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
}
