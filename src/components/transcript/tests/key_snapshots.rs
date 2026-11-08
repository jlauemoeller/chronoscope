//! Snapshots of the transcript in states produced by keys (task 020).

use super::super::fixtures::*;
use super::super::*;
use super::keys::{ch, ctrl};
use super::{item_section, section_id};
use crate::components::turn_block::SectionKey;
use crate::components::turn_block::fixtures as blocks;
use crate::test_support::buffer_to_text;

fn text_at(t: &mut Transcript, width: u16, height: u16) -> String {
    buffer_to_text(&draw(t, width, height))
}

fn marked(text: &str) -> usize {
    text.lines().filter(|row| row.starts_with('▒')).count()
}

#[test]
fn after_j_three_times_the_selection_is_in_the_second_block_at_80x24_text() {
    let mut t = session_transcript(80, 24);
    for _ in 0..3 {
        ch(&mut t, 'j');
    }
    assert_eq!(t.selected().unwrap().turn.to_string(), "t2");
    let text = text_at(&mut t, 80, 24);
    assert!(marked(&text) > 0);
    insta::assert_snapshot!(text);
}

/// A prompt, then a text section of 84 rows (3.5 pages of 24), then a reply.
fn tall_section_session() -> Transcript {
    let rows: String = (1..=84).map(|n| format!("- item {n}\n")).collect();
    let tall = arc(with_id(
        blocks::assistant(None, vec![blocks::text("big", &rows)], None),
        "a1",
    ));
    let mut t = transcript_of(
        vec![
            arc(prompt("p1", "Show me everything.")),
            tall,
            arc(reply("r1", "Done.")),
        ],
        80,
        24,
    );
    assert!(t.select(&item_section("a1", "big")));
    t
}

#[test]
fn paging_through_a_tall_section_after_the_first_j_at_80x24_text() {
    let mut t = tall_section_session();
    let top = t.offset();
    ch(&mut t, 'j');
    assert_eq!(t.offset(), top + 24, "scrolled by a page");
    assert_eq!(t.selected(), Some(&item_section("a1", "big")));
    let text = text_at(&mut t, 80, 24);
    assert!(text.contains("item 25"), "{text}");
    insta::assert_snapshot!(text);
}

#[test]
fn paging_through_a_tall_section_after_the_second_j_at_80x24_text() {
    let mut t = tall_section_session();
    let top = t.offset();
    ch(&mut t, 'j');
    ch(&mut t, 'j');
    assert_eq!(t.selected(), Some(&item_section("a1", "big")));
    assert_eq!(t.offset(), top + 48, "scrolled by a second page");
    let text = text_at(&mut t, 80, 24);
    assert!(
        text.contains("item 49") && text.contains("item 72"),
        "{text}"
    );
    insta::assert_snapshot!(text);
}

#[test]
fn after_the_right_bracket_twice_the_third_block_is_at_the_top_at_80x24_text() {
    let mut t = session_transcript(80, 24);
    ch(&mut t, ']');
    ch(&mut t, ']');
    assert_eq!(t.selected().unwrap().turn.to_string(), "t3");
    let text = text_at(&mut t, 80, 24);
    assert!(text.lines().next().unwrap().starts_with("╭─ User"));
    insta::assert_snapshot!(text);
}

#[test]
fn after_ctrl_d_the_selection_is_the_topmost_fully_visible_section_at_80x24_text() {
    let mut t = session_transcript(80, 24);
    ctrl(&mut t, 'd');
    let text = text_at(&mut t, 80, 24);
    assert!(marked(&text) > 0, "the selected section is on screen");
    assert_ne!(t.selected(), Some(&section_id("t1", SectionKey::Prompt)));
    insta::assert_snapshot!(text);
}

#[test]
fn after_plus_everything_is_unfolded_at_120x40_text() {
    let mut t = session_transcript(120, 40);
    ch(&mut t, '+');
    let text = text_at(&mut t, 120, 40);
    assert!(!text.contains("▶"), "no folded section is left:\n{text}");
    assert!(text.contains("▼"));
    insta::assert_snapshot!(text);
}

#[test]
fn after_minus_everything_is_folded_at_120x40_text() {
    let mut t = session_transcript(120, 40);
    ch(&mut t, '+');
    ch(&mut t, '-');
    let text = text_at(&mut t, 120, 40);
    assert!(!text.contains("▼"), "no unfolded section is left:\n{text}");
    assert!(text.contains("▶"));
    insta::assert_snapshot!(text);
}

/// The session with the system section of the sixth block selected, scrolled
/// as far towards the sixth block as the end of the transcript allows.
fn system_selected() -> Transcript {
    let mut t = session_transcript(80, 24);
    assert!(t.select(&item_section("t6", "i1")));
    let top = t.measured(|cache, m| cache.top(5, m));
    t.scroll_to(top);
    t
}

#[test]
fn with_system_sections_shown_by_default_at_80x24_text() {
    let mut t = system_selected();
    let text = text_at(&mut t, 80, 24);
    assert!(text.contains("notes.md"));
    assert!(marked(&text) > 0);
    insta::assert_snapshot!(text);
}

#[test]
fn after_s_the_system_sections_are_hidden_at_80x24_text() {
    let default = text_at(&mut system_selected(), 80, 24);
    let mut t = system_selected();
    ch(&mut t, 's');
    assert_eq!(t.selected(), Some(&section_id("t6", SectionKey::Prompt)));
    let text = text_at(&mut t, 80, 24);
    assert!(!text.contains("notes.md"));
    assert!(
        marked(&text) > 0,
        "the selection moved to a visible section"
    );
    assert_ne!(text, default);
    insta::assert_snapshot!(text);
}
