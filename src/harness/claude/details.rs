//! What the details overlay shows about a Claude section (SPEC cycle 2 §5.2,
//! §5.3): the fields Claude Code records on the section's lines.
//!
//! The assembler keeps only the byte ranges of those lines (see
//! [`TurnSources`](crate::model::TurnSources)); the line is read again when
//! the overlay opens, as [`extract_result_text`](super::wire) does for large
//! tool results. The fields vary across Claude Code versions, so they are
//! never parsed into the model, and nothing here is shared with
//! [`wire::Entry`](super::wire::Entry), which the assembler reads.
//!
//! The heads below read what the overlay needs and skip the rest of the line
//! without building a value for it. In particular `message.content` is not
//! read at all, and `toolUseResult` (stdout, whole files) goes through an
//! object-or-ignored head. A field of an unexpected JSON type counts as
//! missing and never fails the line. A key that appears twice in an object
//! does fail the line (the derived heads reject a duplicate), which shows
//! `unavailable`; Claude Code does not write such lines.

use std::fmt;
use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::value::MapAccessDeserializer;
use serde::de::{Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};

use crate::model::{
    SectionSource, SourceGroup, SourceGroupKind, SourceKind, SourceRef, SourceRow, SourceValue,
};

/// The groups Claude can give for `source`: Message, Tool call or Prompt by
/// the kind of section, and Environment for all of them.
///
/// A group whose line cannot be read (the file is gone, the range is past its
/// end, the line is not JSON) is `unavailable` and holds what could be read.
pub fn section_details(source: &SectionSource) -> Vec<SourceGroup> {
    let line = read_line(&source.refs.line);
    let mut groups = Vec::new();
    match source.kind {
        SourceKind::Message => groups.push(message_group(line.as_ref())),
        SourceKind::ToolCall => {
            let result = source.refs.result.as_ref().map(read_line);
            groups.push(tool_group(line.as_ref(), result.as_ref()));
        }
        SourceKind::Prompt => groups.push(prompt_group(line.as_ref())),
        SourceKind::Other => {}
    }
    groups.push(environment_group(line.as_ref()));
    groups
}

/// Reads and parses a line; `None` when it cannot be done.
fn read_line(source: &SourceRef) -> Option<Line> {
    let bytes = source.read().ok()?;
    let mut json = serde_json::Deserializer::from_slice(&bytes);
    // Only an object is a transcript line (a derived struct would also be
    // read from an array).
    let line = pick_object::<_, Line>(&mut json).ok()??;
    json.end().ok()?;
    Some(line)
}

/// A group made of `rows`; `unavailable` when the line was not read.
fn group(
    kind: SourceGroupKind,
    line: Option<&Line>,
    rows: impl FnOnce(&Line) -> Vec<SourceRow>,
) -> SourceGroup {
    match line {
        Some(line) => SourceGroup {
            kind,
            lead: Vec::new(),
            rows: rows(line),
            unavailable: false,
        },
        None => SourceGroup::unavailable(kind),
    }
}

fn message_group(line: Option<&Line>) -> SourceGroup {
    group(SourceGroupKind::Message, line, |line| {
        let message = line.message.as_ref();
        let usage = message.and_then(|m| m.usage.as_ref());
        let cache = usage.and_then(|u| u.cache_creation.as_ref());
        let server = usage.and_then(|u| u.server_tool_use.as_ref());
        let cache_miss = line
            .diagnostics
            .as_ref()
            .and_then(|d| d.cache_miss_reason.as_deref())
            .or_else(|| {
                message?
                    .diagnostics
                    .as_ref()
                    .and_then(|d| d.cache_miss_reason.as_deref())
            });
        let mut rows = Rows::default();
        rows.text("message.id", message.and_then(|m| m.id.as_deref()));
        rows.text("requestId", line.request_id.as_deref());
        rows.time("timestamp", line.timestamp.as_deref());
        rows.text("requestedModel", line.requested_model.as_deref());
        rows.text(
            "stop_reason",
            message.and_then(|m| m.stop_reason.as_deref()),
        );
        rows.text(
            "service_tier",
            usage.and_then(|u| u.service_tier.as_deref()),
        );
        rows.text("speed", usage.and_then(|u| u.speed.as_deref()));
        rows.text(
            "inference_geo",
            usage.and_then(|u| u.inference_geo.as_deref()),
        );
        rows.tokens(
            "5m cache writes",
            cache.and_then(|c| c.ephemeral_5m_input_tokens),
        );
        rows.tokens(
            "1h cache writes",
            cache.and_then(|c| c.ephemeral_1h_input_tokens),
        );
        rows.count("web searches", server.and_then(|s| s.web_search_requests));
        rows.count("web fetches", server.and_then(|s| s.web_fetch_requests));
        rows.text("diagnostics.cache_miss_reason", cache_miss);
        rows.0
    })
}

/// The Tool call group: `start` from the call's line (it leads), the rest
/// from the result's line. Either line failing makes the group unavailable,
/// after what was read.
fn tool_group(call: Option<&Line>, result: Option<&Option<Line>>) -> SourceGroup {
    let mut lead = Rows::default();
    if let Some(call) = call {
        lead.time("start", call.timestamp.as_deref());
    }
    let mut rows = Rows::default();
    if let Some(Some(result)) = result {
        let from = result.tool_use_result.as_ref();
        rows.text(
            "permissionDecision",
            result
                .permission_decision
                .as_deref()
                .or_else(|| from?.permission_decision.as_deref()),
        );
        rows.bool("sandboxed", result.sandboxed.or_else(|| from?.sandboxed));
        rows.text(
            "resolvedModel",
            result
                .resolved_model
                .as_deref()
                .or_else(|| from?.resolved_model.as_deref()),
        );
        rows.bool("isAsync", result.is_async.or_else(|| from?.is_async));
    }
    SourceGroup {
        kind: SourceGroupKind::ToolCall,
        lead: lead.0,
        rows: rows.0,
        // A call with no result yet has no result line, which is no failure.
        unavailable: call.is_none() || matches!(result, Some(None)),
    }
}

fn prompt_group(line: Option<&Line>) -> SourceGroup {
    group(SourceGroupKind::Prompt, line, |line| {
        let mut rows = Rows::default();
        rows.time("timestamp", line.timestamp.as_deref());
        rows.text("permissionMode", line.permission_mode.as_deref());
        rows.text("promptSource", line.prompt_source.as_deref());
        rows.0
    })
}

fn environment_group(line: Option<&Line>) -> SourceGroup {
    group(SourceGroupKind::Environment, line, |line| {
        let mut rows = Rows::default();
        rows.text("gitBranch", line.git_branch.as_deref());
        rows.text("cwd", line.cwd.as_deref());
        rows.text("version", line.version.as_deref());
        rows.text("entrypoint", line.entrypoint.as_deref());
        rows.0
    })
}

/// Rows being collected: a field that is missing adds none.
#[derive(Default)]
struct Rows(Vec<SourceRow>);

impl Rows {
    fn push(&mut self, label: &str, value: Option<SourceValue>) {
        if let Some(value) = value {
            self.0.push(SourceRow::new(label, value));
        }
    }

    /// A text field; an empty string is no value, as for `effort` and
    /// `stop_reason` elsewhere.
    fn text(&mut self, label: &str, value: Option<&str>) {
        self.push(
            label,
            value
                .filter(|v| !v.is_empty())
                .map(|v| SourceValue::Text(v.to_owned())),
        );
    }

    fn bool(&mut self, label: &str, value: Option<bool>) {
        self.push(label, value.map(|v| SourceValue::Text(v.to_string())));
    }

    fn tokens(&mut self, label: &str, value: Option<u64>) {
        self.push(label, value.map(SourceValue::Tokens));
    }

    fn count(&mut self, label: &str, value: Option<u64>) {
        self.push(label, value.map(SourceValue::Count));
    }

    /// A timestamp: a point in time when it parses, else as written.
    fn time(&mut self, label: &str, value: Option<&str>) {
        self.push(
            label,
            value
                .filter(|v| !v.is_empty())
                .map(|v| match DateTime::parse_from_rfc3339(v) {
                    Ok(time) => SourceValue::Time(time.with_timezone(&Utc)),
                    Err(_) => SourceValue::Text(v.to_owned()),
                }),
        );
    }
}

// -- the typed heads

/// The fields of a line the overlay can show. Everything else is skipped.
#[derive(Deserialize)]
struct Line {
    #[serde(default, deserialize_with = "pick")]
    timestamp: Option<String>,
    #[serde(default, rename = "requestId", deserialize_with = "pick")]
    request_id: Option<String>,
    #[serde(default, rename = "requestedModel", deserialize_with = "pick")]
    requested_model: Option<String>,
    #[serde(default, rename = "gitBranch", deserialize_with = "pick")]
    git_branch: Option<String>,
    #[serde(default, deserialize_with = "pick")]
    cwd: Option<String>,
    #[serde(default, deserialize_with = "pick")]
    version: Option<String>,
    #[serde(default, deserialize_with = "pick")]
    entrypoint: Option<String>,
    #[serde(default, rename = "permissionMode", deserialize_with = "pick")]
    permission_mode: Option<String>,
    #[serde(default, rename = "promptSource", deserialize_with = "pick")]
    prompt_source: Option<String>,
    #[serde(default, rename = "permissionDecision", deserialize_with = "pick_text")]
    permission_decision: Option<String>,
    #[serde(default, deserialize_with = "pick")]
    sandboxed: Option<bool>,
    #[serde(default, rename = "resolvedModel", deserialize_with = "pick")]
    resolved_model: Option<String>,
    #[serde(default, rename = "isAsync", deserialize_with = "pick")]
    is_async: Option<bool>,
    #[serde(default, deserialize_with = "pick_object")]
    diagnostics: Option<Diagnostics>,
    /// `message.content` is left out on purpose: it is skipped, never read.
    #[serde(default, deserialize_with = "pick_object")]
    message: Option<MessageHead>,
    /// An object, or anything else (a failed call has its error message
    /// there as a string), which is ignored. Never loaded as a value: it can
    /// be very large.
    #[serde(default, rename = "toolUseResult", deserialize_with = "pick_object")]
    tool_use_result: Option<ResultHead>,
}

#[derive(Deserialize)]
struct Diagnostics {
    #[serde(default, deserialize_with = "pick_text")]
    cache_miss_reason: Option<String>,
}

#[derive(Deserialize)]
struct MessageHead {
    #[serde(default, deserialize_with = "pick")]
    id: Option<String>,
    #[serde(default, deserialize_with = "pick")]
    stop_reason: Option<String>,
    #[serde(default, deserialize_with = "pick_object")]
    usage: Option<UsageHead>,
    #[serde(default, deserialize_with = "pick_object")]
    diagnostics: Option<Diagnostics>,
}

#[derive(Deserialize)]
struct UsageHead {
    #[serde(default, deserialize_with = "pick")]
    service_tier: Option<String>,
    #[serde(default, deserialize_with = "pick")]
    speed: Option<String>,
    #[serde(default, deserialize_with = "pick")]
    inference_geo: Option<String>,
    #[serde(default, deserialize_with = "pick_object")]
    cache_creation: Option<CacheCreation>,
    #[serde(default, deserialize_with = "pick_object")]
    server_tool_use: Option<ServerToolUse>,
}

#[derive(Deserialize)]
struct CacheCreation {
    #[serde(default, deserialize_with = "pick")]
    ephemeral_5m_input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "pick")]
    ephemeral_1h_input_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct ServerToolUse {
    #[serde(default, deserialize_with = "pick")]
    web_search_requests: Option<u64>,
    #[serde(default, deserialize_with = "pick")]
    web_fetch_requests: Option<u64>,
}

/// The fields of a `toolUseResult` object the overlay shows.
#[derive(Deserialize)]
struct ResultHead {
    #[serde(default, rename = "permissionDecision", deserialize_with = "pick_text")]
    permission_decision: Option<String>,
    #[serde(default, deserialize_with = "pick")]
    sandboxed: Option<bool>,
    #[serde(default, rename = "resolvedModel", deserialize_with = "pick")]
    resolved_model: Option<String>,
    #[serde(default, rename = "isAsync", deserialize_with = "pick")]
    is_async: Option<bool>,
}

// -- lenient readers

/// A JSON scalar that can be picked from a value of the expected type.
trait Pick: Sized {
    fn from_str(_value: &str) -> Option<Self> {
        None
    }
    fn from_bool(_value: bool) -> Option<Self> {
        None
    }
    fn from_u64(_value: u64) -> Option<Self> {
        None
    }
}

impl Pick for String {
    fn from_str(value: &str) -> Option<Self> {
        Some(value.to_owned())
    }
}

impl Pick for bool {
    fn from_bool(value: bool) -> Option<Self> {
        Some(value)
    }
}

impl Pick for u64 {
    fn from_u64(value: u64) -> Option<Self> {
        Some(value)
    }
}

/// Reads a scalar as `T`; a value of another JSON type is skipped and counts
/// as missing.
struct PickVisitor<T>(PhantomData<T>);

/// Skips the rest of a sequence or a map without building anything.
fn skip_seq<'de, A: SeqAccess<'de>>(mut seq: A) -> Result<(), A::Error> {
    while seq.next_element::<IgnoredAny>()?.is_some() {}
    Ok(())
}

fn skip_map<'de, A: MapAccess<'de>>(mut map: A) -> Result<(), A::Error> {
    while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
    Ok(())
}

impl<'de, T: Pick> Visitor<'de> for PickVisitor<T> {
    type Value = Option<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(T::from_str(value))
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(T::from_bool(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(T::from_u64(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(u64::try_from(value).ok().and_then(T::from_u64))
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
        skip_seq(seq).map(|()| None)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        skip_map(map).map(|()| None)
    }
}

/// `deserialize_with` for a scalar field: its value when it has the JSON type
/// of `T`, else missing.
fn pick<'de, D: Deserializer<'de>, T: Pick>(d: D) -> Result<Option<T>, D::Error> {
    d.deserialize_any(PickVisitor(PhantomData))
}

/// Reads an object as `T`; a value of another JSON type is skipped and counts
/// as missing.
struct ObjectVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
    type Value = Option<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
        skip_seq(seq).map(|()| None)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        T::deserialize(MapAccessDeserializer::new(map)).map(Some)
    }
}

/// `deserialize_with` for an object field: the object read as `T`, else
/// missing. Fields of the object that `T` does not name are skipped.
fn pick_object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    d.deserialize_any(ObjectVisitor(PhantomData))
}

/// A value that is text or a small object of text members (Claude writes a
/// permission decision as `{"decision":"accept","source":"config"}`): text as
/// it is, an object as `key=value` pairs of its scalar members.
struct TextVisitor;

impl<'de> Visitor<'de> for TextVisitor {
    type Value = Option<String>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Some(value.to_owned()))
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
        skip_seq(seq).map(|()| None)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut pairs: Vec<String> = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if let Member(Some(value)) = map.next_value()? {
                pairs.push(format!("{key}={value}"));
            }
        }
        Ok((!pairs.is_empty()).then(|| pairs.join(", ")))
    }
}

/// `deserialize_with` for a field that is text or an object of text members.
fn pick_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    d.deserialize_any(TextVisitor)
}

/// A member of a small object, as text: strings, booleans and numbers. A
/// nested object or array is skipped (`None`).
struct Member(Option<String>);

impl<'de> Deserialize<'de> for Member {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Member;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("any JSON value")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                Ok(Member(Some(value.to_owned())))
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(Member(Some(value.to_string())))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(Member(Some(value.to_string())))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(Member(Some(value.to_string())))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E> {
                Ok(Member(Some(value.to_string())))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Member(None))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                skip_seq(seq).map(|()| Member(None))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                skip_map(map).map(|()| Member(None))
            }
        }

        d.deserialize_any(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Option<Line> {
        let mut de = serde_json::Deserializer::from_str(json);
        let line = pick_object::<_, Line>(&mut de).ok()??;
        de.end().ok()?;
        Some(line)
    }

    #[test]
    fn tool_use_result_is_a_typed_head_and_not_a_value() {
        // The field's type is the head: a result of any size is never
        // materialised as a `serde_json::Value` (or buffered by `untagged`).
        let line = parse(r#"{"toolUseResult":{"isAsync":true,"stdout":"x"}}"#).unwrap();
        let _: &Option<ResultHead> = &line.tool_use_result;
        assert_eq!(line.tool_use_result.unwrap().is_async, Some(true));
    }

    #[test]
    fn tool_use_result_of_another_type_than_an_object_counts_as_missing() {
        for raw in [r#""Error: boom""#, "[true]", "true", "7", "null", "{}"] {
            let line = parse(&format!(r#"{{"toolUseResult":{raw}}}"#)).unwrap();
            let is_async = line.tool_use_result.and_then(|r| r.is_async);
            assert_eq!(is_async, None, "{raw}");
        }
    }

    #[test]
    fn a_big_tool_use_result_and_message_content_do_not_get_in_the_way() {
        let big = "z".repeat(1 << 20);
        let line = parse(&format!(
            r#"{{"message":{{"id":"m","content":[{{"type":"text","text":"{big}"}}]}},
                "toolUseResult":{{"stdout":"{big}","isAsync":false}},"cwd":"/p"}}"#
        ))
        .unwrap();
        assert_eq!(line.cwd.as_deref(), Some("/p"));
        assert_eq!(line.message.unwrap().id.as_deref(), Some("m"));
        assert_eq!(line.tool_use_result.unwrap().is_async, Some(false));
    }

    #[test]
    fn only_a_json_object_is_a_line() {
        for raw in ["[1,2]", "1", r#""x""#, "null", "{} {}", "{"] {
            assert!(parse(raw).is_none(), "{raw}");
        }
        assert!(parse("{}").is_some());
    }

    #[test]
    fn a_permission_decision_is_text_or_its_scalar_members() {
        let text = |raw: &str| {
            parse(&format!(r#"{{"permissionDecision":{raw}}}"#))
                .unwrap()
                .permission_decision
        };
        assert_eq!(text(r#""allow""#).as_deref(), Some("allow"));
        assert_eq!(
            text(r#"{"decision":"accept","n":2,"on":true,"nested":{"a":1},"list":[1]}"#).as_deref(),
            Some("decision=accept, n=2, on=true")
        );
        for raw in ["{}", "5", "true", "null", "[1]", r#"{"a":{"b":1}}"#] {
            assert_eq!(text(raw), None, "{raw}");
        }
    }
}
