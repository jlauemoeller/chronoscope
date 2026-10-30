//! `y` and `Y` (task 025): the transcript answers [`Msg::CopyText`] with the
//! text of the selected section.

use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Key, KeyModifiers};

use super::super::fixtures::*;
use super::super::*;
use super::keys::{ch, ctrl, key};
use super::{down, item_section, section_id};
use crate::components::turn_block::SectionKey;
use crate::components::turn_block::fixtures as blocks;
use crate::model::ItemId;

fn copied(msg: Option<Msg>) -> String {
    match msg {
        Some(Msg::CopyText(text)) => text,
        other => panic!("expected CopyText, got {other:?}"),
    }
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

/// The long-output fixture with the `… N more lines` row selected.
fn on_the_more_row() -> Transcript {
    let mut t = long_output_transcript(80, 24);
    move_to(
        &mut t,
        &section_id("a1", SectionKey::More(ItemId::from("c1"))),
    );
    t
}

fn lines_of(text: &str) -> Vec<&str> {
    text.lines().collect()
}

#[test]
fn y_copies_the_selected_section_as_displayed() {
    let mut t = session_transcript(80, 24);
    move_to(&mut t, &item_section("t2", "c1"));
    let text = copied(ch(&mut t, 'y'));
    // Folded: the description only.
    assert_eq!(text, "Find the BRIO on USB and check available tools");
    assert_eq!(Some(text), t.displayed_text());
}

#[test]
fn y_copies_everything_shown_when_the_section_is_unfolded() {
    let mut t = session_transcript(80, 24);
    move_to(&mut t, &item_section("t2", "c1"));
    ch(&mut t, 'l');
    assert_eq!(
        copied(ch(&mut t, 'y')),
        "Find the BRIO on USB and check available tools\n\nCommand:\n\nlsusb | grep -i brio\n\nResult:\n\nBus 001 Device 004: ID 046d:085e Logitech, Inc. BRIO"
    );
}

#[test]
fn capital_y_copies_the_full_section_whatever_its_fold_state() {
    let mut t = session_transcript(80, 24);
    move_to(&mut t, &item_section("t2", "c1"));
    let folded = copied(ch(&mut t, 'Y'));
    ch(&mut t, 'l');
    let unfolded = copied(ch(&mut t, 'Y'));
    assert_eq!(folded, unfolded);
    assert_eq!(
        folded,
        "Find the BRIO on USB and check available tools\n\nCommand:\n\nlsusb | grep -i brio\n\nResult:\n\nBus 001 Device 004: ID 046d:085e Logitech, Inc. BRIO"
    );
}

#[test]
fn the_shift_modifier_form_of_y_copies_in_full_too() {
    let mut t = session_transcript(80, 24);
    move_to(&mut t, &item_section("t2", "c1"));
    let text = copied(t.on(&key(Key::Char('Y'), KeyModifiers::SHIFT)));
    assert!(text.contains("Result:"), "{text}");
}

#[test]
fn on_the_more_row_y_copies_the_tool_call_as_displayed_with_the_more_row() {
    let mut t = on_the_more_row();
    let text = copied(ch(&mut t, 'y'));
    let lines = lines_of(&text);
    assert_eq!(lines[0], "Print the log");
    assert_eq!(lines[lines.len() - 1], "… 1 234 more lines");
    assert_eq!(lines[lines.len() - 2], "line 200");
    // Description, blank, label, blank, command, blank, label, blank, then
    // the 200 lines of the result and the row.
    assert_eq!(lines.len(), 8 + 200 + 1);
}

#[test]
fn on_the_more_row_capital_y_copies_the_full_tool_call() {
    let mut t = on_the_more_row();
    let text = copied(ch(&mut t, 'Y'));
    let lines = lines_of(&text);
    assert_eq!(lines[0], "Print the log");
    assert_eq!(lines[lines.len() - 1], "line 1434");
    assert!(!text.contains("more lines"));
    assert_eq!(lines.len(), 8 + 1434);
}

#[test]
fn copying_does_not_change_the_selection_or_the_view() {
    let mut t = on_the_more_row();
    let (selected, offset) = (t.selected().cloned(), t.offset());
    ch(&mut t, 'y');
    ch(&mut t, 'Y');
    assert_eq!(t.selected().cloned(), selected);
    assert_eq!(t.offset(), offset);
}

#[test]
fn without_a_selection_the_copy_keys_do_nothing() {
    let mut t = Transcript::new(blocks::theme(), blocks::formatter());
    assert_eq!(ch(&mut t, 'y'), None);
    assert_eq!(ch(&mut t, 'Y'), None);
    assert_eq!(t.displayed_text(), None);
    assert_eq!(t.full_text(), None);
}

#[test]
fn ctrl_c_is_never_a_copy_key() {
    let mut t = session_transcript(80, 24);
    assert_eq!(ctrl(&mut t, 'c'), None);
}

#[test]
fn the_copy_text_follows_a_remapped_key() {
    let mut keymap_config = crate::config::Config::default();
    keymap_config
        .keys
        .entry("browser".to_owned())
        .or_default()
        .insert("copy".to_owned(), vec!["c".to_owned()]);
    let (keymap, warnings) = crate::keymap::Keymap::from_config(&keymap_config.keys);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut t = session_transcript(80, 24).keymap(keymap);
    assert_eq!(ch(&mut t, 'y'), None, "y is unbound now");
    assert_eq!(copied(ch(&mut t, 'c')), t.displayed_text().unwrap());
}

#[test]
fn y_on_a_folded_prompt_copies_the_rows_shown_at_the_transcripts_width() {
    let mut t = session_transcript(80, 24);
    assert_eq!(
        copied(ch(&mut t, 'y')),
        "I have a Logitech BRIO web camera attached to this machine via USB. It works fine - both camera and microphone - but I don't have a way to…"
    );
    // A narrower block shows less of it.
    t.set_viewport(ratatui::layout::Size::new(50, 24));
    let narrow = copied(ch(&mut t, 'y'));
    assert!(narrow.ends_with('…') && narrow.len() < 120, "{narrow}");
    // `Y` is all of it either way.
    assert_eq!(copied(ch(&mut t, 'Y')), blocks::LONG_PROMPT);
}

#[test]
fn r_asks_to_resume_the_open_session_whatever_is_selected() {
    let mut t = session_transcript(80, 24);
    assert_eq!(ch(&mut t, 'R'), Some(Msg::RequestResume(None)));
    assert_eq!(
        t.on(&key(Key::Char('R'), KeyModifiers::SHIFT)),
        Some(Msg::RequestResume(None))
    );
    move_to(&mut t, &item_section("t2", "c1"));
    assert_eq!(ch(&mut t, 'R'), Some(Msg::RequestResume(None)));
    // Without a selection too: the target is the session, not a section.
    let mut empty = Transcript::new(blocks::theme(), blocks::formatter());
    assert_eq!(ch(&mut empty, 'R'), Some(Msg::RequestResume(None)));
    // A lower-case `r` is not the resume key.
    assert_eq!(ch(&mut t, 'r'), None);
}
