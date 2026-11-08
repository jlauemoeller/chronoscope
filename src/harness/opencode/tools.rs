//! OpenCode tool names to [`ToolKind`] and tool descriptions (product §4.1).

use std::path::Path;

use serde_json::Value;

use crate::harness::describe::describe as describe_input;
use crate::model::ToolKind;

/// Maps an OpenCode tool name. Anything not listed is [`ToolKind::Other`]
/// with the name as recorded (`bash`, `write_file`, `skill`, `execute`, MCP
/// tools, ...).
pub fn tool_kind(name: &str) -> ToolKind {
    match name {
        "shell" => ToolKind::Shell,
        "read" => ToolKind::Read,
        "write" => ToolKind::Write,
        "edit" => ToolKind::Edit,
        "grep" => ToolKind::Grep,
        "glob" => ToolKind::Glob,
        "webfetch" => ToolKind::WebFetch,
        "websearch" => ToolKind::WebSearch,
        "subagent" => ToolKind::Subagent,
        other => ToolKind::Other(other.to_owned()),
    }
}

/// Describes a call with the rules shared by every harness: the input's own
/// `description` if present, else a label derived from the input
/// (`Read src/main.rs`, `Grep "pattern" in path`, ...), else the tool name.
/// `cwd` is the session directory, used to shorten paths inside it.
pub fn describe(kind: &ToolKind, name: &str, input: &Value, cwd: Option<&Path>) -> String {
    describe_input(kind, name, input, cwd)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn every_listed_name_maps_to_its_kind() {
        for (name, kind) in [
            ("shell", ToolKind::Shell),
            ("read", ToolKind::Read),
            ("write", ToolKind::Write),
            ("edit", ToolKind::Edit),
            ("grep", ToolKind::Grep),
            ("glob", ToolKind::Glob),
            ("webfetch", ToolKind::WebFetch),
            ("websearch", ToolKind::WebSearch),
            ("subagent", ToolKind::Subagent),
        ] {
            assert_eq!(tool_kind(name), kind, "{name}");
        }
    }

    #[test]
    fn other_names_are_kept_as_they_are() {
        for name in [
            "execute",
            "skill",
            "bash",
            "write_file",
            "mcp__docs__search",
            "",
        ] {
            assert_eq!(tool_kind(name), ToolKind::Other(name.to_owned()), "{name}");
        }
    }

    #[test]
    fn names_are_case_sensitive_and_claude_names_are_not_known() {
        assert_eq!(tool_kind("Shell"), ToolKind::Other("Shell".to_owned()));
        assert_eq!(tool_kind("Bash"), ToolKind::Other("Bash".to_owned()));
        assert_eq!(tool_kind("Task"), ToolKind::Other("Task".to_owned()));
    }

    #[test]
    fn a_description_input_wins() {
        let input = json!({"command": "ls", "description": "List files"});
        assert_eq!(
            describe(&ToolKind::Shell, "shell", &input, None),
            "List files"
        );
    }

    #[test]
    fn labels_are_derived_from_the_input() {
        let cwd = Some(Path::new("/home/u/p"));
        let read = json!({"path": "/home/u/p/src/main.rs"});
        assert_eq!(
            describe(&ToolKind::Read, "read", &read, cwd),
            "Read src/main.rs"
        );
        let edit = json!({"filePath": "/home/u/p/a.rs", "oldString": "x", "newString": "y"});
        assert_eq!(describe(&ToolKind::Edit, "edit", &edit, cwd), "Edit a.rs");
        let grep = json!({"pattern": "fn main", "path": "src"});
        assert_eq!(
            describe(&ToolKind::Grep, "grep", &grep, cwd),
            "Grep \"fn main\" in src"
        );
        let fetch = json!({"url": "https://example.com/docs"});
        assert_eq!(
            describe(&ToolKind::WebFetch, "webfetch", &fetch, cwd),
            "WebFetch example.com/…"
        );
    }

    #[test]
    fn unknown_tools_and_a_shell_without_description_are_described_by_name() {
        let input = json!({"code": "return 1"});
        let other = tool_kind("execute");
        assert_eq!(describe(&other, "execute", &input, None), "execute");
        assert_eq!(
            describe(&ToolKind::Shell, "shell", &json!({"command": "ls"}), None),
            "shell"
        );
    }
}
