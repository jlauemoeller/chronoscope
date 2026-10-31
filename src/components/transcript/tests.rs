//! Behaviour tests of the transcript component.

use ratatui::layout::{Rect, Size};
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::component::Component;
use tuirealm::state::{State, StateValue};

use super::fixtures::*;
use super::*;
use crate::components::turn_block::fixtures as blocks;
use crate::components::turn_block::{SectionKey, derive_sections};
use crate::model::{ItemId, Thinking, TurnId};
use crate::test_support::{buffer_to_text, render};
use crate::theme::{Role, builtin};

mod cache_lookup;
mod copy;
mod follow;
pub(super) mod keys;
mod mouse;
mod property;
mod save;
mod search;
mod subagent;
mod truncation;

pub(super) fn section_id(turn: &str, key: SectionKey) -> SectionId {
    SectionId {
        turn: TurnId::from(turn),
        key,
    }
}

pub(super) fn item_section(turn: &str, item: &str) -> SectionId {
    section_id(turn, SectionKey::Item(ItemId::from(item)))
}

/// Markdown that renders to 40 rows: one list item per row.
fn tall_text() -> String {
    (1..=40).map(|n| format!("- item {n}\n")).collect()
}

pub(super) fn down() -> Cmd {
    Cmd::Move(Direction::Down)
}

pub(super) fn up() -> Cmd {
    Cmd::Move(Direction::Up)
}

fn scroll_down() -> Cmd {
    Cmd::Scroll(Direction::Down)
}

fn scroll_up() -> Cmd {
    Cmd::Scroll(Direction::Up)
}

/// Every row of the transcript at `width`, block by block.
fn all_rows(t: &Transcript, width: u16) -> Vec<String> {
    (0..t.turn_count())
        .flat_map(|i| block_rows(t, i, width))
        .collect()
}

fn selected_turn(t: &Transcript) -> String {
    t.selected().unwrap().turn.to_string()
}

fn total_height(t: &mut Transcript) -> usize {
    t.measured(|cache, m| cache.total(m))
}

// ---- Component basics ----

#[test]
fn builds_from_props_alone_and_renders_without_a_viewport() {
    let mut t = Transcript::new(blocks::theme(), blocks::formatter()).with_turns(session());
    assert!(t.visible_blocks().is_empty(), "no viewport was announced");
    let buf = draw(&mut t, 80, 24);
    let text = text_rows(&buf);
    assert!(text[0].starts_with("╭─"), "{}", text[0]);
    assert_eq!(text, all_rows(&t, 80)[..24]);
}

#[test]
fn the_unprepared_render_equals_the_prepared_one() {
    let mut cold = Transcript::new(blocks::theme(), blocks::formatter()).with_turns(session());
    let mut warm = session_transcript(80, 24);
    let a = buffer_to_text(&draw(&mut cold, 80, 24));
    let b = buffer_to_text(&draw(&mut warm, 80, 24));
    assert_eq!(a, b);
}

#[test]
fn view_changes_nothing_in_the_transcript() {
    let mut t = Transcript::new(blocks::theme(), blocks::formatter()).with_turns(session());
    let (run, computed) = (t.layouts_run(), t.cache().computed());
    draw(&mut t, 80, 24);
    draw(&mut t, 100, 10);
    assert_eq!(t.layouts_run(), run);
    assert_eq!(t.cache().computed(), computed);
    assert_eq!(t.cache().width(), 0);
    assert_eq!(t.offset(), 0);
}

#[test]
fn state_is_the_scroll_offset() {
    let mut t = session_transcript(80, 10);
    assert_eq!(t.state(), State::Single(StateValue::Usize(0)));
    t.perform(scroll_down());
    assert_eq!(t.state(), State::Single(StateValue::Usize(1)));
}

#[test]
fn props_round_trip_through_attr_and_query() {
    let mut t = session_transcript(80, 24);
    t.attr(DIM_UNSELECTED_ATTR, AttrValue::Flag(false));
    t.attr(SHOW_SYSTEM_ATTR, AttrValue::Flag(false));
    t.attr(TRUNCATE_OUTPUT_LINES_ATTR, AttrValue::Length(7));
    let theme = builtin::default_theme();
    t.attr(THEME_ATTR, theme_value(&theme));
    let get = |attr| t.query(attr).map(|q| q.into_owned());
    assert_eq!(get(DIM_UNSELECTED_ATTR), Some(AttrValue::Flag(false)));
    assert_eq!(get(SHOW_SYSTEM_ATTR), Some(AttrValue::Flag(false)));
    assert_eq!(get(TRUNCATE_OUTPUT_LINES_ATTR), Some(AttrValue::Length(7)));
    assert_eq!(get(THEME_ATTR), Some(theme_value(&theme)));
    assert_eq!(get(VIEWPORT_ATTR), Some(viewport_value(Size::new(80, 24))));
    assert_eq!(t.truncate_output_lines(), 7);
    assert!(t.query(Attribute::Custom("nothing")).is_none());
}

#[test]
fn the_formatter_and_viewport_attrs_apply() {
    let mut t = Transcript::new(blocks::theme(), blocks::formatter());
    t.attr(MODEL_UPDATE_ATTR, model_update_value(appended(session())));
    t.attr(VIEWPORT_ATTR, viewport_value(Size::new(60, 12)));
    assert_eq!(t.turn_count(), 8);
    assert_eq!(t.visible_blocks().start, 0);
    let utc = TimestampFormatter::new("%H:%M", true, crate::timestamp::DisplayTz::Local);
    t.attr(TIMESTAMP_FORMATTER_ATTR, payload_value(utc));
    let text = text_rows(&draw(&mut t, 60, 12));
    assert!(text[0].contains("08:35"), "{}", text[0]);
}

#[test]
fn the_background_is_painted_unless_the_theme_is_transparent() {
    let theme = builtin::default_theme();
    let mut t = Transcript::new(theme.clone(), blocks::formatter());
    let buf = draw(&mut t, 20, 4);
    assert_eq!(buf[(5, 2)].bg, theme.role(Role::Background));
    let mut t = Transcript::new(
        theme.clone().with_transparent_background(true),
        blocks::formatter(),
    );
    let buf = draw(&mut t, 20, 4);
    assert_ne!(buf[(5, 2)].bg, theme.role(Role::Background));
}

#[test]
fn nothing_is_drawn_outside_the_area() {
    let mut t = session_transcript(30, 10);
    let area = Rect::new(4, 3, 30, 10);
    let empty = render(40, 16, |frame| {
        frame.render_widget(ratatui::widgets::Clear, frame.area());
    });
    let buf = render(40, 16, |frame| t.view(frame, area));
    for y in 0..16 {
        for x in 0..40 {
            if !area.contains((x, y).into()) {
                assert_eq!(buf[(x, y)], empty[(x, y)], "cell {x},{y} changed");
            }
        }
    }
    assert_eq!(buf[(4, 3)].symbol(), "╭");
}

// ---- Updates ----

#[test]
fn appended_updates_add_blocks() {
    let mut t = session_transcript(80, 24);
    assert_eq!(t.turn_count(), 8);
    t.apply_update(&appended(vec![arc(reply("t9", "One more."))]));
    assert_eq!(t.turn_count(), 9);
    assert_eq!(all_rows(&t, 80).len(), total_height(&mut t));
}

#[test]
fn a_viewport_is_not_needed_for_the_first_selection() {
    let t = Transcript::new(blocks::theme(), blocks::formatter()).with_turns(session());
    assert_eq!(t.selected(), Some(&section_id("t1", SectionKey::Prompt)));
}

#[test]
fn the_selection_starts_at_the_first_section_once_turns_arrive() {
    let mut t = Transcript::new(blocks::theme(), blocks::formatter()).viewport(Size::new(80, 24));
    assert_eq!(t.selected(), None);
    t.apply_update(&appended(session()));
    assert_eq!(t.selected(), Some(&section_id("t1", SectionKey::Prompt)));
}

#[test]
fn updated_replaces_a_turn_in_place_keeping_selection_and_folds() {
    let mut t = session_transcript(80, 24);
    let tool = item_section("t2", "c2");
    let thinking = item_section("t2", "th1");
    t.set_folded(&tool, false);
    t.set_folded(&thinking, false);
    assert!(t.select(&tool));
    let before = all_rows(&t, 80);
    let joined = before.join("\n");
    assert!(joined.contains("sudo: a password is required"));
    assert!(joined.contains("Start with lsusb."));
    let mut changed = (*t.turn(1).unwrap().clone()).clone();
    if let Turn::Assistant(a) = &mut changed {
        a.items.push(blocks::text("t9", "A late addition."));
    }
    t.apply_update(&ModelUpdate::Updated(vec![arc(changed)]));
    assert_eq!(t.turn_count(), 8);
    assert_eq!(t.turn(1).unwrap().id(), &TurnId::from("t2"), "in place");
    assert_eq!(t.selected(), Some(&tool));
    assert_eq!(t.is_folded(&tool), Some(false));
    assert_eq!(t.is_folded(&thinking), Some(false));
    let after = all_rows(&t, 80);
    // Both sections are still unfolded in what is drawn.
    let joined = after.join("\n");
    assert!(joined.contains("sudo: a password is required"));
    assert!(joined.contains("Start with lsusb."));
    assert!(after.len() > before.len());
    assert!(after.iter().any(|row| row.contains("A late addition.")));
}

#[test]
fn updated_turns_with_unknown_ids_are_ignored() {
    let mut t = session_transcript(80, 24);
    t.apply_update(&ModelUpdate::Updated(vec![arc(reply("nope", "x"))]));
    assert_eq!(t.turn_count(), 8);
}

#[test]
fn an_update_invalidates_only_its_block() {
    let mut t = session_transcript(80, 24);
    total_height(&mut t);
    let computed = t.cache().computed();
    t.apply_update(&ModelUpdate::Updated(vec![arc(reply("t7", "Shorter."))]));
    total_height(&mut t);
    assert_eq!(t.cache().computed(), computed + 1);
}

#[test]
fn the_other_model_updates_are_ignored() {
    let mut t = session_transcript(80, 24);
    let before = buffer_to_text(&draw(&mut t, 80, 24));
    for update in [
        ModelUpdate::Progress(0.5),
        ModelUpdate::Loaded,
        ModelUpdate::ParseWarnings(3),
    ] {
        t.apply_update(&update);
    }
    assert_eq!(buffer_to_text(&draw(&mut t, 80, 24)), before);
    assert_eq!(t.turn_count(), 8);
}

#[test]
fn reset_clears_turns_folds_selection_and_offset() {
    let mut t = session_transcript(80, 10);
    let tool = item_section("t2", "c2");
    t.set_folded(&tool, false);
    t.perform(down());
    t.scroll_to(12);
    t.apply_update(&ModelUpdate::Reset);
    assert_eq!(t.turn_count(), 0);
    assert_eq!(t.selected(), None);
    assert_eq!(t.offset(), 0);
    assert!(t.cache().is_empty());
    assert!(t.visible_blocks().is_empty());
    // The fold of the earlier session is gone with it.
    t.apply_update(&appended(session()));
    assert_eq!(t.is_folded(&tool), Some(true));
    assert_eq!(t.selected(), Some(&section_id("t1", SectionKey::Prompt)));
}

#[test]
fn a_vanished_selection_moves_to_the_nearest_following_section() {
    let mut t = session_transcript(80, 24);
    let thinking = item_section("t4", "x2");
    assert!(t.select(&thinking));
    let mut changed = (*t.turn(3).unwrap().clone()).clone();
    if let Turn::Assistant(a) = &mut changed {
        a.items.retain(|item| item.id() != &ItemId::from("x2"));
    }
    t.apply_update(&ModelUpdate::Updated(vec![arc(changed)]));
    assert_eq!(t.selected(), Some(&item_section("t4", "c1")));
}

#[test]
fn a_vanished_selection_moves_to_the_next_block_when_none_follows_in_its_own() {
    let mut t = session_transcript(80, 24);
    assert!(t.select(&section_id("t5", SectionKey::Summary)));
    let empty = with_id(
        blocks::assistant(None, vec![blocks::thinking("r", Thinking::Redacted)], None),
        "t5",
    );
    t.apply_update(&ModelUpdate::Updated(vec![arc(empty)]));
    assert_eq!(t.selected(), Some(&item_section("t6", "i1")));
}

#[test]
fn a_vanished_selection_with_nothing_after_it_moves_to_the_last_section() {
    let mut t = session_transcript(80, 24);
    assert!(t.select(&item_section("t8", "t1")));
    let empty = with_id(
        blocks::assistant(None, vec![blocks::thinking("r", Thinking::Redacted)], None),
        "t8",
    );
    t.apply_update(&ModelUpdate::Updated(vec![arc(empty)]));
    assert_eq!(t.selected(), Some(&item_section("t7", "t1")));
}

#[test]
fn hiding_system_sections_moves_a_selection_inside_one() {
    let mut t = session_transcript(80, 24);
    let system = item_section("t6", "i1");
    assert!(t.select(&system));
    t.attr(SHOW_SYSTEM_ATTR, AttrValue::Flag(false));
    assert_eq!(t.selected(), Some(&section_id("t6", SectionKey::Prompt)));
}

#[test]
fn show_system_invalidates_only_the_blocks_with_injected_items() {
    let mut t = session_transcript(80, 24);
    total_height(&mut t);
    let computed = t.cache().computed();
    t.attr(SHOW_SYSTEM_ATTR, AttrValue::Flag(false));
    total_height(&mut t);
    assert_eq!(t.cache().computed(), computed + 1, "only t6 holds one");
    let text = all_rows(&t, 80).join("\n");
    assert!(!text.contains("notes.md"));
}

// ---- The layout cache ----

fn heights(t: &mut Transcript) -> Vec<usize> {
    let n = t.turn_count();
    t.measured(|cache, m| (0..n).map(|i| cache.height(i, m)).collect())
}

#[test]
fn heights_match_the_standalone_rendering() {
    let mut t = session_transcript(100, 24);
    let hs = heights(&mut t);
    for (i, h) in hs.iter().enumerate() {
        assert_eq!(*h, block_rows(&t, i, 100).len(), "block {i}");
    }
    assert_eq!(hs.iter().sum::<usize>(), total_height(&mut t));
}

#[test]
fn a_width_change_recomputes_heights_and_they_match_the_rendering() {
    let mut t = session_transcript(120, 24);
    let wide = heights(&mut t);
    let computed = t.cache().computed();
    t.set_viewport(Size::new(40, 24));
    let narrow = heights(&mut t);
    assert!(t.cache().computed() >= computed + t.turn_count());
    assert!(narrow.iter().sum::<usize>() > wide.iter().sum::<usize>());
    for (i, h) in narrow.iter().enumerate() {
        assert_eq!(*h, block_rows(&t, i, 40).len(), "block {i}");
    }
}

#[test]
fn a_height_change_alone_does_not_recompute() {
    let mut t = session_transcript(80, 10);
    total_height(&mut t);
    let computed = t.cache().computed();
    t.set_viewport(Size::new(80, 30));
    total_height(&mut t);
    assert_eq!(t.cache().computed(), computed, "same width, same heights");
}

#[test]
fn a_fold_toggle_invalidates_only_that_block() {
    let mut t = session_transcript(80, 24);
    let before = heights(&mut t);
    let computed = t.cache().computed();
    assert!(t.select(&item_section("t2", "c2")));
    let result = t.perform(Cmd::Submit);
    assert!(matches!(result, CmdResult::Visual | CmdResult::Changed(_)));
    let after = heights(&mut t);
    assert_eq!(t.cache().computed(), computed + 1);
    for (i, (b, a)) in before.iter().zip(&after).enumerate() {
        assert_eq!(i == 1, b != a, "block {i}");
    }
}

#[test]
fn selection_moves_do_not_recompute_heights() {
    let mut t = session_transcript(80, 24);
    total_height(&mut t);
    let computed = t.cache().computed();
    for _ in 0..6 {
        t.perform(down());
    }
    assert_eq!(t.cache().computed(), computed);
}

// ---- Rendering only what is visible ----

#[test]
fn only_the_blocks_overlapping_the_viewport_are_laid_out() {
    let turns = (0..2000)
        .map(|i| arc(reply(&format!("n{i}"), "A short reply.")))
        .collect();
    let t = transcript_of(turns, 80, 24);
    let visible = t.visible_blocks();
    // A reply is 5 rows tall: 24 rows overlap blocks 0..=4.
    assert_eq!(visible, 0..5);
    assert_eq!(
        t.layouts_run(),
        visible.len(),
        "one layout per visible block"
    );
    assert_eq!(t.cache().computed(), visible.len());
    // `view` draws exactly the placements of the window, one per block.
    let window = t.window.as_ref().unwrap();
    assert_eq!(
        window.placements(Rect::new(0, 0, 80, 24)).len(),
        visible.len()
    );
}

#[test]
fn drawing_a_frame_runs_no_layout_when_prepared() {
    let turns = (0..2000)
        .map(|i| arc(reply(&format!("n{i}"), "A short reply.")))
        .collect();
    let mut t = transcript_of(turns, 80, 24);
    let run = t.layouts_run();
    draw(&mut t, 80, 24);
    assert_eq!(t.layouts_run(), run);
}

#[test]
fn scrolling_far_down_lays_out_only_what_it_passes_and_shows() {
    let turns: Vec<_> = (0..2000)
        .map(|i| arc(reply(&format!("n{i}"), "A short reply.")))
        .collect();
    let mut t = transcript_of(turns, 80, 24);
    t.scroll_to(5 * 1000);
    let visible = t.visible_blocks();
    assert_eq!(visible, 1000..1005);
    // Blocks 0..1000 are measured to find the offset, then 1000..1005 drawn.
    assert!(t.cache().computed() <= 1006 + 1);
    assert!(
        t.cache().known_height(1500).is_none(),
        "blocks below stay unmeasured"
    );
}

#[test]
fn only_overlapping_blocks_are_placed() {
    let t = session_transcript(80, 24);
    let window = t.window.as_ref().unwrap();
    let placements = window.placements(Rect::new(0, 0, 80, 24));
    let mut rows = 0;
    for (n, skip, rect) in placements {
        let placed = &window.blocks[n];
        assert!(placed.top < window.offset + 24);
        assert!(placed.top + placed.layout.height() > window.offset);
        let visible = (placed.layout.height() - skip).min(24 - usize::from(rect.y));
        assert_eq!(usize::from(rect.height), visible);
        rows += usize::from(rect.height);
    }
    assert_eq!(rows, 24, "the placements fill the viewport exactly");
}

// ---- Clipping ----

#[test]
fn clipped_blocks_equal_the_slice_of_the_full_rendering() {
    let mut t = session_transcript(80, 17);
    let full = all_rows(&t, 80);
    let max = full.len() - 17;
    for offset in (0..=max).step_by(3).chain([max]) {
        t.scroll_to(offset);
        assert_eq!(t.offset(), offset);
        let text = text_rows(&draw(&mut t, 80, 17));
        assert_eq!(text, full[offset..offset + 17], "offset {offset}");
    }
}

#[test]
fn a_block_clipped_at_both_edges_shows_the_right_rows() {
    // A single tall block is clipped at the top and the bottom at once.
    let long = tall_text();
    let turn = arc(with_id(blocks::user(&long, vec![]), "u"));
    let mut t = transcript_of(vec![turn], 60, 12);
    let id = section_id("u", SectionKey::Prompt);
    t.set_folded(&id, false);
    t.scroll_to(9);
    let full = all_rows(&t, 60);
    assert_eq!(text_rows(&draw(&mut t, 60, 12)), full[9..21]);
}

// ---- Scrolling ----

#[test]
fn scroll_clamps_at_the_top() {
    let mut t = session_transcript(80, 10);
    assert_eq!(t.perform(scroll_up()), CmdResult::NoChange);
    assert_eq!(t.offset(), 0);
    t.perform(scroll_down());
    assert_eq!(t.perform(scroll_up()), CmdResult::Changed(t.state()));
    assert_eq!(t.offset(), 0);
}

#[test]
fn scroll_clamps_at_the_bottom() {
    let mut t = session_transcript(80, 10);
    let max = total_height(&mut t) - 10;
    t.scroll_to(max - 1);
    assert_eq!(t.perform(scroll_down()), CmdResult::Changed(t.state()));
    assert_eq!(t.offset(), max);
    assert_eq!(t.perform(scroll_down()), CmdResult::NoChange);
    assert_eq!(t.offset(), max);
    let text = text_rows(&draw(&mut t, 80, 10));
    assert!(
        text[9].starts_with('╰'),
        "the last row is the bottom border"
    );
}

#[test]
fn scroll_to_clamps_to_the_content() {
    let mut t = session_transcript(80, 10);
    let max = total_height(&mut t) - 10;
    t.scroll_to(100_000);
    assert_eq!(t.offset(), max);
}

#[test]
fn content_shorter_than_the_viewport_does_not_scroll() {
    let mut t = transcript_of(vec![arc(reply("a", "Hi"))], 40, 24);
    assert_eq!(t.perform(scroll_down()), CmdResult::NoChange);
    assert_eq!(t.offset(), 0);
}

#[test]
fn scroll_cmds_need_a_viewport() {
    let mut t = Transcript::new(blocks::theme(), blocks::formatter()).with_turns(session());
    assert_eq!(t.perform(scroll_down()), CmdResult::NoChange);
    assert_eq!(t.offset(), 0);
}

#[test]
fn unsupported_cmds_are_invalid() {
    let mut t = session_transcript(80, 10);
    let left = Cmd::Move(Direction::Left);
    assert_eq!(t.perform(left), CmdResult::Invalid(left));
    assert_eq!(t.perform(Cmd::None), CmdResult::NoChange);
}

#[test]
fn a_resize_keeps_the_top_block_at_the_top() {
    let mut t = session_transcript(120, 12);
    t.scroll_to(20);
    let top_block = t.visible_blocks().start;
    t.set_viewport(Size::new(60, 12));
    assert_eq!(t.visible_blocks().start, top_block);
    let full = all_rows(&t, 60);
    let off = t.offset();
    assert_eq!(text_rows(&draw(&mut t, 60, 12)), full[off..off + 12]);
}

// ---- Selection ----

#[test]
fn move_crosses_block_boundaries() {
    let mut t = session_transcript(80, 24);
    assert_eq!(selected_turn(&t), "t1");
    let mut seen = vec![t.selected().unwrap().clone()];
    for _ in 0..40 {
        if t.perform(down()) == CmdResult::NoChange {
            break;
        }
        seen.push(t.selected().unwrap().clone());
    }
    let turns: Vec<_> = seen.iter().map(|id| id.turn.to_string()).collect();
    for turn in ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8"] {
        assert!(turns.iter().any(|t| t == turn), "{turn} was never selected");
    }
    assert!(turns.windows(2).all(|w| w[0] <= w[1]), "{turns:?}");
    assert_eq!(selected_turn(&t), "t8");
    assert_eq!(t.perform(down()), CmdResult::NoChange);
}

#[test]
fn move_up_walks_back_through_every_section() {
    let mut t = session_transcript(80, 24);
    let mut forward = vec![t.selected().unwrap().clone()];
    while t.perform(down()) != CmdResult::NoChange {
        forward.push(t.selected().unwrap().clone());
    }
    let mut backward = vec![t.selected().unwrap().clone()];
    while t.perform(up()) != CmdResult::NoChange {
        backward.push(t.selected().unwrap().clone());
    }
    backward.reverse();
    assert_eq!(forward, backward);
    assert_eq!(selected_turn(&t), "t1");
}

#[test]
fn move_scrolls_the_new_selection_into_view() {
    let mut t = session_transcript(80, 12);
    for step in 0..30 {
        t.perform(down());
        let (offset, window) = (t.offset(), t.window.as_ref().unwrap());
        let selected = t.selected().unwrap();
        let placed = window
            .blocks
            .iter()
            .find(|p| p.layout.section_rows().iter().any(|(id, _)| id == selected))
            .unwrap_or_else(|| panic!("step {step}: the selected block is not drawn"));
        let (_, rows) = placed
            .layout
            .section_rows()
            .iter()
            .find(|(id, _)| id == selected)
            .unwrap();
        let start = placed.top + rows.start;
        let end = placed.top + rows.end;
        assert!(start >= offset && end <= offset + 12, "step {step}");
    }
}

#[test]
fn move_scrolls_minimally() {
    let mut t = session_transcript(80, 24);
    // The first block is short; selecting its neighbour moves nothing while
    // both fit.
    t.perform(down());
    assert_eq!(t.offset(), 0);
}

#[test]
fn move_up_into_a_section_reveals_it_from_above() {
    let mut t = session_transcript(80, 8);
    t.select(&section_id("t8", SectionKey::Item(ItemId::from("t1"))).clone());
    let at = t.offset();
    assert!(at > 0);
    t.perform(up());
    assert_eq!(selected_turn(&t), "t7");
    assert!(t.offset() < at);
}

#[test]
fn the_selected_rows_carry_the_marker() {
    let mut t = session_transcript(80, 24);
    let text = text_rows(&draw(&mut t, 80, 24));
    assert!(text.iter().any(|row| row.starts_with('▒')));
    let marked = text.iter().filter(|row| row.starts_with('▒')).count();
    let id = section_id("t1", SectionKey::Prompt);
    let rows = t.window.as_ref().unwrap().blocks[0]
        .layout
        .section_rows()
        .iter()
        .find(|(i, _)| *i == id)
        .unwrap()
        .1
        .len();
    assert_eq!(marked, rows);
}

#[test]
fn a_section_taller_than_the_viewport_is_paged_through_before_moving() {
    let long = tall_text();
    let turns = vec![
        arc(with_id(blocks::user(&long, vec![]), "u")),
        arc(reply("a", "Next.")),
    ];
    let mut t = transcript_of(turns, 60, 10);
    let id = section_id("u", SectionKey::Prompt);
    t.set_folded(&id, false);
    let mut offsets = Vec::new();
    while selected_turn(&t) == "u" {
        offsets.push(t.offset());
        if t.perform(down()) == CmdResult::NoChange {
            break;
        }
    }
    assert_eq!(selected_turn(&t), "a");
    // The selection moved on only after the offset stopped advancing a page
    // at a time.
    assert!(offsets.len() > 3, "{offsets:?}");
    assert!(offsets.windows(2).all(|w| w[1] > w[0]), "{offsets:?}");
    assert!(offsets.windows(2).all(|w| w[1] - w[0] <= 10), "{offsets:?}");
    assert_eq!(offsets[1] - offsets[0], 10, "a page at a time");
}

#[test]
fn moving_up_through_a_tall_section_pages_back_up() {
    let long = tall_text();
    let turns = vec![
        arc(reply("a", "Before.")),
        arc(with_id(blocks::user(&long, vec![]), "u")),
        arc(reply("b", "After.")),
    ];
    let mut t = transcript_of(turns, 60, 10);
    t.set_folded(&section_id("u", SectionKey::Prompt), false);
    assert!(t.select(&section_id("b", SectionKey::Item(ItemId::from("t1")))));
    t.perform(up());
    assert_eq!(selected_turn(&t), "u");
    let mut offsets = vec![t.offset()];
    while selected_turn(&t) == "u" {
        t.perform(up());
        offsets.push(t.offset());
    }
    assert_eq!(selected_turn(&t), "a");
    assert!(offsets.windows(2).any(|w| w[0] - w[1] == 10), "{offsets:?}");
}

// ---- Folding ----

#[test]
fn submit_toggles_the_fold_of_the_selected_section() {
    let mut t = session_transcript(80, 24);
    let prompt = section_id("t1", SectionKey::Prompt);
    assert_eq!(t.is_folded(&prompt), Some(true));
    let folded_rows = all_rows(&t, 80).len();
    t.perform(Cmd::Submit);
    assert_eq!(t.is_folded(&prompt), Some(false));
    assert!(all_rows(&t, 80).len() > folded_rows);
    t.perform(Cmd::Submit);
    assert_eq!(t.is_folded(&prompt), Some(true));
    assert_eq!(all_rows(&t, 80).len(), folded_rows);
}

#[test]
fn submit_on_a_section_that_cannot_fold_does_nothing() {
    let mut t = session_transcript(80, 24);
    assert!(t.select(&item_section("t7", "t1")));
    assert_eq!(t.perform(Cmd::Submit), CmdResult::NoChange);
}

#[test]
fn submit_keeps_the_unfolded_section_visible() {
    let mut t = session_transcript(80, 10);
    assert!(t.select(&section_id("t3", SectionKey::Prompt)));
    t.perform(Cmd::Submit);
    let text = text_rows(&draw(&mut t, 80, 10));
    assert!(text.iter().any(|row| row.starts_with('▒')));
}

#[test]
fn set_folded_ignores_sections_that_cannot_fold() {
    let mut t = session_transcript(80, 24);
    let computed = t.cache().computed();
    t.set_folded(&item_section("t7", "t1"), true);
    assert_eq!(t.cache().computed(), computed);
    assert_eq!(t.is_folded(&item_section("t7", "t1")), None);
}

#[test]
fn the_selection_uses_the_ids_of_derived_sections() {
    let t = session_transcript(80, 24);
    let ids: Vec<_> = derive_sections(t.turn(1).unwrap(), true)
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert!(ids.contains(&item_section("t2", "c1")));
}

// ---- Selection stays visible ----

/// Whether the rows of the selected section lie inside the viewport.
fn selection_in_view(t: &Transcript) -> bool {
    let window = t.window.as_ref().unwrap();
    let selected = t.selected().unwrap();
    window.blocks.iter().any(|placed| {
        placed
            .layout
            .section_rows()
            .iter()
            .filter(|(id, _)| id == selected)
            .any(|(_, rows)| {
                placed.top + rows.start >= window.offset
                    && placed.top + rows.end <= window.offset + usize::from(window.size.height)
            })
    })
}

fn has_marker(t: &mut Transcript, width: u16, height: u16) -> bool {
    text_rows(&draw(t, width, height))
        .iter()
        .any(|row| row.starts_with('▒'))
}

#[test]
fn a_tall_selection_scrolled_out_of_view_is_revealed_by_move_not_paged() {
    let tall = tall_text();
    for (step, from) in [(down(), 0), (up(), 10_000)] {
        let turns = vec![
            arc(reply("a", "Before.")),
            arc(reply("b", "Before.")),
            arc(reply("c", "Before.")),
            arc(with_id(blocks::user(&tall, vec![]), "u")),
            arc(reply("d", "After.")),
            arc(reply("e", "After.")),
            arc(reply("f", "After.")),
            arc(reply("g", "After.")),
        ];
        let mut t = transcript_of(turns, 60, 10);
        let tall_id = section_id("u", SectionKey::Prompt);
        t.set_folded(&tall_id, false);
        assert!(t.select(&tall_id));
        t.scroll_to(from);
        assert!(!selection_in_view(&t), "scrolled away (from {from})");
        t.perform(step);
        assert_ne!(
            t.selected(),
            Some(&tall_id),
            "the move selects the neighbour"
        );
        assert!(selection_in_view(&t), "and scrolls it into view");
        assert!(has_marker(&mut t, 60, 10));
    }
}

#[test]
fn a_selection_removed_by_hiding_system_sections_is_scrolled_into_view() {
    let mut t = session_transcript(80, 10);
    assert!(t.select(&item_section("t6", "i1")));
    t.scroll_to(0);
    assert!(!selection_in_view(&t));
    let before = t.selected().cloned();
    t.attr(SHOW_SYSTEM_ATTR, AttrValue::Flag(false));
    assert_ne!(t.selected().cloned(), before);
    assert!(selection_in_view(&t));
    assert!(has_marker(&mut t, 80, 10));
}

#[test]
fn a_selection_removed_by_an_update_is_scrolled_into_view() {
    let mut t = session_transcript(80, 12);
    assert!(t.select(&item_section("t4", "c1")));
    // The new version drops the selected tool call and puts a tall text
    // before the stats footer, which the selection moves to: far below.
    let mut changed = (*t.turn(3).unwrap().clone()).clone();
    if let Turn::Assistant(a) = &mut changed {
        a.items = vec![blocks::text("big", &tall_text())];
    }
    t.apply_update(&ModelUpdate::Updated(vec![arc(changed)]));
    let footer = section_id("t4", SectionKey::Footer);
    assert_eq!(t.selected(), Some(&footer));
    assert!(selection_in_view(&t));
    assert!(has_marker(&mut t, 80, 12));
}

#[test]
fn an_update_above_the_viewport_does_not_move_what_is_on_screen() {
    let mut t = session_transcript(80, 12);
    t.scroll_to(40);
    let before = text_rows(&draw(&mut t, 80, 12));
    let mut changed = (*t.turn(0).unwrap().clone()).clone();
    if let Turn::User(u) = &mut changed {
        u.text = "A much shorter prompt.".to_owned();
    }
    t.apply_update(&ModelUpdate::Updated(vec![arc(changed)]));
    assert_eq!(text_rows(&draw(&mut t, 80, 12)), before);
}

// ---- Scrolling reuses layouts ----

#[test]
fn scrolling_lays_out_only_the_newly_exposed_blocks() {
    let mut t = session_transcript(80, 24);
    let visible = t.visible_blocks();
    let run = t.layouts_run();
    t.perform(scroll_down());
    assert_eq!(t.visible_blocks(), visible);
    assert_eq!(t.layouts_run(), run, "no block was laid out again");
    // Scrolling until a block enters the window lays out just that block.
    let mut exposed = 0;
    while t.visible_blocks() == visible {
        t.perform(scroll_down());
    }
    exposed += t.visible_blocks().end - visible.end;
    assert_eq!(t.layouts_run(), run + exposed);
    let full = all_rows(&t, 80);
    let off = t.offset();
    assert_eq!(text_rows(&draw(&mut t, 80, 24)), full[off..off + 24]);
}

// ---- Clipping keeps styles ----

#[test]
fn a_clipped_block_keeps_its_styles() {
    use ratatui::buffer::Buffer;
    use ratatui::widgets::Widget;
    let theme = builtin::default_theme().with_transparent_background(true);
    let turn = arc(with_id(blocks::user(&tall_text(), vec![]), "u"));
    let mut t = Transcript::new(theme, blocks::formatter())
        .with_turns(vec![turn])
        .viewport(Size::new(60, 8));
    t.set_folded(&section_id("u", SectionKey::Prompt), false);
    t.scroll_to(3);
    let screen = draw(&mut t, 60, 8);
    let block = t.content.block(0);
    let full = render(60, block.height(60) as u16, |frame| {
        (&block).render(frame.area(), frame.buffer_mut());
    });
    let mut slice = Buffer::empty(Rect::new(0, 0, 60, 8));
    for y in 0..8 {
        for x in 0..60 {
            slice[(x, y)] = full[(x, y + 3)].clone();
        }
    }
    assert_eq!(
        crate::test_support::buffer_to_styled(&screen),
        crate::test_support::buffer_to_styled(&slice)
    );
}
