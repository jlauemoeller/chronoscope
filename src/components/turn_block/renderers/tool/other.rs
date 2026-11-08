//! The body of every other tool, including MCP tools: `Input:` and `Result:`.

use super::{Body, SectionCtx, ToolBodyRenderer, text_block, with_result};
use crate::model::ToolCall;
use crate::theme::Token;

/// `Input:` with the pretty-printed input JSON (2-space indent), then
/// `Result:`.
#[derive(Debug, Clone, Copy)]
pub struct OtherBody;

impl ToolBodyRenderer for OtherBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        let json = serde_json::to_string_pretty(&call.input).unwrap_or_default();
        let style = ctx.theme.style(Token::Command);
        let blocks = vec![text_block("Input:", &json, style, ctx)];
        with_result(blocks, "Result:", call, ctx)
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
    fn labels_columns_and_tokens() {
        let theme = theme();
        let call = call(
            ToolKind::Other("mcp__github__get_issue".into()),
            json!({"repo": "a/b", "n": 3}),
            Some("issue text"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(
            rows,
            [
                "▼ Do the thing",
                "",
                "  Input:",
                "",
                "     {",
                "       \"n\": 3,",
                "       \"repo\": \"a/b\"",
                "     }",
                "",
                "  Result:",
                "",
                "     issue text",
            ]
        );
        let ctx = SectionCtx::new(&theme, 60, false);
        let lines = ToolCallSection(&call).lines(&ctx);
        assert_eq!(
            fg_of(&lines[2], "Input:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(fg_of(&lines[4], "{"), Some(color(&theme, Role::Accent)));
        assert_eq!(
            fg_of(&lines[11], "issue"),
            Some(color(&theme, Role::Secondary))
        );
    }
}
