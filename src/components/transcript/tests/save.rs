//! `w`, `W`, `e` and `E` (task 026): the transcript answers
//! [`Msg::SaveSection`] and [`Msg::Export`].

use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Key, KeyModifiers};

use super::super::fixtures::*;
use super::super::*;
use super::keys::{ch, ctrl, key};
use super::{down, item_section};
use crate::components::turn_block::SectionKey;
use crate::components::turn_block::export::ExportMeta;
use crate::components::turn_block::fixtures as blocks;
use crate::model::ItemId;

fn move_to(t: &mut Transcript, want: &SectionId) {
    for _ in 0..200 {
        if t.selected() == Some(want) {
            return;
        }
        t.perform(down());
    }
    panic!("never selected {want:?}, stuck at {:?}", t.selected());
}

fn saved(msg: Option<Msg>) -> (String, &'static str) {
    match msg {
        Some(Msg::SaveSection { text, kind }) => (text, kind),
        other => panic!("expected SaveSection, got {other:?}"),
    }
}

fn exported(msg: Option<Msg>) -> (bool, String) {
    match msg {
        Some(Msg::Export { full, document }) => (full, document),
        other => panic!("expected Export, got {other:?}"),
    }
}

#[test]
fn w_saves_the_selected_section_as_displayed_with_its_kind() {
    let mut t = session_transcript(80, 24);
    move_to(&mut t, &item_section("t2", "c1"));
    let (text, kind) = saved(ch(&mut t, 'w'));
    assert_eq!(text, "Find the BRIO on USB and check available tools");
    assert_eq!(kind, "shell");
    assert_eq!(Some(text), t.displayed_text());
}

#[test]
fn ctrl_s_is_save_too() {
    let mut t = session_transcript(80, 24);
    assert_eq!(ctrl(&mut t, 's'), ch(&mut t, 'w'));
}

#[test]
fn capital_w_saves_the_full_content() {
    let mut t = session_transcript(80, 24);
    move_to(&mut t, &item_section("t2", "c1"));
    let (text, kind) = saved(t.on(&key(Key::Char('W'), KeyModifiers::SHIFT)));
    assert!(text.contains("Result:"), "{text}");
    assert_eq!(kind, "shell");
    assert_eq!(Some(text), t.full_text());
}

#[test]
fn the_kind_is_that_of_the_selected_section() {
    let mut t = session_transcript(80, 24);
    // The first section of the fixture is the prompt.
    assert_eq!(saved(ch(&mut t, 'w')).1, "user");
    let mut kinds = Vec::new();
    for _ in 0..40 {
        kinds.push(saved(ch(&mut t, 'w')).1);
        t.perform(down());
    }
    for kind in [
        "user",
        "assistant",
        "thinking",
        "shell",
        "stats",
        "compaction",
        "system",
    ] {
        assert!(kinds.contains(&kind), "{kind} not in {kinds:?}");
    }
}

#[test]
fn on_the_more_row_the_kind_is_that_of_its_tool_call() {
    let mut t = long_output_transcript(80, 24);
    move_to(
        &mut t,
        &super::section_id("a1", SectionKey::More(ItemId::from("c1"))),
    );
    let (text, kind) = saved(ch(&mut t, 'w'));
    assert_eq!(kind, "shell");
    assert!(text.ends_with("… 1 234 more lines"), "{text}");
}

#[test]
fn without_a_selection_the_save_keys_do_nothing_but_the_export_keys_still_answer() {
    let mut t = Transcript::new(blocks::theme(), blocks::formatter());
    assert_eq!(ch(&mut t, 'w'), None);
    assert_eq!(ch(&mut t, 'W'), None);
    assert_eq!(t.file_kind(), None);
    let (full, document) = exported(ch(&mut t, 'e'));
    assert!(!full);
    assert_eq!(document, "\n");
}

#[test]
fn e_and_ctrl_e_export_as_folded_and_capital_e_in_full() {
    let mut t = session_transcript(80, 24);
    let (full, folded) = exported(ch(&mut t, 'e'));
    assert!(!full);
    assert_eq!(exported(ctrl(&mut t, 'e')), (false, folded.clone()));
    assert_eq!(folded, t.export_document(false));
    assert!(!folded.contains("<details>"));
    let (full, everything) = exported(t.on(&key(Key::Char('E'), KeyModifiers::SHIFT)));
    assert!(full);
    assert_eq!(everything, t.export_document(true));
    assert!(everything.contains("<details>"));
}

#[test]
fn the_export_follows_the_folds_and_the_system_toggle() {
    let mut t = session_transcript(80, 24);
    // The injected item of t6 is shown by default.
    let (_, shown) = exported(ch(&mut t, 'e'));
    assert!(shown.contains("> file: /home/jacob/notes.md"), "{shown}");
    assert!(!shown.contains("Use the vendor GUIDs."));
    ch(&mut t, 's');
    let (_, hidden) = exported(ch(&mut t, 'e'));
    assert!(!hidden.contains("notes.md"), "{hidden}");
    // The full export has it anyway.
    let (_, full) = exported(t.on(&key(Key::Char('E'), KeyModifiers::SHIFT)));
    assert!(full.contains("Use the vendor GUIDs."), "{full}");
    // Unfolding a tool call shows its body in the folded export.
    move_to(&mut t, &item_section("t2", "c1"));
    let (_, before) = exported(ch(&mut t, 'e'));
    assert!(!before.contains("**Command:**"));
    ch(&mut t, 'l');
    let (_, after) = exported(ch(&mut t, 'e'));
    assert!(after.contains("**Command:**\n\n```sh\nlsusb | grep -i brio\n```"));
}

#[test]
fn the_document_starts_with_the_metadata_it_was_given() {
    let meta = ExportMeta {
        title: "A session".to_owned(),
        project: "brio".to_owned(),
        session_id: "abc".to_owned(),
        cwd: "/work".to_owned(),
        started: blocks::ts("2026-10-21T08:35:15Z"),
        harness: "Claude".to_owned(),
    };
    let t = session_transcript(80, 24).export_meta(meta);
    assert!(t.export_document(false).starts_with(
        "# A session\n\n- Project: brio\n- Session: abc\n- Working directory: /work\n- Started: 2026-10-21 08:35:15Z\n- Harness: Claude\n\n## User — "
    ));
}

#[test]
fn saving_and_exporting_do_not_change_the_selection_or_the_view() {
    let mut t = session_transcript(80, 24);
    move_to(&mut t, &item_section("t2", "c1"));
    let (selected, offset) = (t.selected().cloned(), t.offset());
    ch(&mut t, 'w');
    ch(&mut t, 'W');
    ch(&mut t, 'e');
    ch(&mut t, 'E');
    assert_eq!(t.selected().cloned(), selected);
    assert_eq!(t.offset(), offset);
}

#[test]
fn the_folded_export_has_the_same_summary_as_y_copies() {
    let mut t = session_transcript(80, 24);
    let copied = match ch(&mut t, 'y') {
        Some(Msg::CopyText(text)) => text,
        other => panic!("{other:?}"),
    };
    // The folded first prompt: two rows of the screen, ending in `…`.
    assert!(copied.ends_with('…'), "{copied}");
    let (_, document) = exported(ch(&mut t, 'e'));
    assert!(document.contains(&format!("\n> {copied}\n")), "{document}");
    assert!(!document.contains(blocks::LONG_PROMPT));
    // The full export has all of it.
    let (_, full) = exported(t.on(&key(Key::Char('E'), KeyModifiers::SHIFT)));
    assert!(full.contains(blocks::LONG_PROMPT));
}
