//! Follow mode, the `↓ N new` count and in-place updates (product §6.2).

use tuirealm::command::Position;
use tuirealm::event::Key;

use serde_json::json;

use super::keys::{ch, press};
use super::*;
use crate::app::Msg;
use crate::model::{Item, ToolStatus};

fn end() -> Cmd {
    Cmd::GoTo(Position::End)
}

fn half_page_up() -> Cmd {
    Cmd::Custom(HALF_PAGE_UP)
}

fn text_turn(id: &str) -> Arc<Turn> {
    arc(reply(id, &format!("A reply for {id}.")))
}

fn appended_turns(ids: &[&str]) -> ModelUpdate {
    ModelUpdate::Appended(ids.iter().map(|id| text_turn(id)).collect())
}

fn total(t: &mut Transcript) -> usize {
    total_height(t)
}

/// Whether the last row of the transcript is the last row of the viewport.
fn shows_last_row(t: &mut Transcript, height: usize) -> bool {
    t.offset() + height == total(t)
}

/// The session at 80x24, scrolled to the end.
fn at_the_end() -> Transcript {
    let mut t = session_transcript(80, 24);
    t.perform(end());
    assert!(t.following());
    t
}

fn last_section_of(t: &Transcript, turn: &str) -> SectionId {
    let at = t.content.index[&TurnId::from(turn)];
    t.sections_of(at).pop().unwrap()
}

// ---- Following at the bottom ----

#[test]
fn a_fresh_transcript_of_tall_content_is_at_the_top_and_not_following() {
    let t = session_transcript(80, 24);
    assert_eq!(t.offset(), 0);
    assert!(!t.following());
}

#[test]
fn short_content_follows() {
    let t = transcript_of(vec![text_turn("t1")], 80, 24);
    assert!(t.following());
}

#[test]
fn at_the_bottom_an_appended_turn_scrolls_into_view() {
    let mut t = at_the_end();
    let before = t.offset();
    t.apply_update(&appended_turns(&["t9"]));
    assert!(t.following());
    assert!(t.offset() > before, "the view moved down");
    assert!(shows_last_row(&mut t, 24));
    let text = text_rows(&draw(&mut t, 80, 24)).join("\n");
    assert!(text.contains("A reply for t9."), "{text}");
    assert_eq!(t.new_sections(), 0);
}

#[test]
fn the_selection_follows_when_it_was_on_the_last_section() {
    let mut t = at_the_end();
    assert_eq!(selected_turn(&t), "t8");
    t.apply_update(&appended_turns(&["t9", "t10"]));
    assert_eq!(t.selected(), Some(&last_section_of(&t, "t10")));
}

#[test]
fn a_selection_elsewhere_is_kept_while_the_view_follows() {
    let mut t = at_the_end();
    t.perform(up());
    let kept = t.selected().cloned().unwrap();
    assert_eq!(kept.turn.to_string(), "t7");
    assert!(t.following(), "t7 is on screen: the last row still is");
    t.apply_update(&appended_turns(&["t9"]));
    assert_eq!(t.selected(), Some(&kept));
    assert!(shows_last_row(&mut t, 24));
}

#[test]
fn a_growing_last_turn_keeps_the_view_at_the_end() {
    let mut t = at_the_end();
    let grown = arc(reply(
        "t8",
        &format!("{}\nA streamed message grew.", tall_text()),
    ));
    t.apply_update(&ModelUpdate::Updated(vec![grown]));
    assert!(t.following());
    assert!(shows_last_row(&mut t, 24));
    assert_eq!(selected_turn(&t), "t8", "the selection stays on its turn");
    let text = text_rows(&draw(&mut t, 80, 24)).join("\n");
    assert!(text.contains("A streamed message grew."), "{text}");
}

#[test]
fn the_selection_moves_to_the_new_last_section_of_a_growing_turn() {
    let mut t = at_the_end();
    assert_eq!(t.selected(), Some(&last_section_of(&t, "t8")));
    let before = t.selected().cloned().unwrap();
    let grown = arc(with_id(
        blocks::assistant(
            None,
            vec![
                blocks::text("t1", "That is everything the camera exposes."),
                blocks::text("t2", "A second item arrived."),
            ],
            None,
        ),
        "t8",
    ));
    t.apply_update(&ModelUpdate::Updated(vec![grown]));
    let last = last_section_of(&t, "t8");
    assert_ne!(last, before, "the turn has a new last section");
    assert_eq!(t.selected(), Some(&last));
    assert!(shows_last_row(&mut t, 24));
    assert_eq!(t.new_sections(), 0);
}

#[test]
fn a_selection_not_on_the_last_section_stays_when_the_last_turn_grows() {
    let mut t = at_the_end();
    t.perform(up());
    let kept = t.selected().cloned().unwrap();
    let grown = arc(with_id(
        blocks::assistant(
            None,
            vec![
                blocks::text("t1", "That is everything the camera exposes."),
                blocks::text("t2", "A second item arrived."),
            ],
            None,
        ),
        "t8",
    ));
    t.apply_update(&ModelUpdate::Updated(vec![grown]));
    assert_eq!(t.selected(), Some(&kept));
    assert!(shows_last_row(&mut t, 24));
}

#[test]
fn an_update_that_brings_the_end_into_view_resumes_following() {
    let mut t = loading_transcript();
    let mut turns: Vec<Arc<Turn>> = (1..=6).map(|n| text_turn(&format!("r{n}"))).collect();
    turns.push(arc(reply("r7", &tall_text())));
    t.apply_update(&ModelUpdate::Appended(turns));
    t.apply_update(&ModelUpdate::Loaded);
    t.perform(end());
    assert!(t.following());
    t.perform(scroll_up());
    t.perform(scroll_up());
    assert!(!t.following());
    // The tall turn shrinks: the end is on screen without the user moving.
    t.apply_update(&ModelUpdate::Updated(vec![text_turn("r7")]));
    assert!(shows_last_row(&mut t, 24), "the end is in view");
    assert!(t.following());
    assert_eq!(t.new_sections(), 0);
    t.apply_update(&appended_turns(&["r8"]));
    assert!(shows_last_row(&mut t, 24));
    assert_eq!(t.new_sections(), 0);
}

#[test]
fn an_update_that_arrives_as_appended_with_a_known_id_also_follows() {
    let mut t = at_the_end();
    let grown = arc(reply("t8", &tall_text()));
    t.apply_update(&ModelUpdate::Appended(vec![grown]));
    assert_eq!(t.turn_count(), 8);
    assert!(shows_last_row(&mut t, 24));
}

#[test]
fn following_survives_a_resize() {
    let mut t = at_the_end();
    t.set_viewport(Size::new(60, 20));
    assert!(t.following());
    assert!(shows_last_row(&mut t, 20));
    t.set_viewport(Size::new(100, 30));
    assert!(t.following());
    assert!(shows_last_row(&mut t, 30));
}

// ---- Scrolled up ----

#[test]
fn scrolling_up_from_the_bottom_stops_following() {
    let mut t = at_the_end();
    t.perform(half_page_up());
    assert!(!t.following());
    assert_eq!(t.new_sections(), 0);
}

#[test]
fn scrolled_up_an_appended_turn_keeps_the_offset_and_the_selection_and_is_counted() {
    let mut t = at_the_end();
    t.perform(half_page_up());
    let offset = t.offset();
    let selected = t.selected().cloned();
    let rows = text_rows(&draw(&mut t, 80, 24));
    t.apply_update(&appended_turns(&["t9", "t10", "t11"]));
    assert!(!t.following());
    assert_eq!(t.offset(), offset);
    assert_eq!(t.selected().cloned(), selected);
    assert_eq!(
        text_rows(&draw(&mut t, 80, 24)),
        rows,
        "the view is unchanged"
    );
    assert_eq!(t.new_sections(), 3);
    t.apply_update(&appended_turns(&["t12"]));
    assert_eq!(t.new_sections(), 4);
    assert_eq!(t.offset(), offset);
}

#[test]
fn a_turn_with_several_sections_counts_them_all() {
    let mut t = at_the_end();
    t.perform(half_page_up());
    let turn = arc(with_id(
        blocks::assistant(
            None,
            vec![
                blocks::text("x1", "First."),
                blocks::text("x2", "Second."),
                blocks::text("x3", "Third."),
            ],
            None,
        ),
        "t9",
    ));
    t.apply_update(&ModelUpdate::Appended(vec![turn]));
    assert_eq!(t.new_sections(), 3);
}

#[test]
fn new_sections_inside_an_updated_last_turn_are_counted() {
    let mut t = at_the_end();
    t.perform(half_page_up());
    let one = |items: Vec<Item>| arc(with_id(blocks::assistant(None, items, None), "t9"));
    t.apply_update(&ModelUpdate::Appended(vec![one(vec![blocks::text(
        "x1", "First.",
    )])]));
    assert_eq!(t.new_sections(), 1);
    // A streamed message grows without a new section: no change.
    t.apply_update(&ModelUpdate::Updated(vec![one(vec![blocks::text(
        "x1",
        "First, longer.",
    )])]));
    assert_eq!(t.new_sections(), 1);
    // A new item in the same turn is a new section.
    t.apply_update(&ModelUpdate::Updated(vec![one(vec![
        blocks::text("x1", "First, longer."),
        blocks::text("x2", "Second."),
    ])]));
    assert_eq!(t.new_sections(), 2);
}

#[test]
fn new_sections_inside_the_last_turn_before_leaving_the_end_are_counted_too() {
    let mut t = at_the_end();
    t.perform(half_page_up());
    let grown = arc(with_id(
        blocks::assistant(
            None,
            vec![
                blocks::text("t1", "That is everything."),
                blocks::text("t2", "And one more thing."),
            ],
            None,
        ),
        "t8",
    ));
    t.apply_update(&ModelUpdate::Updated(vec![grown]));
    assert_eq!(t.new_sections(), 1);
}

#[test]
fn an_update_to_an_earlier_turn_is_not_new_content() {
    let mut t = at_the_end();
    t.perform(half_page_up());
    let earlier = arc(with_id(
        blocks::assistant(
            None,
            vec![
                blocks::text("t1", "Changed."),
                blocks::text("t2", "A second section."),
            ],
            None,
        ),
        "t7",
    ));
    t.apply_update(&ModelUpdate::Updated(vec![earlier]));
    assert_eq!(t.new_sections(), 0);
}

#[test]
fn g_jumps_to_the_end_resets_the_count_and_resumes_following() {
    let mut t = at_the_end();
    t.perform(half_page_up());
    t.apply_update(&appended_turns(&["t9", "t10", "t11"]));
    assert_eq!(t.new_sections(), 3);
    let msg = ch(&mut t, 'G');
    let last = last_section_of(&t, "t11");
    assert_eq!(
        msg,
        Some(Msg::NewSections {
            count: 0,
            selection: Some(last)
        }),
        "the key reports the reset, and the selection it moved"
    );
    assert!(t.following());
    assert_eq!(t.new_sections(), 0);
    assert!(shows_last_row(&mut t, 24));
    assert_eq!(t.selected(), Some(&last_section_of(&t, "t11")));
    // Following again: the next turn scrolls into view.
    t.apply_update(&appended_turns(&["t12"]));
    assert!(shows_last_row(&mut t, 24));
    assert_eq!(t.new_sections(), 0);
}

#[test]
fn the_end_key_does_the_same() {
    let mut t = at_the_end();
    t.perform(half_page_up());
    t.apply_update(&appended_turns(&["t9"]));
    let msg = press(&mut t, Key::End);
    assert_eq!(
        msg,
        Some(Msg::NewSections {
            count: 0,
            selection: Some(last_section_of(&t, "t9")),
        })
    );
    assert!(t.following());
}

#[test]
fn a_key_that_leaves_the_count_alone_reports_only_the_selection_it_moved() {
    let mut t = at_the_end();
    t.perform(half_page_up());
    t.apply_update(&appended_turns(&["t9"]));
    // `j` moves the selection: that is all it reports.
    let msg = ch(&mut t, 'j');
    assert_eq!(msg, t.selected().cloned().map(Msg::SelectionChanged));
    assert!(msg.is_some());
    assert_eq!(t.new_sections(), 1);
}

#[test]
fn scrolling_back_to_the_very_end_resumes_following() {
    let mut t = at_the_end();
    t.perform(scroll_up());
    t.perform(scroll_up());
    assert!(!t.following());
    t.apply_update(&appended_turns(&["t9"]));
    assert_eq!(t.new_sections(), 1);
    // Two rows up, one block more below: scroll until the end is in view.
    while !t.following() {
        let before = t.offset();
        t.perform(scroll_down());
        assert!(t.offset() > before, "the end is reachable");
    }
    assert!(shows_last_row(&mut t, 24));
    assert_eq!(t.new_sections(), 0);
}

#[test]
fn g_at_the_start_leaves_following() {
    let mut t = at_the_end();
    ch(&mut t, 'g');
    assert_eq!(t.offset(), 0);
    assert!(!t.following());
}

#[test]
fn a_selection_that_leaves_the_last_row_off_screen_stops_following() {
    let mut t = at_the_end();
    // Moving the selection up far enough scrolls the view up.
    for _ in 0..12 {
        t.perform(up());
    }
    assert!(!t.following());
    t.apply_update(&appended_turns(&["t9"]));
    assert_eq!(t.new_sections(), 1);
}

#[test]
fn counting_starts_when_the_user_leaves_the_end() {
    let mut t = at_the_end();
    t.apply_update(&appended_turns(&["t9"]));
    assert_eq!(t.new_sections(), 0, "followed, so not counted");
    t.perform(half_page_up());
    t.apply_update(&appended_turns(&["t10"]));
    assert_eq!(t.new_sections(), 1, "only what came after");
}

#[test]
fn the_count_and_following_can_be_queried() {
    let mut t = at_the_end();
    assert_eq!(
        t.query(FOLLOWING_ATTR).unwrap().into_attr(),
        AttrValue::Flag(true)
    );
    t.perform(half_page_up());
    t.apply_update(&appended_turns(&["t9", "t10"]));
    assert_eq!(
        t.query(FOLLOWING_ATTR).unwrap().into_attr(),
        AttrValue::Flag(false)
    );
    assert_eq!(
        t.query(NEW_SECTIONS_ATTR).unwrap().into_attr(),
        AttrValue::Length(2)
    );
}

// ---- The initial load ----

fn loading_transcript() -> Transcript {
    Transcript::new(blocks::theme(), blocks::formatter())
        .loading(true)
        .viewport(Size::new(80, 24))
}

#[test]
fn while_the_initial_load_streams_the_view_stays_at_the_top() {
    let mut t = loading_transcript();
    let turns = session();
    for batch in turns.chunks(2) {
        t.apply_update(&ModelUpdate::Appended(batch.to_vec()));
        assert_eq!(t.offset(), 0);
        assert!(!t.following());
        assert_eq!(t.new_sections(), 0, "the load is not new content");
    }
    assert_eq!(t.turn_count(), 8);
    assert_eq!(selected_turn(&t), "t1");
}

#[test]
fn after_loaded_a_tall_transcript_does_not_follow_and_stays_at_the_top() {
    let mut t = loading_transcript();
    t.apply_update(&ModelUpdate::Appended(session()));
    t.apply_update(&ModelUpdate::Loaded);
    assert!(!t.following());
    assert_eq!(t.offset(), 0);
    t.apply_update(&appended_turns(&["t9"]));
    assert_eq!(t.offset(), 0);
    assert_eq!(t.new_sections(), 1, "counted from Loaded on");
}

#[test]
fn after_loaded_a_transcript_whose_last_row_is_visible_follows() {
    let mut t = loading_transcript();
    t.apply_update(&ModelUpdate::Appended(vec![text_turn("t1")]));
    assert!(!t.following());
    t.apply_update(&ModelUpdate::Loaded);
    assert!(t.following());
    let tall = arc(reply("t2", &tall_text()));
    t.apply_update(&ModelUpdate::Appended(vec![tall]));
    assert!(shows_last_row(&mut t, 24));
}

#[test]
fn after_loaded_a_view_scrolled_to_the_end_by_the_user_follows() {
    let mut t = loading_transcript();
    t.apply_update(&ModelUpdate::Appended(session()));
    t.perform(end());
    assert!(!t.following(), "still loading");
    t.apply_update(&ModelUpdate::Loaded);
    assert!(t.following());
    t.apply_update(&appended_turns(&["t9"]));
    assert!(shows_last_row(&mut t, 24));
}

#[test]
fn a_reset_starts_a_load_again() {
    let mut t = at_the_end();
    t.apply_update(&ModelUpdate::Reset);
    t.apply_update(&ModelUpdate::Appended(session()));
    assert_eq!(t.offset(), 0);
    assert!(!t.following());
    t.apply_update(&ModelUpdate::Loaded);
    assert!(!t.following());
    assert_eq!(t.new_sections(), 0);
}

// ---- Updates in place ----

fn pending_call(result: Option<(&str, bool)>, status: ToolStatus) -> Arc<Turn> {
    arc(with_id(
        blocks::assistant(
            None,
            vec![
                blocks::text("x0", "Running it."),
                blocks::thinking("x1", Thinking::Text("Which tests?".into())),
                blocks::tool(
                    "c1",
                    "Run the tests",
                    json!({"command": "cargo test"}),
                    result,
                    status,
                ),
            ],
            None,
        ),
        "a1",
    ))
}

fn tool_section() -> SectionId {
    item_section("a1", "c1")
}

fn transcript_with_a_pending_call() -> Transcript {
    let mut turns = vec![
        arc(prompt("u1", "Run the tests.")),
        pending_call(None, ToolStatus::Pending),
    ];
    turns.extend((1..=10).map(|n| text_turn(&format!("r{n}"))));
    transcript_of(turns, 80, 14)
}

#[test]
fn an_arriving_tool_result_keeps_selection_folds_and_scroll_offset() {
    let mut t = transcript_with_a_pending_call();
    assert!(t.select(&tool_section()));
    t.set_folded(&tool_section(), false);
    t.set_folded(&item_section("a1", "x1"), true);
    t.content.folds.set_expanded(tool_section());
    t.scroll_to(3);
    let offset = t.offset();
    let new_before = t.new_sections();
    t.apply_update(&ModelUpdate::Updated(vec![pending_call(
        Some(("test result: ok. 12 passed", false)),
        ToolStatus::Ok,
    )]));
    assert_eq!(t.selected(), Some(&tool_section()));
    assert_eq!(t.is_folded(&tool_section()), Some(false));
    assert_eq!(t.is_folded(&item_section("a1", "x1")), Some(true));
    assert!(
        t.content.folds.is_expanded(&tool_section()),
        "expansion stays"
    );
    assert_eq!(t.offset(), offset);
    assert_eq!(t.new_sections(), new_before);
    let text = block_rows(&t, 1, 80).join("\n");
    assert!(text.contains("12 passed"), "the result is shown:\n{text}");
}

#[test]
fn an_update_of_a_visible_turn_does_not_move_the_viewport_when_scrolled_up() {
    let mut t = transcript_with_a_pending_call();
    t.scroll_to(2);
    t.perform(down());
    let selected = t.selected().cloned();
    let offset = t.offset();
    let changed = arc(prompt("u1", "Run the tests, please. All of them."));
    t.apply_update(&ModelUpdate::Updated(vec![changed]));
    assert_eq!(t.selected().cloned(), selected);
    assert_eq!(t.offset(), offset);
    assert!(!t.following());
}

/// The transcript rows of the tool call's block: where it starts, and where
/// its tool section starts and ends.
fn tool_rows(t: &mut Transcript) -> (usize, std::ops::Range<usize>) {
    let at = t.content.index[&TurnId::from("a1")];
    let top = t.measured(|cache, m| cache.top(at, m));
    let rows = t
        .content
        .section_rows(t.turn(at).unwrap(), 80)
        .into_iter()
        .find(|(id, _)| *id == tool_section())
        .unwrap()
        .1;
    (top, top + rows.start..top + rows.end)
}

fn result_of(lines: usize) -> ModelUpdate {
    ModelUpdate::Updated(vec![pending_call(
        Some((&numbered_lines(lines), false)),
        ToolStatus::Ok,
    )])
}

fn height_of_block(t: &mut Transcript, id: &str) -> usize {
    let at = t.content.index[&TurnId::from(id)];
    t.measured(|cache, m| cache.height(at, m))
}

#[test]
fn a_selected_section_that_grows_above_the_viewport_keeps_the_screen_still() {
    let mut t = transcript_with_a_pending_call();
    assert!(t.select(&tool_section()));
    // Unfolded, the call shows its result, so the result changes its height.
    t.set_folded(&tool_section(), false);
    let (_, rows) = tool_rows(&mut t);
    // The whole block is above the top row of the viewport.
    let block_end = {
        let (top, _) = tool_rows(&mut t);
        top + height_of_block(&mut t, "a1")
    };
    t.scroll_to(block_end + 2);
    assert!(t.offset() >= rows.end);
    assert_eq!(t.selected(), Some(&tool_section()));
    let rows_before = text_rows(&draw(&mut t, 80, 14));
    let height = height_of_block(&mut t, "a1");
    t.apply_update(&result_of(6));
    assert_ne!(
        height_of_block(&mut t, "a1"),
        height,
        "the section's height changed"
    );
    assert_eq!(t.selected(), Some(&tool_section()));
    assert_eq!(text_rows(&draw(&mut t, 80, 14)), rows_before);
}

#[test]
fn a_selected_section_above_the_top_row_of_a_block_that_straddles_the_viewport_keeps_the_screen_still()
 {
    let mut t = transcript_with_a_pending_call();
    assert!(t.select(&tool_section()));
    t.set_folded(&tool_section(), false);
    let (_, rows) = tool_rows(&mut t);
    // The selected section ends above the top row, but its block goes on: the
    // top row is the blank row or the bottom border after it.
    t.scroll_to(rows.end);
    assert_eq!(t.offset(), rows.end);
    let block = t.content.index[&TurnId::from("a1")];
    assert!(
        t.visible_blocks().contains(&block),
        "the block straddles the top"
    );
    let rows_before = text_rows(&draw(&mut t, 80, 14));
    let height = height_of_block(&mut t, "a1");
    t.apply_update(&result_of(6));
    assert_ne!(
        height_of_block(&mut t, "a1"),
        height,
        "the section's height changed"
    );
    assert_eq!(t.selected(), Some(&tool_section()));
    assert_eq!(text_rows(&draw(&mut t, 80, 14)), rows_before);
}
