//! The details key (product §4.1): the transcript answers [`Msg::OpenDetails`]
//! with the rows of the selected section, a `… N more lines` row as its tool
//! call, and nothing with no selection.

use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Key, KeyModifiers};

use super::super::fixtures::*;
use super::super::*;
use super::keys::{ch, key};
use super::{down, item_section, section_id};
use crate::components::DetailsContent;
use crate::components::turn_block::SectionKey;
use crate::model::ItemId;

fn opened(msg: Option<Msg>) -> DetailsContent {
    match msg {
        Some(Msg::OpenDetails(request)) => request.content,
        other => panic!("expected OpenDetails, got {other:?}"),
    }
}

fn titles(content: &DetailsContent) -> Vec<&str> {
    content.groups().iter().map(|g| g.title.as_str()).collect()
}

fn move_to(t: &mut Transcript, want: &SectionId) {
    for _ in 0..200 {
        if t.selected() == Some(want) {
            return;
        }
        t.perform(down());
    }
    panic!("never selected {want:?}, stuck at {:?}", t.selected());
}

#[test]
fn the_details_key_answers_the_rows_of_the_selected_section() {
    let mut t = session_transcript(80, 24);
    move_to(&mut t, &item_section("t2", "c1"));
    let content = opened(ch(&mut t, 'i'));
    assert_eq!(titles(&content), ["Turn", "Tool call"]);
    // The turn is the one that holds the call, with its stats.
    let turn = &content.groups()[0];
    assert_eq!(turn.rows[0].label, "model");
    assert!(turn.rows.iter().any(|r| r.label == "duration"));
}

#[test]
fn the_groups_follow_the_kind_of_the_section() {
    let mut t = session_transcript(80, 24);
    let expected = [
        (section_id("t1", SectionKey::Prompt), vec!["Prompt"]),
        (item_section("t2", "t1"), vec!["Turn"]),
        (item_section("t2", "th1"), vec!["Turn"]),
        (item_section("t2", "m1"), vec!["Turn"]),
        (section_id("t2", SectionKey::Footer), vec!["Turn"]),
        (section_id("t5", SectionKey::Summary), vec!["Environment"]),
        (item_section("t6", "i1"), vec!["Environment"]),
    ];
    for (id, groups) in expected {
        move_to(&mut t, &id);
        assert_eq!(titles(&opened(ch(&mut t, 'i'))), groups, "{id:?}");
    }
}

#[test]
fn a_more_row_answers_the_rows_of_its_tool_call() {
    let mut t = long_output_transcript(80, 24);
    let call = item_section("a1", "c1");
    move_to(&mut t, &call);
    let of_call = opened(ch(&mut t, 'i'));
    move_to(
        &mut t,
        &section_id("a1", SectionKey::More(ItemId::from("c1"))),
    );
    let of_more = opened(ch(&mut t, 'i'));
    assert_eq!(of_more, of_call);
    assert_eq!(titles(&of_more), ["Turn", "Tool call"]);
}

#[test]
fn with_nothing_selected_the_key_does_nothing() {
    let mut t = transcript_of(Vec::new(), 80, 24);
    assert_eq!(t.selected(), None);
    assert_eq!(ch(&mut t, 'i'), None);
}

#[test]
fn the_key_changes_nothing_in_the_transcript() {
    let mut t = session_transcript(80, 24);
    move_to(&mut t, &item_section("t2", "c1"));
    let before = (t.selected().cloned(), t.state());
    opened(ch(&mut t, 'i'));
    assert_eq!((t.selected().cloned(), t.state()), before);
}

#[test]
fn the_content_is_not_part_of_copy_save_or_export() {
    let mut t = session_transcript(80, 24);
    move_to(&mut t, &item_section("t2", "c1"));
    let labels = [
        "input tokens",
        "output tokens",
        "thinking tokens",
        "cache read tokens",
    ];
    let texts = [
        t.displayed_text().unwrap(),
        t.full_text().unwrap(),
        t.export_document(true),
        t.export_document(false),
    ];
    for text in texts {
        for label in labels {
            assert!(!text.contains(label), "{label:?} in {text}");
        }
    }
}

#[test]
fn a_remapped_key_answers_and_the_old_one_does_not() {
    let mut raw = crate::config::RawKeys::new();
    raw.entry("browser".to_owned())
        .or_default()
        .insert("details".to_owned(), vec!["x".to_owned()]);
    let (keymap, warnings) = crate::keymap::Keymap::from_config(&raw);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut t = session_transcript(80, 24).keymap(keymap);
    assert_eq!(ch(&mut t, 'i'), None);
    assert!(matches!(ch(&mut t, 'x'), Some(Msg::OpenDetails(_))));
    // Other keys with modifiers are not the details key.
    assert_eq!(t.on(&key(Key::Char('x'), KeyModifiers::CONTROL)), None);
}
