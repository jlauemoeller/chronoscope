//! Snapshots of every UI state of the transcript (task 018).

use ratatui::layout::Size;
use tuirealm::command::{Cmd, Direction};
use tuirealm::component::Component;

use super::super::fixtures::*;
use super::super::*;
use crate::test_support::{buffer_to_styled, buffer_to_text};

fn text_at(t: &mut Transcript, width: u16, height: u16) -> String {
    buffer_to_text(&draw(t, width, height))
}

#[test]
fn top_of_the_transcript_at_80x24_text() {
    let mut t = session_transcript(80, 24);
    insta::assert_snapshot!(text_at(&mut t, 80, 24));
}

#[test]
fn top_of_the_transcript_at_120x40_text() {
    let mut t = session_transcript(120, 40);
    insta::assert_snapshot!(text_at(&mut t, 120, 40));
}

#[test]
fn top_of_the_transcript_at_120x40_styled() {
    let mut t = session_transcript(120, 40);
    insta::assert_snapshot!(buffer_to_styled(&draw(&mut t, 120, 40)));
}

#[test]
fn scrolled_to_the_middle_with_both_edges_clipped_at_80x24_text() {
    let mut t = session_transcript(80, 24);
    // Block 2 starts at row 22 or so; land inside the second block and end
    // inside a later one.
    let top = t.measured(|cache, m| cache.top(1, m));
    t.scroll_to(top + 5);
    let text = text_at(&mut t, 80, 24);
    let rows: Vec<&str> = text.lines().collect();
    assert!(
        !rows[0].starts_with('╭'),
        "the top row is inside a block: {}",
        rows[0]
    );
    assert!(
        !rows[23].starts_with('╰') && !rows[23].starts_with('╭'),
        "the bottom row is inside a block: {}",
        rows[23]
    );
    insta::assert_snapshot!(text);
}

#[test]
fn selection_in_the_fifth_block_after_moves_at_80x24_text() {
    let mut t = session_transcript(80, 24);
    let mut steps = 0;
    while selected_turn_id(&t) != "t5" {
        t.perform(Cmd::Move(Direction::Down));
        steps += 1;
        assert!(steps < 30, "never reached the fifth block");
    }
    assert!(t.offset() > 0, "the selection scrolled into view");
    insta::assert_snapshot!(text_at(&mut t, 80, 24));
}

#[test]
fn a_prompt_unfolded_before_a_resize_at_120x24_text() {
    let mut t = session_transcript(120, 24);
    // Unfold the selected prompt so that its wrapping shows.
    t.perform(Cmd::Submit);
    insta::assert_snapshot!(text_at(&mut t, 120, 24));
}

#[test]
fn after_a_resize_from_120_to_80_columns_text_is_wrapped_again_at_80x24_text() {
    let mut t = session_transcript(120, 24);
    // Unfold the selected prompt so that its wrapping shows.
    t.perform(Cmd::Submit);
    let wide = text_at(&mut t, 120, 24);
    t.set_viewport(Size::new(80, 24));
    let narrow = text_at(&mut t, 80, 24);
    let marked = |text: &str| text.lines().filter(|row| row.starts_with('▒')).count();
    assert!(
        marked(&narrow) > marked(&wide),
        "the prompt wraps onto more rows"
    );
    let mut fresh = session_transcript(80, 24);
    fresh.perform(Cmd::Submit);
    assert_eq!(narrow, text_at(&mut fresh, 80, 24), "as if opened at 80");
    insta::assert_snapshot!(narrow);
}

#[test]
fn dim_unselected_false_at_120x40_styled() {
    let mut dimmed = session_transcript(120, 40);
    let mut t = session_transcript(120, 40).dim_unselected(false);
    let styled = buffer_to_styled(&draw(&mut t, 120, 40));
    assert_ne!(styled, buffer_to_styled(&draw(&mut dimmed, 120, 40)));
    insta::assert_snapshot!(styled);
}

fn selected_turn_id(t: &Transcript) -> String {
    t.selected().unwrap().turn.to_string()
}

// ---- Task 019: truncated tool output ----

use crate::components::turn_block::SectionKey;
use crate::model::ItemId;

fn select_more_row(t: &mut Transcript) {
    let want = SectionId {
        turn: crate::model::TurnId::from("a1"),
        key: SectionKey::More(ItemId::from("c1")),
    };
    for _ in 0..100 {
        if t.selected() == Some(&want) {
            return;
        }
        t.perform(Cmd::Move(Direction::Down));
    }
    panic!("the more row was never selected");
}

#[test]
fn truncated_result_with_the_more_row_selected_at_80x24_text() {
    let mut t = long_output_transcript(80, 24);
    select_more_row(&mut t);
    let text = text_at(&mut t, 80, 24);
    assert_eq!(text.lines().filter(|r| r.starts_with('▒')).count(), 1);
    insta::assert_snapshot!(text);
}

#[test]
fn truncated_result_with_the_more_row_selected_at_80x24_styled() {
    let mut t = long_output_transcript(80, 24);
    select_more_row(&mut t);
    insta::assert_snapshot!(buffer_to_styled(&draw(&mut t, 80, 24)));
}

#[test]
fn expanded_result_after_submitting_the_more_row_at_80x24_text() {
    let mut t = long_output_transcript(80, 24);
    select_more_row(&mut t);
    t.perform(Cmd::Submit);
    let text = text_at(&mut t, 80, 24);
    assert!(!text.contains("more lines"));
    insta::assert_snapshot!(text);
}
