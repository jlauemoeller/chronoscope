//! The harness-agnostic conversation model (SPEC §5.1).
//!
//! Harness adapters translate their native format into these types. Turns
//! are shared as [`Arc<Turn>`] so that [`ModelUpdate`]s can be cloned cheaply
//! into user events, and every turn and item has a stable id so that fold and
//! selection state survives in-place updates.

mod output;

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

pub use output::{ByteSource, LineFormat, OutputSource, ToolOutput, ToolOutputError, ToolResult};

/// Tool results longer than this many bytes are stored lazily (SPEC §9.2).
pub const INLINE_OUTPUT_LIMIT: usize = 4096;

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Arc<str>);

        impl $name {
            /// The id as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(Arc::from(value))
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(Arc::from(value))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// Stable identity of a [`Turn`]; unchanged by in-place updates.
    TurnId
);
string_id!(
    /// Stable identity of an [`Item`] within its turn.
    ItemId
);

/// The agent harness a session came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Harness {
    /// Claude Code.
    Claude,
    /// OpenCode.
    OpenCode,
}

/// A reference to a child session (e.g. a subagent run).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRef {
    /// The child session's id.
    pub id: String,
    /// The child's title or description, when known.
    pub title: Option<String>,
}

/// A whole session.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// Session id.
    pub id: String,
    /// The harness it came from.
    pub harness: Harness,
    /// Project name (derived from the cwd).
    pub project: String,
    /// The real working directory.
    pub cwd: PathBuf,
    /// Session title, when the harness records one.
    pub title: Option<String>,
    /// When the session started.
    pub started_at: DateTime<Utc>,
    /// The conversation.
    pub turns: Vec<Arc<Turn>>,
    /// Child sessions (subagents).
    pub children: Vec<SessionRef>,
}

/// One turn of the conversation.
#[derive(Debug, Clone, PartialEq)]
pub enum Turn {
    /// A human prompt.
    User(UserTurn),
    /// The agent's work up to the next human prompt.
    Assistant(AssistantTurn),
    /// A compaction summary (produced by task 007).
    Compaction(CompactionTurn),
}

impl Turn {
    /// The turn's stable id.
    pub fn id(&self) -> &TurnId {
        match self {
            Turn::User(t) => &t.id,
            Turn::Assistant(t) => &t.id,
            Turn::Compaction(t) => &t.id,
        }
    }

    /// When the turn started.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Turn::User(t) => t.timestamp,
            Turn::Assistant(t) => t.timestamp,
            Turn::Compaction(t) => t.timestamp,
        }
    }
}

/// A human prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct UserTurn {
    /// Stable id (the prompt entry's uuid).
    pub id: TurnId,
    /// When the prompt was sent.
    pub timestamp: DateTime<Utc>,
    /// The prompt text.
    pub text: String,
    /// Injected content preceding or attached to the prompt.
    pub items: Vec<Item>,
}

/// The agent's side of one exchange.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantTurn {
    /// Stable id (the uuid of the turn's first entry).
    pub id: TurnId,
    /// When the turn started.
    pub timestamp: DateTime<Utc>,
    /// The model of the turn's first message.
    pub model: String,
    /// The reasoning effort, when the harness records it.
    pub effort: Option<String>,
    /// The turn's content, in order.
    pub items: Vec<Item>,
    /// The footer statistics, once known.
    pub stats: Option<TurnStats>,
}

/// A compaction summary.
#[derive(Debug, Clone, PartialEq)]
pub struct CompactionTurn {
    /// Stable id.
    pub id: TurnId,
    /// When the compaction happened.
    pub timestamp: DateTime<Utc>,
    /// The summary text.
    pub summary: String,
}

/// Footer statistics of an Assistant turn.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnStats {
    /// Wall-clock duration of the turn.
    pub duration: Duration,
    /// Number of messages in the turn, as reported by the harness.
    pub message_count: usize,
    /// Input plus output tokens, counting each message once.
    pub tokens: Option<u64>,
    /// Cost in US dollars, when known (never for Claude).
    pub cost: Option<f64>,
}

/// One entry of an [`Item::Injected`] group.
#[derive(Debug, Clone, PartialEq)]
pub struct InjectedEntry {
    /// Kind label (defined by task 007).
    pub kind: String,
    /// When it was injected.
    pub timestamp: DateTime<Utc>,
    /// The injected text.
    pub text: String,
}

/// The text of a thinking block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Thinking {
    /// Readable reasoning text.
    Text(String),
    /// Present, but the text is not recorded (signature only).
    Redacted,
}

/// A piece of content inside a turn.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Markdown text.
    Text {
        /// Stable id.
        id: ItemId,
        /// The markdown source.
        text: String,
    },
    /// Injected content, consecutive entries collapsed (produced by task 007).
    Injected {
        /// Stable id.
        id: ItemId,
        /// The collapsed entries.
        entries: Vec<InjectedEntry>,
    },
    /// A reasoning block.
    Thinking {
        /// Stable id.
        id: ItemId,
        /// The text or the redaction marker.
        thinking: Thinking,
    },
    /// A tool invocation.
    ToolCall(ToolCall),
    /// A muted inline note that the model changed mid-turn.
    ModelChange {
        /// Stable id.
        id: ItemId,
        /// The model from this point on.
        model: String,
    },
}

impl Item {
    /// The item's stable id.
    pub fn id(&self) -> &ItemId {
        match self {
            Item::Text { id, .. }
            | Item::Injected { id, .. }
            | Item::Thinking { id, .. }
            | Item::ModelChange { id, .. } => id,
            Item::ToolCall(call) => &call.id,
        }
    }
}

/// The normalised kind of a tool.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolKind {
    /// Shell command.
    Shell,
    /// File read.
    Read,
    /// File write.
    Write,
    /// File edit.
    Edit,
    /// Content search.
    Grep,
    /// File name search.
    Glob,
    /// URL fetch.
    WebFetch,
    /// Web search.
    WebSearch,
    /// Subagent invocation.
    Subagent,
    /// Anything else, with the harness's tool name.
    Other(String),
}

/// Whether a tool call has finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    /// No result yet.
    Pending,
    /// Finished successfully.
    Ok,
    /// Finished with an error.
    Error,
}

/// A tool invocation with its (possibly pending) result.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// Stable id (the harness's tool-use id).
    pub id: ItemId,
    /// The normalised tool kind.
    pub tool: ToolKind,
    /// Short human-readable description.
    pub description: String,
    /// The tool input as recorded.
    pub input: serde_json::Value,
    /// The result, once it has arrived.
    pub result: Option<ToolResult>,
    /// Pending until a result arrives, then Ok or Error.
    pub status: ToolStatus,
    /// Time between the call and its result.
    pub duration: Option<Duration>,
}

/// A change to the model, emitted by an assembler.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelUpdate {
    /// New turns, added at the end.
    Appended(Vec<Arc<Turn>>),
    /// Existing turns replaced in place (matched by [`TurnId`]).
    Updated(Vec<Arc<Turn>>),
}

impl ModelUpdate {
    /// Applies the update to `turns`. Updated turns whose id is not present
    /// are ignored.
    pub fn apply(&self, turns: &mut Vec<Arc<Turn>>) {
        match self {
            ModelUpdate::Appended(new) => turns.extend(new.iter().cloned()),
            ModelUpdate::Updated(changed) => {
                for turn in changed {
                    if let Some(slot) = turns.iter_mut().rev().find(|t| t.id() == turn.id()) {
                        *slot = Arc::clone(turn);
                    }
                }
            }
        }
    }
}
