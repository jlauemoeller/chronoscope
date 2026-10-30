//! Claude tool names to [`ToolKind`] and tool descriptions (SPEC §5.1).

pub use crate::harness::describe::describe;
use crate::model::ToolKind;

/// Maps a Claude tool name (including server tools such as `web_search`).
pub fn tool_kind(name: &str) -> ToolKind {
    match name {
        "Bash" => ToolKind::Shell,
        "Read" => ToolKind::Read,
        "Write" => ToolKind::Write,
        "Edit" | "MultiEdit" => ToolKind::Edit,
        "Grep" => ToolKind::Grep,
        "Glob" => ToolKind::Glob,
        "WebFetch" => ToolKind::WebFetch,
        "WebSearch" | "web_search" => ToolKind::WebSearch,
        "Agent" | "Task" => ToolKind::Subagent,
        other => ToolKind::Other(other.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::{Value, json};

    use super::*;

    fn describe_call(name: &str, input: Value) -> (ToolKind, String) {
        let kind = tool_kind(name);
        let text = describe(&kind, name, &input, Some(Path::new("/home/user/project")));
        (kind, text)
    }

    #[test]
    fn bash_maps_to_shell_and_uses_its_description_input() {
        let (kind, text) = describe_call(
            "Bash",
            json!({"command": "ls -la", "description": "List files"}),
        );
        assert_eq!(kind, ToolKind::Shell);
        assert_eq!(text, "List files");
    }

    #[test]
    fn bash_without_description_is_its_name() {
        let (_, text) = describe_call("Bash", json!({"command": "ls"}));
        assert_eq!(text, "Bash");
    }

    #[test]
    fn read_maps_to_read_with_relative_path() {
        let (kind, text) = describe_call(
            "Read",
            json!({"file_path": "/home/user/project/src/main.rs"}),
        );
        assert_eq!(kind, ToolKind::Read);
        assert_eq!(text, "Read src/main.rs");
    }

    #[test]
    fn read_outside_cwd_keeps_the_absolute_path() {
        let (_, text) = describe_call("Read", json!({"file_path": "/etc/hosts"}));
        assert_eq!(text, "Read /etc/hosts");
    }

    #[test]
    fn path_prefix_match_is_by_component_not_by_string() {
        let (_, text) = describe_call(
            "Read",
            json!({"file_path": "/home/user/project-other/a.rs"}),
        );
        assert_eq!(text, "Read /home/user/project-other/a.rs");
    }

    #[test]
    fn write_maps_to_write() {
        let (kind, text) = describe_call(
            "Write",
            json!({"file_path": "/home/user/project/notes.md", "content": "x"}),
        );
        assert_eq!(kind, ToolKind::Write);
        assert_eq!(text, "Write notes.md");
    }

    #[test]
    fn edit_maps_to_edit() {
        let (kind, text) = describe_call(
            "Edit",
            json!({"file_path": "/home/user/project/src/lib.rs"}),
        );
        assert_eq!(kind, ToolKind::Edit);
        assert_eq!(text, "Edit src/lib.rs");
    }

    #[test]
    fn multi_edit_maps_to_edit() {
        let (kind, text) = describe_call(
            "MultiEdit",
            json!({"file_path": "/home/user/project/src/lib.rs", "edits": []}),
        );
        assert_eq!(kind, ToolKind::Edit);
        assert_eq!(text, "Edit src/lib.rs");
    }

    #[test]
    fn grep_with_path() {
        let (kind, text) = describe_call("Grep", json!({"pattern": "fn main", "path": "src"}));
        assert_eq!(kind, ToolKind::Grep);
        assert_eq!(text, "Grep \"fn main\" in src");
    }

    #[test]
    fn grep_without_path_omits_in_clause() {
        let (_, text) = describe_call("Grep", json!({"pattern": "TODO"}));
        assert_eq!(text, "Grep \"TODO\"");
    }

    #[test]
    fn glob_with_and_without_path() {
        let (kind, with) = describe_call("Glob", json!({"pattern": "**/*.rs", "path": "src"}));
        assert_eq!(kind, ToolKind::Glob);
        assert_eq!(with, "Glob **/*.rs in src");
        let (_, without) = describe_call("Glob", json!({"pattern": "*.toml"}));
        assert_eq!(without, "Glob *.toml");
    }

    #[test]
    fn web_fetch_with_path_ends_in_ellipsis() {
        let (kind, text) = describe_call(
            "WebFetch",
            json!({"url": "https://example.com/docs/page?x=1", "prompt": "p"}),
        );
        assert_eq!(kind, ToolKind::WebFetch);
        assert_eq!(text, "WebFetch example.com/…");
    }

    #[test]
    fn web_fetch_without_path_is_just_the_host() {
        let (_, bare) = describe_call("WebFetch", json!({"url": "https://example.com"}));
        assert_eq!(bare, "WebFetch example.com");
        let (_, slash) = describe_call("WebFetch", json!({"url": "https://example.com/"}));
        assert_eq!(slash, "WebFetch example.com");
    }

    #[test]
    fn web_search_maps_to_web_search() {
        let (kind, text) = describe_call("WebSearch", json!({"query": "rust ratatui"}));
        assert_eq!(kind, ToolKind::WebSearch);
        assert_eq!(text, "WebSearch \"rust ratatui\"");
    }

    #[test]
    fn server_tool_web_search_maps_to_web_search() {
        let (kind, text) = describe_call("web_search", json!({"query": "claude jsonl"}));
        assert_eq!(kind, ToolKind::WebSearch);
        assert_eq!(text, "WebSearch \"claude jsonl\"");
    }

    #[test]
    fn agent_maps_to_subagent_and_uses_its_description() {
        let (kind, text) = describe_call(
            "Agent",
            json!({"description": "Explore the repo", "prompt": "p", "subagent_type": "Explore"}),
        );
        assert_eq!(kind, ToolKind::Subagent);
        assert_eq!(text, "Explore the repo");
    }

    #[test]
    fn task_maps_to_subagent() {
        let (kind, text) = describe_call("Task", json!({"prompt": "p"}));
        assert_eq!(kind, ToolKind::Subagent);
        assert_eq!(text, "Task");
    }

    #[test]
    fn mcp_tools_are_other_and_described_by_name() {
        let (kind, text) = describe_call("mcp__docs__search", json!({"q": "x"}));
        assert_eq!(kind, ToolKind::Other("mcp__docs__search".into()));
        assert_eq!(text, "mcp__docs__search");
    }

    #[test]
    fn unknown_tools_are_other_and_described_by_name() {
        let (kind, text) = describe_call("TodoWrite", json!({"todos": []}));
        assert_eq!(kind, ToolKind::Other("TodoWrite".into()));
        assert_eq!(text, "TodoWrite");
    }

    #[test]
    fn missing_input_falls_back_to_the_tool_name() {
        let (_, text) = describe_call("Read", Value::Null);
        assert_eq!(text, "Read");
    }

    #[test]
    fn empty_description_is_ignored() {
        let (_, text) = describe_call("Bash", json!({"description": "  "}));
        assert_eq!(text, "Bash");
    }
}
