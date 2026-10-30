//! Items the harness adapters share: session titles and the subagent
//! description.

use serde::Deserialize;

/// Title of a session with neither an `ai-title` nor a prompt.
pub const UNTITLED: &str = "(untitled)";

/// The longest prompt-derived title, in characters, including the `…`.
pub const MAX_TITLE_CHARS: usize = 80;

/// Keeps `line` as is when it has at most [`MAX_TITLE_CHARS`] characters,
/// else its first `MAX_TITLE_CHARS - 1` characters followed by `…`.
pub fn truncate_title(line: &str) -> String {
    if line.chars().count() <= MAX_TITLE_CHARS {
        return line.to_owned();
    }
    let mut title: String = line.chars().take(MAX_TITLE_CHARS - 1).collect();
    title.push('…');
    title
}

/// What `agent-<id>.meta.json` says about a subagent run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SubagentInfo {
    /// The agent type (`Explore`, `story-writer`, ...); empty when unknown.
    #[serde(rename = "agentType", default)]
    pub agent_type: String,
    /// The short description the parent gave the run; empty when unknown.
    #[serde(default)]
    pub description: String,
}
