//! Typed views of the JSONL lines the assembler cares about.
//!
//! Every field is optional and unknown fields are ignored, so added or removed
//! fields degrade to missing data. A known field with a different JSON type
//! (other than `null`) still fails the line (or, inside `message.content`, the
//! whole content), which the assembler then skips. The exception is
//! `message.usage`, which is read leniently: a value of the wrong type there
//! counts as missing instead of failing the line.

use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::Value;
use serde_json::value::RawValue;

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

/// A JSONL line. `M` is the type of `message`, `T` that of `toolUseResult`
/// and `F` that of `isApiErrorMessage`; only [`read_entry`] uses other ones
/// than the defaults.
#[derive(Deserialize)]
pub struct Entry<M = Message, T = ToolUseResult, F = ApiErrorFlag> {
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
    /// The structured result of a tool call (`tool_result` entries). Only
    /// `isAsync` is read from it; see [`ToolUseResult`].
    #[serde(rename = "toolUseResult")]
    pub tool_use_result: Option<T>,
    /// Set on the line that carries an API error as its text. A value of
    /// another type than a boolean counts as missing.
    #[serde(rename = "isApiErrorMessage")]
    pub is_api_error_message: Option<F>,
}

/// An entry's `isApiErrorMessage`: `true` only for a boolean `true`; a value
/// of another type counts as missing. Like `message.usage` it is read as raw
/// text, so the check parse in [`read_entry`]'s retry reads the field as
/// [`IgnoredAny`] instead, which does not check its UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiErrorFlag(pub bool);

impl<'de> Deserialize<'de> for ApiErrorFlag {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let flag: Option<bool> = lenient(deserializer)?;
        Ok(Self(flag == Some(true)))
    }
}

impl<T> Entry<Message, T> {
    /// The entry with `tool_use_result` as its `toolUseResult`: the one that
    /// was read in another way.
    fn with_tool_use_result(self, tool_use_result: Option<ToolUseResult>) -> Entry {
        Entry {
            content: self.content,
            command_run: self.command_run,
            attachment: self.attachment,
            is_compact_summary: self.is_compact_summary,
            uuid: self.uuid,
            timestamp: self.timestamp,
            is_meta: self.is_meta,
            origin: self.origin,
            message: self.message,
            effort: self.effort,
            per_turn_effort: self.per_turn_effort,
            api_block_index: self.api_block_index,
            cwd: self.cwd,
            duration_ms: self.duration_ms,
            tool_use_result,
            is_api_error_message: self.is_api_error_message,
        }
    }
}

/// An entry's `toolUseResult`: an object, a string (a failed call has its
/// error message there), or anything else.
///
/// It can be very large (stdout, whole files), so it is not kept as a
/// `Value`: the head reads `isAsync` and the rest is dropped. (`untagged`
/// buffers the value while it tries the arms, so it is read once more than a
/// skipped field would be.) An `isAsync` of another type than a boolean makes
/// the object arm fail, so the value falls to [`Other`](Self::Other) and
/// counts as absent.
///
/// Buffering also checks the value, which skipping it does not (lone
/// surrogate escapes, numbers out of range, invalid UTF-8, deep nesting).
/// A value that fails there would fail the line, so [`read_entry`] reads such
/// a line again with the field unchecked.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum ToolUseResult {
    Object(ToolUseResultHead),
    Other(serde::de::IgnoredAny),
}

/// The one field of a `toolUseResult` object the assembler reads.
///
/// Read by hand so that only a JSON object is accepted (a derived struct
/// would also be read from an array, and `[true]` is not a launch); any
/// field other than `isAsync` is skipped.
pub struct ToolUseResultHead {
    /// `true` for an asynchronous subagent: the result only records the
    /// launch.
    pub is_async: Option<bool>,
}

impl<'de> Deserialize<'de> for ToolUseResultHead {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct HeadVisitor;

        impl<'de> serde::de::Visitor<'de> for HeadVisitor {
            type Value = ToolUseResultHead;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a toolUseResult object")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut is_async = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "isAsync" {
                        is_async = map.next_value::<Option<bool>>()?;
                    } else {
                        map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
                Ok(ToolUseResultHead { is_async })
            }
        }

        deserializer.deserialize_map(HeadVisitor)
    }
}

impl Entry {
    /// Whether the entry's `toolUseResult` says the call ran asynchronously,
    /// so that the result is the launch and not the end of the call.
    pub fn is_async_launch(&self) -> bool {
        matches!(
            self.tool_use_result,
            Some(ToolUseResult::Object(ToolUseResultHead {
                is_async: Some(true)
            }))
        )
    }

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
    /// entries without an `origin`, entries without a text block, and local
    /// commands (see [`local_command_text`](Self::local_command_text)). Human
    /// prompts delivered as `queued_command` attachments are separate (see
    /// `queued_prompt_text`).
    pub fn human_prompt_text(&self) -> Option<String> {
        if self.local_command_text().is_some() {
            return None;
        }
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

    /// The raw text of this entry when it is local-command content (SPEC
    /// cycle 3 §2.1), else `None`: a command echo or its output, handled by
    /// the harness and not a prompt to the model.
    ///
    /// All of these hold: the entry has no `origin` field; it is not `isMeta`,
    /// not a compact summary and holds no `tool_result`; and its text, with
    /// leading whitespace trimmed, starts with `<command-name>` (the echo),
    /// `<local-command-stdout>` or `<local-command-stderr>` (the output).
    pub fn local_command_text(&self) -> Option<String> {
        if self.origin.is_some()
            || self.is_meta == Some(true)
            || self.is_compact_summary == Some(true)
        {
            return None;
        }
        let content = self.message.as_ref()?.content.as_ref()?;
        if let Content::Blocks(blocks) = content
            && blocks
                .iter()
                .any(|b| b.kind.as_deref() == Some("tool_result"))
        {
            return None;
        }
        let text = content_text(content);
        let start = text.trim_start();
        [
            "<command-name>",
            "<local-command-stdout>",
            "<local-command-stderr>",
        ]
        .iter()
        .any(|tag| start.starts_with(tag))
        .then_some(text)
    }

    /// Whether the entry (or its message) is flagged as an API error.
    pub fn is_api_error(&self) -> bool {
        self.is_api_error_message == Some(ApiErrorFlag(true))
            || self
                .message
                .as_ref()
                .is_some_and(|m| m.is_api_error_message == Some(true))
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

/// A [`Message`] that skips `usage`, `stop_reason` and `isApiErrorMessage`,
/// for [`read_entry`]'s retry. Its fields are never read: parsing into it only
/// checks the rest of the line.
///
/// Leaving the lenient fields out is deliberate: invalid UTF-8 inside them
/// keeps the line parsed, as it did before they were read, so they must not
/// be added here (the same goes for the entry's own `isApiErrorMessage`, read
/// as `IgnoredAny` in the check parse).
#[derive(Deserialize)]
#[expect(dead_code, reason = "parsed only to check the line, never read")]
struct MessageWithoutUsage {
    id: Option<String>,
    model: Option<String>,
    content: Option<Content>,
}

/// Reads a JSONL line as an [`Entry`].
///
/// A line that does not parse gets more tries, so that every line that parses
/// with `toolUseResult`, `usage`, `message.stop_reason` and both
/// `isApiErrorMessage` fields skipped still parses:
/// - `toolUseResult` is checked while it is read (see [`ToolUseResult`]), so
///   a value in it that a skipped field would not mind fails the line. The
///   line is read again with the field kept as raw text, which is not
///   checked, and `isAsync` is looked for in that. If even that fails
///   (invalid UTF-8), the field counts as absent.
/// - `usage`, `message.stop_reason` and the two `isApiErrorMessage` fields are
///   kept as raw JSON text (see `lenient`), which must be valid UTF-8, so
///   invalid UTF-8 inside one of them would fail the whole line although the
///   line parses when it is skipped.
pub(crate) fn read_entry(line: &[u8]) -> Result<Entry, serde_json::Error> {
    serde_json::from_slice(line).or_else(|err| lenient_entry(line).ok_or(err))
}

/// The entry of a line that fails to parse because of `toolUseResult`, of
/// invalid UTF-8 inside `message.usage`, `message.stop_reason`,
/// `message.isApiErrorMessage` or the top-level `isApiErrorMessage`, or of
/// several of them; `None` for any other line.
///
/// The line is first parsed with `toolUseResult` unchecked. If that fails
/// too, the damage is looked for in the other fields: the line is parsed with
/// `usage`, `stop_reason` and both `isApiErrorMessage` fields skipped as
/// well, which does not check their UTF-8. That keeps a line with invalid
/// UTF-8 anywhere else (a text block, `cwd`, ...) unparsed, as it is on
/// `main`: only damage inside those fields is forgiven, and the readable
/// values of the rest stay (a damaged one counts as missing or lossy).
fn lenient_entry(line: &[u8]) -> Option<Entry> {
    if let Some(entry) = entry_with_unchecked_tool_use_result(line) {
        return Some(entry);
    }
    if std::str::from_utf8(line).is_ok() {
        return None;
    }
    serde_json::from_slice::<Entry<MessageWithoutUsage, IgnoredAny, IgnoredAny>>(line).ok()?;
    let text = String::from_utf8_lossy(line);
    serde_json::from_str(&text)
        .ok()
        .or_else(|| unchecked_str(&text))
}

/// The entry of `line`, `toolUseResult` kept as raw text (not checked) and
/// read for `isAsync` only; absent when it cannot even be kept as text.
fn entry_with_unchecked_tool_use_result(line: &[u8]) -> Option<Entry> {
    if let Ok(entry) = serde_json::from_slice::<Entry<Message, Box<RawValue>>>(line) {
        return Some(entry.with_raw_tool_use_result());
    }
    serde_json::from_slice::<Entry<Message, IgnoredAny>>(line)
        .ok()
        .map(|entry| entry.with_tool_use_result(None))
}

/// [`entry_with_unchecked_tool_use_result`] for text.
fn unchecked_str(text: &str) -> Option<Entry> {
    if let Ok(entry) = serde_json::from_str::<Entry<Message, Box<RawValue>>>(text) {
        return Some(entry.with_raw_tool_use_result());
    }
    serde_json::from_str::<Entry<Message, IgnoredAny>>(text)
        .ok()
        .map(|entry| entry.with_tool_use_result(None))
}

impl Entry<Message, Box<RawValue>> {
    /// The entry with `isAsync` read from the raw `toolUseResult`: a value
    /// that is not an object with a boolean `isAsync` counts as absent.
    fn with_raw_tool_use_result(self) -> Entry {
        let head = self
            .tool_use_result
            .as_ref()
            .and_then(|raw| serde_json::from_str::<ToolUseResultHead>(raw.get()).ok())
            .map(ToolUseResult::Object);
        self.with_tool_use_result(head)
    }
}

#[derive(Deserialize)]
pub struct Message {
    pub id: Option<String>,
    pub model: Option<String>,
    pub content: Option<Content>,
    #[serde(default, deserialize_with = "lenient_object")]
    pub usage: Option<Usage>,
    /// Why the message ended; null on most lines. A value of another type
    /// than a string counts as missing.
    #[serde(default, deserialize_with = "lenient")]
    pub stop_reason: Option<String>,
    /// `isApiErrorMessage` inside the message, which counts like the one on
    /// the entry.
    #[serde(rename = "isApiErrorMessage", default, deserialize_with = "lenient")]
    pub is_api_error_message: Option<bool>,
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

    fn read(raw: &str) -> Entry {
        let line = format!(r#"{{"type":"user","toolUseResult":{raw}}}"#);
        read_entry(line.as_bytes()).unwrap()
    }

    #[test]
    fn tool_use_result_is_the_head_and_not_a_value() {
        // The field's type is the untagged head: a result of any size is
        // never materialised as a `serde_json::Value`.
        let entry = read(r#"{"isAsync":true,"stdout":"x"}"#);
        let _: &Option<ToolUseResult> = &entry.tool_use_result;
        assert!(entry.is_async_launch());
    }

    #[test]
    fn only_a_boolean_true_is_an_asynchronous_launch() {
        assert!(read(r#"{"isAsync":true}"#).is_async_launch());
        for raw in [
            r#"{"isAsync":false}"#,
            r#"{"isAsync":"true"}"#,
            r#"{"isAsync":1}"#,
            r#"{"isAsync":null}"#,
            r#"{}"#,
            r#""isAsync""#,
            "[true]",
            "true",
            "null",
        ] {
            assert!(!read(raw).is_async_launch(), "{raw}");
        }
    }

    // -- local-command content (SPEC cycle 3 §2.1)

    fn user_entry(extra: &str, content: &str) -> Entry {
        let line = format!(
            r#"{{"type":"user"{extra},"message":{{"role":"user","content":{}}}}}"#,
            serde_json::to_string(content).unwrap()
        );
        read_entry(line.as_bytes()).unwrap()
    }

    const ECHO: &str = "<command-name>/effort</command-name>\n  <command-args></command-args>";

    #[test]
    fn an_entry_without_origin_that_starts_with_a_command_tag_is_local_command_content() {
        for text in [
            ECHO,
            "<local-command-stdout>Cancelled</local-command-stdout>",
            "<local-command-stderr>oops</local-command-stderr>",
            "  \n\t<command-name>/clear</command-name>",
        ] {
            let entry = user_entry("", text);
            assert_eq!(
                entry.local_command_text().as_deref(),
                Some(text),
                "{text:?}"
            );
            assert_eq!(entry.human_prompt_text(), None, "{text:?}");
        }
    }

    #[test]
    fn an_entry_with_an_origin_is_not_local_command_content() {
        for origin in [
            r#","origin":{"kind":"human"}"#,
            r#","origin":{"kind":"task-notification"}"#,
            r#","origin":{}"#,
        ] {
            let entry = user_entry(origin, ECHO);
            assert_eq!(entry.local_command_text(), None, "{origin}");
        }
        // A human prompt that looks like an echo stays a prompt.
        let human = user_entry(r#","origin":{"kind":"human"}"#, ECHO);
        assert_eq!(human.human_prompt_text().as_deref(), Some(ECHO));
    }

    #[test]
    fn a_meta_entry_a_compact_summary_and_a_tool_result_are_not_local_command_content() {
        assert_eq!(
            user_entry(r#","isMeta":true"#, ECHO).local_command_text(),
            None
        );
        assert!(
            user_entry(r#","isMeta":false"#, ECHO)
                .local_command_text()
                .is_some()
        );
        assert_eq!(
            user_entry(r#","isCompactSummary":true"#, ECHO).local_command_text(),
            None
        );
        let result = format!(
            r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"t","content":{}}}]}}}}"#,
            serde_json::to_string(ECHO).unwrap()
        );
        let entry = read_entry(result.as_bytes()).unwrap();
        assert_eq!(entry.local_command_text(), None);
        let mixed = format!(
            r#"{{"type":"user","message":{{"content":[{{"type":"text","text":{}}},{{"type":"tool_result","tool_use_id":"t","content":"x"}}]}}}}"#,
            serde_json::to_string(ECHO).unwrap()
        );
        assert_eq!(
            read_entry(mixed.as_bytes()).unwrap().local_command_text(),
            None
        );
    }

    #[test]
    fn other_text_is_not_local_command_content() {
        for text in [
            "<command-message>factory</command-message>\n<command-name>/factory</command-name>",
            "<bash-input>ls</bash-input>",
            "<bash-stdout>x</bash-stdout>",
            "<bash-stderr>x</bash-stderr>",
            "/clear",
            "text <command-name>/clear</command-name>",
            "<local-command-caveat>The command below was run locally.</local-command-caveat>",
            "",
        ] {
            let entry = user_entry("", text);
            assert_eq!(entry.local_command_text(), None, "{text:?}");
        }
        // The `<command-message>` echo without `origin` stays a prompt under
        // the older-format rule.
        let echo = user_entry("", "<command-message>doctor</command-message>");
        assert!(echo.human_prompt_text().is_some());
    }

    #[test]
    fn the_text_is_read_from_text_blocks_too() {
        let line = r#"{"type":"user","message":{"content":[{"type":"text","text":"<command-name>/clear</command-name>"}]}}"#;
        let entry = read_entry(line.as_bytes()).unwrap();
        assert_eq!(
            entry.local_command_text().as_deref(),
            Some("<command-name>/clear</command-name>")
        );
        assert_eq!(entry.human_prompt_text(), None);
    }
}
