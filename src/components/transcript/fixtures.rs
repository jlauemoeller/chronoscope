//! A fixture session and drawing helpers for the transcript tests.

use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use serde_json::json;
use tuirealm::component::Component;

use super::*;
use crate::components::turn_block::fixtures as blocks;
use crate::model::{Thinking, ToolStatus, Turn, TurnId};
use crate::test_support::render;

/// `turn` with the id `id`.
pub fn with_id(mut turn: Turn, id: &str) -> Turn {
    let id = TurnId::from(id);
    match &mut turn {
        Turn::User(t) => t.id = id,
        Turn::Assistant(t) => t.id = id,
        Turn::Compaction(t) => t.id = id,
    }
    turn
}

pub fn arc(turn: Turn) -> Arc<Turn> {
    Arc::new(turn)
}

/// A short user turn.
pub fn prompt(id: &str, text: &str) -> Turn {
    with_id(blocks::user(text, vec![]), id)
}

/// An assistant turn with a text item.
pub fn reply(id: &str, text: &str) -> Turn {
    with_id(
        blocks::assistant(None, vec![blocks::text("t1", text)], None),
        id,
    )
}

/// A session of eight turns: user, assistant with tools, user, assistant
/// with thinking and a failing tool, compaction, user with a system item,
/// and two assistant replies.
pub fn session() -> Vec<Arc<Turn>> {
    let failing = blocks::tool(
        "c1",
        "Read the UVC descriptor dump",
        json!({"command": "cat /tmp/lsusb.txt"}),
        Some(("cat: /tmp/lsusb.txt: No such file or directory", true)),
        ToolStatus::Error,
    );
    vec![
        arc(with_id(blocks::user(blocks::LONG_PROMPT, vec![]), "t1")),
        arc(with_id(blocks::full_assistant(), "t2")),
        arc(prompt(
            "t3",
            "Dump the full descriptors and look for extension units.",
        )),
        arc(with_id(
            blocks::assistant(
                Some("high"),
                vec![
                    blocks::text("x1", "Reading the dump now."),
                    blocks::thinking(
                        "x2",
                        Thinking::Text("Look for the Logitech XU GUIDs.".into()),
                    ),
                    failing,
                ],
                Some(blocks::stats()),
            ),
            "t4",
        )),
        arc(with_id(
            blocks::compaction(
                "The user is investigating a Logitech BRIO camera.\n\n- UVC descriptors were dumped\n- Extension units are next",
            ),
            "t5",
        )),
        arc(with_id(
            blocks::user(
                "Continue with the extension units.",
                vec![blocks::injected(
                    "i1",
                    &[("file", "/home/jacob/notes.md\nUse the vendor GUIDs.")],
                )],
            ),
            "t6",
        )),
        arc(reply(
            "t7",
            "The extension units are listed under `VideoControl`:\n\n- one for exposure\n- one for pan and tilt",
        )),
        arc(reply(
            "t8",
            "That is everything the camera exposes over plain UVC. Anything beyond that needs the vendor protocol, which is not documented.",
        )),
    ]
}

/// The turns of `session()` as an `Appended` update.
pub fn appended(turns: Vec<Arc<Turn>>) -> ModelUpdate {
    ModelUpdate::Appended(turns)
}

/// A transcript holding `turns`, prepared for a `width` x `height` viewport.
pub fn transcript_of(turns: Vec<Arc<Turn>>, width: u16, height: u16) -> Transcript {
    Transcript::new(blocks::theme(), blocks::formatter())
        .with_turns(turns)
        .viewport(Size::new(width, height))
}

/// The fixture session in a `width` x `height` viewport.
pub fn session_transcript(width: u16, height: u16) -> Transcript {
    transcript_of(session(), width, height)
}

/// Draws the transcript into a fresh `width` x `height` buffer.
pub fn draw(transcript: &mut Transcript, width: u16, height: u16) -> Buffer {
    render(width, height, |frame| {
        transcript.view(frame, Rect::new(0, 0, width, height));
    })
}

/// The standalone rows of block `index`, as the `TurnBlock` widget alone
/// draws them at `width` with the transcript's current state.
pub fn block_rows(transcript: &Transcript, index: usize, width: u16) -> Vec<String> {
    let content = &transcript.content;
    let block = content.block(index);
    let height = u16::try_from(block.height(width)).unwrap();
    let buf = render(width, height, |frame| {
        ratatui::widgets::Widget::render(&block, frame.area(), frame.buffer_mut());
    });
    blocks::rows(&buf)
}

pub fn text_rows(buf: &Buffer) -> Vec<String> {
    blocks::rows(buf)
}

/// The text `line 1\nline 2\n...\nline {n}`.
pub fn numbered_lines(n: usize) -> String {
    (1..=n)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// An assistant turn `a1` whose Shell call `c1` has a result of `lines` lines.
pub fn long_output_turn(lines: usize) -> Arc<Turn> {
    let text = numbered_lines(lines);
    arc(with_id(
        blocks::assistant(
            Some("high"),
            vec![blocks::tool(
                "c1",
                "Print the log",
                json!({"command": "cat /var/log/build.log"}),
                Some((&text, false)),
                ToolStatus::Ok,
            )],
            None,
        ),
        "a1",
    ))
}

/// A transcript of `long_output_turn(1434)` with its tool call unfolded.
pub fn long_output_transcript(width: u16, height: u16) -> Transcript {
    let mut t = transcript_of(vec![long_output_turn(1434)], width, height);
    t.set_folded(
        &crate::components::turn_block::SectionId {
            turn: TurnId::from("a1"),
            key: crate::components::turn_block::SectionKey::Item(crate::model::ItemId::from("c1")),
        },
        false,
    );
    t
}

/// A session whose turns hold `usb` five times (smart case, so `USB` counts):
/// once in the text of assistant turn `a1`, once in the result of its Shell
/// call `c1` (folded by default), once in the prompt `u2` and twice in the
/// text of `a2`. Nothing else holds it.
pub fn search_session() -> Vec<Arc<Turn>> {
    let call = blocks::tool(
        "c1",
        "List the devices",
        json!({"command": "ls /dev/video*"}),
        Some(("/dev/video0\n/dev/video1 comes from the usb driver", false)),
        ToolStatus::Ok,
    );
    vec![
        arc(prompt("u1", "Why is my camera not listed?")),
        arc(with_id(
            blocks::assistant(
                Some("high"),
                vec![blocks::text("x1", "It does show up over USB."), call],
                None,
            ),
            "a1",
        )),
        arc(prompt("u2", "Is that a USB 3 device?")),
        arc(reply("a2", "Yes, USB 3 over a usb-c cable.")),
    ]
}
