//! The WebFetch and WebSearch bodies.

use super::{Body, SectionCtx, ToolBodyRenderer, compact_json, str_field, text_block, with_result};
use crate::model::ToolCall;
use crate::theme::Token;

/// `URL:` with the URL (and the prompt on the next line), then `Result:`.
#[derive(Debug, Clone, Copy)]
pub struct WebFetchBody;

/// `Query:` with the query, then `Result:`.
#[derive(Debug, Clone, Copy)]
pub struct WebSearchBody;

impl ToolBodyRenderer for WebFetchBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        let style = ctx.theme.style(Token::Command);
        let mut text = match str_field(&call.input, &["url"]) {
            Some(url) => url.to_owned(),
            None => compact_json(&call.input),
        };
        if let Some(prompt) = str_field(&call.input, &["prompt"]) {
            text.push('\n');
            text.push_str(prompt);
        }
        let blocks = vec![text_block("URL:", &text, style, ctx)];
        with_result(blocks, "Result:", call, ctx)
    }
}

impl ToolBodyRenderer for WebSearchBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        let style = ctx.theme.style(Token::Command);
        let text = match str_field(&call.input, &["query"]) {
            Some(query) => query.to_owned(),
            None => compact_json(&call.input),
        };
        let blocks = vec![text_block("Query:", &text, style, ctx)];
        with_result(blocks, "Result:", call, ctx)
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
    fn web_fetch_labels_columns_and_tokens() {
        let theme = theme();
        let call = call(
            ToolKind::WebFetch,
            json!({"url": "https://example.com/docs", "prompt": "Summarise the page"}),
            Some("A page about docs."),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(
            rows,
            [
                "▼ Do the thing",
                "",
                "  URL:",
                "",
                "     https://example.com/docs",
                "     Summarise the page",
                "",
                "  Result:",
                "",
                "     A page about docs.",
            ]
        );
        let ctx = SectionCtx::new(&theme, 60, false);
        let lines = ToolCallSection(&call).lines(&ctx);
        assert_eq!(fg_of(&lines[2], "URL:"), Some(color(&theme, Role::Warning)));
        assert_eq!(fg_of(&lines[4], "https"), Some(color(&theme, Role::Accent)));
        assert_eq!(
            fg_of(&lines[9], "A page"),
            Some(color(&theme, Role::Secondary))
        );
    }

    #[test]
    fn web_search_labels_columns_and_tokens() {
        let theme = theme();
        let call = call(
            ToolKind::WebSearch,
            json!({"query": "logitech brio uvc extension units"}),
            Some("1. Some result"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(
            rows,
            [
                "▼ Do the thing",
                "",
                "  Query:",
                "",
                "     logitech brio uvc extension units",
                "",
                "  Result:",
                "",
                "     1. Some result",
            ]
        );
        let ctx = SectionCtx::new(&theme, 60, false);
        let lines = ToolCallSection(&call).lines(&ctx);
        assert_eq!(
            fg_of(&lines[2], "Query:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(
            fg_of(&lines[4], "logitech"),
            Some(color(&theme, Role::Accent))
        );
        assert_eq!(fg_of(&lines[8], "1."), Some(color(&theme, Role::Secondary)));
    }
}
