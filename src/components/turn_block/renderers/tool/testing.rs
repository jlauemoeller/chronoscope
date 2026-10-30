//! Helpers for the body renderer tests.

use ratatui::style::Color;
use ratatui::text::Line;
use serde_json::Value;

use super::*;
use crate::model::{ItemId, ToolOutput, ToolResult};
use crate::theme::{Role, Theme, Token, TokenOverride, builtin};

/// Colours no role has, so that a colour names the token it came from.
pub const DIFF_ADDED: Color = Color::Rgb(1, 2, 3);
pub const DIFF_REMOVED: Color = Color::Rgb(4, 5, 6);
pub const KEYWORD: Color = Color::Rgb(7, 8, 9);
pub const PLAIN: Color = Color::Rgb(10, 11, 12);

/// A theme whose output, command, label, diff and error tokens all differ.
pub fn theme() -> Theme {
    let base = builtin::default_theme();
    let fg = |role| TokenOverride::fg(base.role(role));
    base.clone()
        .with_token(Token::ToolDescription, fg(Role::Primary))
        .with_token(Token::Label, fg(Role::Warning))
        .with_token(Token::Command, fg(Role::Accent))
        .with_token(Token::Output, fg(Role::Secondary))
        .with_token(Token::ErrorOutput, fg(Role::Destructive))
        .with_token(Token::DiffAdded, TokenOverride::fg(DIFF_ADDED))
        .with_token(Token::DiffRemoved, TokenOverride::fg(DIFF_REMOVED))
        .with_token(Token::SyntaxKeyword, TokenOverride::fg(KEYWORD))
        .with_token(Token::SyntaxPlain, TokenOverride::fg(PLAIN))
}

pub fn color(theme: &Theme, role: Role) -> Color {
    theme.role(role)
}

/// A finished call of `kind` with the given input and result text.
pub fn call(kind: ToolKind, input: Value, result: Option<&str>) -> ToolCall {
    ToolCall {
        id: ItemId::from("c1"),
        tool: kind,
        description: "Do the thing".to_owned(),
        input,
        status: if result.is_some() {
            ToolStatus::Ok
        } else {
            ToolStatus::Pending
        },
        result: result.map(|text| ToolResult {
            output: ToolOutput::inline(text.to_owned()),
            is_error: false,
        }),
        duration: None,
    }
}

/// The body of `call` at `width` text cells, unfolded.
pub fn body(call: &ToolCall, theme: &Theme, width: u16) -> Body {
    body_renderer(&call.tool).body(call, &SectionCtx::new(theme, width, false))
}

/// The whole section at `width`, unfolded, as text rows.
pub fn section(call: &ToolCall, theme: &Theme, width: u16) -> Vec<String> {
    let ctx = SectionCtx::new(theme, width, false);
    texts(&ToolCallSection(call).lines(&ctx))
}

pub fn texts(lines: &[Line<'_>]) -> Vec<String> {
    lines.iter().map(ToString::to_string).collect()
}

/// The foreground of the first span of `line` whose text contains `needle`.
pub fn fg_of(line: &Line<'_>, needle: &str) -> Option<Color> {
    line.spans
        .iter()
        .find(|s| s.content.contains(needle))
        .and_then(|s| s.style.fg)
}
