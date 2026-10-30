//! The Shell body: `Command:` and `Result:`.

use super::{Body, SectionCtx, ToolBodyRenderer, compact_json, str_field, text_block, with_result};
use crate::model::ToolCall;
use crate::theme::Token;

/// `Command:` with the whole command line (char-wrapped), then `Result:`.
#[derive(Debug, Clone, Copy)]
pub struct ShellBody;

impl ToolBodyRenderer for ShellBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        let command = match str_field(&call.input, &["command", "cmd"]) {
            Some(command) => command.to_owned(),
            None => compact_json(&call.input),
        };
        let style = ctx.theme.style(Token::Command);
        let blocks = vec![text_block("Command:", &command, style, ctx)];
        with_result(blocks, "Result:", call, ctx)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::testing::*;
    use super::super::{SectionRenderer, ToolCallSection, ToolKind, ToolStatus};
    use super::*;
    use crate::theme::Role;

    #[test]
    fn labels_columns_and_tokens() {
        let theme = theme();
        let call = call(ToolKind::Shell, json!({"command": "ls -la"}), Some("a\nb"));
        let rows = section(&call, &theme, 60);
        assert_eq!(
            rows,
            [
                "▼ Do the thing",
                "",
                "  Command:",
                "",
                "     ls -la",
                "",
                "  Result:",
                "",
                "     a",
                "     b",
            ]
        );
        let ctx = SectionCtx::new(&theme, 60, false);
        let lines = ToolCallSection(&call).lines(&ctx);
        assert_eq!(
            fg_of(&lines[2], "Command:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(fg_of(&lines[4], "ls"), Some(color(&theme, Role::Accent)));
        assert_eq!(
            fg_of(&lines[6], "Result:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(fg_of(&lines[8], "a"), Some(color(&theme, Role::Secondary)));
    }

    #[test]
    fn a_long_path_is_char_wrapped_at_the_body_width() {
        let theme = theme();
        let path = format!("/{}", "p".repeat(199));
        let call = call(
            ToolKind::Shell,
            json!({"command": format!("cat {path}")}),
            Some(&path),
        );
        let body = body(&call, &theme, 40);
        let body_width = 40 - 3;
        let command = &body.blocks[0].lines;
        assert!(command.len() >= 6);
        assert!(command.iter().all(|l| l.width() <= body_width));
        assert!(
            command[..command.len() - 1]
                .iter()
                .all(|l| l.width() == body_width)
        );
        let joined: String = command.iter().map(ToString::to_string).collect();
        assert_eq!(joined, format!("cat {path}"));
        let result: String = body.blocks[1]
            .lines
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(result, path);
    }

    #[test]
    fn a_failed_call_shows_its_result_in_error_output() {
        let theme = theme();
        let mut call = call(ToolKind::Shell, json!({"command": "false"}), Some("boom"));
        call.status = ToolStatus::Error;
        let body = body(&call, &theme, 60);
        let style = body.blocks[1].lines[0].spans[0].style;
        assert_eq!(style.fg, Some(color(&theme, Role::Destructive)));
    }

    #[test]
    fn a_missing_command_falls_back_to_the_input_json() {
        let theme = theme();
        let call = call(ToolKind::Shell, json!({"cwd": "/x"}), Some("ok"));
        let body = body(&call, &theme, 60);
        assert_eq!(body.blocks[0].lines[0].to_string(), r#"{"cwd":"/x"}"#);
    }

    #[test]
    fn a_pending_call_shows_muted_pending() {
        let theme = theme();
        let call = call(ToolKind::Shell, json!({"command": "ls"}), None);
        let body = body(&call, &theme, 60);
        let line = &body.blocks[1].lines[0];
        assert_eq!(line.to_string(), "(pending)");
        assert_eq!(
            line.spans[0].style.fg,
            Some(color(&theme, Role::MutedForeground))
        );
    }
}
