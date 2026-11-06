//! The mouse in the transcript (task 035): the wheel, and clicks on sections,
//! summary lines and `… N more lines` rows. Positions are computed from the
//! layout cache, never read off the screen.

use std::ops::Range;

use ratatui::layout::Size;
use tuirealm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use super::super::fixtures::*;
use super::super::*;
use super::keys::{ch, press};
use super::{item_section, section_id};
use crate::app::RoutedMouse;
use crate::components::turn_block::SectionKey;
use crate::model::ItemId;
use tuirealm::event::Key;

const LEFT: MouseEventKind = MouseEventKind::Down(MouseButton::Left);

fn routed(t: &Transcript, kind: MouseEventKind, column: u16, row: u16) -> Event<UserEvent> {
    Event::User(UserEvent::Mouse(RoutedMouse {
        kind,
        column,
        row,
        area: t.viewport.expect("a viewport"),
    }))
}

fn wheel(t: &mut Transcript, down: bool) -> Option<Msg> {
    let kind = if down {
        MouseEventKind::ScrollDown
    } else {
        MouseEventKind::ScrollUp
    };
    t.on(&routed(t, kind, 10, 3))
}

fn click_at(t: &mut Transcript, column: u16, row: u16) -> Option<Msg> {
    t.on(&routed(t, LEFT, column, row))
}

/// The rows of section `id` in the transcript, from the layout cache.
fn rows_of(t: &mut Transcript, id: &SectionId) -> Range<usize> {
    for block in 0..t.turn_count() {
        let top = t.measured(|cache, m| cache.top(block, m));
        let rows = t
            .content
            .section_rows(t.turn(block).unwrap(), t.nav_width());
        if let Some((_, r)) = rows.into_iter().find(|(i, _)| i == id) {
            return top + r.start..top + r.end;
        }
    }
    panic!("no section {id:?}");
}

/// The row of the viewport that shows row `dy` of section `id`.
fn screen_row(t: &mut Transcript, id: &SectionId, dy: usize) -> u16 {
    let row = rows_of(t, id).start + dy;
    let height = usize::from(t.viewport.unwrap().height);
    assert!(
        (t.offset()..t.offset() + height).contains(&row),
        "row {row} of {id:?} is not on screen (offset {})",
        t.offset()
    );
    u16::try_from(row - t.offset()).unwrap()
}

/// A left press on row `dy` of section `id`, in column `column`.
fn click(t: &mut Transcript, id: &SectionId, dy: usize, column: u16) -> Option<Msg> {
    let row = screen_row(t, id, dy);
    click_at(t, column, row)
}

fn sections(t: &Transcript, block: usize) -> Vec<SectionId> {
    t.sections_of(block)
}

fn is_folded(t: &Transcript, id: &SectionId) -> bool {
    t.is_folded(id) == Some(true)
}

/// The fixture session in 80x60 (everything of the first blocks is on
/// screen): the thinking section of the second block, folded.
fn thinking(t: &Transcript) -> SectionId {
    sections(t, 1)[1].clone()
}

// ---- The wheel ----

#[test]
fn a_wheel_step_scrolls_three_rows() {
    let mut t = session_transcript(80, 10);
    assert_eq!(wheel(&mut t, true), Some(Msg::TranscriptChanged));
    assert_eq!(t.offset(), 3);
    wheel(&mut t, true);
    assert_eq!(t.offset(), 6);
    assert_eq!(wheel(&mut t, false), Some(Msg::TranscriptChanged));
    assert_eq!(t.offset(), 3);
}

#[test]
fn the_wheel_goes_the_same_way_as_the_page_keys() {
    let mut by_wheel = session_transcript(80, 10);
    let mut by_cmd = session_transcript(80, 10);
    wheel(&mut by_wheel, true);
    by_cmd.perform(Cmd::Custom(WHEEL_DOWN));
    assert_eq!(by_wheel.presentation(), by_cmd.presentation());
    assert_eq!(WHEEL_ROWS, 3);
}

#[test]
fn the_wheel_stops_at_the_ends_and_says_nothing_there() {
    let mut t = session_transcript(80, 10);
    assert_eq!(wheel(&mut t, false), None, "at the top");
    assert_eq!(t.offset(), 0);
    t.perform(Cmd::GoTo(Position::End));
    let end = t.offset();
    assert_eq!(wheel(&mut t, true), None, "at the end");
    assert_eq!(t.offset(), end);
    // Fewer than three rows to go: scrolls to the end and no further.
    t.perform(Cmd::Scroll(Direction::Up));
    assert_eq!(wheel(&mut t, true), Some(Msg::TranscriptChanged));
    assert_eq!(t.offset(), end);
}

#[test]
fn a_selection_that_left_the_viewport_moves_to_the_topmost_fully_visible_section() {
    let mut t = session_transcript(80, 10);
    let first = t.selected().cloned().unwrap();
    for _ in 0..4 {
        wheel(&mut t, true);
        let selected = t.selected().cloned().unwrap();
        let (top, bottom) = (t.offset(), t.offset() + 10);
        let mut all: Vec<(SectionId, Range<usize>)> = Vec::new();
        for block in 0..t.turn_count() {
            for id in sections(&t, block) {
                let rows = rows_of(&mut t, &id);
                all.push((id, rows));
            }
        }
        let fully = |rows: &Range<usize>| rows.start >= top && rows.end <= bottom;
        let selected_rows = all.iter().find(|(id, _)| *id == selected).unwrap();
        let topmost = all.iter().find(|(_, rows)| fully(rows)).map(|(id, _)| id);
        if !fully(&selected_rows.1) {
            assert_eq!(topmost, None, "offset {top}: a section is fully visible");
        } else if selected != first {
            assert_eq!(Some(&selected), topmost, "offset {top}");
        }
    }
    assert_ne!(t.selected(), Some(&first), "the first section is long gone");
}

#[test]
fn a_selection_that_stays_in_the_viewport_stays() {
    let mut t = session_transcript(80, 40);
    let text = sections(&t, 1)[0].clone();
    assert!(t.select(&text));
    wheel(&mut t, true);
    assert_eq!(t.offset(), 3);
    assert_eq!(t.selected(), Some(&text));
}

#[test]
fn the_wheel_scrolls_without_moving_the_selection_into_view() {
    // The wheel never scrolls the selection back: it follows the view.
    let mut t = session_transcript(80, 10);
    let first = t.selected().cloned().unwrap();
    for _ in 0..3 {
        wheel(&mut t, true);
    }
    assert_eq!(t.offset(), 9);
    assert_ne!(t.selected(), Some(&first));
}

#[test]
fn a_wheel_up_stops_following_and_a_wheel_down_to_the_end_resumes_it() {
    let mut t = session_transcript(80, 24);
    t.perform(Cmd::GoTo(Position::End));
    assert!(t.following());
    assert_eq!(wheel(&mut t, false), Some(Msg::TranscriptChanged));
    assert!(!t.following());
    // One step down reaches the end again (3 rows up, 3 rows down).
    wheel(&mut t, true);
    assert!(t.following());
}

#[test]
fn new_sections_are_counted_after_a_wheel_up_and_reset_at_the_end() {
    let mut t = session_transcript(80, 24);
    t.perform(Cmd::GoTo(Position::End));
    wheel(&mut t, false);
    assert!(!t.following());
    t.attr(
        MODEL_UPDATE_ATTR,
        model_update_value(ModelUpdate::Appended(vec![arc(reply("t9", "More."))])),
    );
    assert_eq!(t.new_sections(), 1);
    // The new turn pushed the end away: a step down is still short of it.
    assert_eq!(wheel(&mut t, true), Some(Msg::TranscriptChanged));
    assert!(!t.following());
    assert_eq!(t.new_sections(), 1);
    // Scrolls down to the end: following resumes and the count resets.
    let mut answer = None;
    for _ in 0..5 {
        answer = wheel(&mut t, true).or(answer);
    }
    assert!(t.following());
    assert_eq!(t.new_sections(), 0);
    assert_eq!(answer, Some(Msg::NewSections(0)));
}

// ---- Clicks on sections ----

#[test]
fn a_click_on_the_summary_line_toggles_the_fold_and_selects_the_section() {
    let mut t = session_transcript(80, 60);
    let id = thinking(&t);
    assert!(is_folded(&t, &id));
    assert_ne!(t.selected(), Some(&id));
    assert_eq!(click(&mut t, &id, 0, 12), Some(Msg::TranscriptChanged));
    assert_eq!(t.selected(), Some(&id));
    assert!(!is_folded(&t, &id), "unfolded");
    assert_eq!(click(&mut t, &id, 0, 12), Some(Msg::TranscriptChanged));
    assert!(is_folded(&t, &id), "folded again");
    assert_eq!(t.selected(), Some(&id));
}

#[test]
fn a_click_on_the_marker_cells_toggles_too() {
    for column in [2, 3] {
        let mut t = session_transcript(80, 60);
        let id = thinking(&t);
        click(&mut t, &id, 0, column);
        assert!(!is_folded(&t, &id), "column {column}");
    }
}

#[test]
fn a_click_on_the_first_row_of_a_wrapped_summary_toggles_it() {
    // The first prompt is folded and wraps over two rows.
    let mut t = session_transcript(80, 60);
    let id = t.selected().cloned().unwrap();
    assert!(rows_of(&mut t, &id).len() >= 2);
    assert!(is_folded(&t, &id));
    click(&mut t, &id, 0, 20);
    assert!(!is_folded(&t, &id));
}

#[test]
fn a_click_on_the_body_selects_the_section_without_toggling() {
    let mut t = session_transcript(80, 60);
    let id = thinking(&t);
    t.set_folded(&id, false);
    assert!(rows_of(&mut t, &id).len() > 1, "the body has rows");
    assert_eq!(click(&mut t, &id, 1, 10), Some(Msg::TranscriptChanged));
    assert_eq!(t.selected(), Some(&id));
    assert!(!is_folded(&t, &id), "still unfolded");
    // And a folded section's second row, where its summary wraps.
    let mut t = session_transcript(80, 60);
    let prompt = t.selected().cloned().unwrap();
    t.perform(Cmd::Move(Direction::Down));
    assert_ne!(t.selected(), Some(&prompt));
    assert_eq!(click(&mut t, &prompt, 1, 10), Some(Msg::TranscriptChanged));
    assert_eq!(t.selected(), Some(&prompt));
    assert!(
        is_folded(&t, &prompt),
        "the second summary row is a body row"
    );
}

#[test]
fn a_click_on_a_section_that_cannot_fold_only_selects_it() {
    let mut t = session_transcript(80, 60);
    let text = sections(&t, 1)[0].clone();
    assert_eq!(t.is_folded(&text), None, "assistant text does not fold");
    assert_eq!(click(&mut t, &text, 0, 10), Some(Msg::TranscriptChanged));
    assert_eq!(t.selected(), Some(&text));
    // Already selected: nothing changed.
    assert_eq!(click(&mut t, &text, 1, 10), None);
}

#[test]
fn a_click_does_not_scroll_and_the_section_stays_under_the_pointer() {
    let mut t = session_transcript(80, 10);
    wheel(&mut t, true);
    wheel(&mut t, true);
    let offset = t.offset();
    // The section on the row the pointer is on, found by its rows.
    let row = 6;
    let mut under = None;
    for block in 0..t.turn_count() {
        for id in sections(&t, block) {
            if rows_of(&mut t, &id).contains(&(offset + row)) {
                under = Some(id);
            }
        }
    }
    let under = under.expect("a section is under row 6");
    click_at(&mut t, 10, u16::try_from(row).unwrap());
    assert_eq!(t.offset(), offset);
    assert_eq!(t.selected(), Some(&under));
}

#[test]
fn folding_by_click_keeps_the_summary_row_where_it_was() {
    let mut t = session_transcript(80, 60);
    let id = thinking(&t);
    let row = screen_row(&mut t, &id, 0);
    click(&mut t, &id, 0, 10);
    assert_eq!(screen_row(&mut t, &id, 0), row);
    click(&mut t, &id, 0, 10);
    assert_eq!(screen_row(&mut t, &id, 0), row);
}

#[test]
fn a_click_on_the_borders_the_padding_and_the_gaps_selects_nothing() {
    let mut t = session_transcript(80, 60);
    let id = thinking(&t);
    let before = t.presentation();
    let summary = screen_row(&mut t, &id, 0);
    // The left and right border, the blank cell inside the left border, and
    // the padding before the right border.
    for column in [0, 1, 76, 77, 78, 79] {
        assert_eq!(click_at(&mut t, column, summary), None, "column {column}");
    }
    // The blank row under the summary line, between two sections.
    let gap = summary + 1;
    assert_eq!(click_at(&mut t, 10, gap), None, "the gap");
    // The top border (the title) and the bottom border of the second block.
    let (first, last) = (sections(&t, 1)[0].clone(), sections(&t, 1).pop().unwrap());
    let block = rows_of(&mut t, &first).start - 2;
    let top = u16::try_from(block - t.offset()).unwrap();
    assert_eq!(click_at(&mut t, 10, top), None, "the title row");
    assert_eq!(
        click_at(&mut t, 10, top + 1),
        None,
        "the blank row under it"
    );
    let end = rows_of(&mut t, &last).end;
    let bottom = u16::try_from(end + 1 - t.offset()).unwrap();
    assert_eq!(click_at(&mut t, 10, bottom), None, "the bottom border");
    assert_eq!(t.presentation(), before);
}

#[test]
fn a_click_below_the_last_block_selects_nothing() {
    let mut t = transcript_of(vec![arc(prompt("u1", "Hello"))], 80, 20);
    let before = t.presentation();
    assert_eq!(click_at(&mut t, 10, 15), None);
    assert_eq!(t.presentation(), before);
}

#[test]
fn a_click_before_the_viewport_is_announced_does_nothing() {
    let mut t =
        Transcript::new(super::blocks::theme(), super::blocks::formatter()).with_turns(session());
    let event = Event::User(UserEvent::Mouse(RoutedMouse {
        kind: LEFT,
        column: 10,
        row: 3,
        area: Size::new(80, 24),
    }));
    assert_eq!(t.on(&event), None);
    assert_eq!(wheel_event(&mut t), None);
}

fn wheel_event(t: &mut Transcript) -> Option<Msg> {
    t.on(&Event::User(UserEvent::Mouse(RoutedMouse {
        kind: MouseEventKind::ScrollDown,
        column: 10,
        row: 3,
        area: Size::new(80, 24),
    })))
}

#[test]
fn other_buttons_kinds_and_the_raw_event_do_nothing() {
    let mut t = session_transcript(80, 60);
    let id = thinking(&t);
    let row = screen_row(&mut t, &id, 0);
    let before = t.presentation();
    for kind in [
        MouseEventKind::Down(MouseButton::Right),
        MouseEventKind::Down(MouseButton::Middle),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Moved,
        MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight,
    ] {
        assert_eq!(t.on(&routed(&t, kind, 10, row)), None, "{kind:?}");
    }
    // The raw event arrives in screen coordinates: it is not for us.
    let raw = |kind| {
        Event::Mouse(MouseEvent {
            kind,
            modifiers: KeyModifiers::NONE,
            column: 10,
            row,
        })
    };
    assert_eq!(t.on(&raw(LEFT)), None);
    assert_eq!(t.on(&raw(MouseEventKind::ScrollDown)), None);
    assert_eq!(t.presentation(), before);
}

// ---- Truncation rows ----

fn tool_id() -> SectionId {
    item_section("a1", "c1")
}

fn more_id() -> SectionId {
    section_id("a1", SectionKey::More(ItemId::from("c1")))
}

fn lines_shown(t: &Transcript) -> usize {
    block_rows(t, 0, t.viewport.unwrap().width)
        .iter()
        .filter(|row| {
            let body = row.trim_matches(['│', '▒', ' ']);
            body.strip_prefix("line ")
                .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
        })
        .count()
}

/// The 1434 line result, with the viewport scrolled to its `… more lines`
/// row.
fn at_the_more_row() -> Transcript {
    let mut t = long_output_transcript(80, 24);
    let more = rows_of(&mut t, &more_id());
    t.scroll_to(more.start - 10);
    t
}

#[test]
fn a_click_on_the_more_row_expands_the_output_and_selects_the_tool_call() {
    let mut t = at_the_more_row();
    assert_eq!(lines_shown(&t), 200);
    assert_eq!(t.selected(), Some(&tool_id()));
    assert_eq!(
        click(&mut t, &more_id(), 0, 12),
        Some(Msg::TranscriptChanged)
    );
    assert_eq!(lines_shown(&t), 1434);
    assert_eq!(t.selected(), Some(&tool_id()));
    assert!(
        block_rows(&t, 0, 80)
            .iter()
            .all(|row| !row.contains("more lines"))
    );
    assert_eq!(t.is_folded(&tool_id()), Some(false), "not folded by it");
}

#[test]
fn a_click_on_the_tool_calls_last_output_line_does_not_expand_it() {
    let mut t = at_the_more_row();
    let tool = rows_of(&mut t, &tool_id());
    let last = tool.len() - 1;
    click(&mut t, &tool_id(), last, 12);
    assert_eq!(lines_shown(&t), 200);
    assert_eq!(t.is_folded(&tool_id()), Some(false));
}

#[test]
fn a_click_on_the_more_row_is_the_same_as_enter_on_it() {
    let mut by_click = at_the_more_row();
    click(&mut by_click, &more_id(), 0, 12);
    let mut by_key = at_the_more_row();
    assert!(by_key.select(&more_id()));
    press(&mut by_key, Key::Enter);
    assert_eq!(by_click.selected(), by_key.selected());
    assert_eq!(lines_shown(&by_click), lines_shown(&by_key));
    assert_eq!(by_click.content.folds, by_key.content.folds);
}

// ---- The search's temporary unfolding ----

fn searching() -> Transcript {
    transcript_of(search_session(), 100, 30)
}

fn search(t: &mut Transcript, query: &str) {
    t.attr(
        SEARCH_QUERY_ATTR,
        search_start_value(SearchStart {
            query: query.to_owned(),
            generation: 1,
        }),
    );
    let answer = payload_from_value::<crate::transcript_search::SearchRequest>(
        &t.query(SEARCH_REQUEST_ATTR).unwrap().into_attr(),
    )
    .unwrap()
    .run();
    t.attr(SEARCH_RESULTS_ATTR, search_results_value(answer));
}

#[test]
fn a_click_on_the_summary_of_a_section_the_search_unfolded_makes_it_the_users() {
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    assert_eq!(
        t.is_folded(&tool_id()),
        Some(false),
        "the search unfolded it"
    );
    click(&mut t, &tool_id(), 0, 10);
    assert_eq!(t.is_folded(&tool_id()), Some(false), "kept, as Enter does");
    // The selection moving away no longer refolds it.
    ch(&mut t, 'j');
    assert_ne!(t.selected(), Some(&tool_id()));
    assert_eq!(t.is_folded(&tool_id()), Some(false));
}

#[test]
fn a_click_on_another_section_refolds_what_the_search_unfolded() {
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    assert_eq!(t.is_folded(&tool_id()), Some(false));
    let other = item_section("a1", "x1");
    click(&mut t, &other, 0, 10);
    assert_eq!(t.selected(), Some(&other));
    assert_eq!(t.is_folded(&tool_id()), Some(true), "refolded");
}

// ---- The rule and the hint of a subagent's title row ----

use super::subagent::with_subagents_at;
use crate::components::turn_block::fixtures as blocks;

/// The transcript of `with_subagents` and its first subagent call `s1`.
fn subagent_rig(width: u16) -> (Transcript, SectionId) {
    let t = with_subagents_at(width, 40);
    (t, item_section("a1", "s1"))
}

#[test]
fn a_click_on_the_rule_or_the_hint_of_a_folded_subagent_row_selects_and_unfolds_it() {
    // At 100 columns the row is `Explore the repo (Explore)  ────  a to open
    // transcript`: the rule is columns 31 to 75, the hint 78 to 97.
    for column in [31, 50, 75, 78, 90, 96, 97] {
        let (mut t, id) = subagent_rig(100);
        assert!(is_folded(&t, &id));
        assert_eq!(click(&mut t, &id, 0, column), Some(Msg::TranscriptChanged));
        assert_eq!(t.selected(), Some(&id), "column {column}");
        assert!(!is_folded(&t, &id), "column {column}");
    }
}

#[test]
fn a_click_on_the_blank_cell_before_the_border_does_nothing() {
    let (mut t, id) = subagent_rig(100);
    let before = t.presentation();
    assert_eq!(click(&mut t, &id, 0, 98), None);
    assert_eq!(click(&mut t, &id, 0, 99), None);
    assert!(is_folded(&t, &id));
    assert_eq!(t.presentation(), before);
}

#[test]
fn the_padding_of_every_other_row_still_does_nothing() {
    let (mut t, id) = subagent_rig(100);
    t.set_folded(&id, false);
    let before = t.presentation();
    let rows = rows_of(&mut t, &id);
    // The body rows of the unfolded subagent and a shell call's row.
    for dy in 1..rows.len() {
        for column in [96, 97, 98] {
            assert_eq!(click(&mut t, &id, dy, column), None, "row {dy}, {column}");
        }
    }
    let shell = item_section("a1", "sh");
    for column in [96, 97, 98] {
        assert_eq!(click(&mut t, &shell, 0, column), None, "shell, {column}");
    }
    assert_eq!(t.presentation(), before);
}

#[test]
fn a_description_that_fills_the_text_area_has_no_padding_to_click_on() {
    // At 40 columns the text area is 32 cells: `d` x 22 and ` (Explore)`
    // fill it, so the row has neither rule nor hint.
    let call = blocks::tool_of(
        crate::model::ToolKind::Subagent,
        "s1",
        &"d".repeat(22),
        serde_json::json!({"subagent_type": "Explore", "prompt": "Look"}),
        Some(("Done.", false)),
        crate::model::ToolStatus::Ok,
    );
    let turn = arc(with_id(blocks::assistant(None, vec![call], None), "a1"));
    let mut t = transcript_of(vec![turn], 40, 20);
    let id = item_section("a1", "s1");
    let before = t.presentation();
    for column in [36, 37, 38] {
        assert_eq!(click(&mut t, &id, 0, column), None, "column {column}");
    }
    assert_eq!(t.presentation(), before);
    assert!(is_folded(&t, &id));
    // The text itself still toggles the fold.
    assert_eq!(click(&mut t, &id, 0, 35), Some(Msg::TranscriptChanged));
    assert!(!is_folded(&t, &id));
}

#[test]
fn a_click_on_the_hints_own_row_selects_the_section_and_leaves_it_unfolded() {
    // At 40 columns the unfolded hint has no room beside the description and
    // is on a row of its own: `────────────  a to open transcript`.
    let (mut t, id) = subagent_rig(40);
    t.set_folded(&id, false);
    let other = item_section("a1", "sh");
    assert!(t.select(&other));
    for column in [4, 20, 37] {
        assert!(t.select(&other));
        assert_eq!(
            click(&mut t, &id, 1, column),
            Some(Msg::TranscriptChanged),
            "{column}"
        );
        assert_eq!(t.selected(), Some(&id), "column {column}");
        assert!(!is_folded(&t, &id), "still unfolded at {column}");
    }
    // Column W - 2 of that row is blank padding.
    assert!(t.select(&other));
    assert_eq!(click(&mut t, &id, 1, 38), None);
    assert_eq!(t.selected(), Some(&other));
}
