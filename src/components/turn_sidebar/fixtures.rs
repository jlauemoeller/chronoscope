//! Turns and sidebars for the tests. The turns and rows of
//! [`turn_rows::fixtures`](crate::components::turn_rows::fixtures) come
//! with them.

use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuirealm::component::Component;

use super::*;
use crate::app::layout::browser_areas;
use crate::components::transcript::fixtures::with_id;
use crate::components::turn_block::fixtures as blocks;
pub use crate::components::turn_rows::fixtures::*;
use crate::model::{ByteSource, Item, LineFormat, OutputSource, ToolOutput, ToolResult, Turn};
use crate::test_support::render;

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
