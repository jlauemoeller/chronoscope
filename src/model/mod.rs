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
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(test)]
pub(crate) use output::text_calls;
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

/// Identity of a tool call: the [`ItemId`] of its [`ToolCall`] (for Claude,
/// the `tool_use` id).
pub type ToolCallId = ItemId;

string_id!(
    /// Identity of a project within its harness: the real working directory
    /// for Claude (the encoded directory name when no cwd is known).
    ProjectId
);
string_id!(
    /// Identity of a session within its harness: the transcript's file stem
    /// for Claude, and `<parent-uuid>/subagents/agent-<agentId>` for a
    /// Claude subagent transcript.
    SessionId
);

/// The agent harness a session came from.
///
/// The numbers are stable (they may be persisted) and never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum HarnessId {
    /// Claude Code.
    Claude = 1,
    /// OpenCode.
    OpenCode = 2,
    /// Codex.
    Codex = 3,
    /// Pi.
    Pi = 4,
}

impl HarnessId {
    /// Every harness, in number order.
    pub const ALL: [HarnessId; 4] = [
        HarnessId::Claude,
        HarnessId::OpenCode,
        HarnessId::Codex,
        HarnessId::Pi,
    ];

    /// The stable number, 1 to 4.
    pub fn number(self) -> u8 {
        self as u8
    }

    /// The harness with the given stable number.
    pub fn from_number(number: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|h| h.number() == number)
    }

    /// The stable, lowercase name that starts every stats cache key of the
    /// harness (`claude` gives keys `claude:...`). Unlike the display name it
    /// never changes, because cache files outlive versions.
    pub fn cache_prefix(self) -> &'static str {
        match self {
            HarnessId::Claude => "claude",
            HarnessId::OpenCode => "opencode",
            HarnessId::Codex => "codex",
            HarnessId::Pi => "pi",
        }
    }

    /// The name shown to the user.
    pub fn display_name(self) -> &'static str {
        match self {
            HarnessId::Claude => "Claude",
            HarnessId::OpenCode => "OpenCode",
            HarnessId::Codex => "Codex",
            HarnessId::Pi => "Pi",
        }
    }
}

impl fmt::Display for HarnessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

/// A project: the sessions of one working directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// Identity (the real cwd; see [`ProjectId`]).
    pub id: ProjectId,
    /// The harness it came from.
    pub harness: HarnessId,
    /// Display name: the configured alias, else the cwd's basename.
    pub name: String,
    /// The real working directory; `None` when the harness data names none
    /// (Claude files without any `cwd`).
    pub cwd: Option<PathBuf>,
    /// The newest last activity of its sessions.
    pub last_activity: DateTime<Utc>,
    /// Number of sessions.
    pub session_count: usize,
}

/// One session as listed by a harness, without its conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    /// Session id.
    pub id: SessionId,
    /// The harness it came from.
    pub harness: HarnessId,
    /// The project it belongs to.
    pub project: ProjectId,
    /// The real working directory, when known.
    pub cwd: Option<PathBuf>,
    /// The session title (SPEC §4.1).
    pub title: String,
    /// The first human prompt, when there is one.
    pub first_prompt: Option<String>,
    /// When the session started.
    pub started_at: DateTime<Utc>,
    /// The later of the last entry's timestamp and the file's mtime.
    pub last_activity: DateTime<Utc>,
    /// The transcript file.
    pub path: PathBuf,
    /// Whether the harness records the session as idle (OpenCode
    /// `time_idle`): `Some(false)` while it runs, `None` when the harness
    /// does not record it (Claude).
    pub idle: Option<bool>,
}

/// A session counts as active when its last activity is at most this old.
pub const ACTIVE_WINDOW: chrono::Duration = chrono::Duration::seconds(60);

/// Whether `session` is active at `now` (SPEC §7.2): the harness says it is
/// not idle, or it had activity within the last [`ACTIVE_WINDOW`]. The
/// selector's `●` marker and the browser's `● live` header share this rule.
pub fn is_active(session: &SessionSummary, now: DateTime<Utc>) -> bool {
    session.idle == Some(false) || now.signed_duration_since(session.last_activity) <= ACTIVE_WINDOW
}

/// What the session list shows to the right of a row (SPEC §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStats {
    /// The number of User turns.
    pub turns: usize,
    /// The last entry's timestamp minus the first entry's.
    #[serde(with = "duration_millis")]
    pub duration: Duration,
}

/// Serialises a [`Duration`] as whole milliseconds.
mod duration_millis {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(duration: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_millis(u64::deserialize(d)?))
    }
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
    pub harness: HarnessId,
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
    /// A compaction summary.
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
    /// Kind label: the Claude attachment type (`file`, `hook_system_message`,
    /// ...), the `system` subtype (`local_command`, ...) or `meta` for
    /// `isMeta` user entries.
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
    /// Injected content, consecutive entries collapsed.
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
    /// New turns, added at the end. The transcript treats a turn whose id it
    /// already holds as an update of that turn.
    Appended(Vec<Arc<Turn>>),
    /// Existing turns replaced in place (matched by [`TurnId`]).
    Updated(Vec<Arc<Turn>>),
    /// Loading progress: the fraction of the transcript's bytes parsed, from
    /// 0.0 to 1.0. Sent after each batch of turns.
    Progress(f32),
    /// The initial parse is complete.
    Loaded,
    /// How many lines, entries and content blocks could not be parsed (the
    /// `N entries could not be parsed` note). Sent with [`Loaded`](Self::Loaded)
    /// and again whenever it changes.
    ParseWarnings(usize),
    /// Clears every turn: the session is being reloaded from scratch. Only
    /// the transcript consumes it; the App is to forward it to the
    /// transcript like [`Appended`](Self::Appended).
    Reset,
    /// When the session's data last changed, for the browser's `● live`
    /// marker (SPEC §7.2). Sent after the initial load and after every
    /// processed change. `idle` is the harness's own idleness flag
    /// (OpenCode's `time_idle`); `None` when the harness has none.
    Activity {
        last_modified: SystemTime,
        idle: Option<bool>,
    },
}

impl ModelUpdate {
    /// Applies the update to `turns`. Updated turns whose id is not present
    /// are ignored. `Reset` clears the turns. `Progress`, `Loaded`,
    /// `ParseWarnings` and `Activity` do not touch the turns.
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
            ModelUpdate::Reset => turns.clear(),
            ModelUpdate::Progress(_)
            | ModelUpdate::Loaded
            | ModelUpdate::ParseWarnings(_)
            | ModelUpdate::Activity { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::*;

    fn session(last_activity: DateTime<Utc>, idle: Option<bool>) -> SessionSummary {
        SessionSummary {
            id: SessionId::from("s"),
            harness: HarnessId::Claude,
            project: ProjectId::from("/p"),
            cwd: None,
            title: "t".to_owned(),
            first_prompt: None,
            started_at: last_activity,
            last_activity,
            path: PathBuf::from("/s.jsonl"),
            idle,
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-03-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn cache_prefixes_are_distinct_lowercase_names() {
        let prefixes: Vec<_> = HarnessId::ALL.iter().map(|h| h.cache_prefix()).collect();
        assert_eq!(prefixes, ["claude", "opencode", "codex", "pi"]);
    }

    #[test]
    fn not_idle_is_active_however_old_the_activity() {
        let old = now() - TimeDelta::days(3);
        assert!(is_active(&session(old, Some(false)), now()));
    }

    #[test]
    fn activity_30_seconds_ago_is_active() {
        let recent = now() - TimeDelta::seconds(30);
        assert!(is_active(&session(recent, None), now()));
    }

    #[test]
    fn activity_90_seconds_ago_without_idle_is_not_active() {
        let older = now() - TimeDelta::seconds(90);
        assert!(!is_active(&session(older, None), now()));
    }

    #[test]
    fn idle_true_with_old_activity_is_not_active() {
        let older = now() - TimeDelta::seconds(90);
        assert!(!is_active(&session(older, Some(true)), now()));
    }

    #[test]
    fn the_window_is_sixty_seconds() {
        let at = |secs| session(now() - TimeDelta::seconds(secs), None);
        assert!(is_active(&at(60), now()));
        assert!(!is_active(&at(61), now()));
    }

    #[test]
    fn activity_in_the_future_is_active() {
        let ahead = now() + TimeDelta::seconds(5);
        assert!(is_active(&session(ahead, None), now()));
    }

    #[test]
    fn reset_clears_the_turns() {
        let turn = Arc::new(Turn::Compaction(CompactionTurn {
            id: TurnId::from("c"),
            timestamp: now(),
            summary: "s".to_owned(),
        }));
        let mut turns = Vec::new();
        ModelUpdate::Appended(vec![turn]).apply(&mut turns);
        assert_eq!(turns.len(), 1);
        ModelUpdate::Reset.apply(&mut turns);
        assert!(turns.is_empty());
    }
}
