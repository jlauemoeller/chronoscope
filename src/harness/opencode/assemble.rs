//! Assembly of `session_message` rows into turns (product §4.1, §4.6).
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
//! Compaction is not supported yet: product §10 lists the OpenCode compaction
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
use crate::harness::shared::{lenient, lenient_object};
use crate::model::{
    AssistantTurn, InjectedEntry, Item, ItemId, ModelUpdate, NoticeLevel, Thinking, TokenUsage,
    ToolCall, ToolOutput, ToolResult, ToolStatus, Turn, TurnId, TurnSources, TurnStats, UserTurn,
};

/// What the id of an injected item starts with; the rest is the id of the
/// first row it was made from.
const INJECTED_PREFIX: &str = "injected:";

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
    /// `session_message.time_updated`, in Unix milliseconds. Not used for
    /// assembly; the live watcher compares it to find rows updated in place.
    pub updated: i64,
    /// `session_message.data`, a JSON document.
    pub data: String,
}

#[derive(Deserialize, Default, Clone)]
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

/// A value of an unexpected JSON type counts as missing, so it never turns a
/// parsed row into an unparsed one.
#[derive(Deserialize)]
struct Tokens {
    #[serde(default, deserialize_with = "lenient")]
    input: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    output: Option<u64>,
    /// Separate from `output` in the database.
    #[serde(default, deserialize_with = "lenient")]
    reasoning: Option<u64>,
    #[serde(default, deserialize_with = "lenient_object")]
    cache: Option<CacheTokens>,
}

#[derive(Deserialize)]
struct CacheTokens {
    #[serde(default, deserialize_with = "lenient")]
    read: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    write: Option<u64>,
}

impl Tokens {
    /// The model's view of the row's usage. A missing number counts as zero.
    /// OpenCode keeps `reasoning` apart from `output`; the model's `output`
    /// includes it.
    fn usage(&self) -> TokenUsage {
        let reasoning = self.reasoning.unwrap_or_default();
        let cache = self.cache.as_ref();
        TokenUsage {
            input: self.input.unwrap_or_default(),
            output: self.output.unwrap_or_default().saturating_add(reasoning),
            cache_read: cache.and_then(|c| c.read).unwrap_or_default(),
            cache_write: cache.and_then(|c| c.write).unwrap_or_default(),
            reasoning,
        }
    }
}

/// The usage of an assistant row's `data` as the assembler reads it (so
/// `reasoning` is part of `output`), for the session's token total. The row
/// must parse as the assembler parses it: a JSON object whose known fields
/// (`time`, `model`, `content`, `cost`) have the expected types. A row the
/// assembler skips as unparsed adds nothing, and so does a row without
/// `tokens`; a `tokens` field of the wrong type counts as missing.
pub(super) fn row_usage(data: &str) -> Option<TokenUsage> {
    if !data.trim_start().starts_with('{') {
        return None;
    }
    let parsed: AssistantData = serde_json::from_str(data).ok()?;
    parsed.tokens.as_ref().map(Tokens::usage)
}

#[derive(Deserialize)]
struct AssistantData {
    time: Option<Time>,
    model: Option<Model>,
    content: Option<Vec<Value>>,
    cost: Option<f64>,
    #[serde(default, deserialize_with = "lenient_object")]
    tokens: Option<Tokens>,
    /// How the message ended. Absent, or of an unexpected type, is normal.
    #[serde(default, deserialize_with = "lenient")]
    finish: Option<String>,
    #[serde(default, deserialize_with = "lenient_error")]
    error: Option<RowError>,
}

/// The `error` of an assistant row: `type` and `message` of an object, or a
/// plain string, which counts as the message.
#[derive(Deserialize)]
pub(super) struct RowError {
    #[serde(default, rename = "type", deserialize_with = "lenient")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    message: Option<String>,
}

impl RowError {
    /// Whether the error has a `type` or a `message` that shows anything.
    pub(super) fn has_text(&self) -> bool {
        trimmed(self.kind.as_deref()).is_some() || trimmed(self.message.as_deref()).is_some()
    }
}

/// `deserialize_with` helper: an object is read as a [`RowError`], a string is
/// the message; any other value counts as missing.
pub(super) fn lenient_error<'de, D>(deserializer: D) -> Result<Option<RowError>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;
    let raw = raw.get();
    Ok(if raw.starts_with('{') {
        serde_json::from_str(raw).ok()
    } else if raw.starts_with('"') {
        serde_json::from_str(raw).ok().map(|message| RowError {
            kind: None,
            message: Some(message),
        })
    } else {
        None
    })
}

/// A part of an error or a finish, trimmed; `None` when nothing is left.
fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

/// The level and text of an `error` of an assistant row (SPEC cycle 2 §4.2):
/// `aborted` is a warning, anything else an error, and the text is
/// `<type>: <message>` with whichever parts exist. Shared by the notice and
/// by the details overlay's `error` row, so that they say the same.
pub(super) fn error_text(error: Option<&RowError>) -> (NoticeLevel, String) {
    let kind = trimmed(error.and_then(|e| e.kind.as_deref()));
    let message = trimmed(error.and_then(|e| e.message.as_deref()));
    match (kind, message) {
        (Some(kind), message) if kind == "aborted" => (
            NoticeLevel::Warning,
            match message {
                Some(message) => format!("{kind}: {message}"),
                None => kind,
            },
        ),
        (Some(kind), Some(message)) => (NoticeLevel::Error, format!("{kind}: {message}")),
        (Some(only), None) | (None, Some(only)) => (NoticeLevel::Error, only),
        (None, None) => (NoticeLevel::Error, "error".to_owned()),
    }
}

/// The notice for how an assistant row ended (SPEC cycle 2 §4.2); `None` for
/// a normal ending.
fn ending_notice(row: &Row, data: &AssistantData) -> Option<Item> {
    let finish = trimmed(data.finish.as_deref())?;
    let (level, text) = match finish.as_str() {
        "stop" | "tool-calls" => return None,
        "error" => error_text(data.error.as_ref()),
        other => (NoticeLevel::Warning, format!("stopped: {other}")),
    };
    Some(Item::Notice {
        id: ItemId::from(format!("notice:{}", row.id)),
        level,
        text,
    })
}

/// What one assistant row contributes to its turn's stats.
#[derive(Debug, Clone, Copy)]
struct Contribution {
    created: i64,
    end: i64,
    tokens: Option<TokenUsage>,
    cost: Option<f64>,
}

/// State of an Assistant turn being built.
struct Builder {
    /// Index of the turn in `Assembler::turns`.
    idx: usize,
    /// Position of each item in the turn, for in-place replacement. Empty
    /// while the builder is archived (see `Assembler::archived`).
    index: HashMap<ItemId, usize>,
    /// The model of the most recent row.
    last_model: String,
    /// What each assistant row of the turn contributes, in row order, so
    /// that a row updated in place replaces its share instead of adding to
    /// it.
    rows: Vec<(String, Contribution)>,
}

impl Builder {
    fn new(idx: usize, model: &str) -> Self {
        Self {
            idx,
            index: HashMap::new(),
            last_model: model.to_owned(),
            rows: Vec::new(),
        }
    }

    /// Records the share of row `id`, replacing the one it had.
    fn contribute(&mut self, id: &str, share: Contribution) {
        match self.rows.iter_mut().rev().find(|(row, _)| row == id) {
            Some((_, old)) => *old = share,
            None => self.rows.push((id.to_owned(), share)),
        }
    }

    /// The turn's stats, summed over its rows in order.
    fn stats(&self) -> TurnStats {
        let shares = || self.rows.iter().map(|(_, share)| share);
        let tokens = shares().filter_map(|share| share.tokens).fold(
            None,
            |sum: Option<TokenUsage>, usage| {
                let mut sum = sum.unwrap_or_default();
                sum.accumulate(&usage);
                Some(sum)
            },
        );
        let cost = shares()
            .filter_map(|share| share.cost)
            .fold(None, |sum: Option<f64>, c| {
                Some(sum.unwrap_or_default() + c)
            });
        TurnStats {
            duration: duration(
                shares().map(|share| share.created).min(),
                shares().map(|share| share.end).max(),
            ),
            message_count: self.rows.len(),
            tokens,
            cost,
        }
    }
}

/// Incremental OpenCode turn assembler.
pub struct Assembler {
    cwd: Option<PathBuf>,
    turns: Vec<Arc<Turn>>,
    cur: Option<Builder>,
    /// The builders of the Assistant turns that are no longer the current
    /// one, kept for rows updated in place after a later turn began. Their
    /// `index` is empty and rebuilt from the turn when needed.
    archived: HashMap<usize, Builder>,
    /// The index in `turns` of the turn each `user` and `assistant` row went
    /// into.
    row_turn: HashMap<String, usize>,
    dirty: BTreeSet<usize>,
    /// How many of `turns` `take_updates` has handed out.
    sent_len: usize,
    /// The ids of the rows whose `data` could not be parsed.
    unparsed: BTreeSet<String>,
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
            archived: HashMap::new(),
            row_turn: HashMap::new(),
            dirty: BTreeSet::new(),
            sent_len: 0,
            unparsed: BTreeSet::new(),
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
        self.unparsed.len()
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
            Ok(data) => {
                self.unparsed.remove(&row.id);
                Some(data)
            }
            Err(err) => {
                self.unparsed.insert(row.id.clone());
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
        self.archive_current();
        self.row_turn
            .insert(id.as_str().to_owned(), self.turns.len());
        self.user_flushed = false;
        let items: Vec<Item> = take_held(&mut self.held, &mut self.held_key)
            .into_iter()
            .collect();
        let mut sources = TurnSources::default();
        sources.set_own_row(id.as_str());
        for item in &items {
            note_item(&mut sources, item, id.as_str(), false);
        }
        self.turns.push(Arc::new(Turn::User(UserTurn {
            id,
            timestamp,
            text: text.unwrap_or_default(),
            items,
            sources,
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
                        // Held entries are always an `Injected` item, which
                        // names its own row; the row given is not used.
                        upsert(turn, b, item, "");
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
                (_, item) => {
                    // An `Injected` item names its own row.
                    note_item(&mut turn.sources, &item, "", false);
                    turn.items.push(item);
                }
            }
            self.user_flushed = true;
            self.dirty.insert(last);
        }
    }

    fn on_assistant(&mut self, row: &Row, data: AssistantData) {
        self.flush_current();
        let held_item = take_held(&mut self.held, &mut self.held_key);
        let time = data.time.clone().unwrap_or_default();
        let created = time.created.unwrap_or(row.created);
        let model = model_id(&data);

        let mut b = match self.cur.take() {
            Some(b) => b,
            None => {
                let idx = self.turns.len();
                let mut sources = TurnSources::default();
                sources.set_turn_row(&row.id);
                self.turns.push(Arc::new(Turn::Assistant(AssistantTurn {
                    id: TurnId::from(row.id.as_str()),
                    timestamp: millis(created),
                    model: model.clone(),
                    effort: None,
                    items: Vec::new(),
                    stats: None,
                    sources,
                })));
                Builder::new(idx, &model)
            }
        };
        self.row_turn.insert(row.id.clone(), b.idx);
        self.apply_assistant(&mut b, row, data, held_item, true);
        self.dirty.insert(b.idx);
        self.cur = Some(b);
    }

    /// Adds (or, for a row seen before, replaces) the contribution of an
    /// assistant row to the turn of `b`. `new`: the row is the next one in
    /// `seq` order; it may change the turn's model, which an update of an
    /// older row does not.
    fn apply_assistant(
        &mut self,
        b: &mut Builder,
        row: &Row,
        data: AssistantData,
        held_item: Option<Item>,
        new: bool,
    ) {
        let time = data.time.clone().unwrap_or_default();
        let created = time.created.unwrap_or(row.created);
        let model = model_id(&data);
        let effort = data
            .model
            .as_ref()
            .and_then(|m| m.variant.as_deref())
            .map(str::trim)
            .filter(|v| !v.is_empty() && *v != DEFAULT_VARIANT)
            .map(str::to_owned);

        let items: Vec<Item> = data
            .content
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter_map(|(i, part)| self.part_item(part, row, i))
            .collect();
        let notice = ending_notice(row, &data);
        b.contribute(
            &row.id,
            Contribution {
                created,
                end: time.completed.or(time.streamed).unwrap_or(created),
                tokens: data.tokens.as_ref().map(Tokens::usage),
                cost: data.cost,
            },
        );

        if let Turn::Assistant(turn) = Arc::make_mut(&mut self.turns[b.idx]) {
            if let Some(item) = held_item {
                upsert(turn, b, item, &row.id);
            }
            if turn.effort.is_none() && effort.is_some() {
                turn.effort = effort;
            }
            if new && !model.is_empty() {
                if turn.model.is_empty() {
                    turn.model.clone_from(&model);
                } else if model != b.last_model {
                    let item = Item::ModelChange {
                        id: ItemId::from(format!("model:{}", row.id)),
                        model: model.clone(),
                    };
                    upsert(turn, b, item, &row.id);
                }
                b.last_model = model;
            }
            for item in items {
                upsert(turn, b, item, &row.id);
            }
            match notice {
                Some(item) => {
                    upsert(turn, b, item, &row.id);
                }
                None => remove(turn, b, &ItemId::from(format!("notice:{}", row.id))),
            }
            turn.stats = Some(b.stats());
        }
    }

    /// Moves the current Assistant builder (if any) to the archive.
    fn archive_current(&mut self) {
        if let Some(mut b) = self.cur.take() {
            b.index = HashMap::new();
            self.archived.insert(b.idx, b);
        }
    }

    /// Feeds a row that was fed before and has changed in place (OpenCode
    /// updates a streaming message's `data` and `time_updated`). The turn the
    /// row went into is rebuilt from its new content: items keep their ids
    /// and positions (parts that are new are added at the end of the turn,
    /// after the items of later rows, and so is a notice the row gains now),
    /// a replaced tool part keeps its place, and the turn's stats count the
    /// row once. A row that now ends normally loses its notice, which shifts
    /// the items after it. Rows that were not fed before are ignored: feed those with
    /// [`push_row`](Self::push_row), in `seq` order.
    pub fn replace_row(&mut self, row: &Row) {
        let Some(&idx) = self.row_turn.get(&row.id) else {
            log::debug!(
                "session_message {} was not seen before; not replaced",
                row.id
            );
            return;
        };
        match row.kind.as_str() {
            "user" => {
                let Some(data) = self.parse::<UserData>(row) else {
                    return;
                };
                if let Turn::User(turn) = Arc::make_mut(&mut self.turns[idx]) {
                    turn.timestamp = timestamp(data.time.as_ref(), row);
                    turn.text = data.text.unwrap_or_default();
                    self.dirty.insert(idx);
                }
            }
            "assistant" => {
                let Some(data) = self.parse::<AssistantData>(row) else {
                    return;
                };
                let Some((mut b, current)) = self.take_builder(idx) else {
                    return;
                };
                self.apply_assistant(&mut b, row, data, None, false);
                self.dirty.insert(idx);
                self.put_builder(b, current);
            }
            // Injected rows are not tracked; they do not stream.
            _ => {}
        }
    }

    /// The builder of the Assistant turn `idx`, with its index of items, and
    /// whether it was the current one.
    fn take_builder(&mut self, idx: usize) -> Option<(Builder, bool)> {
        let current = self.cur.as_ref().is_some_and(|b| b.idx == idx);
        let mut b = if current {
            self.cur.take()?
        } else {
            self.archived.remove(&idx)?
        };
        if let Turn::Assistant(turn) = &*self.turns[idx] {
            b.index = turn
                .items
                .iter()
                .enumerate()
                .map(|(pos, item)| (item.id().clone(), pos))
                .collect();
        }
        Some((b, current))
    }

    /// Puts back a builder taken by [`take_builder`](Self::take_builder).
    fn put_builder(&mut self, mut b: Builder, current: bool) {
        if current {
            self.cur = Some(b);
        } else {
            b.index = HashMap::new();
            self.archived.insert(b.idx, b);
        }
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
        id: ItemId::from(format!("{INJECTED_PREFIX}{key}")),
        entries: std::mem::take(held),
    })
}

/// Inserts `item` or replaces the item with the same id in place (a repeated
/// tool part keeps its position). The turn's sources say which row the item
/// is read from: for a new item, `row`; for a replaced one, the row it now
/// comes from (a later row can send a tool part again). See [`note_item`].
fn upsert(turn: &mut AssistantTurn, b: &mut Builder, item: Item, row: &str) {
    if let Some(&pos) = b.index.get(item.id()) {
        note_item(&mut turn.sources, &item, row, true);
        turn.items[pos] = item;
    } else {
        note_item(&mut turn.sources, &item, row, false);
        b.index.insert(item.id().clone(), turn.items.len());
        turn.items.push(item);
    }
}

/// Records where `item` was read from: the `row`, except for injected
/// entries, which are identified by their first row (`injected:<row>`). A
/// tool call is a part of the row. `again`: the item is in the table already
/// and its entry is replaced.
fn note_item(sources: &mut TurnSources, item: &Item, row: &str, again: bool) {
    let row = match item {
        Item::Injected { id, .. } => id.as_str().strip_prefix(INJECTED_PREFIX).unwrap_or(row),
        _ => row,
    };
    let call = matches!(item, Item::ToolCall(_));
    if again {
        sources.set_row_item(item.id(), row, call);
    } else {
        sources.add_row_item(item.id(), row, call);
    }
}

/// Removes the item with the given id, if the turn has it. The positions of
/// the items after it change, so the index is rebuilt.
fn remove(turn: &mut AssistantTurn, b: &mut Builder, id: &ItemId) {
    if let Some(pos) = b.index.remove(id) {
        turn.items.remove(pos);
        turn.sources.remove_item(id);
        b.index = turn
            .items
            .iter()
            .enumerate()
            .map(|(pos, item)| (item.id().clone(), pos))
            .collect();
    }
}

/// The model id of an assistant row's data; empty without one.
fn model_id(data: &AssistantData) -> String {
    data.model
        .as_ref()
        .and_then(|m| m.id.clone())
        .unwrap_or_default()
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

#[cfg(test)]
mod tests {
    mod replace {
        use super::super::*;

        fn user(id: &str, seq: i64, text: &str) -> Row {
            Row {
                id: id.to_owned(),
                kind: "user".to_owned(),
                seq,
                created: 1_000 * seq,
                updated: 1_000 * seq,
                data: format!(
                    r#"{{"time":{{"created":{}}},"text":"{text}"}}"#,
                    1_000 * seq
                ),
            }
        }

        /// An assistant row with one text part, a tool part in state `status`
        /// and `tokens` output tokens.
        fn assistant(id: &str, seq: i64, text: &str, status: &str, tokens: u64) -> Row {
            let result = if status == "completed" {
                r#","content":[{"type":"text","text":"done"}]"#
            } else {
                ""
            };
            Row {
                id: id.to_owned(),
                kind: "assistant".to_owned(),
                seq,
                created: 1_000 * seq,
                updated: 1_000 * seq,
                data: format!(
                    r#"{{"time":{{"created":{c},"completed":{e}}},"model":{{"id":"m"}},"content":[{{"type":"text","text":"{text}"}},{{"type":"tool","id":"call-{id}","name":"shell","state":{{"status":"{status}","input":{{"command":"ls"}}{result}}}}}],"cost":0.5,"tokens":{{"input":10,"output":{tokens}}}}}"#,
                    c = 1_000 * seq,
                    e = 1_000 * seq + 500,
                ),
            }
        }

        fn assemble(rows: &[Row]) -> Vec<Arc<Turn>> {
            let mut asm = Assembler::new();
            for row in rows {
                asm.push_row(row);
            }
            asm.into_turns()
        }

        fn updated_ids(updates: &[ModelUpdate]) -> Vec<String> {
            updates
                .iter()
                .filter_map(|u| match u {
                    ModelUpdate::Updated(turns) => Some(turns),
                    _ => None,
                })
                .flatten()
                .map(|t| t.id().to_string())
                .collect()
        }

        #[test]
        fn a_streamed_row_replaced_in_place_equals_a_fresh_assembly_of_the_final_rows() {
            let rows = [
                user("u1", 1, "go"),
                assistant("a1", 2, "first", "completed", 5),
                assistant("a2", 3, "second", "pending", 1),
            ];
            let mut asm = Assembler::new();
            for row in &rows {
                asm.push_row(row);
            }
            asm.finish();
            let _ = asm.take_updates();

            let grown = assistant("a2", 3, "second, longer", "completed", 40);
            asm.replace_row(&grown);
            let updates = asm.take_updates();
            assert_eq!(updated_ids(&updates), ["a1"], "{updates:?}");
            assert_eq!(updates.len(), 1);

            let expected = assemble(&[rows[0].clone(), rows[1].clone(), grown]);
            assert_eq!(asm.turns(), expected.as_slice());
            let Turn::Assistant(turn) = &*asm.turns()[1] else {
                panic!("an assistant turn")
            };
            // The replaced row counts once: two rows, 10 + 10 input and 5 + 40
            // output tokens.
            let stats = turn.stats.as_ref().unwrap();
            assert_eq!(stats.message_count, 2);
            assert_eq!(
                stats.tokens,
                Some(TokenUsage {
                    input: 20,
                    output: 45,
                    ..TokenUsage::default()
                })
            );
            assert_eq!(stats.cost, Some(1.0));
            // The items keep their ids: the pending tool call is now complete.
            let ids: Vec<&str> = turn.items.iter().map(|i| i.id().as_str()).collect();
            assert_eq!(ids, ["a1:0", "call-a1", "a2:0", "call-a2"]);
        }

        #[test]
        fn rows_pushed_after_a_replacement_still_extend_the_current_turn() {
            let mut asm = Assembler::new();
            asm.push_row(&user("u1", 1, "go"));
            asm.push_row(&assistant("a1", 2, "x", "pending", 1));
            asm.finish();
            let _ = asm.take_updates();
            asm.replace_row(&assistant("a1", 2, "xy", "completed", 2));
            asm.push_row(&assistant("a2", 3, "z", "completed", 3));
            asm.push_row(&user("u2", 4, "again"));
            asm.finish();
            let expected = assemble(&[
                user("u1", 1, "go"),
                assistant("a1", 2, "xy", "completed", 2),
                assistant("a2", 3, "z", "completed", 3),
                user("u2", 4, "again"),
            ]);
            assert_eq!(asm.turns(), expected.as_slice());
        }

        #[test]
        fn a_row_of_an_earlier_turn_can_be_replaced_after_a_later_turn_began() {
            let rows = [
                user("u1", 1, "go"),
                assistant("a1", 2, "old", "pending", 1),
                user("u2", 3, "next"),
                assistant("a2", 4, "new", "completed", 2),
            ];
            let mut asm = Assembler::new();
            for row in &rows {
                asm.push_row(row);
            }
            asm.finish();
            let _ = asm.take_updates();

            let late = assistant("a1", 2, "old, finished", "completed", 9);
            asm.replace_row(&late);
            let updates = asm.take_updates();
            assert_eq!(updated_ids(&updates), ["a1"], "{updates:?}");
            let expected = assemble(&[
                rows[0].clone(),
                late.clone(),
                rows[2].clone(),
                rows[3].clone(),
            ]);
            assert_eq!(asm.turns(), expected.as_slice());

            // And again, and the current turn is still extended afterwards.
            let later = assistant("a1", 2, "old, finished twice", "completed", 9);
            asm.replace_row(&later);
            asm.push_row(&assistant("a3", 5, "more", "completed", 1));
            asm.finish();
            let expected = assemble(&[
                rows[0].clone(),
                later,
                rows[2].clone(),
                rows[3].clone(),
                assistant("a3", 5, "more", "completed", 1),
            ]);
            assert_eq!(asm.turns(), expected.as_slice());
        }

        #[test]
        fn a_user_row_replaced_changes_its_text_and_keeps_its_id() {
            let mut asm = Assembler::new();
            asm.push_row(&user("u1", 1, "draft"));
            asm.finish();
            let _ = asm.take_updates();
            asm.replace_row(&user("u1", 1, "final"));
            let updates = asm.take_updates();
            assert_eq!(updated_ids(&updates), ["u1"]);
            let Turn::User(turn) = &*asm.turns()[0] else {
                panic!("a user turn")
            };
            assert_eq!(turn.text, "final");
            assert_eq!(asm.turns().len(), 1);
        }

        #[test]
        fn a_row_that_was_never_fed_is_not_replaced() {
            let mut asm = Assembler::new();
            asm.push_row(&user("u1", 1, "hi"));
            asm.finish();
            let _ = asm.take_updates();
            asm.replace_row(&assistant("a9", 9, "ghost", "completed", 1));
            assert!(asm.take_updates().is_empty());
            assert_eq!(asm.turns().len(), 1);
        }
    }

    mod token {
        use super::super::*;

        fn user(id: &str, seq: i64, text: &str) -> Row {
            Row {
                id: id.to_owned(),
                kind: "user".to_owned(),
                seq,
                created: 1_000 * seq,
                updated: 1_000 * seq,
                data: format!(
                    r#"{{"time":{{"created":{}}},"text":"{text}"}}"#,
                    1_000 * seq
                ),
            }
        }

        /// An assistant row with one text part and the raw `tokens` JSON.
        fn assistant_with_tokens(id: &str, seq: i64, tokens: &str) -> Row {
            Row {
                id: id.to_owned(),
                kind: "assistant".to_owned(),
                seq,
                created: 1_000 * seq,
                updated: 1_000 * seq,
                data: format!(
                    r#"{{"time":{{"created":{c},"completed":{c}}},"model":{{"id":"m"}},"content":[{{"type":"text","text":"hi"}}],"cost":0.5,"tokens":{tokens}}}"#,
                    c = 1_000 * seq,
                ),
            }
        }

        /// Assembles a user row and `rows`; returns the assembler.
        fn assembler_with(rows: &[Row]) -> Assembler {
            let mut asm = Assembler::new();
            asm.push_row(&user("u1", 1, "go"));
            for row in rows {
                asm.push_row(row);
            }
            asm.finish();
            asm
        }

        fn turn_tokens(asm: &Assembler) -> Option<TokenUsage> {
            let Turn::Assistant(turn) = &*asm.turns()[1] else {
                panic!("an assistant turn")
            };
            turn.stats.as_ref().unwrap().tokens
        }

        #[test]
        fn cache_tokens_are_summed_over_the_rows() {
            let asm = assembler_with(&[
                assistant_with_tokens(
                    "a1",
                    2,
                    r#"{"input":10,"output":20,"reasoning":0,"cache":{"read":1000,"write":300}}"#,
                ),
                assistant_with_tokens(
                    "a2",
                    3,
                    r#"{"input":1,"output":2,"reasoning":0,"cache":{"read":5,"write":7}}"#,
                ),
            ]);
            assert_eq!(
                turn_tokens(&asm),
                Some(TokenUsage {
                    input: 11,
                    output: 22,
                    cache_read: 1005,
                    cache_write: 307,
                    reasoning: 0,
                })
            );
        }

        #[test]
        fn opencode_reasoning_is_added_to_output_once() {
            // One real row has reasoning 683 and output 120: the model's output
            // is 803 and reasoning stays an "of which" figure.
            let asm = assembler_with(&[assistant_with_tokens(
                "a1",
                2,
                r#"{"input":50,"output":120,"reasoning":683,"cache":{"read":0,"write":10}}"#,
            )]);
            let tokens = turn_tokens(&asm).unwrap();
            assert_eq!(tokens.output, 803);
            assert_eq!(tokens.reasoning, 683);
            assert_eq!(tokens.total(), 50 + 803 + 10);
        }

        #[test]
        fn a_row_updated_in_place_replaces_its_cache_share() {
            let mut asm = assembler_with(&[assistant_with_tokens(
                "a1",
                2,
                r#"{"input":10,"output":20,"reasoning":5,"cache":{"read":100,"write":50}}"#,
            )]);
            asm.replace_row(&assistant_with_tokens(
                "a1",
                2,
                r#"{"input":10,"output":30,"reasoning":8,"cache":{"read":120,"write":50}}"#,
            ));
            assert_eq!(
                turn_tokens(&asm),
                Some(TokenUsage {
                    input: 10,
                    output: 38,
                    cache_read: 120,
                    cache_write: 50,
                    reasoning: 8,
                })
            );
        }

        #[test]
        fn token_fields_of_an_unexpected_type_count_as_missing_and_keep_the_row_parsed() {
            let asm = assembler_with(&[
                assistant_with_tokens(
                    "a1",
                    2,
                    r#"{"input":"ten","output":20,"reasoning":[1],"cache":{"read":"x","write":7}}"#,
                ),
                assistant_with_tokens("a2", 3, r#"{"input":1,"output":2,"cache":"none"}"#),
                // A `tokens` that is not an object is missing as a whole.
                assistant_with_tokens("a3", 4, r#""lots""#),
            ]);
            assert_eq!(asm.unparsed_count(), 0);
            let Turn::Assistant(turn) = &*asm.turns()[1] else {
                panic!("an assistant turn")
            };
            assert_eq!(turn.stats.as_ref().unwrap().message_count, 3);
            assert_eq!(
                turn.items.len(),
                3,
                "the text of every row is kept: {:?}",
                turn.items
            );
            assert_eq!(
                turn_tokens(&asm),
                Some(TokenUsage {
                    input: 1,
                    output: 22,
                    cache_write: 7,
                    ..TokenUsage::default()
                })
            );
        }

        #[test]
        fn rows_without_tokens_leave_the_token_figures_unknown() {
            // An explicit `null`, and a row with no `tokens` key at all.
            let mut absent = assistant_with_tokens("a2", 3, "null");
            absent.data = absent.data.replace(r#","tokens":null"#, "");
            assert!(!absent.data.contains("tokens"), "{}", absent.data);
            let asm = assembler_with(&[assistant_with_tokens("a1", 2, "null"), absent]);
            assert_eq!(asm.unparsed_count(), 0);
            assert_eq!(turn_tokens(&asm), None);
        }

        #[test]
        fn an_out_of_range_number_in_tokens_keeps_the_row_parsed() {
            // `1e400` does not fit an `f64`; as a new field, as an unknown field
            // and nested in `cache` it counts as missing and fails nothing.
            let asm = assembler_with(&[
                assistant_with_tokens("a1", 2, r#"{"output":7,"cache":{"read":1e400,"write":3}}"#),
                assistant_with_tokens("a2", 3, r#"{"output":5,"unknown":1e400}"#),
                assistant_with_tokens("a3", 4, r#"{"input":1e400,"reasoning":1e400}"#),
            ]);
            assert_eq!(asm.unparsed_count(), 0);
            let Turn::Assistant(turn) = &*asm.turns()[1] else {
                panic!("an assistant turn")
            };
            assert_eq!(turn.items.len(), 3, "{:?}", turn.items);
            assert_eq!(
                turn_tokens(&asm),
                Some(TokenUsage {
                    output: 12,
                    cache_write: 3,
                    ..TokenUsage::default()
                })
            );
        }
    }
}
