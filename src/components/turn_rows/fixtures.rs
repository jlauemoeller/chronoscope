//! Turns and their rows for the tests of [`TurnRows`] and of the components
//! that show them.

use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::components::transcript::fixtures::with_id;
use crate::components::turn_block::fixtures as blocks;
use crate::model::{Thinking, TurnStats};

/// A User turn with a prompt.
pub fn user(id: &str) -> Turn {
    with_id(blocks::user("A question.", vec![]), id)
}

/// An Assistant turn with a text and no stats (it is still running).
pub fn running(id: &str) -> Turn {
    with_id(
        blocks::assistant(None, vec![blocks::text("t1", "An answer.")], None),
        id,
    )
}

/// An Assistant turn with a text and stats of `secs` seconds.
pub fn assistant(id: &str, secs: u64) -> Turn {
    with_id(
        blocks::assistant(
            None,
            vec![blocks::text("t1", "An answer.")],
            Some(stats(secs)),
        ),
        id,
    )
}

pub fn stats(secs: u64) -> TurnStats {
    TurnStats {
        duration: Duration::from_secs(secs),
        ..blocks::stats()
    }
}

/// A Compaction turn.
pub fn compaction(id: &str) -> Turn {
    with_id(blocks::compaction("A summary."), id)
}

/// An Assistant turn holding only redacted thinking (no section, no stats).
pub fn redacted_only(id: &str) -> Turn {
    with_id(
        blocks::assistant(None, vec![blocks::thinking("th", Thinking::Redacted)], None),
        id,
    )
}

/// A User turn with only injected content (a System section, no prompt).
pub fn system_only(id: &str) -> Turn {
    with_id(
        blocks::user(
            "",
            vec![blocks::injected("i1", &[("file", "/notes.md\nUse it.")])],
        ),
        id,
    )
}

/// An Assistant turn with only an error notice.
pub fn notice_only(id: &str) -> Turn {
    with_id(
        blocks::assistant(
            None,
            vec![blocks::notice(
                "n1",
                crate::model::NoticeLevel::Error,
                "The turn ended with an error.",
            )],
            None,
        ),
        id,
    )
}

pub fn rows_of(turns: &[Turn]) -> TurnRows {
    TurnRows::from_turns(turns)
}

/// The turns of the wireframe: User, Assistant 4m 3s, User, Assistant 3h 4m,
/// Compaction, Assistant 2h 45m.
pub fn wireframe() -> Vec<Turn> {
    vec![
        user("u1"),
        assistant("a1", 4 * 60 + 3),
        user("u2"),
        assistant("a2", 3 * 3600 + 4 * 60),
        compaction("c1"),
        assistant("a3", 2 * 3600 + 45 * 60),
    ]
}

/// A session of `turns` turns: a User and an Assistant turn alternate, and
/// every tenth turn is a Compaction.
pub fn long_session(turns: usize) -> Vec<Turn> {
    (0..turns)
        .map(|i| match i {
            _ if i % 10 == 9 => compaction(&format!("c{i}")),
            _ if i % 2 == 0 => user(&format!("u{i}")),
            _ => assistant(&format!("a{i}"), 30 + 97 * i as u64),
        })
        .collect()
}

/// The rows of `turns` as the kinds in order, with `Start` and `End`.
pub fn kinds(rows: &TurnRows) -> Vec<&'static str> {
    (0..rows.row_count())
        .map(|i| match rows.row(i).expect("a row") {
            Row::Start => "Start",
            Row::End => "End",
            Row::Connector => "|",
            Row::Blank => " ",
            Row::Entry(entry) => match entry.kind {
                EntryKind::User => "User",
                EntryKind::Assistant => "Assistant",
                EntryKind::Compaction => "Compaction",
            },
        })
        .collect()
}

pub fn shared(turns: &[Turn]) -> Vec<Arc<Turn>> {
    turns.iter().cloned().map(Arc::new).collect()
}

/// A User turn with a prompt and injected content.
pub fn with_id_user_items(id: &str) -> Turn {
    with_id(
        blocks::user(
            "A question.",
            vec![blocks::injected("i1", &[("file", "/notes.md\nUse it.")])],
        ),
        id,
    )
}

/// The id `id`.
pub fn id(id: &str) -> TurnId {
    TurnId::from(id)
}

/// An `Appended` update of `turns`.
pub fn appended(turns: Vec<Turn>) -> ModelUpdate {
    ModelUpdate::Appended(shared(&turns))
}

/// An `Updated` update of `turns`.
pub fn updated(turns: Vec<Turn>) -> ModelUpdate {
    ModelUpdate::Updated(shared(&turns))
}
