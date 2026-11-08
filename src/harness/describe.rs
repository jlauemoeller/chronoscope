//! Tool descriptions shared by the harness adapters (product §4.1): the tool's
//! own `description` input, else a label derived from its input, else the
//! tool name.

use std::path::Path;

use serde_json::Value;

use crate::model::ToolKind;

/// Describes a tool call: the tool's own `description` input if present,
/// otherwise a label derived from its input. `cwd` is the session's working
/// directory, used to shorten paths inside it.
pub fn describe(kind: &ToolKind, name: &str, input: &Value, cwd: Option<&Path>) -> String {
    if let Some(description) = str_input(input, "description") {
        return description.to_owned();
    }
    let derived = match kind {
        ToolKind::Read | ToolKind::Write | ToolKind::Edit => {
            path_input(input).map(|p| format!("{} {}", label(kind), display_path(p, cwd)))
        }
        ToolKind::Grep => {
            str_input(input, "pattern").map(|pattern| match str_input(input, "path") {
                Some(path) => format!("Grep \"{pattern}\" in {path}"),
                None => format!("Grep \"{pattern}\""),
            })
        }
        ToolKind::Glob => {
            str_input(input, "pattern").map(|pattern| match str_input(input, "path") {
                Some(path) => format!("Glob {pattern} in {path}"),
                None => format!("Glob {pattern}"),
            })
        }
        ToolKind::WebFetch => {
            str_input(input, "url").map(|url| format!("WebFetch {}", url_label(url)))
        }
        ToolKind::WebSearch => {
            str_input(input, "query").map(|query| format!("WebSearch \"{query}\""))
        }
        ToolKind::Shell | ToolKind::Subagent | ToolKind::Other(_) => None,
    };
    derived.unwrap_or_else(|| name.to_owned())
}

fn label(kind: &ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "Read",
        ToolKind::Write => "Write",
        _ => "Edit",
    }
}

fn str_input<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn path_input(input: &Value) -> Option<&str> {
    ["file_path", "path", "notebook_path", "filePath"]
        .into_iter()
        .find_map(|key| str_input(input, key))
}

/// Shows `path` relative to `cwd` when it lies inside it.
fn display_path(path: &str, cwd: Option<&Path>) -> String {
    if let Some(cwd) = cwd
        && let Ok(rest) = Path::new(path).strip_prefix(cwd)
        && !rest.as_os_str().is_empty()
    {
        return rest.display().to_string();
    }
    path.to_owned()
}

/// `host/…` for a URL with a path, just `host` otherwise.
fn url_label(url: &str) -> String {
    let without_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let end = without_scheme
        .find(['/', '?', '#'])
        .unwrap_or(without_scheme.len());
    let (authority, rest) = without_scheme.split_at(end);
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    if rest.is_empty() || rest == "/" {
        host.to_owned()
    } else {
        format!("{host}/…")
    }
}
