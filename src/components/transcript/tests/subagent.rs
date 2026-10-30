//! `a` and the back key (task 027): the transcript answers
//! [`Msg::OpenSubagent`] on a subagent section and [`Msg::CloseSubagent`]
//! when it is nested.

use serde_json::json;
use tuirealm::event::Key;

use super::super::fixtures::*;
use super::super::*;
use super::keys::{ch, press};
use super::{down, item_section, section_id};
use crate::components::turn_block::SectionKey;
use crate::components::turn_block::fixtures as blocks;
use crate::model::{ItemId, ToolKind, ToolStatus};

/// A user turn and an assistant turn with a text, a shell call and two
/// subagent calls: `s1` (an Explore agent) and `s2` (no agent type, so its
/// description names it).
fn with_subagents() -> Transcript {
    let subagent = |id: &str, description: &str, input: serde_json::Value| {
        blocks::tool_of(
            ToolKind::Subagent,
            id,
            description,
            input,
            Some(("Found it.", false)),
            ToolStatus::Ok,
        )
    };
    let items = vec![
        blocks::text("x1", "I will ask two agents."),
        blocks::tool(
            "sh",
            "List the files",
            json!({"command": "ls"}),
            Some(("a b", false)),
            ToolStatus::Ok,
        ),
        subagent(
            "s1",
            "Explore the repo",
            json!({"subagent_type": "Explore", "prompt": "Look around"}),
        ),
        subagent("s2", "Write the story", json!({"prompt": "Write a story"})),
    ];
    transcript_of(
        vec![
            arc(prompt("u1", "Do the thing")),
            arc(with_id(blocks::assistant(None, items, None), "a1")),
        ],
        100,
        30,
    )
}

fn move_to(t: &mut Transcript, want: &SectionId) {
    for _ in 0..50 {
        if t.selected() == Some(want) {
            return;
        }
        t.perform(down());
    }
    panic!("never selected {want:?}, stuck at {:?}", t.selected());
}

#[test]
fn a_on_a_subagent_section_asks_to_open_its_transcript() {
    let mut t = with_subagents();
    move_to(&mut t, &item_section("a1", "s1"));
    assert_eq!(
        ch(&mut t, 'a'),
        Some(Msg::OpenSubagent {
            call_id: ItemId::from("s1"),
            label: "Explore".to_owned(),
        })
    );
}

#[test]
fn the_label_is_the_description_without_an_agent_type() {
    let mut t = with_subagents();
    move_to(&mut t, &item_section("a1", "s2"));
    assert_eq!(
        ch(&mut t, 'a'),
        Some(Msg::OpenSubagent {
            call_id: ItemId::from("s2"),
            label: "Write the story".to_owned(),
        })
    );
}

#[test]
fn a_works_whether_the_section_is_folded_or_not() {
    let mut t = with_subagents();
    move_to(&mut t, &item_section("a1", "s1"));
    ch(&mut t, 'l');
    assert!(matches!(ch(&mut t, 'a'), Some(Msg::OpenSubagent { .. })));
    ch(&mut t, 'h');
    assert!(matches!(ch(&mut t, 'a'), Some(Msg::OpenSubagent { .. })));
}

#[test]
fn a_on_the_more_row_of_a_subagent_opens_it_too() {
    let mut t = with_subagents();
    let lines: String = (1..=300).map(|n| format!("line {n}\n")).collect();
    let long = blocks::tool_of(
        ToolKind::Subagent,
        "s9",
        "Long",
        json!({"subagent_type": "Plan", "prompt": "p"}),
        Some((&lines, false)),
        ToolStatus::Ok,
    );
    t.apply_update(&ModelUpdate::Appended(vec![arc(with_id(
        blocks::assistant(None, vec![long], None),
        "a2",
    ))]));
    assert!(t.select(&item_section("a2", "s9")));
    ch(&mut t, 'l');
    assert!(t.select(&section_id("a2", SectionKey::More(ItemId::from("s9")))));
    assert_eq!(
        ch(&mut t, 'a'),
        Some(Msg::OpenSubagent {
            call_id: ItemId::from("s9"),
            label: "Plan".to_owned(),
        })
    );
}

#[test]
fn a_on_any_other_section_returns_nothing() {
    let mut t = with_subagents();
    for id in [
        section_id("u1", SectionKey::Prompt),
        item_section("a1", "x1"),
        item_section("a1", "sh"),
    ] {
        assert!(t.select(&id));
        assert_eq!(ch(&mut t, 'a'), None, "{id:?}");
    }
}

#[test]
fn a_with_nothing_selected_returns_nothing() {
    let mut t = Transcript::new(
        crate::components::turn_block::fixtures::theme(),
        crate::components::turn_block::fixtures::formatter(),
    );
    assert_eq!(ch(&mut t, 'a'), None);
}

#[test]
fn a_does_not_change_the_transcript() {
    let mut t = with_subagents();
    move_to(&mut t, &item_section("a1", "s1"));
    let before = t.presentation();
    ch(&mut t, 'a');
    assert_eq!(t.presentation(), before);
}

#[test]
fn backspace_closes_a_nested_transcript() {
    let mut t = with_subagents().nested(true);
    assert_eq!(press(&mut t, Key::Backspace), Some(Msg::CloseSubagent));
}

#[test]
fn backspace_in_the_session_itself_returns_nothing() {
    let mut t = with_subagents();
    assert_eq!(press(&mut t, Key::Backspace), None);
}
