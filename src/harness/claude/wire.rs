//! Typed views of the JSONL lines the assembler cares about.
//!
//! Every field is optional and unknown fields are ignored, so added or removed
//! fields degrade to missing data. A known field with a different JSON type
//! (other than `null`) still fails the line (or, inside `message.content`, the
//! whole content), which the assembler then skips.

use serde::Deserialize;
use serde_json::Value;

/// The few fields used to classify a line before (maybe) parsing it fully.
/// Everything else on the line is skipped without being materialised.
#[derive(Deserialize)]
pub struct Head {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub subtype: Option<String>,
    pub version: Option<Value>,
    /// Present on conversation entries; its presence tells an unknown entry
    /// type with content from session metadata.
    pub message: Option<serde::de::IgnoredAny>,
    pub attachment: Option<AttachmentHead>,
}

/// The kind of an `attachment` entry's payload.
#[derive(Deserialize)]
pub struct AttachmentHead {
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

#[derive(Deserialize)]
pub struct Entry {
    /// Top-level `content` of `system` entries.
    pub content: Option<Value>,
    #[serde(rename = "commandRun")]
    pub command_run: Option<Value>,
    pub attachment: Option<Value>,
    #[serde(rename = "isCompactSummary")]
    pub is_compact_summary: Option<bool>,
    pub uuid: Option<String>,
    pub timestamp: Option<String>,
    #[serde(rename = "isMeta")]
    pub is_meta: Option<bool>,
    pub origin: Option<Value>,
    pub message: Option<Message>,
    pub effort: Option<Value>,
    #[serde(rename = "perTurnEffort")]
    pub per_turn_effort: Option<Value>,
    #[serde(rename = "apiBlockIndex")]
    pub api_block_index: Option<usize>,
    pub cwd: Option<String>,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<u64>,
    #[serde(rename = "messageCount")]
    pub message_count: Option<u64>,
}

impl Entry {
    /// `origin.kind`, when the entry has an `origin` object.
    pub fn origin_kind(&self) -> Option<&str> {
        self.origin.as_ref()?.get("kind")?.as_str()
    }

    /// `effort`, falling back to `perTurnEffort`.
    pub fn effort(&self) -> Option<String> {
        [&self.effort, &self.per_turn_effort]
            .into_iter()
            .flatten()
            .find_map(|v| v.as_str().filter(|s| !s.is_empty()))
            .map(str::to_owned)
    }
}

#[derive(Deserialize)]
pub struct Message {
    pub id: Option<String>,
    pub model: Option<String>,
    pub content: Option<Content>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Blocks(Vec<Block>),
    Other(serde::de::IgnoredAny),
}

#[derive(Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Deserialize, Default)]
pub struct Block {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub text: Option<String>,
    pub thinking: Option<String>,
    pub signature: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub input: Option<Value>,
    pub tool_use_id: Option<String>,
    pub content: Option<Value>,
    pub is_error: Option<bool>,
}

impl Block {
    /// Whether this block delivers a tool result (`tool_result`, or a
    /// server-tool variant such as `advisor_tool_result`).
    pub fn is_result(&self) -> bool {
        self.kind
            .as_deref()
            .is_some_and(|k| k == "tool_result" || k.ends_with("_tool_result"))
    }
}

/// Flattens a `tool_result` `content` value to text: a string as is, an array
/// of text blocks joined by newlines, an object by its `text` field (or its
/// compact JSON).
pub(crate) fn result_text(content: Option<&Value>) -> String {
    match content {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                Value::String(s) => Some(s.clone()),
                Value::Object(o) => match o.get("text").and_then(Value::as_str) {
                    Some(text) => Some(text.to_owned()),
                    None => search_hit(o),
                },
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(Value::Object(o)) => match o.get("text").and_then(Value::as_str) {
            Some(text) => text.to_owned(),
            // Encrypted payloads (`advisor_redacted_result`) carry no text.
            None if is_redacted(o) => String::new(),
            None => Value::Object(o.clone()).to_string(),
        },
        Some(other) => other.to_string(),
    }
}

fn is_redacted(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|t| t.ends_with("_redacted_result"))
}

/// Renders a web-search hit (`{"title", "url", ...}`) as `title — url`.
fn search_hit(object: &serde_json::Map<String, Value>) -> Option<String> {
    let url = object.get("url").and_then(Value::as_str)?;
    match object.get("title").and_then(Value::as_str) {
        Some(title) if !title.is_empty() => Some(format!("{title} — {url}")),
        _ => Some(url.to_owned()),
    }
}

/// Re-extracts the text of the result for `call_id` from one JSONL `line`.
///
/// Used to materialise large tool outputs on demand; it applies the same
/// flattening as the assembler.
pub(crate) fn extract_result_text(line: &[u8], call_id: &str) -> Option<String> {
    let entry: Entry = serde_json::from_slice(line).ok()?;
    let Content::Blocks(blocks) = entry.message?.content? else {
        return None;
    };
    blocks
        .iter()
        .find(|b| b.is_result() && b.tool_use_id.as_deref() == Some(call_id))
        .map(|b| result_text(b.content.as_ref()))
}
