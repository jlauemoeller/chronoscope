//! Incremental assembly of Claude JSONL lines into turns (SPEC §5.1, §7.1).
//!
//! Feed lines (or raw byte chunks) as they arrive; each call returns the
//! [`ModelUpdate`]s it caused. Feeding a transcript in any chunking yields the
//! same final turns.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

use super::inject::{
    NEWEST_KNOWN_VERSION, attachment_text, is_injected_attachment, is_injected_system,
    is_known_noise_attachment, is_metadata_type, is_noise_type, local_command_text,
    queued_prompt_text, system_text, version_newer,
};
use super::tools::{describe, tool_kind};
use super::wire::{Block, Content, Entry, Head, Usage, content_text, read_entry, result_text};
use crate::model::{
    AssistantTurn, ByteSource, CompactionTurn, InjectedEntry, Item, ItemId, LineFormat,
    ModelUpdate, NoticeLevel, OutputSource, Thinking, TokenUsage, ToolCall, ToolOutput, ToolResult,
    ToolStatus, Turn, TurnId, TurnSources, TurnStats, UserTurn,
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
    /// Message ids seen in this turn, with their number in the turn's
    /// [`TurnSources`].
    messages: HashMap<String, usize>,
    /// How many of those messages are not `<synthetic>`: the turn's message
    /// count.
    counted: usize,
    /// When the turn started: the timestamp of the prompt, else of the first
    /// assistant entry. `None` when that entry has no usable timestamp.
    start: Option<DateTime<Utc>>,
    /// The duration set by the turn's latest `turn_duration` entry. The turn
    /// has a footer once this is set.
    duration: Option<Duration>,
    /// Latest token usage per message id.
    usage: HashMap<String, TokenUsage>,
    /// Without `apiBlockIndex`: the next block number per message id.
    next_block: HashMap<String, usize>,
    /// Without `apiBlockIndex`: the most recent block of each message.
    last_block: HashMap<String, ItemId>,
    /// Signatures of redacted thinking blocks (to tell re-sends from
    /// distinct blocks when there is no `apiBlockIndex`).
    signatures: HashMap<ItemId, String>,
    /// The last non-null `stop_reason` per message id, within this turn: a
    /// message id that continues after a human prompt starts over.
    stop: HashMap<String, String>,
    /// Message ids flagged as API errors on any of their lines.
    api_errors: HashSet<String>,
    /// The text blocks of flagged messages, by message id and block number
    /// (`apiBlockIndex` plus position, else the next free number), so that a
    /// line sent again replaces its blocks instead of adding them.
    error_text: HashMap<String, BTreeMap<usize, String>>,
}

impl Builder {
    /// The turn's footer, once a `turn_duration` entry has set the duration.
    fn stats(&self) -> Option<TurnStats> {
        Some(TurnStats {
            duration: self.duration?,
            message_count: self.counted,
            tokens: self.tokens(),
            cost: None,
        })
    }

    /// The usage summed over the turn's messages, each counted once.
    fn tokens(&self) -> Option<TokenUsage> {
        if self.usage.is_empty() {
            return None;
        }
        let mut sum = TokenUsage::default();
        for usage in self.usage.values() {
            sum.accumulate(usage);
        }
        Some(sum)
    }
}

/// The model's view of a message's `usage`. A missing number counts as zero.
/// `thinking_tokens` is already part of `output_tokens`, so it only fills
/// `reasoning` and is not added to `output`.
fn token_usage(usage: &Usage) -> TokenUsage {
    TokenUsage {
        input: usage.input_tokens.unwrap_or_default(),
        output: usage.output_tokens.unwrap_or_default(),
        cache_read: usage.cache_read_input_tokens.unwrap_or_default(),
        cache_write: usage.cache_creation_input_tokens.unwrap_or_default(),
        reasoning: usage
            .output_tokens_details
            .as_ref()
            .and_then(|d| d.thinking_tokens)
            .unwrap_or_default(),
    }
}

/// Where a tool call lives, for attaching its result.
#[derive(Clone, Copy)]
struct CallRef {
    turn: usize,
    item: usize,
    started: DateTime<Utc>,
    /// The call's number in the turn's [`TurnSources`].
    source: usize,
}

/// Incremental Claude turn assembler.
pub struct Assembler {
    source: ByteSource,
    cwd: Option<PathBuf>,
    last_ts: Option<DateTime<Utc>>,
    first_ts: Option<DateTime<Utc>>,
    turns: Vec<Arc<Turn>>,
    cur: Option<Builder>,
    calls: HashMap<String, CallRef>,
    dirty: BTreeSet<usize>,
    pending: Vec<u8>,
    pending_start: u64,
    /// Number of lines processed, for log messages (1-based).
    lines: u64,
    /// Lines and blocks that were skipped because they could not be parsed.
    unparsed: usize,
    /// Allowlisted entries waiting for the next entry to decide their turn.
    held: Vec<InjectedEntry>,
    /// Identity (uuid or line offset) of the first held entry.
    held_key: Option<String>,
    /// The line of the first held entry.
    held_line: Option<Range<u64>>,
    /// Attachment kinds that are neither allowlisted nor known noise.
    unknown_kinds: BTreeMap<String, usize>,
    /// Unknown kinds already written to the log.
    logged_kinds: HashSet<String>,
    /// Whether the last User turn already received entries flushed after its
    /// prompt (they collapse with later consecutive entries).
    user_flushed: bool,
    /// Index of a Compaction turn still waiting for its summary entry.
    compaction_pending: Option<usize>,
    /// The usable timestamp of the prompt that started the turn about to
    /// begin; taken by its first assistant entry, dropped by a compaction.
    prompt_ts: Option<DateTime<Utc>>,
    /// The `version` last compared against the newest known one.
    last_version: Option<String>,
    /// Whether the newer-version warning has been logged.
    warned_version: bool,
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
            first_ts: None,
            turns: Vec::new(),
            cur: None,
            calls: HashMap::new(),
            dirty: BTreeSet::new(),
            pending: Vec::new(),
            pending_start: 0,
            lines: 0,
            unparsed: 0,
            held: Vec::new(),
            held_key: None,
            held_line: None,
            unknown_kinds: BTreeMap::new(),
            logged_kinds: HashSet::new(),
            user_flushed: false,
            compaction_pending: None,
            prompt_ts: None,
            last_version: None,
            warned_version: false,
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

    /// Consumes the assembler, returning its turns. Injected entries still
    /// waiting for a turn go into the current User or Assistant turn if
    /// there is one; with no turn yet, or a Compaction turn last, they are
    /// dropped (and logged at debug level).
    pub fn into_turns(mut self) -> Vec<Arc<Turn>> {
        self.end_of_input();
        self.turns
    }

    /// How many lines, unknown entries and unknown content blocks were
    /// skipped because they could not be parsed (the browser's
    /// `N entries could not be parsed` note). Noise and session metadata are
    /// not counted.
    pub fn unparsed_count(&self) -> usize {
        self.unparsed
    }

    /// Attachment kinds seen that are neither allowlisted nor known noise,
    /// with how often each occurred. They are skipped (not counted as
    /// unparsed) and logged so the allowlist can be extended.
    pub fn unknown_attachment_kinds(&self) -> &BTreeMap<String, usize> {
        &self.unknown_kinds
    }

    /// The timestamp of the first conversation entry that has one. Only
    /// entries the assembler handles count (user and assistant entries, the
    /// system entries it uses and injected attachments); timestamped noise
    /// and metadata entries do not.
    pub fn first_timestamp(&self) -> Option<DateTime<Utc>> {
        self.first_ts
    }

    /// The timestamp of the last conversation entry that has one; see
    /// [`first_timestamp`](Self::first_timestamp) for which entries count.
    pub fn last_timestamp(&self) -> Option<DateTime<Utc>> {
        self.last_ts
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
    ///
    /// An assembler is fed either with `push_line` or with
    /// [`push_chunk`](Self::push_chunk) / [`finish`](Self::finish), not both:
    /// only the latter track byte offsets.
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
        // Bytes already buffered contain no newline: search only the new ones.
        let mut search_from = buf.len();
        buf.extend_from_slice(chunk);
        let mut consumed = 0;
        while let Some(rel) = buf[search_from..].iter().position(|&b| b == b'\n') {
            let end = search_from + rel;
            let start = self.pending_start + consumed as u64;
            self.process(
                &buf[consumed..end],
                &(start..self.pending_start + end as u64),
            );
            consumed = end + 1;
            search_from = consumed;
        }
        buf.drain(..consumed);
        self.pending_start += consumed as u64;
        self.pending = buf;
        self.collect(start_len)
    }

    /// Declares the end of a batch for a growing transcript: injected
    /// entries still waiting for the next entry go into the current turn and
    /// stay there. Unlike [`finish`](Self::finish) it leaves a trailing
    /// partial line buffered, so more bytes can still complete it.
    pub fn flush_held(&mut self) -> Vec<ModelUpdate> {
        let start_len = self.turns.len();
        self.end_of_input();
        self.collect(start_len)
    }

    /// Declares the end of the input: flushes a trailing line that has no
    /// newline, then places held injected entries in the current turn (as
    /// [`flush_held`](Self::flush_held) does).
    pub fn finish(&mut self) -> Vec<ModelUpdate> {
        let start_len = self.turns.len();
        let buf = std::mem::take(&mut self.pending);
        if !buf.is_empty() {
            let start = self.pending_start;
            self.process(&buf, &(start..start + buf.len() as u64));
            self.pending_start += buf.len() as u64;
        }
        self.end_of_input();
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
        self.lines += 1;
        if line.iter().all(u8::is_ascii_whitespace) {
            return;
        }
        let head = match serde_json::from_slice::<Head>(line) {
            Ok(head) => head,
            Err(err) => {
                self.skip_unparsed(&format!("not a valid entry: {err}"));
                return;
            }
        };
        self.check_version(&head);
        let kind = head.kind.as_deref().unwrap_or_default();
        match kind {
            "user" | "assistant" => {
                if let Some(entry) = self.parse_entry(line) {
                    let ts = self.timestamp(&entry);
                    if kind == "user" {
                        self.on_user(entry, ts, range);
                    } else {
                        self.on_assistant(entry, ts, range);
                    }
                }
            }
            "system" => {
                let sub = head.subtype.as_deref().unwrap_or_default();
                if (matches!(sub, "turn_duration" | "compact_boundary") || is_injected_system(sub))
                    && let Some(entry) = self.parse_entry(line)
                {
                    let ts = self.timestamp(&entry);
                    match sub {
                        "turn_duration" => self.on_turn_duration(&entry, range),
                        "compact_boundary" => self.on_compact_boundary(&entry, ts, range),
                        _ => {
                            let text = system_text(
                                sub,
                                entry.content.as_ref(),
                                entry.command_run.as_ref(),
                            );
                            self.hold(sub, ts, text, entry.uuid.as_deref(), range);
                        }
                    }
                }
            }
            "attachment" => {
                let attachment = head.attachment.and_then(|a| a.kind).unwrap_or_default();
                if is_injected_attachment(&attachment) {
                    if let Some(entry) = self.parse_entry(line) {
                        let ts = self.timestamp(&entry);
                        self.on_attachment(&attachment, &entry, ts, range);
                    }
                } else if !is_known_noise_attachment(&attachment) {
                    let label = if attachment.is_empty() {
                        "<missing>".to_owned()
                    } else {
                        attachment
                    };
                    *self.unknown_kinds.entry(label).or_default() += 1;
                }
            }
            k if is_noise_type(k) => {}
            k if is_metadata_type(k) => {
                log::debug!("line {}: skipping session metadata entry {k:?}", self.lines);
            }
            k => {
                if head.message.is_some() {
                    self.skip_unparsed(&format!("unknown entry type {k:?}"));
                } else {
                    log::debug!("line {}: skipping metadata entry {k:?}", self.lines);
                }
            }
        }
    }

    /// Parses a conversation line in full, noting the session cwd. A failure
    /// counts the line as unparsed.
    fn parse_entry(&mut self, line: &[u8]) -> Option<Entry> {
        let entry = match read_entry(line) {
            Ok(entry) => entry,
            Err(err) => {
                self.skip_unparsed(&format!("unreadable entry: {err}"));
                return None;
            }
        };
        if self.cwd.is_none()
            && let Some(cwd) = entry.cwd.as_deref().filter(|c| !c.is_empty())
        {
            self.cwd = Some(PathBuf::from(cwd));
        }
        Some(entry)
    }

    /// Counts and logs something that was skipped because it could not be
    /// understood.
    fn skip_unparsed(&mut self, what: &str) {
        self.unparsed += 1;
        log::warn!("line {}: skipped, {what}", self.lines);
    }

    /// Logs one warning per session when an entry's `version` is newer than
    /// [`NEWEST_KNOWN_VERSION`].
    fn check_version(&mut self, head: &Head) {
        if self.warned_version {
            return;
        }
        let Some(version) = head.version.as_ref().and_then(|v| v.as_str()) else {
            return;
        };
        if self.last_version.as_deref() == Some(version) {
            return;
        }
        self.last_version = Some(version.to_owned());
        if version_newer(version, NEWEST_KNOWN_VERSION) {
            self.warned_version = true;
            log::warn!(
                "line {}: transcript version {version} is newer than the newest known \
                 {NEWEST_KNOWN_VERSION}; parsing on a best-effort basis",
                self.lines
            );
        }
    }

    /// Counts the unknown content blocks of a user or assistant message.
    fn check_blocks(&mut self, content: &Content) {
        match content {
            Content::Blocks(blocks) => {
                for block in blocks {
                    if !is_known_block(block.kind.as_deref()) {
                        self.skip_unparsed(&format!("unknown content block {:?}", block.kind));
                    }
                }
            }
            Content::Other(_) => self.skip_unparsed("unreadable message content"),
            Content::Text(_) => {}
        }
    }

    fn timestamp(&mut self, entry: &Entry) -> DateTime<Utc> {
        let parsed = usable_timestamp(entry);
        if let Some(ts) = parsed {
            self.first_ts.get_or_insert(ts);
            self.last_ts = Some(ts);
        }
        parsed.or(self.last_ts).unwrap_or(DateTime::UNIX_EPOCH)
    }

    /// Holds an injected entry until the next entry decides its turn.
    fn hold(
        &mut self,
        kind: &str,
        timestamp: DateTime<Utc>,
        text: String,
        uuid: Option<&str>,
        range: &Range<u64>,
    ) {
        if self.held.is_empty() {
            self.held_key = Some(entry_id(uuid, range).to_string());
            self.held_line = Some(range.clone());
        }
        self.held.push(InjectedEntry {
            kind: kind.to_owned(),
            timestamp,
            text,
        });
    }

    fn on_attachment(&mut self, kind: &str, entry: &Entry, ts: DateTime<Utc>, range: &Range<u64>) {
        let Some(attachment) = &entry.attachment else {
            return;
        };
        let queued = if kind == "queued_command" {
            queued_prompt_text(attachment, entry.origin.as_ref())
        } else {
            None
        };
        if let Some(text) = queued {
            let id = entry_id(entry.uuid.as_deref(), range);
            self.start_user_turn(id, ts, usable_timestamp(entry), text, range);
        } else {
            let text = attachment_text(kind, attachment);
            self.hold(kind, ts, text, entry.uuid.as_deref(), range);
        }
    }

    /// Starts a User turn; held injected entries become its first item.
    /// `usable` is the prompt's own timestamp, when it has one: the start of
    /// the Assistant turn that follows. `range` is the prompt's line.
    fn start_user_turn(
        &mut self,
        id: TurnId,
        timestamp: DateTime<Utc>,
        usable: Option<DateTime<Utc>>,
        text: String,
        range: &Range<u64>,
    ) {
        self.cur = None;
        self.prompt_ts = usable;
        self.user_flushed = false;
        self.compaction_pending = None;
        let mut sources = TurnSources::new();
        sources.set_own(&self.source, range);
        let held = take_held(&mut self.held, &mut self.held_key, &mut self.held_line);
        if let Some(held) = &held {
            held.add_to(&mut sources, &self.source);
        }
        let items = held.map(|held| held.item).into_iter().collect();
        self.turns.push(Arc::new(Turn::User(UserTurn {
            id,
            timestamp,
            text,
            items,
            sources,
        })));
    }

    /// Places held injected entries in the current turn: the open Assistant
    /// turn, or a User turn that has no Assistant turn yet. Without such a
    /// turn (none yet, or a Compaction turn last) they stay held for the next
    /// one.
    fn flush_current(&mut self) {
        if self.held.is_empty() {
            return;
        }
        if let Some(b) = self.cur.as_mut() {
            let Some(held) = take_held(&mut self.held, &mut self.held_key, &mut self.held_line)
            else {
                return;
            };
            if let Turn::Assistant(turn) = Arc::make_mut(&mut self.turns[b.idx]) {
                let HeldItem { item, line } = held;
                let id = item.id().clone();
                match (turn.items.last_mut(), item) {
                    // Deliberately wider than "only noise between": a tool
                    // result or turn footer between two injected runs makes
                    // no item of its own, so the entries are still adjacent
                    // on screen and collapse into the previous item.
                    (
                        Some(Item::Injected { entries, .. }),
                        Item::Injected { entries: more, .. },
                    ) => {
                        entries.extend(more);
                    }
                    (_, item) => {
                        upsert(turn, b, item, &mut false);
                        if let Some(line) = &line {
                            turn.sources.add_line(&self.source, &id, line);
                        }
                    }
                }
                self.dirty.insert(b.idx);
            }
        } else if let Some(last) = self.turns.len().checked_sub(1)
            && matches!(&*self.turns[last], Turn::User(_))
            && let Some(HeldItem { item, line }) =
                take_held(&mut self.held, &mut self.held_key, &mut self.held_line)
            && let Turn::User(turn) = Arc::make_mut(&mut self.turns[last])
        {
            let id = item.id().clone();
            match (turn.items.last_mut(), item) {
                // An earlier flush after the prompt: same collapsing as in an
                // Assistant turn, so a live batch boundary changes nothing.
                // The item `start_user_turn` put first precedes the prompt
                // and stays separate.
                (Some(Item::Injected { entries, .. }), Item::Injected { entries: more, .. })
                    if self.user_flushed =>
                {
                    entries.extend(more);
                }
                (_, item) => {
                    turn.items.push(item);
                    if let Some(line) = &line {
                        turn.sources.add_line(&self.source, &id, line);
                    }
                }
            }
            self.user_flushed = true;
            self.dirty.insert(last);
        }
    }

    /// End of input (or of a batch that the caller declares final): held
    /// entries go to the current turn and unknown attachment kinds are
    /// logged once each.
    fn end_of_input(&mut self) {
        self.flush_current();
        if !self.held.is_empty() {
            let n = self.held.len();
            log::debug!(
                "{n} injected {} ha{} no turn to go into yet (none, or a compaction last)",
                if n == 1 { "entry" } else { "entries" },
                if n == 1 { "s" } else { "ve" },
            );
        }
        for (kind, count) in &self.unknown_kinds {
            if self.logged_kinds.insert(kind.clone()) {
                log::info!("unknown attachment kind {kind:?} ({count}\u{d7})");
            }
        }
    }

    fn on_compact_boundary(&mut self, entry: &Entry, ts: DateTime<Utc>, range: &Range<u64>) {
        self.flush_current();
        self.cur = None;
        self.prompt_ts = None;
        self.compaction_pending = Some(self.turns.len());
        self.turns.push(Arc::new(Turn::Compaction(CompactionTurn {
            id: entry_id(entry.uuid.as_deref(), range),
            timestamp: ts,
            summary: String::new(),
            sources: own_sources(&self.source, range),
        })));
    }

    /// The `isCompactSummary` entry following a boundary supplies the
    /// summary; it is never a prompt.
    fn on_compact_summary(
        &mut self,
        entry: &Entry,
        ts: DateTime<Utc>,
        range: &Range<u64>,
        summary: String,
    ) {
        if let Some(idx) = self.compaction_pending.take() {
            if let Turn::Compaction(turn) = Arc::make_mut(&mut self.turns[idx]) {
                turn.summary = summary;
                self.dirty.insert(idx);
            }
        } else {
            // A summary without a boundary: still a compaction.
            self.flush_current();
            self.cur = None;
            self.prompt_ts = None;
            self.turns.push(Arc::new(Turn::Compaction(CompactionTurn {
                id: entry_id(entry.uuid.as_deref(), range),
                timestamp: ts,
                summary,
                sources: own_sources(&self.source, range),
            })));
        }
    }

    fn on_user(&mut self, entry: Entry, ts: DateTime<Utc>, range: &Range<u64>) {
        let Some(content) = entry.message.as_ref().and_then(|m| m.content.as_ref()) else {
            return;
        };
        self.check_blocks(content);
        let blocks: &[Block] = match content {
            Content::Blocks(blocks) => blocks,
            _ => &[],
        };
        if blocks
            .iter()
            .any(|b| b.kind.as_deref() == Some("tool_result"))
        {
            self.flush_current();
            self.compaction_pending = None;
            let is_async = entry.is_async_launch();
            for block in blocks.iter().filter(|b| b.is_result()) {
                self.attach_result(block, ts, range, is_async);
            }
            return;
        }
        if entry.is_compact_summary == Some(true) {
            let summary = content_text(content);
            self.on_compact_summary(&entry, ts, range, summary);
            return;
        }
        if entry.is_meta == Some(true) {
            let text = content_text(content);
            self.hold("meta", ts, text, entry.uuid.as_deref(), range);
            return;
        }
        // A local command is handled by the harness and is no prompt: it is
        // held like any injected entry. `cur` and `prompt_ts` are left alone,
        // so the open Assistant turn continues and keeps its start.
        if let Some(raw) = entry.local_command_text() {
            let text = local_command_text(&raw);
            self.hold("local_command", ts, text, entry.uuid.as_deref(), range);
            return;
        }
        let Some(text) = entry.human_prompt_text() else {
            return;
        };
        let id = entry_id(entry.uuid.as_deref(), range);
        self.start_user_turn(id, ts, usable_timestamp(&entry), text, range);
    }

    fn on_assistant(&mut self, entry: Entry, ts: DateTime<Utc>, range: &Range<u64>) {
        let effort = entry.effort();
        let is_async = entry.is_async_launch();
        // Only a new turn needs it (its start).
        let usable = self
            .cur
            .is_none()
            .then(|| usable_timestamp(&entry))
            .flatten();
        let api_error_line = entry.is_api_error();
        let Some(message) = entry.message else {
            return;
        };
        // An empty reason is no reason: it would give the notice `stopped: `.
        let stop_reason = message.stop_reason.filter(|s| !s.is_empty());
        if let Some(content) = &message.content {
            self.check_blocks(content);
        }
        self.flush_current();
        self.compaction_pending = None;
        // Still held when the previous turn cannot take them (none, or a
        // Compaction turn): they open this turn.
        let mut held_item = take_held(&mut self.held, &mut self.held_key, &mut self.held_line);
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
                    sources: TurnSources::new(),
                })));
                Builder {
                    idx,
                    index: HashMap::new(),
                    last_model: if is_real_model(&model) {
                        model.clone()
                    } else {
                        String::new()
                    },
                    messages: HashMap::new(),
                    counted: 0,
                    start: self.prompt_ts.take().or(usable),
                    duration: None,
                    usage: HashMap::new(),
                    next_block: HashMap::new(),
                    last_block: HashMap::new(),
                    signatures: HashMap::new(),
                    stop: HashMap::new(),
                    api_errors: HashSet::new(),
                    error_text: HashMap::new(),
                }
            }
        };

        let mut changed = false;
        let mut results: Vec<Block> = Vec::new();
        if let Turn::Assistant(turn) = Arc::make_mut(&mut self.turns[b.idx]) {
            if let Some(held) = held_item.take() {
                held.add_to(&mut turn.sources, &self.source);
                upsert(turn, &mut b, held.item, &mut changed);
            }
            if turn.effort.is_none() && effort.is_some() {
                turn.effort = effort;
                changed = true;
            }
            let known = b.messages.get(&msg_id).copied();
            let first_sight = known.is_none();
            let message_no = known.unwrap_or_else(|| {
                let number = turn.sources.add_message(&self.source, range);
                b.messages.insert(msg_id.clone(), number);
                number
            });
            if first_sight {
                if model != SYNTHETIC_MODEL {
                    b.counted += 1;
                }
                let real = is_real_model(&model);
                if real && !is_real_model(&turn.model) {
                    // The turn started with a placeholder message: the first
                    // real model is the turn's model, not a change.
                    turn.model.clone_from(&model);
                    changed = true;
                } else if real && model != b.last_model {
                    let id = ItemId::from(format!("model:{msg_id}"));
                    if !b.index.contains_key(&id) {
                        turn.sources.add_message_item(&id, message_no);
                    }
                    let item = Item::ModelChange {
                        id,
                        model: model.clone(),
                    };
                    upsert(turn, &mut b, item, &mut changed);
                }
                if real {
                    b.last_model = model;
                }
            }

            if api_error_line && b.api_errors.insert(msg_id.clone()) {
                // The message turns out to be an error: text already shown
                // for it is the error's text and moves to the notice.
                take_message_text(turn, &mut b, &mut self.calls, &msg_id, &mut changed);
            }
            let api_error = b.api_errors.contains(&msg_id);
            let stop_changed = stop_reason.is_some();
            changed |= turn
                .sources
                .note_message_line(message_no, range, stop_changed);
            if let Some(reason) = stop_reason {
                b.stop.insert(msg_id.clone(), reason);
            }

            let blocks = match message.content {
                Some(Content::Blocks(blocks)) => blocks,
                Some(Content::Text(text)) => vec![Block {
                    kind: Some("text".to_owned()),
                    text: Some(text),
                    ..Block::default()
                }],
                _ => Vec::new(),
            };
            let api_index = entry.api_block_index;
            // The numbers given to the error blocks of this line.
            let mut error_keys: Vec<usize> = Vec::new();
            for (i, block) in blocks.into_iter().enumerate() {
                match block.kind.as_deref() {
                    Some("text") if api_error => {
                        // The error is shown as a notice instead of text.
                        let text = block.text.unwrap_or_default();
                        if !text.is_empty() {
                            let blocks = b.error_text.entry(msg_id.clone()).or_default();
                            let key = error_block_key(blocks, &error_keys, api_index, i, &text);
                            error_keys.push(key);
                            blocks.insert(key, text);
                        }
                    }
                    Some("text") => {
                        let text = block.text.unwrap_or_default();
                        if !text.is_empty() {
                            let id = block_id(
                                turn,
                                &mut b,
                                &msg_id,
                                api_index,
                                i,
                                |old, _| matches!(old, Item::Text { text: o, .. } if text.starts_with(o.as_str())),
                            );
                            note_block(&mut b, &msg_id, api_index, &id);
                            if !b.index.contains_key(&id) {
                                turn.sources.add_message_item(&id, message_no);
                            }
                            upsert(turn, &mut b, Item::Text { id, text }, &mut changed);
                        }
                    }
                    Some("thinking") => {
                        let signature = block.signature;
                        let thinking = match block.thinking.filter(|t| !t.is_empty()) {
                            Some(text) => Thinking::Text(text),
                            None => Thinking::Redacted,
                        };
                        let id =
                            block_id(turn, &mut b, &msg_id, api_index, i, |old, old_sig| {
                                match (old, &thinking) {
                                    (
                                        Item::Thinking {
                                            thinking: Thinking::Text(o),
                                            ..
                                        },
                                        Thinking::Text(n),
                                    ) => n.starts_with(o.as_str()),
                                    (
                                        Item::Thinking {
                                            thinking: Thinking::Redacted,
                                            ..
                                        },
                                        Thinking::Redacted,
                                    ) => old_sig.is_some() && old_sig == signature.as_deref(),
                                    _ => false,
                                }
                            });
                        if let Some(sig) = signature {
                            b.signatures.insert(id.clone(), sig);
                        }
                        note_block(&mut b, &msg_id, api_index, &id);
                        if !b.index.contains_key(&id) {
                            turn.sources.add_message_item(&id, message_no);
                        }
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
                        note_block(&mut b, &msg_id, api_index, &call.id);
                        // A call sent again is linked already; a new one is added without
                        // looking for it in the table, which would scan it.
                        let source = match self.calls.get(&call_id) {
                            Some(known) if known.turn == b.idx => known.source,
                            _ => turn.sources.add_call(&self.source, &call.id, range),
                        };
                        let item = upsert(turn, &mut b, Item::ToolCall(call), &mut changed);
                        self.calls.entry(call_id).or_insert(CallRef {
                            turn: b.idx,
                            item,
                            started: ts,
                            source,
                        });
                    }
                    _ if block.is_result() => results.push(block),
                    _ => {}
                }
            }

            if stop_changed || api_error {
                let notice_id = ItemId::from(format!("notice:{msg_id}"));
                match message_notice(&b, &msg_id) {
                    Some((level, text)) => {
                        if !b.index.contains_key(&notice_id) {
                            turn.sources.add_message_item(&notice_id, message_no);
                        }
                        let item = Item::Notice {
                            id: notice_id,
                            level,
                            text,
                        };
                        upsert(turn, &mut b, item, &mut changed);
                    }
                    None => remove(turn, &mut b, &mut self.calls, &notice_id, &mut changed),
                }
            }

            if let Some(usage) = &message.usage {
                b.usage.insert(msg_id, token_usage(usage));
            }
            // Once the turn has a footer, its count and tokens follow later
            // messages.
            if turn.stats.is_some() {
                let stats = b.stats();
                if turn.stats != stats {
                    turn.stats = stats;
                    changed = true;
                }
            }
        }
        if changed {
            self.dirty.insert(b.idx);
        }
        self.cur = Some(b);
        for block in &results {
            self.attach_result(block, ts, range, is_async);
        }
    }

    /// A `turn_duration` entry sets the turn's duration (the last entry
    /// wins; entries are never summed): wall-clock time from the turn's start
    /// to the entry's timestamp, or the entry's own `durationMs` when either
    /// end has no usable timestamp or the span would be negative.
    fn on_turn_duration(&mut self, entry: &Entry, range: &Range<u64>) {
        self.flush_current();
        self.compaction_pending = None;
        let Some(b) = self.cur.as_mut() else {
            return;
        };
        let span = match (b.start, usable_timestamp(entry)) {
            (Some(start), Some(end)) => (end - start).to_std().ok(),
            _ => None,
        };
        let Some(duration) = span.or(entry.duration_ms.map(Duration::from_millis)) else {
            return;
        };
        b.duration = Some(duration);
        let stats = b.stats();
        let idx = b.idx;
        if let Turn::Assistant(turn) = Arc::make_mut(&mut self.turns[idx]) {
            let mut changed = turn.sources.set_footer(&self.source, range);
            if turn.stats != stats {
                turn.stats = stats;
                changed = true;
            }
            if changed {
                self.dirty.insert(idx);
            }
        }
    }

    /// Attaches the result in `block` to the call it names, if known.
    ///
    /// The call's duration is the time to the result. An asynchronous launch
    /// (`is_async`, from the entry's `toolUseResult`) only records the start
    /// of the work, so the call gets none.
    fn attach_result(
        &mut self,
        block: &Block,
        ts: DateTime<Utc>,
        range: &Range<u64>,
        is_async: bool,
    ) {
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
        let duration = if is_async {
            None
        } else {
            (ts - call.started).to_std().ok()
        };

        let unchanged = matches!(
            &*self.turns[call.turn],
            Turn::Assistant(t) if matches!(
                t.items.get(call.item),
                Some(Item::ToolCall(c))
                    if c.result.as_ref() == Some(&result)
                        && c.status == status
                        && c.duration == duration
            )
        ) && self.turns[call.turn]
            .sources()
            .has_result(call.source, range);
        if unchanged {
            return;
        }
        if let Turn::Assistant(turn) = Arc::make_mut(&mut self.turns[call.turn]) {
            turn.sources.set_result(&self.source, call.source, range);
            if let Some(Item::ToolCall(c)) = turn.items.get_mut(call.item) {
                c.result = Some(result);
                c.status = status;
                c.duration = duration;
            }
            self.dirty.insert(call.turn);
        }
    }
}

/// Content blocks the adapter understands (even when it shows nothing for
/// them). Anything else is skipped and counted.
fn is_known_block(kind: Option<&str>) -> bool {
    kind.is_some_and(|k| {
        matches!(
            k,
            "text"
                | "thinking"
                | "redacted_thinking"
                | "tool_use"
                | "server_tool_use"
                | "mcp_tool_use"
                | "tool_result"
                | "image"
                | "document"
        ) || k.ends_with("_tool_result")
    })
}

/// Stop reasons that end a message normally.
fn is_normal_stop(reason: &str) -> bool {
    matches!(
        reason,
        "end_turn" | "tool_use" | "stop_sequence" | "pause_turn"
    )
}

/// The notice for how a message ended (SPEC cycle 2 §4.2), `None` for a
/// normal ending. An API error is shown as an `Error` with its text, and
/// wins over the stop reason; a flagged message without text has none. Else
/// the last non-null `stop_reason` decides.
fn message_notice(b: &Builder, msg_id: &str) -> Option<(NoticeLevel, String)> {
    if b.api_errors.contains(msg_id) {
        let text = b
            .error_text
            .get(msg_id)?
            .values()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        return (!text.trim().is_empty()).then_some((NoticeLevel::Error, text));
    }
    let reason = b.stop.get(msg_id)?;
    (!is_normal_stop(reason)).then(|| (NoticeLevel::Warning, format!("stopped: {reason}")))
}

/// Turns the held injected entries into one `Injected` item, identified by
/// the first entry.
fn take_held(
    held: &mut Vec<InjectedEntry>,
    key: &mut Option<String>,
    line: &mut Option<Range<u64>>,
) -> Option<HeldItem> {
    if held.is_empty() {
        return None;
    }
    let key = key.take().unwrap_or_default();
    Some(HeldItem {
        item: Item::Injected {
            id: ItemId::from(format!("injected:{key}")),
            entries: std::mem::take(held),
        },
        line: line.take(),
    })
}

/// The `Injected` item the held entries made, and the line of the first.
struct HeldItem {
    item: Item,
    line: Option<Range<u64>>,
}

impl HeldItem {
    /// Records the line of the item in `sources`, now that it goes into a
    /// turn. The item is new, so nothing is looked up.
    fn add_to(&self, sources: &mut TurnSources, bytes: &ByteSource) {
        if let Some(line) = &self.line {
            sources.add_line(bytes, self.item.id(), line);
        }
    }
}

/// The entry's own `timestamp`, when it parses.
fn usable_timestamp(entry: &Entry) -> Option<DateTime<Utc>> {
    entry
        .timestamp
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&Utc))
}

fn is_real_model(model: &str) -> bool {
    !model.is_empty() && model != SYNTHETIC_MODEL
}

/// The id for a text or thinking block.
///
/// With `apiBlockIndex` it is `<message.id>:<apiBlockIndex + position>`.
/// Without it (older transcripts, one block per line) a per-message counter
/// numbers the blocks, so distinct blocks never collide. The one exception is
/// a re-send: when the message's most recent block is still the previous
/// block and `is_resend(old_item, old_signature)` holds (same kind, text
/// extended or identical, same signature), that block is replaced.
fn block_id(
    turn: &AssistantTurn,
    b: &mut Builder,
    msg_id: &str,
    api_index: Option<usize>,
    position: usize,
    is_resend: impl Fn(&Item, Option<&str>) -> bool,
) -> ItemId {
    if let Some(base) = api_index {
        return ItemId::from(format!("{msg_id}:{}", base + position));
    }
    if let Some(last) = b.last_block.get(msg_id)
        && let Some(&pos) = b.index.get(last)
        && is_resend(&turn.items[pos], b.signatures.get(last).map(String::as_str))
    {
        return last.clone();
    }
    let next = b.next_block.entry(msg_id.to_owned()).or_default();
    let id = ItemId::from(format!("{msg_id}:{next}"));
    *next += 1;
    id
}

/// Records `id` as the message's most recent block (only needed without
/// `apiBlockIndex`).
fn note_block(b: &mut Builder, msg_id: &str, api_index: Option<usize>, id: &ItemId) {
    if api_index.is_none() {
        b.last_block.insert(msg_id.to_owned(), id.clone());
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

/// The number of an error text block. With `apiBlockIndex` it is that index
/// plus the block's position. Without it, a text that equals (first) or
/// extends a block the message already has (not one from this line) is a
/// re-send and replaces it; any other text gets the next free number.
fn error_block_key(
    blocks: &BTreeMap<usize, String>,
    taken: &[usize],
    api_index: Option<usize>,
    position: usize,
    text: &str,
) -> usize {
    if let Some(base) = api_index {
        return base + position;
    }
    let free = || blocks.iter().rev().filter(|(key, _)| !taken.contains(key));
    free()
        .find(|(_, old)| old.as_str() == text)
        .or_else(|| free().find(|(_, old)| text.starts_with(old.as_str())))
        .map(|(&key, _)| key)
        .unwrap_or_else(|| blocks.keys().next_back().map_or(0, |last| last + 1))
}

/// Moves the text blocks the turn already shows for message `msg_id` into the
/// message's error text, and removes them from the turn. Their ids end in the
/// block number (`<message id>:<number>`), which stays their number there.
fn take_message_text(
    turn: &mut AssistantTurn,
    b: &mut Builder,
    calls: &mut HashMap<String, CallRef>,
    msg_id: &str,
    changed: &mut bool,
) {
    let taken: Vec<(ItemId, usize, String)> = turn
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Text { id, text } => {
                let number = id.as_str().strip_prefix(msg_id)?.strip_prefix(':')?;
                Some((id.clone(), number.parse().ok()?, text.clone()))
            }
            _ => None,
        })
        .collect();
    for (id, number, text) in taken {
        b.error_text
            .entry(msg_id.to_owned())
            .or_default()
            .insert(number, text);
        remove(turn, b, calls, &id, changed);
    }
}

/// Removes the item with the given id from the turn being built, if it has
/// it. The items after it shift, so the positions kept in the builder's index
/// and in `calls` (for a tool result still to come) are renewed.
///
/// Injected items that the removal leaves side by side collapse into the
/// first one (consecutive injected entries are one section, SPEC §5.6).
fn remove(
    turn: &mut AssistantTurn,
    b: &mut Builder,
    calls: &mut HashMap<String, CallRef>,
    id: &ItemId,
    changed: &mut bool,
) {
    let Some(pos) = b.index.remove(id) else {
        return;
    };
    turn.items.remove(pos);
    shift_after(b, calls, pos);
    *changed = true;
    if pos > 0
        && pos < turn.items.len()
        && matches!(turn.items[pos - 1], Item::Injected { .. })
        && matches!(turn.items[pos], Item::Injected { .. })
        && let Item::Injected {
            id: second,
            entries,
        } = turn.items.remove(pos)
    {
        b.index.remove(&second);
        shift_after(b, calls, pos);
        if let Item::Injected { entries: first, .. } = &mut turn.items[pos - 1] {
            first.extend(entries);
        }
    }
}

/// Renews the positions of the items after `pos`, which an item taken out at
/// `pos` moved up by one.
fn shift_after(b: &mut Builder, calls: &mut HashMap<String, CallRef>, pos: usize) {
    for at in b.index.values_mut().filter(|at| **at > pos) {
        *at -= 1;
    }
    for call in calls
        .values_mut()
        .filter(|c| c.turn == b.idx && c.item > pos)
    {
        call.item -= 1;
    }
}

/// The sources of a turn that is one line.
fn own_sources(bytes: &ByteSource, range: &Range<u64>) -> TurnSources {
    let mut sources = TurnSources::new();
    sources.set_own(bytes, range);
    sources
}

fn entry_id(uuid: Option<&str>, range: &Range<u64>) -> TurnId {
    match uuid {
        Some(uuid) if !uuid.is_empty() => TurnId::from(uuid),
        _ => TurnId::from(format!("L{}", range.start)),
    }
}
