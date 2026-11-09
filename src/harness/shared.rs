//! Items the harness adapters share: lenient fields, session titles and
//! the subagent description.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::value::RawValue;

/// `deserialize_with` helper (together with `#[serde(default)]`): a value of
/// an unexpected JSON type, or one that does not fit `T` (a number out of
/// range, say), counts as missing instead of failing the whole line or row.
///
/// The value is kept as raw JSON text rather than read into a
/// `serde_json::Value`, which would itself reject an out-of-range number
/// anywhere inside it. This needs a `serde_json` deserializer, which is what
/// both adapters use.
///
/// Two limits:
/// - A `RawValue` must be valid UTF-8, so invalid UTF-8 inside the value
///   still fails the parse of the whole line. The Claude adapter handles
///   that case in `read_entry` (`harness/claude/wire.rs`); OpenCode rows are
///   `String`s already.
/// - A duplicate key inside the value makes the whole value count as
///   missing.
pub(crate) fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: DeserializeOwned,
{
    let raw = Box::<RawValue>::deserialize(deserializer)?;
    Ok(serde_json::from_str(raw.get()).ok())
}

/// Like [`lenient`] for a struct: only a JSON object is read (serde would
/// also read a struct from an array).
pub(crate) fn lenient_object<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: DeserializeOwned,
{
    let raw = Box::<RawValue>::deserialize(deserializer)?;
    if raw.get().starts_with('{') {
        Ok(serde_json::from_str(raw.get()).ok())
    } else {
        Ok(None)
    }
}

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

/// The title of a session that has no title of its own: the first line of
/// its first prompt that is not blank, trimmed and cut by
/// [`truncate_title`], else [`UNTITLED`].
pub fn title_from_prompt(prompt: Option<&str>) -> String {
    prompt
        .and_then(|p| p.lines().map(str::trim).find(|l| !l.is_empty()))
        .map_or_else(|| UNTITLED.to_owned(), truncate_title)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_prompt_or_an_empty_one_is_untitled() {
        assert_eq!(title_from_prompt(None), UNTITLED);
        assert_eq!(title_from_prompt(Some("")), UNTITLED);
        assert_eq!(title_from_prompt(Some("  \n\t\n")), UNTITLED);
    }

    #[test]
    fn the_title_is_the_first_line_that_is_not_blank() {
        assert_eq!(
            title_from_prompt(Some("\n   \nFix the login test\nand the other one")),
            "Fix the login test"
        );
    }

    #[test]
    fn the_title_is_trimmed() {
        assert_eq!(
            title_from_prompt(Some("  \tFix the login test \r\n")),
            "Fix the login test"
        );
    }

    #[test]
    fn a_long_line_is_cut_with_an_ellipsis() {
        let line = "x".repeat(MAX_TITLE_CHARS + 5);
        let title = title_from_prompt(Some(&line));
        assert_eq!(title.chars().count(), MAX_TITLE_CHARS);
        assert!(title.ends_with('…'), "{title}");
        let exact = "y".repeat(MAX_TITLE_CHARS);
        assert_eq!(title_from_prompt(Some(&exact)), exact);
    }
}
