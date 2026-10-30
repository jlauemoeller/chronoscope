//! Assembly of `session_message` rows into turns (SPEC §5.1, §5.6).
//!
//! Feed the rows of one session in `seq` order with
//! [`push_row`](Assembler::push_row) and collect what changed with
//! [`take_updates`](Assembler::take_updates). Feeding the rows in any paging
//! yields the same final turns, because nothing is decided at a page boundary.
//!
//! - A `user` row starts a User turn.
//! - The `assistant` rows up to the next `user` row form one Assistant turn:
//!   `text` parts become `Text` items, `reasoning` parts `Thinking` items
//!   and `tool` parts `ToolCall`s, which already carry their result.
//! - `synthetic` and `system` rows are injected content, placed the way the
//!   Claude adapter places its injected entries: they wait for the next row
//!   to decide their turn. A `user` row takes them as the items before its
//!   prompt; any other row (and the end of the input) puts them into the
//!   current turn.
//! - `idle` rows are skipped.
//!
//! Compaction is not supported yet: SPEC §11 lists the OpenCode compaction
//! format as unverified (`session_pending` rows of type `compaction`), the
//! observed databases have no such table and no compaction row among the
//! messages, so no `Turn::Compaction` is ever produced and no format is
//! guessed.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use super::tools::{describe, tool_kind};
use crate::model::{
    AssistantTurn, InjectedEntry, Item, ItemId, ModelUpdate, Thinking, ToolCall, ToolOutput,
    ToolResult, ToolStatus, Turn, TurnId, TurnStats, UserTurn,
};

/// The variant that means "no particular effort".
const DEFAULT_VARIANT: &str = "default";

/// One `session_message` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// `session_message.id`.
    pub id: String,
    /// `session_message.type`: `user`, `assistant`, `synthetic`, `system`
    /// or `idle`.
    pub kind: String,
    /// `session_message.seq`.
    pub seq: i64,
    /// `session_message.time_created`, in Unix milliseconds.
    pub created: i64,
    /// `session_message.data`, a JSON document.
    pub data: String,
}

#[derive(Deserialize, Default)]
struct Time {
    created: Option<i64>,
    streamed: Option<i64>,
    completed: Option<i64>,
}

#[derive(Deserialize)]
struct UserData {
    time: Option<Time>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct NoteData {
    time: Option<Time>,
    text: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct Model {
    id: Option<String>,
    variant: Option<String>,
}

#[derive(Deserialize)]
struct Tokens {
    input: Option<u64>,
    output: Option<u64>,
}

#[derive(Deserialize)]
struct AssistantData {
    time: Option<Time>,
    model: Option<Model>,
    content: Option<Vec<Value>>,
    cost: Option<f64>,
    tokens: Option<Tokens>,
}

/// State of the Assistant turn currently being built.
struct Builder {
    /// Index of the turn in `Assembler::turns`.
    idx: usize,
    /// Position of each item in the turn, for in-place replacement.
    index: HashMap<ItemId, usize>,
    /// The model of the most recent row.
    last_model: String,
    /// Assistant rows in the turn.
    rows: usize,
    tokens: Option<u64>,
    cost: Option<f64>,
    first_created: Option<i64>,
    last_end: Option<i64>,
}

/// Incremental OpenCode turn assembler.
pub struct Assembler {
    cwd: Option<PathBuf>,
    turns: Vec<Arc<Turn>>,
    cur: Option<Builder>,
    dirty: BTreeSet<usize>,
    /// How many of `turns` `take_updates` has handed out.
    sent_len: usize,
    unparsed: usize,
    held: Vec<InjectedEntry>,
    held_key: Option<String>,
    /// Whether the last User turn already received entries flushed after its
    /// prompt (they collapse with later consecutive entries).
    user_flushed: bool,
    /// Row types already reported as unknown.
    unknown: BTreeSet<String>,
}

impl Default for Assembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Assembler {
    /// An empty assembler.
    pub fn new() -> Self {
        Self {
            cwd: None,
            turns: Vec::new(),
            cur: None,
            dirty: BTreeSet::new(),
            sent_len: 0,
            unparsed: 0,
            held: Vec::new(),
            held_key: None,
            user_flushed: false,
            unknown: BTreeSet::new(),
        }
    }

    /// Sets the session directory, used to shorten paths in descriptions.
    #[must_use]
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// The turns assembled so far.
    pub fn turns(&self) -> &[Arc<Turn>] {
        &self.turns
    }

    /// How many rows were skipped because their `data` could not be parsed.
    pub fn unparsed_count(&self) -> usize {
        self.unparsed
    }

    /// Declares the end of the input: injected entries still waiting go into
    /// the current turn. With no turn to take them (none yet) they are
    /// dropped.
    pub fn finish(&mut self) {
        self.flush_current();
        if !self.held.is_empty() {
            log::debug!(
                "{} injected entries have no turn to go into",
                self.held.len()
            );
        }
    }

    /// Consumes the assembler, returning its turns, after [`finish`](Self::finish).
    pub fn into_turns(mut self) -> Vec<Arc<Turn>> {
        self.finish();
        self.turns
    }

    /// The changes since the last call: the already handed out turns that
    /// changed (`Updated`), then the new turns (`Appended`).
    pub fn take_updates(&mut self) -> Vec<ModelUpdate> {
        let dirty = std::mem::take(&mut self.dirty);
        let updated: Vec<Arc<Turn>> = dirty
            .into_iter()
            .filter(|&i| i < self.sent_len)
            .map(|i| Arc::clone(&self.turns[i]))
            .collect();
        let appended = self.turns[self.sent_len..].to_vec();
        self.sent_len = self.turns.len();
        let mut out = Vec::new();
        if !updated.is_empty() {
            out.push(ModelUpdate::Updated(updated));
        }
        if !appended.is_empty() {
            out.push(ModelUpdate::Appended(appended));
        }
        out
    }

    /// Feeds one row. Rows must come in `seq` order.
    pub fn push_row(&mut self, row: &Row) {
        match row.kind.as_str() {
            "user" => {
                if let Some(data) = self.parse::<UserData>(row) {
                    let ts = timestamp(data.time.as_ref(), row);
                    self.start_user_turn(TurnId::from(row.id.as_str()), ts, data.text);
                }
            }
            "assistant" => {
                if let Some(data) = self.parse::<AssistantData>(row) {
                    self.on_assistant(row, data);
                }
            }
            "synthetic" | "system" => {
                if let Some(data) = self.parse::<NoteData>(row) {
                    let ts = timestamp(data.time.as_ref(), row);
                    let text = note_text(&row.kind, &data);
                    self.hold(&row.kind, ts, text, &row.id);
                }
            }
            "idle" => {}
            other => {
                if self.unknown.insert(other.to_owned()) {
                    log::info!("skipping rows of unknown type {other:?}");
                }
            }
        }
    }

    /// Parses a row's `data`; a failure counts the row as unparsed.
    fn parse<T: for<'de> Deserialize<'de>>(&mut self, row: &Row) -> Option<T> {
        // `data` is a JSON object; serde would also read a struct from an
        // array, which is not this format.
        let parsed = if row.data.trim_start().starts_with('{') {
            serde_json::from_str(&row.data)
        } else {
            Err(<serde_json::Error as serde::de::Error>::custom(
                "data is not a JSON object",
            ))
        };
        match parsed {
            Ok(data) => Some(data),
            Err(err) => {
                self.unparsed += 1;
                log::warn!(
                    "session_message {} (seq {}, type {}): skipped, {err}",
                    row.id,
                    row.seq,
                    row.kind
                );
                None
            }
        }
    }

    /// Holds an injected entry until the next row decides its turn.
    fn hold(&mut self, kind: &str, timestamp: DateTime<Utc>, text: String, id: &str) {
        if self.held.is_empty() {
            self.held_key = Some(id.to_owned());
        }
        self.held.push(InjectedEntry {
            kind: kind.to_owned(),
            timestamp,
            text,
        });
    }

    /// Starts a User turn; held injected entries become its first item.
    fn start_user_turn(&mut self, id: TurnId, timestamp: DateTime<Utc>, text: Option<String>) {
        self.cur = None;
        self.user_flushed = false;
        let items = take_held(&mut self.held, &mut self.held_key)
            .into_iter()
            .collect();
        self.turns.push(Arc::new(Turn::User(UserTurn {
            id,
            timestamp,
            text: text.unwrap_or_default(),
            items,
        })));
    }

    /// Places held injected entries in the current turn: the open Assistant
    /// turn, or a User turn that has no Assistant turn yet. Without such a
    /// turn they stay held for the next one.
    fn flush_current(&mut self) {
        if self.held.is_empty() {
            return;
        }
        if let Some(b) = self.cur.as_mut() {
            let Some(item) = take_held(&mut self.held, &mut self.held_key) else {
                return;
            };
            if let Turn::Assistant(turn) = Arc::make_mut(&mut self.turns[b.idx]) {
                match (turn.items.last_mut(), item) {
                    (
                        Some(Item::Injected { entries, .. }),
                        Item::Injected { entries: more, .. },
                    ) => entries.extend(more),
                    (_, item) => {
                        upsert(turn, b, item);
                    }
                }
                self.dirty.insert(b.idx);
            }
        } else if let Some(last) = self.turns.len().checked_sub(1)
            && matches!(&*self.turns[last], Turn::User(_))
            && let Some(item) = take_held(&mut self.held, &mut self.held_key)
            && let Turn::User(turn) = Arc::make_mut(&mut self.turns[last])
        {
            match (turn.items.last_mut(), item) {
                (Some(Item::Injected { entries, .. }), Item::Injected { entries: more, .. })
                    if self.user_flushed =>
                {
                    entries.extend(more);
                }
                (_, item) => turn.items.push(item),
            }
            self.user_flushed = true;
            self.dirty.insert(last);
        }
    }

    fn on_assistant(&mut self, row: &Row, data: AssistantData) {
        self.flush_current();
        let held_item = take_held(&mut self.held, &mut self.held_key);
        let time = data.time.unwrap_or_default();
        let created = time.created.unwrap_or(row.created);
        let model = data
            .model
            .as_ref()
            .and_then(|m| m.id.clone())
            .unwrap_or_default();
        let effort = data
            .model
            .as_ref()
            .and_then(|m| m.variant.as_deref())
            .map(str::trim)
            .filter(|v| !v.is_empty() && *v != DEFAULT_VARIANT)
            .map(str::to_owned);

        let mut b = match self.cur.take() {
            Some(b) => b,
            None => {
                let idx = self.turns.len();
                self.turns.push(Arc::new(Turn::Assistant(AssistantTurn {
                    id: TurnId::from(row.id.as_str()),
                    timestamp: millis(created),
                    model: model.clone(),
                    effort: None,
                    items: Vec::new(),
                    stats: None,
                })));
                Builder {
                    idx,
                    index: HashMap::new(),
                    last_model: model.clone(),
                    rows: 0,
                    tokens: None,
                    cost: None,
                    first_created: None,
                    last_end: None,
                }
            }
        };

        let items: Vec<Item> = data
            .content
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter_map(|(i, part)| self.part_item(part, row, i))
            .collect();
        b.rows += 1;
        if let Some(tokens) = &data.tokens {
            let total = tokens.input.unwrap_or_default() + tokens.output.unwrap_or_default();
            b.tokens = Some(b.tokens.unwrap_or_default() + total);
        }
        if let Some(cost) = data.cost {
            b.cost = Some(b.cost.unwrap_or_default() + cost);
        }
        b.first_created = Some(b.first_created.map_or(created, |first| first.min(created)));
        let end = time.completed.or(time.streamed).unwrap_or(created);
        b.last_end = Some(b.last_end.map_or(end, |last| last.max(end)));

        if let Turn::Assistant(turn) = Arc::make_mut(&mut self.turns[b.idx]) {
            if let Some(item) = held_item {
                upsert(turn, &mut b, item);
            }
            if turn.effort.is_none() && effort.is_some() {
                turn.effort = effort;
            }
            if !model.is_empty() {
                if turn.model.is_empty() {
                    turn.model.clone_from(&model);
                } else if model != b.last_model {
                    let item = Item::ModelChange {
                        id: ItemId::from(format!("model:{}", row.id)),
                        model: model.clone(),
                    };
                    upsert(turn, &mut b, item);
                }
                b.last_model = model;
            }
            for item in items {
                upsert(turn, &mut b, item);
            }
            turn.stats = Some(TurnStats {
                duration: duration(b.first_created, b.last_end),
                message_count: b.rows,
                tokens: b.tokens,
                cost: b.cost,
            });
        }
        self.dirty.insert(b.idx);
        self.cur = Some(b);
    }

    /// The item of content part `index` of `row`; `None` for a part that
    /// shows nothing.
    fn part_item(&self, part: &Value, row: &Row, index: usize) -> Option<Item> {
        let own_id = || ItemId::from(format!("{}:{index}", row.id));
        let text = part.get("text").and_then(Value::as_str).unwrap_or_default();
        match part.get("type").and_then(Value::as_str) {
            Some("text") if !text.trim().is_empty() => Some(Item::Text {
                id: own_id(),
                text: text.to_owned(),
            }),
            Some("reasoning") if !text.trim().is_empty() => Some(Item::Thinking {
                id: own_id(),
                thinking: Thinking::Text(text.to_owned()),
            }),
            Some("tool") => Some(Item::ToolCall(self.tool_call(part, own_id()))),
            _ => None,
        }
    }

    fn tool_call(&self, part: &Value, fallback_id: ItemId) -> ToolCall {
        let id = part
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map_or(fallback_id, ItemId::from);
        let name = part.get("name").and_then(Value::as_str).unwrap_or_default();
        let state = part.get("state");
        let input = state
            .and_then(|s| s.get("input"))
            .cloned()
            .unwrap_or(Value::Null);
        let tool = tool_kind(name);
        let description = describe(&tool, name, &input, self.cwd.as_deref());
        let (status, result) = match state.and_then(|s| s.get("status")).and_then(Value::as_str) {
            Some("completed") => (ToolStatus::Ok, Some(result(state, false))),
            Some("error") => (ToolStatus::Error, Some(result(state, true))),
            _ => (ToolStatus::Pending, None),
        };
        let time = part.get("time");
        let at = |key: &str| time.and_then(|t| t.get(key)).and_then(Value::as_i64);
        let duration = match (at("created"), at("completed")) {
            (Some(start), Some(end)) if end >= start => {
                u64::try_from(end - start).ok().map(Duration::from_millis)
            }
            _ => None,
        };
        ToolCall {
            id,
            tool,
            description,
            input,
            result,
            status,
            duration,
        }
    }
}

/// The result of a finished tool: the text parts of `state.content`, else
/// (for a failure) `state.error.message`.
fn result(state: Option<&Value>, is_error: bool) -> ToolResult {
    let mut text = state
        .and_then(|s| s.get("content"))
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    if text.is_empty()
        && is_error
        && let Some(message) = state
            .and_then(|s| s.get("error"))
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
    {
        text = message.to_owned();
    }
    ToolResult {
        output: ToolOutput::inline(text),
        is_error,
    }
}

/// The text of a `synthetic` or `system` row. A `system` row's `description`
/// (`Instructions updated: ...`) goes first, so that it is what the folded
/// header shows.
fn note_text(kind: &str, data: &NoteData) -> String {
    let text = data.text.as_deref().unwrap_or_default();
    let description = data
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| kind == "system" && !d.is_empty());
    match description {
        Some(description) if text.trim().is_empty() => description.to_owned(),
        Some(description) => format!("{description}\n\n{text}"),
        None => text.to_owned(),
    }
}

/// Turns the held injected entries into one `Injected` item, identified by
/// the first entry's row.
fn take_held(held: &mut Vec<InjectedEntry>, key: &mut Option<String>) -> Option<Item> {
    if held.is_empty() {
        return None;
    }
    let key = key.take().unwrap_or_default();
    Some(Item::Injected {
        id: ItemId::from(format!("injected:{key}")),
        entries: std::mem::take(held),
    })
}

/// Inserts `item` or replaces the item with the same id in place (a repeated
/// tool part keeps its position).
fn upsert(turn: &mut AssistantTurn, b: &mut Builder, item: Item) {
    if let Some(&pos) = b.index.get(item.id()) {
        turn.items[pos] = item;
    } else {
        b.index.insert(item.id().clone(), turn.items.len());
        turn.items.push(item);
    }
}

fn timestamp(time: Option<&Time>, row: &Row) -> DateTime<Utc> {
    millis(time.and_then(|t| t.created).unwrap_or(row.created))
}

/// A Unix time in milliseconds.
pub(super) fn millis(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or(DateTime::UNIX_EPOCH)
}

fn duration(first: Option<i64>, last: Option<i64>) -> Duration {
    match (first, last) {
        (Some(first), Some(last)) if last > first => {
            Duration::from_millis(u64::try_from(last - first).unwrap_or_default())
        }
        _ => Duration::ZERO,
    }
}
