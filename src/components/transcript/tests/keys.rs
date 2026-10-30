//! Key handling and navigation (task 020): `on()` resolves keys through the
//! keymap, `perform()` does the work.

use std::path::Path;

use ratatui::layout::Size;
use tuirealm::command::{Cmd, CmdResult, Position};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

use super::*;
use crate::app::UserEvent;
use crate::config::Config;
use crate::keymap::Keymap;

pub(crate) fn key(code: Key, modifiers: KeyModifiers) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent::new(code, modifiers))
}

pub(crate) fn press(t: &mut Transcript, code: Key) -> Option<Msg> {
    t.on(&key(code, KeyModifiers::NONE))
}

pub(crate) fn ch(t: &mut Transcript, c: char) -> Option<Msg> {
    press(t, Key::Char(c))
}

pub(crate) fn ctrl(t: &mut Transcript, c: char) -> Option<Msg> {
    t.on(&key(Key::Char(c), KeyModifiers::CONTROL))
}

fn selected(t: &Transcript) -> SectionId {
    t.selected().cloned().expect("a section is selected")
}

/// The sections of block `index` as `(id, rows)`, rows counted from the top
/// of the transcript.
fn spans(t: &mut Transcript) -> Vec<(SectionId, std::ops::Range<usize>)> {
    let mut out = Vec::new();
    for block in 0..t.turn_count() {
        let top = t.measured(|cache, m| cache.top(block, m));
        let rows = t
            .content
            .section_rows(t.turn(block).unwrap(), t.nav_width());
        out.extend(
            rows.into_iter()
                .map(|(id, r)| (id, top + r.start..top + r.end)),
        );
    }
    out
}

fn span_of(t: &mut Transcript, id: &SectionId) -> std::ops::Range<usize> {
    spans(t)
        .into_iter()
        .find(|(i, _)| i == id)
        .expect("section exists")
        .1
}

/// The screen row of `id`'s first row (negative above the viewport).
fn screen_row(t: &mut Transcript, id: &SectionId) -> isize {
    span_of(t, id).start as isize - t.offset() as isize
}

fn all_selectable(t: &Transcript) -> Vec<SectionId> {
    (0..t.turn_count()).flat_map(|i| t.sections_of(i)).collect()
}

fn marked_rows(t: &mut Transcript, width: u16, height: u16) -> Vec<String> {
    text_rows(&draw(t, width, height))
        .into_iter()
        .filter(|r| r.starts_with('▒'))
        .collect()
}

// ---- Moving between sections ----

#[test]
fn j_and_down_select_the_next_section_k_and_up_the_previous() {
    let mut t = session_transcript(80, 24);
    let all = all_selectable(&t);
    ch(&mut t, 'j');
    assert_eq!(selected(&t), all[1]);
    press(&mut t, Key::Down);
    assert_eq!(selected(&t), all[2]);
    press(&mut t, Key::Down);
    assert_eq!(selected(&t), all[3]);
    ch(&mut t, 'k');
    assert_eq!(selected(&t), all[2]);
    press(&mut t, Key::Up);
    assert_eq!(selected(&t), all[1]);
}

#[test]
fn moving_crosses_blocks_and_stops_at_the_first_and_last_section() {
    let mut t = session_transcript(80, 24);
    let all = all_selectable(&t);
    ch(&mut t, 'k');
    assert_eq!(selected(&t), all[0], "no wrap at the start");
    for _ in 0..all.len() + 5 {
        ch(&mut t, 'j');
    }
    assert_eq!(&selected(&t), all.last().unwrap(), "no wrap at the end");
    assert_eq!(selected(&t).turn, TurnId::from("t8"));
    assert!(
        t.selected().is_some() && t.offset() > 0,
        "the last section was scrolled into view"
    );
}

#[test]
fn a_selection_move_scrolls_the_section_into_view() {
    let mut t = session_transcript(80, 10);
    for _ in 0..6 {
        ch(&mut t, 'j');
    }
    let id = selected(&t);
    let rows = span_of(&mut t, &id);
    assert!(rows.start >= t.offset() && rows.end <= t.offset() + 10);
    assert!(t.offset() > 0);
}

/// A transcript whose second block holds one section of 35 rows, in a
/// viewport of 10 rows: it takes three page scrolls to see its end.
fn tall_section_transcript() -> (Transcript, SectionId) {
    let tall = arc(with_id(
        blocks::assistant(None, vec![blocks::text("big", &tall_text_of(35))], None),
        "a1",
    ));
    let t = transcript_of(
        vec![
            arc(prompt("p1", "Show me everything.")),
            tall,
            arc(reply("r1", "Done.")),
        ],
        80,
        10,
    );
    (t, item_section("a1", "big"))
}

fn tall_text_of(rows: usize) -> String {
    (1..=rows).map(|n| format!("- item {n}\n")).collect()
}

#[test]
fn three_presses_page_through_a_tall_section_and_the_fourth_selects_the_next_one() {
    let (mut t, tall) = tall_section_transcript();
    assert!(t.select(&tall));
    let rows = span_of(&mut t, &tall);
    assert_eq!(rows.len(), 35, "the section is 3.5 pages tall");
    // Arrived from above: its top is the top row.
    ch(&mut t, 'k');
    ch(&mut t, 'j');
    assert_eq!(selected(&t), tall);
    assert_eq!(t.offset(), rows.start);
    for (press_no, expected) in [10, 20, 25].into_iter().enumerate() {
        ch(&mut t, 'j');
        assert_eq!(
            selected(&t),
            tall,
            "press {}: still the tall section",
            press_no + 1
        );
        assert_eq!(t.offset(), rows.start + expected, "press {}", press_no + 1);
    }
    assert_eq!(t.offset() + 10, rows.end, "the last row is visible");
    ch(&mut t, 'j');
    assert_eq!(
        selected(&t),
        item_section("r1", "t1"),
        "the fourth press moves on"
    );
}

#[test]
fn three_presses_scroll_by_whole_pages_through_a_three_page_section() {
    let thirty = arc(with_id(
        blocks::assistant(None, vec![blocks::text("big", &tall_text_of(30))], None),
        "a1",
    ));
    let mut t = transcript_of(
        vec![
            arc(prompt("p1", "Show me.")),
            thirty,
            arc(reply("r1", "Done.")),
        ],
        80,
        10,
    );
    let tall = item_section("a1", "big");
    assert_eq!(span_of(&mut t, &tall).len(), 30);
    ch(&mut t, 'j');
    assert_eq!(selected(&t), tall);
    let start = span_of(&mut t, &tall).start;
    assert_eq!(t.offset(), start, "the section's top is the top row");
    ch(&mut t, 'j');
    assert_eq!(t.offset(), start + 10, "a page");
    ch(&mut t, 'j');
    assert_eq!(t.offset(), start + 20, "another page");
    assert_eq!(selected(&t), tall);
    ch(&mut t, 'j');
    assert_eq!(selected(&t), item_section("r1", "t1"));
}

#[test]
fn paging_up_mirrors_paging_down_from_the_section_top() {
    let (mut t, tall) = tall_section_transcript();
    // Arrive from below: the bottom of the section is shown.
    assert!(t.select(&item_section("r1", "t1")));
    ch(&mut t, 'k');
    assert_eq!(selected(&t), tall);
    let rows = span_of(&mut t, &tall);
    assert_eq!(t.offset() + 10, rows.end);
    for expected in [15, 5, 0] {
        ch(&mut t, 'k');
        assert_eq!(selected(&t), tall);
        assert_eq!(t.offset(), rows.start + expected);
    }
    ch(&mut t, 'k');
    assert_eq!(selected(&t), section_id("p1", SectionKey::Prompt));
}

// ---- Turn blocks ----

#[test]
fn right_bracket_and_left_bracket_jump_between_blocks_with_the_top_border_on_top() {
    let mut t = session_transcript(80, 24);
    let tops: Vec<usize> = (0..t.turn_count())
        .map(|b| t.measured(|cache, m| cache.top(b, m)))
        .collect();
    ch(&mut t, ']');
    assert_eq!(selected(&t).turn, TurnId::from("t2"));
    assert_eq!(t.offset(), tops[1]);
    ch(&mut t, ']');
    assert_eq!(selected(&t).turn, TurnId::from("t3"));
    assert_eq!(t.offset(), tops[2]);
    let rows = text_rows(&draw(&mut t, 80, 24));
    assert!(rows[0].starts_with('╭'), "{}", rows[0]);
    ch(&mut t, '[');
    assert_eq!(selected(&t).turn, TurnId::from("t2"));
    assert_eq!(t.offset(), tops[1]);
    ch(&mut t, '[');
    ch(&mut t, '[');
    assert_eq!(
        selected(&t),
        section_id("t1", SectionKey::Prompt),
        "stops at the first block"
    );
}

#[test]
fn a_jump_selects_the_first_section_of_the_block() {
    let mut t = session_transcript(80, 24);
    ch(&mut t, ']');
    assert_eq!(selected(&t), t.sections_of(1)[0]);
    assert!(t.select(&item_section("t2", "c2")));
    ch(&mut t, ']');
    assert_eq!(selected(&t), t.sections_of(2)[0]);
}

#[test]
fn a_jump_near_the_end_scrolls_as_far_as_the_transcript_allows() {
    let mut t = session_transcript(80, 24);
    for _ in 0..7 {
        ch(&mut t, ']');
    }
    assert_eq!(selected(&t).turn, TurnId::from("t8"));
    let total = total_height(&mut t);
    assert_eq!(t.offset(), total - 24);
    ch(&mut t, ']');
    assert_eq!(
        selected(&t).turn,
        TurnId::from("t8"),
        "no block after the last"
    );
}

#[test]
fn turn_jumps_skip_blocks_that_hold_only_system_content() {
    let only_system = arc(with_id(
        blocks::user("", vec![blocks::injected("i9", &[("file", "/etc/hosts")])]),
        "sys",
    ));
    let mut t = transcript_of(
        vec![
            arc(prompt("p1", "first")),
            only_system,
            arc(reply("r1", "second")),
        ],
        80,
        24,
    );
    assert!(
        t.sections_of(1)
            .iter()
            .any(|id| id.turn == TurnId::from("sys")),
        "the system section is visible and selectable by j"
    );
    ch(&mut t, ']');
    assert_eq!(selected(&t).turn, TurnId::from("r1"));
    ch(&mut t, '[');
    assert_eq!(selected(&t).turn, TurnId::from("p1"));
}

#[test]
fn a_jump_into_a_block_that_starts_with_system_content_selects_its_prompt() {
    let mut t = session_transcript(80, 24);
    assert!(t.select(&section_id("t5", SectionKey::Summary)));
    ch(&mut t, ']');
    assert_eq!(selected(&t), section_id("t6", SectionKey::Prompt));
    let tops = t.measured(|cache, m| cache.top(5, m));
    assert_eq!(t.offset(), tops.min(total_height(&mut t) - 24));
}

// ---- Page scrolling ----

#[test]
fn ctrl_d_and_ctrl_u_scroll_half_a_page() {
    let mut t = session_transcript(80, 24);
    ctrl(&mut t, 'd');
    assert_eq!(t.offset(), 12);
    ctrl(&mut t, 'd');
    assert_eq!(t.offset(), 24);
    ctrl(&mut t, 'u');
    assert_eq!(t.offset(), 12);
    ctrl(&mut t, 'u');
    ctrl(&mut t, 'u');
    assert_eq!(t.offset(), 0, "clamped at the top");
}

#[test]
fn page_down_and_page_up_scroll_a_full_page() {
    let mut t = session_transcript(80, 24);
    press(&mut t, Key::PageDown);
    assert_eq!(t.offset(), 24);
    let end = total_height(&mut t) - 24;
    assert!(end > 24 && end < 48, "the second page is clamped: {end}");
    press(&mut t, Key::PageDown);
    assert_eq!(t.offset(), end);
    press(&mut t, Key::PageUp);
    assert_eq!(t.offset(), end - 24);
    press(&mut t, Key::PageUp);
    assert_eq!(t.offset(), 0);
}

#[test]
fn an_odd_height_rounds_a_half_page_down() {
    let mut t = session_transcript(80, 11);
    ctrl(&mut t, 'd');
    assert_eq!(t.offset(), 5);
}

#[test]
fn page_scrolling_stops_when_the_last_row_reaches_the_bottom() {
    let mut t = session_transcript(80, 24);
    let total = total_height(&mut t);
    for _ in 0..40 {
        press(&mut t, Key::PageDown);
    }
    assert_eq!(t.offset(), total - 24);
    assert_eq!(
        t.perform(Cmd::Custom(PAGE_DOWN)),
        CmdResult::NoChange,
        "already at the end"
    );
}

#[test]
fn scrolling_moves_a_selection_that_left_the_viewport_to_the_topmost_fully_visible_section() {
    let mut t = session_transcript(80, 24);
    assert_eq!(selected(&t), section_id("t1", SectionKey::Prompt));
    ctrl(&mut t, 'd');
    let offset = t.offset();
    let expected = spans(&mut t)
        .into_iter()
        .find(|(_, r)| r.start >= offset && r.end <= offset + 24)
        .map(|(id, _)| id)
        .unwrap();
    assert_eq!(selected(&t), expected);
    assert_ne!(selected(&t), section_id("t1", SectionKey::Prompt));
}

#[test]
fn a_selection_that_stays_fully_visible_is_kept_while_scrolling() {
    let mut t = session_transcript(80, 24);
    // A section in the lower half of the screen, still on screen after half
    // a page.
    let id = spans(&mut t)
        .into_iter()
        .find(|(_, r)| r.start >= 14 && r.end <= 23)
        .map(|(id, _)| id)
        .expect("a section in rows 14..23");
    assert!(t.select(&id));
    assert_eq!(t.offset(), 0);
    ctrl(&mut t, 'd');
    assert_eq!(t.offset(), 12, "the view scrolled");
    let rows = span_of(&mut t, &id);
    assert!(rows.start >= 12 && rows.end <= 36, "still fully visible");
    assert_eq!(selected(&t), id);
}

/// The topmost section that is fully inside the viewport.
fn topmost_fully_visible(t: &mut Transcript, height: usize) -> SectionId {
    let offset = t.offset();
    spans(t)
        .into_iter()
        .find(|(_, r)| r.start >= offset && r.end <= offset + height)
        .map(|(id, _)| id)
        .expect("a fully visible section")
}

#[test]
fn every_page_key_moves_the_selection_to_the_topmost_fully_visible_section() {
    for (code, mods, from_end) in [
        (Key::PageUp, KeyModifiers::NONE, true),
        (Key::Char('u'), KeyModifiers::CONTROL, true),
        (Key::PageDown, KeyModifiers::NONE, false),
        (Key::Char('d'), KeyModifiers::CONTROL, false),
    ] {
        let mut t = session_transcript(80, 24);
        if from_end {
            // The last section is selected and the view is at the end.
            press(&mut t, Key::Char('G'));
        }
        let (before, was) = (t.offset(), selected(&t));
        t.on(&key(code, mods));
        assert_ne!(t.offset(), before, "{code:?} scrolled");
        let expected = topmost_fully_visible(&mut t, 24);
        assert_ne!(
            expected, was,
            "{code:?}: the old selection left the viewport"
        );
        assert_eq!(selected(&t), expected, "{code:?}");
    }
}

#[test]
fn when_no_section_is_fully_visible_the_selection_moves_to_the_one_at_the_top_row() {
    let (mut t, tall) = tall_section_transcript();
    assert!(t.select(&tall));
    let start = span_of(&mut t, &tall).start;
    t.scroll_to(start + 2);
    // Half a page down: rows start+7..start+17 are inside the tall section.
    ctrl(&mut t, 'd');
    assert_eq!(t.offset(), start + 7);
    assert_eq!(selected(&t), tall, "the section at the top row");
    let (mut t, tall) = tall_section_transcript();
    // Select the prompt, then scroll so that only the tall section is in view.
    let start = span_of(&mut t, &tall).start;
    t.scroll_to(start + 3);
    assert_eq!(selected(&t), section_id("p1", SectionKey::Prompt));
    ctrl(&mut t, 'd');
    assert_eq!(selected(&t), tall);
}

#[test]
fn page_keys_do_nothing_when_the_transcript_fits_the_viewport() {
    let mut t = transcript_of(
        vec![arc(prompt("p1", "short")), arc(reply("r1", "short"))],
        80,
        24,
    );
    let before = text_rows(&draw(&mut t, 80, 24));
    for k in [Key::PageDown, Key::PageUp] {
        press(&mut t, k);
    }
    ctrl(&mut t, 'd');
    ctrl(&mut t, 'u');
    assert_eq!(t.offset(), 0);
    assert_eq!(selected(&t), section_id("p1", SectionKey::Prompt));
    assert_eq!(text_rows(&draw(&mut t, 80, 24)), before);
}

#[test]
fn page_keys_without_a_viewport_do_nothing() {
    let mut t = Transcript::new(blocks::theme(), blocks::formatter()).with_turns(session());
    ctrl(&mut t, 'd');
    press(&mut t, Key::PageDown);
    assert_eq!(t.offset(), 0);
}

// ---- Start and end ----

#[test]
fn g_and_home_select_the_first_section_and_scroll_to_the_start() {
    for start in [Key::Char('g'), Key::Home] {
        let mut t = session_transcript(80, 24);
        for _ in 0..9 {
            ch(&mut t, 'j');
        }
        press(&mut t, Key::PageDown);
        assert!(t.offset() > 0);
        press(&mut t, start);
        assert_eq!(selected(&t), section_id("t1", SectionKey::Prompt));
        assert_eq!(t.offset(), 0);
    }
}

#[test]
fn shift_g_and_end_select_the_last_section_and_scroll_to_the_end() {
    for end in [Key::Char('G'), Key::End] {
        let mut t = session_transcript(80, 24);
        let total = total_height(&mut t);
        press(&mut t, end);
        assert_eq!(selected(&t), item_section("t8", "t1"));
        assert_eq!(t.offset(), total - 24);
        let rows = text_rows(&draw(&mut t, 80, 24));
        assert!(rows[23].starts_with('╰'), "{}", rows[23]);
    }
}

#[test]
fn start_and_end_report_no_change_when_already_there() {
    let mut t = session_transcript(80, 24);
    assert_eq!(t.perform(Cmd::GoTo(Position::Begin)), CmdResult::NoChange);
    t.perform(Cmd::GoTo(Position::End));
    assert_eq!(t.perform(Cmd::GoTo(Position::End)), CmdResult::NoChange);
}

// ---- Folding ----

fn prompt_section() -> SectionId {
    section_id("t1", SectionKey::Prompt)
}

#[test]
fn enter_and_space_toggle_the_selected_section() {
    let mut t = session_transcript(80, 24);
    assert_eq!(t.is_folded(&prompt_section()), Some(true));
    press(&mut t, Key::Enter);
    assert_eq!(t.is_folded(&prompt_section()), Some(false));
    ch(&mut t, ' ');
    assert_eq!(t.is_folded(&prompt_section()), Some(true));
}

#[test]
fn h_and_left_fold_l_and_right_unfold() {
    let mut t = session_transcript(80, 24);
    ch(&mut t, 'l');
    assert_eq!(t.is_folded(&prompt_section()), Some(false));
    ch(&mut t, 'l');
    assert_eq!(
        t.is_folded(&prompt_section()),
        Some(false),
        "already unfolded"
    );
    ch(&mut t, 'h');
    assert_eq!(t.is_folded(&prompt_section()), Some(true));
    press(&mut t, Key::Right);
    assert_eq!(t.is_folded(&prompt_section()), Some(false));
    press(&mut t, Key::Left);
    assert_eq!(t.is_folded(&prompt_section()), Some(true));
}

#[test]
fn fold_keys_ignore_sections_that_cannot_fold() {
    let mut t = session_transcript(80, 24);
    let text = item_section("t2", "t1");
    assert!(t.select(&text));
    let before = text_rows(&draw(&mut t, 80, 24));
    let offset = t.offset();
    for k in [Key::Enter, Key::Left, Key::Right] {
        press(&mut t, k);
    }
    for c in ['h', 'l', ' '] {
        ch(&mut t, c);
    }
    assert_eq!(t.is_folded(&text), None);
    assert_eq!(text_rows(&draw(&mut t, 80, 24)), before);
    assert_eq!(t.offset(), offset);
    assert_eq!(
        t.perform(Cmd::Custom(FOLD)),
        CmdResult::NoChange,
        "a section that cannot fold"
    );
}

#[test]
fn minus_folds_everything_plus_and_equals_unfold_everything() {
    let mut t = session_transcript(80, 24);
    let foldable = |t: &Transcript| -> Vec<(SectionId, Option<bool>)> {
        (0..t.turn_count())
            .flat_map(|i| t.sections_of(i))
            .map(|id| {
                let folded = t.is_folded(&id);
                (id, folded)
            })
            .collect()
    };
    ch(&mut t, '+');
    let all = foldable(&t);
    assert!(all.iter().any(|(_, f)| f.is_some()));
    assert!(all.iter().all(|(_, f)| *f != Some(true)), "{all:?}");
    ch(&mut t, '-');
    let all = foldable(&t);
    assert!(all.iter().all(|(_, f)| *f != Some(false)), "{all:?}");
    ch(&mut t, '=');
    assert!(foldable(&t).iter().all(|(_, f)| *f != Some(true)));
}

#[test]
fn fold_all_and_unfold_all_include_hidden_system_sections() {
    let mut t = session_transcript(80, 24).show_system(false);
    ch(&mut t, '+');
    ch(&mut t, 's');
    assert_eq!(t.is_folded(&item_section("t6", "i1")), Some(false));
    ch(&mut t, '-');
    assert_eq!(t.is_folded(&item_section("t6", "i1")), Some(true));
}

#[test]
fn fold_all_with_nothing_to_fold_changes_nothing() {
    let mut t = session_transcript(80, 24);
    assert_eq!(t.perform(Cmd::Custom(FOLD_ALL)), CmdResult::NoChange);
    t.perform(Cmd::Custom(UNFOLD_ALL));
    assert_eq!(t.perform(Cmd::Custom(UNFOLD_ALL)), CmdResult::NoChange);
}

#[test]
fn a_fold_change_keeps_the_selected_sections_screen_row() {
    let mut t = session_transcript(80, 24);
    let tool = item_section("t2", "c2");
    assert!(t.select(&tool));
    // Scroll so that the section sits in the middle of the screen.
    let start = span_of(&mut t, &tool).start;
    t.scroll_to(start.saturating_sub(8));
    let row = screen_row(&mut t, &tool);
    assert_eq!(row, 8);
    ch(&mut t, 'l');
    assert_eq!(t.is_folded(&tool), Some(false));
    assert_eq!(screen_row(&mut t, &tool), row, "unfolding");
    ch(&mut t, 'h');
    assert_eq!(screen_row(&mut t, &tool), row, "folding");
}

#[test]
fn unfolding_near_the_bottom_does_not_scroll_the_view() {
    let mut t = session_transcript(80, 24);
    let tool = item_section("t2", "c2");
    assert!(t.select(&tool));
    let start = span_of(&mut t, &tool).start;
    t.scroll_to(start.saturating_sub(20));
    let (offset, row) = (t.offset(), screen_row(&mut t, &tool));
    assert!(row >= 16, "the section is near the bottom: {row}");
    ch(&mut t, 'l');
    assert_eq!(t.is_folded(&tool), Some(false));
    assert!(
        span_of(&mut t, &tool).end > offset + 24,
        "it extends below the screen"
    );
    assert_eq!(t.offset(), offset, "the view did not follow it down");
    assert_eq!(screen_row(&mut t, &tool), row);
}

#[test]
fn fold_all_keeps_the_selected_sections_screen_row() {
    let mut t = session_transcript(80, 24);
    let tool = item_section("t4", "c1");
    assert!(t.select(&tool));
    let start = span_of(&mut t, &tool).start;
    t.scroll_to(start.saturating_sub(5));
    let row = screen_row(&mut t, &tool);
    ch(&mut t, '+');
    assert_eq!(screen_row(&mut t, &tool), row, "unfold all");
    ch(&mut t, '-');
    assert_eq!(screen_row(&mut t, &tool), row, "fold all");
    assert_eq!(selected(&t), tool);
}

#[test]
fn the_screen_row_is_kept_only_when_the_end_of_the_transcript_allows_it() {
    let mut t = session_transcript(80, 24);
    ch(&mut t, 'G');
    let last = selected(&t);
    let row = screen_row(&mut t, &last);
    ch(&mut t, '+');
    // Unfolding above the last section moves it down; the view follows, so
    // it stays at the end and the section keeps its screen row.
    let total = total_height(&mut t);
    assert_eq!(t.offset(), total - 24);
    assert_eq!(selected(&t), last);
    assert_eq!(screen_row(&mut t, &last), row);
}

#[test]
fn fold_all_on_a_selected_truncation_row_moves_to_its_tool_call() {
    let mut t = long_output_transcript(80, 24);
    let more = SectionId {
        turn: TurnId::from("a1"),
        key: SectionKey::More(ItemId::from("c1")),
    };
    for _ in 0..100 {
        if t.selected() == Some(&more) {
            break;
        }
        ch(&mut t, 'j');
    }
    assert_eq!(t.selected(), Some(&more));
    ch(&mut t, '-');
    assert_eq!(t.selected(), Some(&item_section("a1", "c1")));
}

#[test]
fn enter_on_a_truncation_row_expands_and_fold_keys_ignore_it() {
    let mut t = long_output_transcript(80, 24);
    let more = SectionId {
        turn: TurnId::from("a1"),
        key: SectionKey::More(ItemId::from("c1")),
    };
    while t.selected() != Some(&more) {
        ch(&mut t, 'j');
    }
    ch(&mut t, 'h');
    ch(&mut t, 'l');
    assert_eq!(t.selected(), Some(&more), "fold keys ignore the row");
    press(&mut t, Key::Enter);
    assert_eq!(t.selected(), Some(&item_section("a1", "c1")));
}

// ---- System sections ----

#[test]
fn s_hides_and_shows_every_system_section() {
    let mut t = session_transcript(80, 24);
    let system = item_section("t6", "i1");
    assert!(t.sections_of(5).contains(&system));
    let shown = all_rows_joined(&t);
    assert!(shown.contains("▶ file: /home/jacob/notes.md"), "{shown}");
    ch(&mut t, 's');
    assert!(!t.sections_of(5).contains(&system));
    assert!(!all_rows_joined(&t).contains("notes.md"));
    assert_eq!(
        t.query(SHOW_SYSTEM_ATTR).unwrap().into_owned(),
        AttrValue::Flag(false)
    );
    ch(&mut t, 's');
    assert!(t.sections_of(5).contains(&system));
    assert_eq!(all_rows_joined(&t), shown);
}

fn all_rows_joined(t: &Transcript) -> String {
    all_rows(t, 80).join("\n")
}

#[test]
fn with_show_system_false_system_sections_start_hidden_and_s_shows_them() {
    let mut t = session_transcript(80, 24).show_system(false);
    let system = item_section("t6", "i1");
    assert!(!t.sections_of(5).contains(&system));
    ch(&mut t, 's');
    assert!(t.sections_of(5).contains(&system));
    let mut via_attr = session_transcript(80, 24);
    via_attr.attr(SHOW_SYSTEM_ATTR, AttrValue::Flag(false));
    assert!(!via_attr.sections_of(5).contains(&system));
}

#[test]
fn hiding_system_content_moves_a_selected_system_section_to_the_next_visible_one() {
    let mut t = session_transcript(80, 24);
    let system = item_section("t6", "i1");
    assert!(t.select(&system));
    ch(&mut t, 's');
    assert_eq!(selected(&t), section_id("t6", SectionKey::Prompt));
    let next = section_id("t6", SectionKey::Prompt);
    assert!(t.sections_of(5).contains(&next));
}

#[test]
fn hiding_system_content_selects_the_previous_section_when_none_follows() {
    let injected = blocks::injected("i1", &[("file", "/tmp/x")]);
    let turn = arc(with_id(
        blocks::assistant(None, vec![blocks::text("t1", "Reply."), injected], None),
        "a1",
    ));
    let mut t = transcript_of(vec![arc(prompt("p1", "hi")), turn], 80, 24);
    assert!(t.select(&item_section("a1", "i1")));
    ch(&mut t, 's');
    assert_eq!(selected(&t), item_section("a1", "t1"));
}

#[test]
fn hiding_system_content_never_leaves_the_selection_on_a_hidden_section() {
    let mut t = session_transcript(80, 24);
    ch(&mut t, 's');
    for _ in 0..30 {
        ch(&mut t, 'j');
        assert_ne!(
            selected(&t),
            item_section("t6", "i1"),
            "system sections are not stops"
        );
    }
}

#[test]
fn moving_and_jumping_do_not_stop_on_hidden_system_sections() {
    let mut t = session_transcript(80, 24).show_system(false);
    assert!(t.select(&section_id("t5", SectionKey::Summary)));
    ch(&mut t, 'j');
    assert_eq!(selected(&t), section_id("t6", SectionKey::Prompt));
}

// ---- Keymap ----

fn remapped(toml: &str) -> Transcript {
    let (config, warnings) = Config::from_toml_str(toml, Path::new("/home/tester"));
    assert!(warnings.is_empty(), "{warnings:?}");
    let (keymap, warnings) = Keymap::from_config(&config.keys);
    assert!(warnings.is_empty(), "{warnings:?}");
    session_transcript(80, 24).keymap(keymap)
}

/// Like [`remapped`], for a remap that takes a key from another action,
/// which warns.
fn remapped_with_warnings(toml: &str) -> Transcript {
    let (config, warnings) = Config::from_toml_str(toml, Path::new("/home/tester"));
    assert!(warnings.is_empty(), "{warnings:?}");
    let (keymap, warnings) = Keymap::from_config(&config.keys);
    assert!(!warnings.is_empty());
    session_transcript(80, 24).keymap(keymap)
}

#[test]
fn a_remapped_key_works_and_the_old_one_does_nothing() {
    let mut t = remapped("[keys.browser]\nnext = \"J\"\n");
    let all = all_selectable(&t);
    ch(&mut t, 'j');
    assert_eq!(selected(&t), all[0], "j is unbound now");
    press(&mut t, Key::Char('J'));
    assert_eq!(selected(&t), all[1]);
    // The old arrow binding went with the remap.
    press(&mut t, Key::Down);
    assert_eq!(selected(&t), all[1]);
}

#[test]
fn shifted_letters_arrive_with_the_shift_modifier() {
    let mut t = remapped("[keys.browser]\nnext = \"J\"\n");
    let all = all_selectable(&t);
    t.on(&key(Key::Char('J'), KeyModifiers::SHIFT));
    assert_eq!(selected(&t), all[1]);
}

#[test]
fn a_remap_applies_to_every_action_it_names() {
    let mut t = remapped("[keys.browser]\nfold_all = \"x\"\ntoggle_system = \"Z\"\n");
    ch(&mut t, 'x');
    assert!(t.sections_of(5).contains(&item_section("t6", "i1")));
    ch(&mut t, 'Z');
    assert!(!t.sections_of(5).contains(&item_section("t6", "i1")));
    ch(&mut t, 's');
    assert!(
        !t.sections_of(5).contains(&item_section("t6", "i1")),
        "s is unbound"
    );
}

#[test]
fn keys_of_other_contexts_and_actions_of_other_tasks_do_nothing() {
    let mut t = session_transcript(80, 24);
    let before = text_rows(&draw(&mut t, 80, 24));
    // `/` search, `y` copy, `q` quit are not for the transcript to act on.
    for c in ['/', 'y', 'q', 'Z'] {
        ch(&mut t, c);
    }
    press(&mut t, Key::Esc);
    press(&mut t, Key::Tab);
    assert_eq!(text_rows(&draw(&mut t, 80, 24)), before);
    assert_eq!(selected(&t), prompt_section());
    let non_key = Event::<UserEvent>::Tick;
    assert_eq!(t.on(&non_key), None);
}

#[test]
fn a_chord_binding_waits_for_its_second_key() {
    let mut t = remapped_with_warnings("[keys.browser]\nnext = \"g j\"\n");
    let all = all_selectable(&t);
    ch(&mut t, 'g');
    assert_eq!(selected(&t), all[0]);
    ch(&mut t, 'j');
    assert_eq!(selected(&t), all[1]);
}

#[test]
fn unknown_custom_cmds_are_invalid() {
    let mut t = session_transcript(80, 24);
    assert_eq!(
        t.perform(Cmd::Custom("frobnicate")),
        CmdResult::Invalid(Cmd::Custom("frobnicate"))
    );
}

#[test]
fn navigation_without_turns_does_nothing() {
    let mut t = Transcript::new(blocks::theme(), blocks::formatter()).viewport(Size::new(80, 24));
    for c in ['j', 'k', ']', '[', 'g', 'G', 'l', 'h', '+', '-', 's'] {
        ch(&mut t, c);
    }
    ctrl(&mut t, 'd');
    press(&mut t, Key::PageDown);
    press(&mut t, Key::Enter);
    assert_eq!(t.selected(), None);
    assert_eq!(t.offset(), 0);
}

#[test]
fn the_marker_is_on_the_selected_section_after_keys() {
    let mut t = session_transcript(80, 24);
    for _ in 0..3 {
        ch(&mut t, 'j');
    }
    assert!(!marked_rows(&mut t, 80, 24).is_empty());
}

// ---- Redraw messages ----

#[test]
fn a_key_that_changes_the_view_answers_transcript_changed() {
    let mut t = session_transcript(80, 24);
    for c in ['l', 'j', 's', 'G'] {
        assert_eq!(ch(&mut t, c), Some(Msg::TranscriptChanged), "{c}");
    }
    assert_eq!(ctrl(&mut t, 'u'), Some(Msg::TranscriptChanged));
    assert_eq!(press(&mut t, Key::Home), Some(Msg::TranscriptChanged));
}

#[test]
fn a_key_that_changes_nothing_answers_nothing() {
    let mut t = session_transcript(80, 24);
    assert_eq!(ch(&mut t, 'k'), None, "at the first section");
    assert_eq!(ch(&mut t, '['), None, "no block before the first");
    assert_eq!(ch(&mut t, 'g'), None, "already at the start");
    assert_eq!(ch(&mut t, 'h'), None, "already folded");
    assert_eq!(ch(&mut t, '-'), None, "everything is folded");
    assert_eq!(ch(&mut t, 'z'), None, "unbound");
    assert_eq!(ch(&mut t, '/'), None, "search is another task's");
    assert_eq!(ctrl(&mut t, 'u'), None, "at the top");
    assert_eq!(t.on(&Event::<UserEvent>::Tick), None);
}

// ---- A jump keeps the selection on screen ----

/// Block `u2` has 40 rows of unfolded system content in front of its
/// one-row prompt, in a 10-row viewport.
fn long_system_transcript() -> Transcript {
    let file = tall_text_of(40);
    let u2 = arc(with_id(
        blocks::user(
            "The prompt.",
            vec![blocks::injected("i1", &[("file", &file)])],
        ),
        "u2",
    ));
    transcript_of(
        vec![arc(prompt("p1", "first")), u2, arc(reply("r1", "Done."))],
        80,
        10,
    )
}

fn assert_selection_on_screen(t: &mut Transcript, height: usize) {
    let id = selected(t);
    let rows = span_of(t, &id);
    assert!(
        rows.start >= t.offset() && rows.end <= t.offset() + height,
        "{id:?} at {rows:?} is off screen at offset {}",
        t.offset()
    );
}

#[test]
fn a_jump_past_a_tall_unfolded_system_section_keeps_the_prompt_on_screen() {
    let mut t = long_system_transcript();
    ch(&mut t, '+');
    ch(&mut t, ']');
    assert_eq!(selected(&t), section_id("u2", SectionKey::Prompt));
    assert_selection_on_screen(&mut t, 10);
    ch(&mut t, ']');
    assert_eq!(selected(&t).turn, TurnId::from("r1"));
    assert_selection_on_screen(&mut t, 10);
    ch(&mut t, '[');
    assert_eq!(selected(&t), section_id("u2", SectionKey::Prompt));
    assert_selection_on_screen(&mut t, 10);
}

#[test]
fn a_jump_keeps_the_top_border_on_top_while_the_prompt_still_fits() {
    let mut t = long_system_transcript();
    ch(&mut t, ']');
    assert_eq!(selected(&t), section_id("u2", SectionKey::Prompt));
    let top = t.measured(|cache, m| cache.top(1, m));
    assert_eq!(t.offset(), top, "the system section is folded: all fits");
    assert_selection_on_screen(&mut t, 10);
}

// ---- Small viewports and showing system content ----

#[test]
fn g_in_a_one_row_viewport_shows_the_selected_last_section() {
    let mut t = transcript_of(
        vec![arc(prompt("p1", "first")), arc(reply("r1", "Yes."))],
        80,
        1,
    );
    ch(&mut t, 'G');
    assert_eq!(selected(&t), item_section("r1", "t1"));
    assert_selection_on_screen(&mut t, 1);
    let rows = text_rows(&draw(&mut t, 80, 1));
    assert!(rows[0].contains("Yes."), "{}", rows[0]);
}

#[test]
fn g_on_a_tall_last_section_still_shows_its_end() {
    let (mut t, tall) = tall_section_transcript();
    ch(&mut t, 'j');
    ch(&mut t, 'j');
    ch(&mut t, 'G');
    assert_eq!(selected(&t), item_section("r1", "t1"));
    let _ = tall;
    let total = total_height(&mut t);
    assert_eq!(t.offset(), total - 10);
}

#[test]
fn half_a_page_is_at_least_one_row() {
    let mut t = transcript_of(
        vec![arc(prompt("p1", "first")), arc(reply("r1", "Yes."))],
        80,
        1,
    );
    ctrl(&mut t, 'd');
    assert_eq!(t.offset(), 1);
}

#[test]
fn showing_system_content_keeps_the_top_of_the_transcript_on_screen() {
    let mut t = session_transcript(80, 12).show_system(false);
    assert_eq!(selected(&t), section_id("t1", SectionKey::Prompt));
    ch(&mut t, 's');
    ch(&mut t, 's');
    ch(&mut t, 's');
    assert_eq!(t.offset(), 0);
    let rows = text_rows(&draw(&mut t, 80, 12));
    assert!(rows[0].starts_with("╭─ User"), "{}", rows[0]);
}

#[test]
fn showing_system_content_scrolls_a_pushed_down_selection_into_view() {
    let mut t = session_transcript(80, 12).show_system(false);
    assert!(t.select(&section_id("t6", SectionKey::Prompt)));
    ch(&mut t, 's');
    assert_selection_on_screen(&mut t, 12);
}
