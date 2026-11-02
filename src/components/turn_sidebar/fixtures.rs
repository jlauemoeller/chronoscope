//! Turns and sidebars for the tests.

use std::sync::Arc;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuirealm::component::Component;

use super::*;
use crate::app::layout::browser_areas;
use crate::components::transcript::fixtures::with_id;
use crate::components::turn_block::fixtures as blocks;
use crate::model::{
    ByteSource, Item, LineFormat, OutputSource, Thinking, ToolOutput, ToolResult, Turn, TurnStats,
};
use crate::test_support::render;

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

/// Durations from the widest (`9999h 59m`, 9 cells) down to `0s`, widest
/// first so that they are on screen in a short pane.
pub fn widest_durations() -> Vec<Turn> {
    let secs = [
        9_999 * 3600 + 59 * 60,
        100 * 3600 + 30 * 60,
        26 * 3600 + 30 * 60,
        59 * 60 + 59,
        59,
        0,
    ];
    secs.iter()
        .enumerate()
        .flat_map(|(i, &s)| [user(&format!("u{i}")), assistant(&format!("a{i}"), s)])
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

/// The sidebar of `turns` in the default theme.
pub fn sidebar(turns: &[Turn]) -> TurnSidebar {
    TurnSidebar::new(blocks::theme()).with_rows(rows_of(turns))
}

/// The sidebar's area on a screen of `width` x `height`.
pub fn pane_area(width: u16, height: u16) -> Rect {
    browser_areas(Rect::new(0, 0, width, height))
        .sidebar
        .expect("the screen is wide enough for the sidebar")
}

/// Draws into a frame of exactly `width` x `height`.
pub fn draw(sidebar: &mut TurnSidebar, width: u16, height: u16) -> Buffer {
    render(width, height, |frame| {
        sidebar.view(frame, frame.area());
    })
}

pub fn shared(turns: &[Turn]) -> Vec<Arc<Turn>> {
    turns.iter().cloned().map(Arc::new).collect()
}

/// An Assistant turn `id` whose only tool call has a lazy result: its
/// output is stored as a reference to a transcript line, so reading it
/// moves both `text_calls` and `source_reads`.
pub fn lazy_output_turn(id: &str) -> Turn {
    let text = "alpha\nbeta\ngamma";
    let line = serde_json::json!({"message": {"content": [
        {"type": "tool_result", "tool_use_id": "c1", "content": text}
    ]}})
    .to_string();
    let range = 0..line.len() as u64;
    let output = ToolOutput::lazy(
        text.lines().count(),
        text.len(),
        OutputSource {
            bytes: ByteSource::Memory(Arc::from(line.into_bytes())),
            range,
            format: LineFormat::Claude,
            call_id: "c1".to_owned(),
        },
    );
    let mut turn = with_id(
        blocks::assistant(
            None,
            vec![blocks::tool(
                "c1",
                "Print the log",
                serde_json::json!({"command": "cat /var/log/build.log"}),
                None,
                crate::model::ToolStatus::Ok,
            )],
            None,
        ),
        id,
    );
    let Turn::Assistant(assistant) = &mut turn else {
        unreachable!()
    };
    let Item::ToolCall(call) = &mut assistant.items[0] else {
        unreachable!()
    };
    call.result = Some(ToolResult {
        output,
        is_error: false,
    });
    turn
}
