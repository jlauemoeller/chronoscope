//! The Grep and Glob bodies: `Pattern:` and `Matches:`.

use serde_json::Value;

use super::{Body, SectionCtx, ToolBodyRenderer, compact_json, str_field, text_block, with_result};
use crate::model::ToolCall;
use crate::theme::Token;

/// `Pattern:` with the pattern, ` in <path>` and any glob / type filters,
/// then `Matches:`.
#[derive(Debug, Clone, Copy)]
pub struct GrepBody;

/// `Pattern:` with the pattern and ` in <path>`, then `Matches:`.
#[derive(Debug, Clone, Copy)]
pub struct GlobBody;

fn pattern_line(input: &Value, filters: bool) -> String {
    let Some(pattern) = str_field(input, &["pattern", "query"]) else {
        return compact_json(input);
    };
    let mut line = pattern.to_owned();
    if let Some(path) = str_field(input, &["path"]) {
        line.push_str(" in ");
        line.push_str(path);
    }
    if filters {
        for (key, name) in [("glob", "glob"), ("type", "type")] {
            if let Some(value) = str_field(input, &[key]) {
                line.push_str(&format!(" ({name}: {value})"));
            }
        }
    }
    line
}

fn body(call: &ToolCall, ctx: &SectionCtx<'_>, filters: bool) -> Body {
    let style = ctx.theme.style(Token::Command);
    let line = pattern_line(&call.input, filters);
    let blocks = vec![text_block("Pattern:", &line, style, ctx)];
    with_result(blocks, "Matches:", call, ctx)
}

impl ToolBodyRenderer for GrepBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        body(call, ctx, true)
    }
}

impl ToolBodyRenderer for GlobBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        body(call, ctx, false)
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
    fn grep_labels_columns_and_tokens() {
        let theme = theme();
        let call = call(
            ToolKind::Grep,
            json!({"pattern": "fn main", "path": "src", "glob": "*.rs", "type": "rust"}),
            Some("src/main.rs:1:fn main()"),
        );
        let rows = section(&call, &theme, 80);
        assert_eq!(
            rows,
            [
                "▼ Do the thing",
                "",
                "  Pattern:",
                "",
                "     fn main in src (glob: *.rs) (type: rust)",
                "",
                "  Matches:",
                "",
                "     src/main.rs:1:fn main()",
            ]
        );
        let ctx = SectionCtx::new(&theme, 80, false);
        let lines = ToolCallSection(&call).lines(&ctx);
        assert_eq!(
            fg_of(&lines[2], "Pattern:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(
            fg_of(&lines[4], "fn main"),
            Some(color(&theme, Role::Accent))
        );
        assert_eq!(
            fg_of(&lines[8], "src/main"),
            Some(color(&theme, Role::Secondary))
        );
    }

    #[test]
    fn glob_shows_the_path_but_no_filters() {
        let theme = theme();
        let call = call(
            ToolKind::Glob,
            json!({"pattern": "**/*.toml", "path": "/repo", "glob": "ignored"}),
            Some("Cargo.toml"),
        );
        let rows = section(&call, &theme, 80);
        assert_eq!(rows[4], "     **/*.toml in /repo");
        assert_eq!(rows[6], "  Matches:");
        assert_eq!(rows[8], "     Cargo.toml");
    }
}
