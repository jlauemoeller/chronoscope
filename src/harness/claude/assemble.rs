//! Incremental assembly of Claude JSONL lines into turns (SPEC §5.1, §7.1).
//!
//! Feed lines (or raw byte chunks) as they arrive; each call returns the
//! [`ModelUpdate`]s it caused. Feeding a transcript in any chunking yields the
//! same final turns.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::tools::{describe, tool_kind};
use super::wire::{Block, Content, Entry, Head, result_text};
use crate::model::{
    AssistantTurn, ByteSource, Item, ItemId, LineFormat, ModelUpdate, OutputSource, Thinking,
    ToolCall, ToolOutput, ToolResult, ToolStatus, Turn, TurnId, TurnStats, UserTurn,
};

/// Placeholder model name Claude Code writes for locally generated messages;
/// it never counts as a model change.
const SYNTHETIC_MODEL: &str = "<synthetic>";

/// State of the Assistant turn currently being built.
struct Builder {
    /// Index of the turn in `Assembler::turns`.
    idx: usize,
    /// Position of each item in the turn, for in-place replacement.
    index: HashMap<ItemId, usize>,
    /// The model of the most recent message.
    last_model: String,
    /// Message ids seen in this turn.
    messages: HashSet<String>,
    /// Latest input+output tokens per message id.
    usage: HashMap<String, u64>,
}

impl Builder {
    fn tokens(&self) -> Option<u64> {
        if self.usage.is_empty() {
            None
        } else {
            Some(self.usage.values().sum())
        }
    }
}

/// Where a tool call lives, for attaching its result.
#[derive(Clone, Copy)]
struct CallRef {
    turn: usize,
    item: usize,
    started: DateTime<Utc>,
}

/// Incremental Claude turn assembler.
pub struct Assembler {
    source: ByteSource,
    cwd: Option<PathBuf>,
    last_ts: Option<DateTime<Utc>>,
    turns: Vec<Arc<Turn>>,
    cur: Option<Builder>,
    calls: HashMap<String, CallRef>,
    dirty: BTreeSet<usize>,
    pending: Vec<u8>,
    pending_start: u64,
}

impl Assembler {
    /// Creates an assembler. `source` is where large tool outputs are
    /// re-read from; the byte ranges passed to [`push_line`](Self::push_line)
    /// must be absolute offsets into it.
    pub fn new(source: ByteSource) -> Self {
        Self {
            source,
            cwd: None,
            last_ts: None,
            turns: Vec::new(),
            cur: None,
            calls: HashMap::new(),
            dirty: BTreeSet::new(),
            pending: Vec::new(),
            pending_start: 0,
        }
    }

    /// Sets the session cwd up front, used to shorten paths in descriptions.
    /// Otherwise the first `cwd` seen in an entry is used.
    #[must_use]
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// The turns assembled so far.
    pub fn turns(&self) -> &[Arc<Turn>] {
        &self.turns
    }

    /// Consumes the assembler, returning its turns.
    pub fn into_turns(self) -> Vec<Arc<Turn>> {
        self.turns
    }

    /// The session cwd, if known.
    pub fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    /// Absolute offset of the first byte not yet consumed as a full line.
    pub fn buffered_from(&self) -> u64 {
        self.pending_start
    }

    /// Feeds one complete line (without its newline). `range` is the line's
    /// absolute byte range in the source.
    pub fn push_line(&mut self, line: &[u8], range: Range<u64>) -> Vec<ModelUpdate> {
        let start_len = self.turns.len();
        self.process(line, &range);
        self.collect(start_len)
    }

    /// Feeds raw bytes as they are read. A trailing partial line is buffered
    /// until its newline arrives. Offsets are tracked from zero, from the
    /// first chunk.
    pub fn push_chunk(&mut self, chunk: &[u8]) -> Vec<ModelUpdate> {
        let start_len = self.turns.len();
        let mut buf = std::mem::take(&mut self.pending);
        buf.extend_from_slice(chunk);
        let mut consumed = 0;
        while let Some(rel) = buf[consumed..].iter().position(|&b| b == b'\n') {
            let line = &buf[consumed..consumed + rel];
            let start = self.pending_start + consumed as u64;
            self.process(line, &(start..start + rel as u64));
            consumed += rel + 1;
        }
        buf.drain(..consumed);
        self.pending_start += consumed as u64;
        self.pending = buf;
        self.collect(start_len)
    }

    /// Flushes a trailing line that has no newline.
    pub fn finish(&mut self) -> Vec<ModelUpdate> {
        let start_len = self.turns.len();
        let buf = std::mem::take(&mut self.pending);
        if !buf.is_empty() {
            let start = self.pending_start;
            self.process(&buf, &(start..start + buf.len() as u64));
            self.pending_start += buf.len() as u64;
        }
        self.collect(start_len)
    }

    /// Assembles a whole in-memory transcript.
    pub fn from_memory(bytes: Arc<[u8]>) -> Self {
        let mut asm = Self::new(ByteSource::Memory(Arc::clone(&bytes)));
        asm.push_chunk(&bytes);
        asm.finish();
        asm
    }

    /// Assembles a whole transcript file, reading it in chunks.
    pub fn from_file(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let mut file = File::open(&path)?;
        let mut asm = Self::new(ByteSource::File(Arc::new(path)));
        let mut buf = vec![0; 64 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            asm.push_chunk(&buf[..n]);
        }
        asm.finish();
        Ok(asm)
    }

    fn collect(&mut self, start_len: usize) -> Vec<ModelUpdate> {
        let dirty = std::mem::take(&mut self.dirty);
        let updated: Vec<Arc<Turn>> = dirty
            .into_iter()
            .filter(|&i| i < start_len)
            .map(|i| Arc::clone(&self.turns[i]))
            .collect();
        let appended = self.turns[start_len..].to_vec();
        let mut out = Vec::new();
        if !updated.is_empty() {
            out.push(ModelUpdate::Updated(updated));
        }
        if !appended.is_empty() {
            out.push(ModelUpdate::Appended(appended));
        }
        out
    }

    fn process(&mut self, line: &[u8], range: &Range<u64>) {
        let Ok(head) = serde_json::from_slice::<Head>(line) else {
            return;
        };
        let relevant = match head.kind.as_deref() {
            Some("user" | "assistant") => true,
            Some("system") => head.subtype.as_deref() == Some("turn_duration"),
            _ => false,
        };
        if !relevant {
            return;
        }
        let Ok(entry) = serde_json::from_slice::<Entry>(line) else {
            return;
        };
        if self.cwd.is_none()
            && let Some(cwd) = entry.cwd.as_deref().filter(|c| !c.is_empty())
        {
            self.cwd = Some(PathBuf::from(cwd));
        }
        let ts = self.timestamp(&entry);
        match head.kind.as_deref() {
            Some("user") => self.on_user(entry, ts, range),
            Some("assistant") => self.on_assistant(entry, ts, range),
            _ => self.on_turn_duration(&entry),
        }
    }

    fn timestamp(&mut self, entry: &Entry) -> DateTime<Utc> {
        let parsed = entry
            .timestamp
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.with_timezone(&Utc));
        if let Some(ts) = parsed {
            self.last_ts = Some(ts);
        }
        parsed.or(self.last_ts).unwrap_or(DateTime::UNIX_EPOCH)
    }

    fn on_user(&mut self, entry: Entry, ts: DateTime<Utc>, range: &Range<u64>) {
        let Some(content) = entry.message.as_ref().and_then(|m| m.content.as_ref()) else {
            return;
        };
        let blocks: &[Block] = match content {
            Content::Blocks(blocks) => blocks,
            _ => &[],
        };
        if blocks
            .iter()
            .any(|b| b.kind.as_deref() == Some("tool_result"))
        {
            for block in blocks.iter().filter(|b| b.is_result()) {
                self.attach_result(block, ts, range);
            }
            return;
        }
        if entry.is_meta {
            return;
        }
        let has_text = match content {
            Content::Text(_) => true,
            Content::Blocks(blocks) => blocks.iter().any(|b| b.kind.as_deref() == Some("text")),
            Content::Other(_) => false,
        };
        let is_prompt = match entry.origin {
            Some(_) => entry.origin_kind() == Some("human"),
            None => has_text,
        };
        if !is_prompt {
            return;
        }
        let text = match content {
            Content::Text(s) => s.clone(),
            Content::Blocks(blocks) => blocks
                .iter()
                .filter(|b| b.kind.as_deref() == Some("text"))
                .filter_map(|b| b.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n"),
            Content::Other(_) => String::new(),
        };
        let id = entry_id(entry.uuid.as_deref(), range);
        self.cur = None;
        self.turns.push(Arc::new(Turn::User(UserTurn {
            id,
            timestamp: ts,
            text,
            items: Vec::new(),
        })));
    }

    fn on_assistant(&mut self, entry: Entry, ts: DateTime<Utc>, range: &Range<u64>) {
        let effort = entry.effort();
        let Some(message) = entry.message else {
            return;
        };
        let msg_id = message
            .id
            .clone()
            .or_else(|| entry.uuid.clone())
            .unwrap_or_else(|| format!("L{}", range.start));
        let model = message.model.clone().unwrap_or_default();

        let mut b = match self.cur.take() {
            Some(b) => b,
            None => {
                let idx = self.turns.len();
                self.turns.push(Arc::new(Turn::Assistant(AssistantTurn {
                    id: entry_id(entry.uuid.as_deref(), range),
                    timestamp: ts,
                    model: model.clone(),
                    effort: None,
                    items: Vec::new(),
                    stats: None,
                })));
                Builder {
                    idx,
                    index: HashMap::new(),
                    last_model: model.clone(),
                    messages: HashSet::new(),
                    usage: HashMap::new(),
                }
            }
        };

        let mut changed = false;
        let mut results: Vec<Block> = Vec::new();
        if let Turn::Assistant(turn) = Arc::make_mut(&mut self.turns[b.idx]) {
            if turn.effort.is_none() && effort.is_some() {
                turn.effort = effort;
                changed = true;
            }
            if b.messages.insert(msg_id.clone()) {
                let real = !model.is_empty() && model != SYNTHETIC_MODEL;
                if real && model != b.last_model {
                    let item = Item::ModelChange {
                        id: ItemId::from(format!("model:{msg_id}")),
                        model: model.clone(),
                    };
                    upsert(turn, &mut b, item, &mut changed);
                }
                if real {
                    b.last_model = model;
                }
            }

            let blocks = match message.content {
                Some(Content::Blocks(blocks)) => blocks,
                Some(Content::Text(text)) => vec![Block {
                    kind: Some("text".to_owned()),
                    text: Some(text),
                    thinking: None,
                    id: None,
                    name: None,
                    input: None,
                    tool_use_id: None,
                    content: None,
                    is_error: None,
                }],
                _ => Vec::new(),
            };
            let base = entry.api_block_index.unwrap_or(0);
            for (i, block) in blocks.into_iter().enumerate() {
                let index = base + i;
                match block.kind.as_deref() {
                    Some("text") => {
                        let text = block.text.unwrap_or_default();
                        if !text.is_empty() {
                            let id = ItemId::from(format!("{msg_id}:{index}"));
                            upsert(turn, &mut b, Item::Text { id, text }, &mut changed);
                        }
                    }
                    Some("thinking") => {
                        let thinking = match block.thinking.filter(|t| !t.is_empty()) {
                            Some(text) => Thinking::Text(text),
                            None => Thinking::Redacted,
                        };
                        let id = ItemId::from(format!("{msg_id}:{index}"));
                        upsert(turn, &mut b, Item::Thinking { id, thinking }, &mut changed);
                    }
                    Some("tool_use" | "server_tool_use") => {
                        let Some(call_id) = block.id else { continue };
                        let name = block.name.unwrap_or_default();
                        let input = block.input.unwrap_or(serde_json::Value::Null);
                        let tool = tool_kind(&name);
                        let description = describe(&tool, &name, &input, self.cwd.as_deref());
                        let call = ToolCall {
                            id: ItemId::from(call_id.as_str()),
                            tool,
                            description,
                            input,
                            result: None,
                            status: ToolStatus::Pending,
                            duration: None,
                        };
                        let item = upsert(turn, &mut b, Item::ToolCall(call), &mut changed);
                        self.calls.entry(call_id).or_insert(CallRef {
                            turn: b.idx,
                            item,
                            started: ts,
                        });
                    }
                    _ if block.is_result() => results.push(block),
                    _ => {}
                }
            }

            if let Some(usage) = &message.usage {
                let tokens = usage.input_tokens.unwrap_or_default()
                    + usage.output_tokens.unwrap_or_default();
                b.usage.insert(msg_id, tokens);
                if let Some(stats) = &mut turn.stats {
                    let total = b.tokens();
                    if stats.tokens != total {
                        stats.tokens = total;
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.dirty.insert(b.idx);
        }
        self.cur = Some(b);
        for block in &results {
            self.attach_result(block, ts, range);
        }
    }

    fn on_turn_duration(&mut self, entry: &Entry) {
        let (Some(b), Some(ms)) = (&self.cur, entry.duration_ms) else {
            return;
        };
        let stats = TurnStats {
            duration: std::time::Duration::from_millis(ms),
            message_count: entry
                .message_count
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or_default(),
            tokens: b.tokens(),
            cost: None,
        };
        let idx = b.idx;
        if let Turn::Assistant(turn) = Arc::make_mut(&mut self.turns[idx])
            && turn.stats.as_ref() != Some(&stats)
        {
            turn.stats = Some(stats);
            self.dirty.insert(idx);
        }
    }

    /// Attaches the result in `block` to the call it names, if known.
    fn attach_result(&mut self, block: &Block, ts: DateTime<Utc>, range: &Range<u64>) {
        let Some(call_id) = block.tool_use_id.as_deref() else {
            return;
        };
        let Some(call) = self.calls.get(call_id).copied() else {
            return;
        };
        let text = result_text(block.content.as_ref());
        let source = &self.source;
        let output = ToolOutput::from_text(text, || OutputSource {
            bytes: source.clone(),
            range: range.clone(),
            format: LineFormat::Claude,
            call_id: call_id.to_owned(),
        });
        let is_error = block.is_error.unwrap_or(false);
        let result = ToolResult { output, is_error };
        let status = if is_error {
            ToolStatus::Error
        } else {
            ToolStatus::Ok
        };
        let duration = (ts - call.started).to_std().ok();

        let unchanged = matches!(
            &*self.turns[call.turn],
            Turn::Assistant(t) if matches!(
                t.items.get(call.item),
                Some(Item::ToolCall(c))
                    if c.result.as_ref() == Some(&result)
                        && c.status == status
                        && c.duration == duration
            )
        );
        if unchanged {
            return;
        }
        if let Turn::Assistant(turn) = Arc::make_mut(&mut self.turns[call.turn])
            && let Some(Item::ToolCall(c)) = turn.items.get_mut(call.item)
        {
            c.result = Some(result);
            c.status = status;
            c.duration = duration;
            self.dirty.insert(call.turn);
        }
    }
}

/// Inserts `item` or replaces the item with the same id in place (a repeated
/// tool call keeps the result already attached). Returns its position.
fn upsert(turn: &mut AssistantTurn, b: &mut Builder, mut item: Item, changed: &mut bool) -> usize {
    if let Some(&pos) = b.index.get(item.id()) {
        if let (Item::ToolCall(old), Item::ToolCall(new)) = (&turn.items[pos], &mut item) {
            new.result = old.result.clone();
            new.status = old.status;
            new.duration = old.duration;
        }
        if turn.items[pos] != item {
            turn.items[pos] = item;
            *changed = true;
        }
        pos
    } else {
        let pos = turn.items.len();
        b.index.insert(item.id().clone(), pos);
        turn.items.push(item);
        *changed = true;
        pos
    }
}

fn entry_id(uuid: Option<&str>, range: &Range<u64>) -> TurnId {
    match uuid {
        Some(uuid) if !uuid.is_empty() => TurnId::from(uuid),
        _ => TurnId::from(format!("L{}", range.start)),
    }
}
