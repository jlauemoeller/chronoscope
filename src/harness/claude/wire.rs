//! Typed views of the JSONL lines the assembler cares about.
//!
//! Every field is optional and unknown fields are ignored, so added or removed
//! fields degrade to missing data. A known field with a different JSON type
//! (other than `null`) still fails the line (or, inside `message.content`, the
//! whole content), which the assembler then skips. The exception is
//! `message.usage`, which is read leniently: a value of the wrong type there
//! counts as missing instead of failing the line.

use serde::Deserialize;
use serde_json::Value;

use crate::harness::shared::{lenient, lenient_object};

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

/// A JSONL line. `M` is the type of `message`; only [`read_entry`] uses
/// another one than the default.
#[derive(Deserialize)]
pub struct Entry<M = Message> {
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
    pub message: Option<M>,
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

    /// The text of this entry when it is a human prompt, else `None`.
    ///
    /// The one definition of "human prompt" (SPEC §3.1) shared by the
    /// assembler and session discovery. Not a prompt: entries that carry any
    /// `tool_result` block, compaction summaries, `isMeta` entries (injected,
    /// not typed), entries whose `origin.kind` is not `human`, and, for
    /// entries without an `origin`, entries without a text block. Human
    /// prompts delivered as `queued_command` attachments are separate (see
    /// `queued_prompt_text`).
    pub fn human_prompt_text(&self) -> Option<String> {
        let content = self.message.as_ref()?.content.as_ref()?;
        let blocks: &[Block] = match content {
            Content::Blocks(blocks) => blocks,
            _ => &[],
        };
        if blocks
            .iter()
            .any(|b| b.kind.as_deref() == Some("tool_result"))
            || self.is_compact_summary == Some(true)
            || self.is_meta == Some(true)
        {
            return None;
        }
        let has_text = match content {
            Content::Text(_) => true,
            Content::Blocks(blocks) => blocks.iter().any(|b| b.kind.as_deref() == Some("text")),
            Content::Other(_) => false,
        };
        let is_prompt = match self.origin {
            Some(_) => self.origin_kind() == Some("human"),
            None => has_text,
        };
        is_prompt.then(|| content_text(content))
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

/// A [`Message`] that skips `usage`, for [`read_entry`]'s retry. Its fields
/// are never read: parsing into it only checks the rest of the line.
#[derive(Deserialize)]
#[expect(dead_code, reason = "parsed only to check the line, never read")]
struct MessageWithoutUsage {
    id: Option<String>,
    model: Option<String>,
    content: Option<Content>,
}

/// Reads a JSONL line as an [`Entry`].
///
/// `usage` is kept as raw JSON text (see `lenient`), which must be valid
/// UTF-8, so invalid UTF-8 inside it would fail the whole line although the
/// line parses when `usage` is skipped. Such a line gets a second try.
pub(crate) fn read_entry(line: &[u8]) -> Result<Entry, serde_json::Error> {
    serde_json::from_slice(line).or_else(|err| entry_with_invalid_utf8_in_usage(line).ok_or(err))
}

/// The entry of a line whose only invalid UTF-8 is inside `message.usage`,
/// read with the invalid bytes replaced by U+FFFD; `None` for any other line.
///
/// The line is first parsed with `usage` skipped, which does not check its
/// UTF-8. That keeps a line with invalid UTF-8 anywhere else (a text block,
/// `cwd`, ...) unparsed, as it is on `main`: only the damage inside `usage`
/// is forgiven, and the readable token figures stay.
fn entry_with_invalid_utf8_in_usage(line: &[u8]) -> Option<Entry> {
    if std::str::from_utf8(line).is_ok() {
        return None;
    }
    serde_json::from_slice::<Entry<MessageWithoutUsage>>(line).ok()?;
    serde_json::from_str(&String::from_utf8_lossy(line)).ok()
}

#[derive(Deserialize)]
pub struct Message {
    pub id: Option<String>,
    pub model: Option<String>,
    pub content: Option<Content>,
    #[serde(default, deserialize_with = "lenient_object")]
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Blocks(Vec<Block>),
    Other(serde::de::IgnoredAny),
}

/// Token counts of a message. A value of an unexpected JSON type counts as
/// missing, so it never turns a parsed line into an unparsed one.
#[derive(Deserialize)]
pub struct Usage {
    #[serde(default, deserialize_with = "lenient")]
    pub input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub output_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub cache_read_input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub cache_creation_input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient_object")]
    pub output_tokens_details: Option<OutputDetails>,
}

#[derive(Deserialize)]
pub struct OutputDetails {
    /// Already part of `output_tokens`.
    #[serde(default, deserialize_with = "lenient")]
    pub thinking_tokens: Option<u64>,
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
    let entry = read_entry(line).ok()?;
    let Content::Blocks(blocks) = entry.message?.content? else {
        return None;
    };
    blocks
        .iter()
        .find(|b| b.is_result() && b.tool_use_id.as_deref() == Some(call_id))
        .map(|b| result_text(b.content.as_ref()))
}

/// The text of message content: a string as is, text blocks joined by
/// newlines.
pub(super) fn content_text(content: &Content) -> String {
    match content {
        Content::Text(s) => s.clone(),
        Content::Blocks(blocks) => blocks
            .iter()
            .filter(|b| b.kind.as_deref() == Some("text"))
            .filter_map(|b| b.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n"),
        Content::Other(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An assistant line with an `advisor_tool_result` block `c1` whose
    /// content is `content`, and the raw `usage` JSON.
    fn advisor_line(content: &[u8], usage: &[u8]) -> Vec<u8> {
        let mut line = br#"{"type":"assistant","message":{"id":"m","content":[{"type":"advisor_tool_result","tool_use_id":"c1","content":""#.to_vec();
        line.extend_from_slice(content);
        line.extend_from_slice(br#""}],"usage":"#);
        line.extend_from_slice(usage);
        line.extend_from_slice(b"}}");
        line
    }

    #[test]
    fn a_result_is_read_from_a_line_with_invalid_utf8_in_usage() {
        let line = advisor_line(b"hello", b"{\"output_tokens\":7,\"service_tier\":\"\xff\"}");
        // The unmodified parse fails; the retry recovers the line.
        assert!(serde_json::from_slice::<Entry>(&line).is_err());
        assert_eq!(extract_result_text(&line, "c1").as_deref(), Some("hello"));
    }

    #[test]
    fn a_result_is_not_read_from_a_line_with_invalid_utf8_in_the_result() {
        let line = advisor_line(b"hel\xfflo", br#"{"output_tokens":7}"#);
        assert_eq!(extract_result_text(&line, "c1"), None);
    }
}
