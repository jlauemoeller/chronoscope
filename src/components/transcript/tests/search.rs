//! Searching the transcript (SPEC §6.2, task 024): the attribute exchange
//! with the App, the jumps, the temporary unfolding, the highlights.

use tuirealm::command::Cmd;

use super::super::fixtures::*;
use super::super::*;
use super::keys::{ch, press};
use super::{down, item_section, section_id};
use crate::components::turn_block::SectionKey;
use crate::components::turn_block::fixtures as blocks;
use crate::model::{ByteSource, ItemId, LineFormat, OutputSource, ToolOutput, ToolStatus};
use crate::transcript_search::{SearchMatches, SearchPosition, SearchRequest};
use ratatui::layout::Size;
use serde_json::json;
use tuirealm::event::Key;

const WIDTH: u16 = 100;
const HEIGHT: u16 = 30;

fn tool() -> SectionId {
    item_section("a1", "c1")
}

fn text_x1() -> SectionId {
    item_section("a1", "x1")
}

fn position(t: &Transcript) -> Option<SearchPosition> {
    let value = t.query(SEARCH_POSITION_ATTR).unwrap().into_attr();
    payload_from_value::<Option<SearchPosition>>(&value).unwrap()
}

fn shown(t: &Transcript) -> Option<String> {
    position(t).map(|p| p.to_string())
}

fn request(t: &Transcript) -> Option<SearchRequest> {
    let value = t.query(SEARCH_REQUEST_ATTR)?.into_attr();
    payload_from_value::<SearchRequest>(&value)
}

/// Plays the App: hands the transcript the query and the answer to the
/// request it makes.
fn search_with(t: &mut Transcript, generation: u64, query: &str) -> Option<String> {
    t.attr(
        SEARCH_QUERY_ATTR,
        search_start_value(SearchStart {
            query: query.to_owned(),
            generation,
        }),
    );
    let answer = request(t)
        .expect("the transcript waits for an answer")
        .run();
    t.attr(SEARCH_RESULTS_ATTR, search_results_value(answer));
    shown(t)
}

fn search(t: &mut Transcript, query: &str) -> Option<String> {
    search_with(t, 1, query)
}

fn searching() -> Transcript {
    transcript_of(search_session(), WIDTH, HEIGHT)
}

fn unfolded(t: &Transcript, id: &SectionId) -> bool {
    t.is_folded(id) == Some(false)
}

fn folded(t: &Transcript, id: &SectionId) -> bool {
    t.is_folded(id) == Some(true)
}

/// The text of the cells drawn with the `search_match` background, row by
/// row, runs of cells joined.
fn highlighted(t: &mut Transcript) -> Vec<String> {
    let bg = blocks::theme().role(crate::theme::Role::Primary);
    let size = t.viewport.unwrap();
    let (width, height) = (size.width, size.height);
    let buf = draw(t, width, height);
    let mut found = Vec::new();
    for y in 0..height {
        let mut run = String::new();
        for x in 0..width {
            let cell = &buf[(x, y)];
            if cell.bg == bg && cell.symbol() != " " {
                run.push_str(cell.symbol());
            } else if !run.is_empty() {
                found.push(std::mem::take(&mut run));
            }
        }
        if !run.is_empty() {
            found.push(run);
        }
    }
    found
}

fn rows(t: &mut Transcript) -> String {
    let size = t.viewport.unwrap();
    text_rows(&draw(t, size.width, size.height)).join("\n")
}

// ---- The exchange with the App ----

#[test]
fn a_query_makes_a_request_with_a_snapshot_of_the_turns() {
    let mut t = searching();
    assert!(request(&t).is_none(), "nothing is waited for yet");
    t.attr(
        SEARCH_QUERY_ATTR,
        search_start_value(SearchStart {
            query: "usb".to_owned(),
            generation: 4,
        }),
    );
    let req = request(&t).unwrap();
    assert_eq!((req.generation, req.query.as_str()), (4, "usb"));
    assert_eq!(req.corpus.turns().len(), 4);
    assert!(req.corpus.show_system());
    assert_eq!(position(&t), None, "no answer yet");
}

#[test]
fn the_snapshot_carries_whether_system_content_is_shown() {
    let mut t = searching();
    t.attr(SHOW_SYSTEM_ATTR, AttrValue::Flag(false));
    t.attr(
        SEARCH_QUERY_ATTR,
        search_start_value(SearchStart {
            query: "x".to_owned(),
            generation: 1,
        }),
    );
    assert!(!request(&t).unwrap().corpus.show_system());
}

#[test]
fn an_empty_query_waits_for_nothing() {
    let mut t = searching();
    t.attr(
        SEARCH_QUERY_ATTR,
        search_start_value(SearchStart {
            query: String::new(),
            generation: 1,
        }),
    );
    assert!(request(&t).is_none());
}

#[test]
fn an_answer_to_another_generation_is_dropped() {
    let mut t = searching();
    t.attr(
        SEARCH_QUERY_ATTR,
        search_start_value(SearchStart {
            query: "usb".to_owned(),
            generation: 5,
        }),
    );
    let mut answer = request(&t).unwrap();
    answer.generation = 4;
    t.attr(SEARCH_RESULTS_ATTR, search_results_value(answer.run()));
    assert_eq!(position(&t), None, "the answer to 4 is not for 5");
    assert!(highlighted(&mut t).is_empty());
    // The right one is taken.
    answer.generation = 5;
    t.attr(SEARCH_RESULTS_ATTR, search_results_value(answer.run()));
    assert_eq!(shown(&t).as_deref(), Some("1/5"));
}

#[test]
fn an_answer_nobody_waits_for_is_dropped() {
    let mut t = searching();
    t.attr(
        SEARCH_RESULTS_ATTR,
        search_results_value(SearchMatches::new(1, Vec::new())),
    );
    assert_eq!(position(&t), None);
    // And a second answer to the same request.
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/5"));
    t.attr(
        SEARCH_RESULTS_ATTR,
        search_results_value(SearchMatches::new(1, Vec::new())),
    );
    assert_eq!(shown(&t).as_deref(), Some("1/5"));
}

#[test]
fn an_answer_after_a_reset_is_dropped() {
    let mut t = searching();
    t.attr(
        SEARCH_QUERY_ATTR,
        search_start_value(SearchStart {
            query: "usb".to_owned(),
            generation: 1,
        }),
    );
    let answer = request(&t).unwrap().run();
    t.apply_update(&ModelUpdate::Reset);
    t.apply_update(&ModelUpdate::Appended(search_session()));
    t.attr(SEARCH_RESULTS_ATTR, search_results_value(answer));
    assert_eq!(position(&t), None);
    assert!(highlighted(&mut t).is_empty());
}

#[test]
fn a_reset_clears_the_search() {
    let mut t = searching();
    search(&mut t, "usb");
    assert!(position(&t).is_some());
    t.apply_update(&ModelUpdate::Reset);
    assert_eq!(position(&t), None);
    assert!(request(&t).is_none());
}

// ---- Jumping ----

#[test]
fn the_search_jumps_to_the_first_match_at_or_after_the_selection() {
    let mut t = searching();
    assert_eq!(t.selected(), Some(&section_id("u1", SectionKey::Prompt)));
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/5"));
    assert_eq!(t.selected(), Some(&text_x1()));
}

#[test]
fn it_starts_at_the_selected_section_when_it_holds_a_match() {
    let mut t = searching();
    // Select the prompt `u2`, which holds the third match.
    while t.selected() != Some(&section_id("u2", SectionKey::Prompt)) {
        t.perform(down());
    }
    assert_eq!(search(&mut t, "usb").as_deref(), Some("3/5"));
    assert_eq!(t.selected(), Some(&section_id("u2", SectionKey::Prompt)));
}

#[test]
fn it_wraps_to_the_first_match_when_none_is_at_or_after_the_selection() {
    let mut t = searching();
    t.perform(Cmd::GoTo(Position::End));
    assert_eq!(t.selected(), Some(&item_section("a2", "t1")));
    // Select the last section, then look for something only before it.
    assert_eq!(search(&mut t, "camera").as_deref(), Some("1/1"));
    assert_eq!(t.selected(), Some(&section_id("u1", SectionKey::Prompt)));
}

#[test]
fn no_matches_is_zero_of_zero_and_the_selection_stays() {
    let mut t = searching();
    let before = t.selected().cloned();
    assert_eq!(search(&mut t, "no such text").as_deref(), Some("0/0"));
    assert_eq!(t.selected().cloned(), before);
    assert!(highlighted(&mut t).is_empty());
    assert_eq!(ch(&mut t, 'n'), None, "nothing to step to");
    assert_eq!(ch(&mut t, 'N'), None);
    assert_eq!(shown(&t).as_deref(), Some("0/0"));
}

#[test]
fn n_and_capital_n_cycle_through_the_matches_with_wrap_around() {
    let mut t = searching();
    search(&mut t, "usb");
    let mut seen = vec![shown(&t).unwrap()];
    for _ in 0..5 {
        assert!(matches!(
            ch(&mut t, 'n'),
            Some(Msg::SearchPosition(Some(_)))
        ));
        seen.push(shown(&t).unwrap());
    }
    assert_eq!(seen, ["1/5", "2/5", "3/5", "4/5", "5/5", "1/5"]);
    let mut back = Vec::new();
    for _ in 0..5 {
        ch(&mut t, 'N');
        back.push(shown(&t).unwrap());
    }
    assert_eq!(back, ["5/5", "4/5", "3/5", "2/5", "1/5"]);
}

#[test]
fn a_key_reports_the_position_as_a_message() {
    let mut t = searching();
    search(&mut t, "usb");
    assert_eq!(
        ch(&mut t, 'n'),
        Some(Msg::SearchPosition(Some(SearchPosition {
            current: 2,
            total: 5
        })))
    );
}

#[test]
fn each_match_selects_its_section() {
    let mut t = searching();
    search(&mut t, "usb");
    let mut sections = vec![t.selected().cloned().unwrap()];
    for _ in 0..4 {
        ch(&mut t, 'n');
        sections.push(t.selected().cloned().unwrap());
    }
    assert_eq!(
        sections,
        [
            text_x1(),
            tool(),
            section_id("u2", SectionKey::Prompt),
            item_section("a2", "t1"),
            item_section("a2", "t1"),
        ]
    );
}

#[test]
fn the_slash_key_asks_the_app_for_the_prompt() {
    let mut t = searching();
    assert_eq!(ch(&mut t, '/'), Some(Msg::OpenSearch));
}

#[test]
fn n_without_a_search_does_nothing() {
    let mut t = searching();
    let before = t.selected().cloned();
    assert_eq!(ch(&mut t, 'n'), None);
    assert_eq!(ch(&mut t, 'N'), None);
    assert_eq!(press(&mut t, Key::Esc), None);
    assert_eq!(t.selected().cloned(), before);
}

#[test]
fn a_match_far_down_a_tall_section_is_scrolled_into_view() {
    let text: String = (1..=300)
        .map(|n| {
            if n == 250 {
                "- the needle line\n".to_owned()
            } else {
                format!("- line {n}\n")
            }
        })
        .collect();
    let turn = arc(with_id(
        blocks::assistant(None, vec![blocks::text("x1", &text)], None),
        "a1",
    ));
    let mut t = transcript_of(vec![turn], WIDTH, HEIGHT);
    assert_eq!(search(&mut t, "needle").as_deref(), Some("1/1"));
    let shown_rows = rows(&mut t);
    assert!(shown_rows.contains("the needle line"), "{shown_rows}");
    assert!(t.offset() > 200, "{}", t.offset());
}

// ---- Temporary unfolding ----

#[test]
fn a_jump_into_a_folded_tool_call_unfolds_it() {
    let mut t = searching();
    search(&mut t, "usb");
    assert!(folded(&t, &tool()));
    ch(&mut t, 'n');
    assert_eq!(t.selected(), Some(&tool()));
    assert!(unfolded(&t, &tool()));
    let shown_rows = rows(&mut t);
    assert!(
        shown_rows.contains("comes from the usb driver"),
        "{shown_rows}"
    );
}

#[test]
fn moving_the_selection_to_another_section_refolds_it() {
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    assert!(unfolded(&t, &tool()));
    ch(&mut t, 'j');
    assert_ne!(t.selected(), Some(&tool()));
    assert!(folded(&t, &tool()));
    assert_eq!(shown(&t).as_deref(), Some("2/5"), "the position stays");
    // Moving back does not unfold it.
    ch(&mut t, 'k');
    assert_eq!(t.selected(), Some(&tool()));
    assert!(folded(&t, &tool()));
}

#[test]
fn every_way_of_moving_the_selection_refolds_it() {
    for key in ['j', 'k', 'g', 'G', ']', '['] {
        let mut t = searching();
        search(&mut t, "usb");
        ch(&mut t, 'n');
        assert!(unfolded(&t, &tool()), "{key}");
        ch(&mut t, key);
        assert_ne!(t.selected(), Some(&tool()), "{key} moved the selection");
        assert!(folded(&t, &tool()), "{key}");
    }
}

#[test]
fn a_jump_to_another_match_refolds_the_previous_one() {
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    assert!(unfolded(&t, &tool()));
    ch(&mut t, 'n');
    assert!(folded(&t, &tool()));
    ch(&mut t, 'N');
    assert!(unfolded(&t, &tool()));
}

#[test]
fn enter_on_the_unfolded_section_makes_the_unfolding_explicit() {
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    assert!(unfolded(&t, &tool()));
    press(&mut t, Key::Enter);
    assert!(unfolded(&t, &tool()), "Enter keeps what the search opened");
    ch(&mut t, 'j');
    assert_ne!(t.selected(), Some(&tool()));
    assert!(
        unfolded(&t, &tool()),
        "and it stays open when the selection leaves"
    );
    // A second Enter is the toggle it always is.
    ch(&mut t, 'k');
    press(&mut t, Key::Enter);
    assert!(folded(&t, &tool()));
}

#[test]
fn unfold_and_fold_commands_also_end_the_temporary_state() {
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    ch(&mut t, 'l');
    ch(&mut t, 'j');
    assert!(unfolded(&t, &tool()), "`l` unfolds explicitly");
    // Folding it by hand folds it, and it stays folded.
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    ch(&mut t, 'h');
    assert!(folded(&t, &tool()));
    ch(&mut t, 'j');
    assert!(folded(&t, &tool()));
}

#[test]
fn unfold_all_makes_every_unfolding_explicit() {
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    ch(&mut t, '+');
    ch(&mut t, 'j');
    assert!(unfolded(&t, &tool()));
}

#[test]
fn a_section_the_user_had_unfolded_stays_unfolded() {
    let mut t = searching();
    t.set_folded(&tool(), false);
    search(&mut t, "usb");
    ch(&mut t, 'n');
    assert!(unfolded(&t, &tool()));
    ch(&mut t, 'j');
    assert!(unfolded(&t, &tool()), "the search did not unfold it");
}

#[test]
fn escape_clears_the_temporary_unfold() {
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    assert!(unfolded(&t, &tool()));
    assert_eq!(press(&mut t, Key::Esc), Some(Msg::SearchPosition(None)));
    assert!(folded(&t, &tool()));
    assert_eq!(t.selected(), Some(&tool()), "the selection stays");
}

#[test]
fn a_new_search_refolds_what_the_previous_one_unfolded() {
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    assert!(unfolded(&t, &tool()));
    assert_eq!(search_with(&mut t, 2, "camera").as_deref(), Some("1/1"));
    assert!(folded(&t, &tool()));
}

#[test]
fn the_screen_row_of_the_selection_stays_when_a_section_above_refolds() {
    // Many turns above, so that the unfolded call is not at the top.
    let mut turns: Vec<_> = (0..10)
        .map(|n| arc(prompt(&format!("p{n}"), "a prompt")))
        .collect();
    turns.extend(search_session());
    let mut t = transcript_of(turns, WIDTH, HEIGHT);
    search(&mut t, "usb");
    ch(&mut t, 'n');
    assert_eq!(t.selected(), Some(&tool()));
    ch(&mut t, 'j');
    let after = rows(&mut t);
    let marker_rows: Vec<&str> = after.lines().filter(|r| r.starts_with('▒')).collect();
    assert_eq!(marker_rows.len(), 1, "{after}");
    assert!(
        marker_rows[0].contains("Is that a USB 3 device?"),
        "{after}"
    );
}

// ---- Output beyond the truncation limit ----

fn long_log(lines: usize, needle_at: usize) -> Transcript {
    let text: String = (1..=lines)
        .map(|n| {
            if n == needle_at {
                "the needle line\n".to_owned()
            } else {
                format!("line {n}\n")
            }
        })
        .collect();
    let turn = arc(with_id(
        blocks::assistant(
            None,
            vec![blocks::tool(
                "c1",
                "Print the log",
                json!({"command": "cat build.log"}),
                Some((&text, false)),
                ToolStatus::Ok,
            )],
            None,
        ),
        "a1",
    ));
    transcript_of(vec![turn], WIDTH, HEIGHT)
}

#[test]
fn a_match_beyond_the_truncation_limit_expands_the_output_temporarily() {
    let mut t = long_log(500, 350);
    assert_eq!(t.truncate_output_lines(), 200);
    assert_eq!(search(&mut t, "needle").as_deref(), Some("1/1"));
    assert!(unfolded(&t, &tool()));
    assert!(
        t.content.folds.is_expanded(&tool()),
        "expanded for the match"
    );
    let shown_rows = rows(&mut t);
    assert!(shown_rows.contains("the needle line"), "{shown_rows}");
}

#[test]
fn leaving_the_section_undoes_the_expansion_as_well_as_the_unfolding() {
    let mut turns = vec![arc(prompt("u0", "a prompt before"))];
    turns.extend(long_log(500, 350).content.turns.clone());
    let mut t = transcript_of(turns, WIDTH, HEIGHT);
    assert_eq!(search(&mut t, "needle").as_deref(), Some("1/1"));
    assert!(t.content.folds.is_expanded(&tool()));
    ch(&mut t, '[');
    assert_ne!(t.selected(), Some(&tool()));
    assert!(folded(&t, &tool()));
    assert!(!t.content.folds.is_expanded(&tool()));
}

#[test]
fn a_match_within_the_limit_does_not_expand_the_output() {
    let mut t = long_log(500, 50);
    assert_eq!(search(&mut t, "needle").as_deref(), Some("1/1"));
    assert!(unfolded(&t, &tool()));
    assert!(!t.content.folds.is_expanded(&tool()));
}

/// Presses `j` (a tall section is paged through first) until `more` is
/// selected.
fn move_to_more(t: &mut Transcript, more: &SectionId) {
    for _ in 0..50 {
        if t.selected() == Some(more) {
            return;
        }
        ch(t, 'j');
    }
    panic!("never reached the truncation row, at {:?}", t.selected());
}

#[test]
fn the_truncation_row_of_the_section_counts_as_the_section() {
    let mut t = long_log(500, 50);
    search(&mut t, "needle");
    assert!(unfolded(&t, &tool()));
    let more = section_id("a1", SectionKey::More(ItemId::from("c1")));
    move_to_more(&mut t, &more);
    assert!(
        unfolded(&t, &tool()),
        "the call under the cursor stays open"
    );
}

#[test]
fn enter_on_the_truncation_row_keeps_its_expansion() {
    let mut t = long_log(500, 50);
    search(&mut t, "needle");
    let more = section_id("a1", SectionKey::More(ItemId::from("c1")));
    move_to_more(&mut t, &more);
    press(&mut t, Key::Enter);
    assert!(t.content.folds.is_expanded(&tool()));
}

#[test]
fn a_lazy_output_is_found_and_shown_in_full() {
    let text = format!("{}\nthe lazy needle", "padding line\n".repeat(400));
    let line = json!({"message": {"content": [
        {"type": "tool_result", "tool_use_id": "c1", "content": text}
    ]}})
    .to_string();
    let len = line.len() as u64;
    let output = ToolOutput::from_text(text, || OutputSource {
        bytes: ByteSource::Memory(std::sync::Arc::from(line.into_bytes())),
        range: 0..len,
        format: LineFormat::Claude,
        call_id: "c1".to_owned(),
    });
    assert!(output.is_lazy());
    let mut call = blocks::tool("c1", "Print", json!({}), None, ToolStatus::Ok);
    if let crate::model::Item::ToolCall(c) = &mut call {
        c.result = Some(crate::model::ToolResult {
            output,
            is_error: false,
        });
    }
    let turn = arc(with_id(blocks::assistant(None, vec![call], None), "a1"));
    let mut t = transcript_of(vec![turn], WIDTH, HEIGHT);
    assert_eq!(search(&mut t, "lazy needle").as_deref(), Some("1/1"));
    assert!(rows(&mut t).contains("the lazy needle"));
}

// ---- Highlighting ----

#[test]
fn every_visible_occurrence_is_drawn_with_the_search_match_style() {
    let mut t = searching();
    search(&mut t, "usb");
    let theme = blocks::theme();
    let style = theme.style(crate::theme::Token::SearchMatch);
    assert_eq!(style.bg, Some(theme.role(crate::theme::Role::Primary)));
    assert_eq!(
        style.fg,
        Some(theme.role(crate::theme::Role::PrimaryForeground))
    );
    // The text of the match in the assistant text, the prompt `u2` and the
    // text `a2`; the folded call shows none of its.
    assert_eq!(highlighted(&mut t), ["USB", "USB", "USB", "usb"]);
}

#[test]
fn highlights_follow_the_case_rule_of_the_query() {
    let mut t = searching();
    search(&mut t, "USB");
    assert_eq!(highlighted(&mut t), ["USB", "USB", "USB"]);
}

#[test]
fn the_unfolded_result_shows_its_match_highlighted() {
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    assert!(highlighted(&mut t).contains(&"usb".to_owned()));
    let all = highlighted(&mut t);
    assert_eq!(all.len(), 5, "{all:?}");
}

#[test]
fn highlighting_leaves_the_selection_marker_and_the_borders_alone() {
    let mut t = searching();
    search(&mut t, "usb");
    let bg = blocks::theme().role(crate::theme::Role::Primary);
    let buf = draw(&mut t, WIDTH, HEIGHT);
    for y in 0..HEIGHT {
        for x in [0, 1, WIDTH - 1] {
            assert_ne!(buf[(x, y)].bg, bg, "{x},{y}");
        }
    }
    assert!(text_rows(&buf).iter().any(|r| r.starts_with('▒')));
}

#[test]
fn escape_removes_the_highlights_and_the_position() {
    let mut t = searching();
    search(&mut t, "usb");
    assert!(!highlighted(&mut t).is_empty());
    assert_eq!(press(&mut t, Key::Esc), Some(Msg::SearchPosition(None)));
    assert!(highlighted(&mut t).is_empty());
    assert_eq!(position(&t), None);
    // The keys are inert again.
    assert_eq!(ch(&mut t, 'n'), None);
    assert_eq!(press(&mut t, Key::Esc), None);
}

#[test]
fn a_new_search_replaces_the_highlights_of_the_previous_one() {
    let mut t = searching();
    search(&mut t, "usb");
    search_with(&mut t, 2, "camera");
    let all = highlighted(&mut t);
    assert_eq!(all, ["camera"]);
}

#[test]
fn highlights_do_not_change_the_layout() {
    let mut plain = searching();
    let before = text_rows(&draw(&mut plain, WIDTH, HEIGHT));
    let mut t = searching();
    search(&mut t, "usb");
    ch(&mut t, 'n');
    ch(&mut t, 'N');
    let after = text_rows(&draw(&mut t, WIDTH, HEIGHT));
    // Only the selection marker and the fold state differ; the heights of
    // the blocks do not.
    assert_eq!(before.len(), after.len());
    assert_eq!(
        total_height(&mut plain),
        total_height(&mut t),
        "same height once the call is folded again"
    );
}

#[test]
fn a_hit_in_a_wide_character_row_is_highlighted_by_columns() {
    let turn = arc(with_id(
        blocks::assistant(None, vec![blocks::text("x1", "日本語 needle 日本語")], None),
        "a1",
    ));
    let mut t = transcript_of(vec![turn], WIDTH, HEIGHT);
    search(&mut t, "needle");
    assert_eq!(highlighted(&mut t), ["needle"]);
}

fn total_height(t: &mut Transcript) -> usize {
    t.measured(|cache, m| cache.total(m))
}

// ---- Finding the row of a match ----

/// The tool call `c1` of turn `a1` printing `lines` (one per line), in a
/// transcript the size of an 80x24 screen's viewport.
fn log_of(lines: &[String]) -> Transcript {
    let text = lines.join("\n");
    let turn = arc(with_id(
        blocks::assistant(
            None,
            vec![blocks::tool(
                "c1",
                "Print the log",
                json!({"command": "cat build.log"}),
                Some((&text, false)),
                ToolStatus::Ok,
            )],
            None,
        ),
        "a1",
    ));
    transcript_of(vec![turn], 80, 17)
}

fn filler(count: usize, hit_at: usize, hit: &str) -> Vec<String> {
    (1..=count)
        .map(|n| {
            if n == hit_at {
                hit.to_owned()
            } else {
                format!("line {n}")
            }
        })
        .collect()
}

#[test]
fn a_query_that_a_label_also_contains_finds_the_real_match_beyond_the_limit() {
    for (query, hit) in [
        ("result", "the final result here"),
        ("command", "the last command here"),
    ] {
        let mut t = log_of(&filler(500, 350, hit));
        assert_eq!(search(&mut t, query).as_deref(), Some("1/1"), "{query}");
        assert!(t.content.folds.is_expanded(&tool()), "{query}: expanded");
        let screen = rows(&mut t);
        assert!(
            screen.contains(hit),
            "{query}: the hit is on screen\n{screen}"
        );
        let marked = highlighted(&mut t);
        assert!(
            marked.contains(&query.to_owned()),
            "{query}: {marked:?}\n{screen}"
        );
        // The row of the hit is the highlighted one.
        let hit_row = screen.lines().find(|r| r.contains(hit)).unwrap();
        assert!(hit_row.contains(query));
    }
}

#[test]
fn a_label_is_not_taken_for_a_match_within_the_limit_either() {
    let mut t = log_of(&filler(150, 120, "the final result here"));
    assert_eq!(search(&mut t, "result").as_deref(), Some("1/1"));
    assert!(!t.content.folds.is_expanded(&tool()));
    assert!(rows(&mut t).contains("the final result here"));
}

fn markdown_turn(text: &str, width: u16) -> Transcript {
    let turn = arc(with_id(
        blocks::assistant(None, vec![blocks::text("x1", text)], None),
        "a1",
    ));
    transcript_of(vec![turn], width, 17)
}

#[test]
fn a_hit_that_is_not_drawn_does_not_shift_the_ones_after_it() {
    let mut text = String::from("See [the docs](https://usb.example.com/x).\n\n");
    text.extend((1..=30).map(|n| format!("- filler {n}\n")));
    text.push_str("- first usb here\n- second usb there\n");
    let mut t = markdown_turn(&text, 80);
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/3"));
    ch(&mut t, 'n');
    assert_eq!(shown(&t).as_deref(), Some("2/3"));
    let screen = rows(&mut t);
    assert!(screen.contains("first usb here"), "{screen}");
    assert!(highlighted(&mut t).contains(&"usb".to_owned()));
    ch(&mut t, 'n');
    assert_eq!(shown(&t).as_deref(), Some("3/3"));
    let screen = rows(&mut t);
    assert!(screen.contains("second usb there"), "{screen}");
    assert!(highlighted(&mut t).contains(&"usb".to_owned()));
}

#[test]
fn a_hit_that_wraps_across_rows_does_not_shift_the_ones_after_it() {
    // Narrow, so that a line breaks between the two words of the query for
    // some of the items.
    let mut text = String::new();
    text.extend((1..=30).map(|n| format!("- filler {n}\n")));
    for n in 1..=6 {
        let pad = "p".repeat(n * 2);
        text.push_str(&format!("- item{n} {pad} alpha beta tail\n"));
    }
    let mut t = markdown_turn(&text, 36);
    assert_eq!(search(&mut t, "alpha beta").as_deref(), Some("1/6"));
    let mut seen = Vec::new();
    for n in 1..=6 {
        let screen = rows(&mut t);
        assert!(screen.contains(&format!("item{n}")), "match {n}\n{screen}");
        seen.push(n);
        ch(&mut t, 'n');
    }
    assert_eq!(seen.len(), 6);
    assert_eq!(shown(&t).as_deref(), Some("1/6"));
}

#[test]
fn the_same_line_twice_finds_each_occurrence() {
    let mut lines = filler(60, 10, "needle");
    lines[40] = "needle".to_owned();
    let mut t = log_of(&lines);
    search(&mut t, "needle");
    ch(&mut t, 'n');
    assert_eq!(shown(&t).as_deref(), Some("2/2"));
    // The second `needle` is line 41; the first is line 10.
    let screen = rows(&mut t);
    assert!(
        screen.contains("line 40") || screen.contains("line 42"),
        "{screen}"
    );
}

// ---- Folding takes the search's expansion with it ----

fn expanded_by_search(keys: &[char]) -> Transcript {
    let mut t = long_log(500, 350);
    t.set_viewport(Size::new(WIDTH, HEIGHT));
    search(&mut t, "needle");
    assert!(t.content.folds.is_expanded(&tool()));
    for &key in keys {
        ch(&mut t, key);
    }
    t
}

fn assert_truncated_again(mut t: Transcript, what: &str) {
    assert!(unfolded(&t, &tool()), "{what}: unfolded again");
    assert!(
        !t.content.folds.is_expanded(&tool()),
        "{what}: not expanded"
    );
    let screen = rows(&mut t);
    assert!(!screen.contains("the needle line"), "{what}\n{screen}");
    let mut seen = String::new();
    for _ in 0..40 {
        seen = rows(&mut t);
        if seen.contains("more lines") {
            break;
        }
        ch(&mut t, 'j');
    }
    assert!(seen.contains("more lines"), "{what}: the truncation row");
}

#[test]
fn folding_the_section_drops_the_expansion_the_search_made() {
    assert_truncated_again(expanded_by_search(&['h', 'l']), "h then l");
}

#[test]
fn fold_all_drops_the_expansion_the_search_made() {
    assert_truncated_again(expanded_by_search(&['-', 'l']), "- then l");
}

#[test]
fn enter_on_a_section_the_search_only_expanded_folds_it_and_the_expansion_goes() {
    let mut t = long_log(500, 350);
    t.set_folded(&tool(), false);
    search(&mut t, "needle");
    assert!(t.content.folds.is_expanded(&tool()));
    press(&mut t, Key::Enter);
    assert!(folded(&t, &tool()));
    press(&mut t, Key::Enter);
    assert_truncated_again(t, "Enter, Enter");
}

#[test]
fn unfold_all_keeps_what_the_search_opened() {
    let mut t = expanded_by_search(&['+']);
    ch(&mut t, 'g');
    assert!(unfolded(&t, &tool()));
}

// ---- Matches in sections that are not there ----

#[test]
fn a_thousand_matches_in_a_hidden_section_do_not_stop_the_stepping() {
    let noise = "x ".repeat(1500);
    let system = blocks::user(
        "a prompt",
        vec![blocks::injected("i1", &[("file", noise.as_str())])],
    );
    let turns = vec![
        arc(with_id(system, "u1")),
        arc(reply("a1", "one more x at the end")),
    ];
    let mut t = transcript_of(turns, WIDTH, HEIGHT);
    assert_eq!(
        search(&mut t, "x").as_deref().map(|p| p.ends_with("/1501")),
        Some(true)
    );
    ch(&mut t, 's');
    ch(&mut t, 'n');
    assert_eq!(t.selected(), Some(&item_section("a1", "t1")));
}

// ---- A line that is the end of another, and lines drawn differently ----

/// After the jump: the position, the hit's row is on screen and drawn with
/// the `search_match` style.
fn assert_hit_shown(t: &mut Transcript, position: &str, row_text: &str, hit: &str) {
    assert_eq!(shown(t).as_deref(), Some(position));
    let screen = rows(t);
    assert!(
        screen.contains(row_text),
        "{row_text:?} on screen\n{screen}"
    );
    let marked = highlighted(t);
    assert!(
        marked.iter().any(|m| m == hit),
        "{hit:?} highlighted, not {marked:?}\n{screen}"
    );
}

fn suffix_case(lines: Vec<String>, query: &str) -> Transcript {
    let mut t = log_of(&lines);
    assert_eq!(search(&mut t, query).as_deref(), Some("1/2"));
    ch(&mut t, 'n');
    t
}

fn assert_near(t: &mut Transcript, hit_line: usize, query: &str) {
    assert_eq!(shown(t).as_deref(), Some("2/2"));
    let screen = rows(t);
    assert!(
        screen.contains(&format!("line {}", hit_line - 1))
            || screen.contains(&format!("line {}", hit_line + 1)),
        "{screen}"
    );
    assert!(highlighted(t).iter().any(|m| m == query), "{screen}");
}

#[test]
fn a_line_that_ends_an_earlier_one_beyond_the_limit_is_expanded_and_shown() {
    let mut lines = filler(500, 350, "done");
    lines[4] = "build done".to_owned();
    let mut t = suffix_case(lines, "done");
    assert!(t.content.folds.is_expanded(&tool()));
    assert_near(&mut t, 350, "done");
}

#[test]
fn a_line_that_ends_an_earlier_one_within_the_limit_is_shown() {
    let mut lines = filler(150, 120, "done");
    lines[4] = "build done".to_owned();
    let mut t = suffix_case(lines, "done");
    assert_near(&mut t, 120, "done");
}

#[test]
fn a_longer_earlier_line_with_a_prefix_beyond_the_limit_is_expanded_and_shown() {
    let mut lines = filler(500, 350, "usb disconnected");
    lines[10] = "error: usb disconnected".to_owned();
    let mut t = suffix_case(lines, "usb");
    assert!(t.content.folds.is_expanded(&tool()));
    assert_near(&mut t, 350, "usb");
}

#[test]
fn a_longer_earlier_line_with_a_prefix_within_the_limit_is_shown() {
    let mut lines = filler(150, 120, "usb disconnected");
    lines[10] = "error: usb disconnected".to_owned();
    let mut t = suffix_case(lines, "usb");
    assert_near(&mut t, 120, "usb");
}

/// 41 paragraphs: `filler 0`..`filler 19`, `hit`, `more filler 0`..
fn paragraphs_around(hit: &str) -> String {
    let before = (0..20).map(|n| format!("filler {n}"));
    let after = (0..20).map(|n| format!("more filler {n}"));
    before
        .chain(std::iter::once(hit.to_owned()))
        .chain(after)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn assert_paragraph_hit(hit: &str, query: &str, drawn: &str) {
    let mut t = markdown_turn(&paragraphs_around(hit), 80);
    assert_eq!(search(&mut t, query).as_deref(), Some("1/1"), "{hit}");
    assert_hit_shown(&mut t, "1/1", drawn, query);
}

#[test]
fn a_hit_before_a_code_span_is_scrolled_into_view() {
    assert_paragraph_hit("Plug it into the `usb` port.", "usb", "into the usb port.");
}

#[test]
fn a_hit_before_bold_text_is_scrolled_into_view() {
    assert_paragraph_hit("Plug it into the **USB** hub.", "USB", "into the USB hub.");
}

#[test]
fn a_hit_before_a_link_is_scrolled_into_view() {
    assert_paragraph_hit(
        "The usb hub, see [the docs](https://example.org).",
        "usb",
        "The usb hub, see the docs",
    );
}

#[test]
fn a_hit_before_a_code_span_further_on_the_line_is_scrolled_into_view() {
    assert_paragraph_hit(
        "The usb hub is `fine` here.",
        "usb",
        "The usb hub is fine here.",
    );
}

#[test]
fn two_hits_with_code_spans_each_land_on_their_own_row() {
    let filler_block = |tag: &str| {
        (0..15)
            .map(|n| format!("{tag} {n}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let text = format!(
        "{}\n\nUse the `usb` cable.\n\n{}\n\nUse the `usb` hub.\n\n{}",
        filler_block("a"),
        filler_block("b"),
        filler_block("c")
    );
    let mut t = markdown_turn(&text, 80);
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/2"));
    assert_hit_shown(&mut t, "1/2", "Use the usb cable.", "usb");
    ch(&mut t, 'n');
    assert_hit_shown(&mut t, "2/2", "Use the usb hub.", "usb");
    ch(&mut t, 'N');
    assert_hit_shown(&mut t, "1/2", "Use the usb cable.", "usb");
}

#[test]
fn a_hit_in_a_multi_line_string_of_a_generic_tool_input_is_scrolled_into_view() {
    let body = (1..=120)
        .map(|n| {
            if n == 100 {
                "the usb here".to_owned()
            } else {
                format!("line {n}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let call = blocks::tool_of(
        crate::model::ToolKind::Other("mcp__x__post".to_owned()),
        "c1",
        "Post it",
        json!({"title": "x", "body": body}),
        Some(("ok", false)),
        ToolStatus::Ok,
    );
    let turn = arc(with_id(blocks::assistant(None, vec![call], None), "a1"));
    let mut t = transcript_of(vec![turn], 80, 17);
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/1"));
    let screen = rows(&mut t);
    // The string is wrapped inside the JSON; the hit's row is on screen.
    assert!(screen.contains("usb here"), "{screen}");
    assert!(highlighted(&mut t).iter().any(|m| m == "usb"), "{screen}");
}

fn tool_turn(kind: crate::model::ToolKind, input: serde_json::Value, output: &str) -> Transcript {
    let call = blocks::tool_of(
        kind,
        "c1",
        "Do it",
        input,
        Some((output, false)),
        ToolStatus::Ok,
    );
    let turn = arc(with_id(blocks::assistant(None, vec![call], None), "a1"));
    transcript_of(vec![turn], 80, 17)
}

#[test]
fn a_hit_in_numbered_read_output_lands_on_its_own_row() {
    let text: String = (1..=150)
        .map(|n| {
            if n == 120 {
                format!("{n:>6}→the usb here\n")
            } else {
                format!("{n:>6}→text {n}\n")
            }
        })
        .collect();
    let mut t = tool_turn(
        crate::model::ToolKind::Read,
        json!({"file_path": "/a/b.rs"}),
        &text,
    );
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/1"));
    assert_hit_shown(&mut t, "1/1", "the usb here", "usb");
}

#[test]
fn a_hit_on_a_plus_line_of_an_edit_diff_lands_on_its_own_row() {
    let old: String = (1..=60).map(|n| format!("old {n}\n")).collect();
    let new: String = (1..=60)
        .map(|n| {
            if n == 45 {
                "new usb here\n".to_owned()
            } else {
                format!("new {n}\n")
            }
        })
        .collect();
    let mut t = tool_turn(
        crate::model::ToolKind::Edit,
        json!({"file_path": "/a/b.rs", "old_string": old, "new_string": new}),
        "ok",
    );
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/1"));
    assert_hit_shown(&mut t, "1/1", "+new usb here", "usb");
}

#[test]
fn a_hit_on_a_markdown_bullet_lands_on_its_own_row() {
    let mut text: String = (1..=30).map(|n| format!("- filler {n}\n")).collect();
    text.push_str("- the usb item\n");
    text.extend((31..=60).map(|n| format!("- filler {n}\n")));
    let mut t = markdown_turn(&text, 80);
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/1"));
    assert_hit_shown(&mut t, "1/1", "• the usb item", "usb");
}

#[test]
fn a_hit_on_a_quote_line_lands_on_its_own_row() {
    let mut text = (1..=20)
        .map(|n| format!("filler {n}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    text.push_str("\n\n> the usb quote\n\n");
    text.push_str(
        &(21..=40)
            .map(|n| format!("filler {n}"))
            .collect::<Vec<_>>()
            .join("\n\n"),
    );
    let mut t = markdown_turn(&text, 80);
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/1"));
    assert_hit_shown(&mut t, "1/1", "│ the usb quote", "usb");
}

#[test]
fn a_hit_on_a_numbered_list_line_lands_on_its_own_row() {
    let mut text: String = (1..=30).map(|n| format!("{n}. filler {n}\n")).collect();
    text.push_str("31. the usb step\n");
    text.extend((32..=60).map(|n| format!("{n}. filler {n}\n")));
    let mut t = markdown_turn(&text, 80);
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/1"));
    assert_hit_shown(&mut t, "1/1", "the usb step", "usb");
}

// ---- Markers are text outside markdown; the window of an undrawn match ----

/// The hit's neighbour `line N` is on screen, and `hit` is highlighted.
fn near(t: &mut Transcript, position: &str, line: usize, hit: &str) {
    assert_eq!(shown(t).as_deref(), Some(position));
    let screen = rows(t);
    assert!(
        screen.contains(&format!("line {}", line - 1))
            || screen.contains(&format!("line {}", line + 1)),
        "line {line} on screen\n{screen}"
    );
    assert!(
        highlighted(t).iter().any(|m| m == hit),
        "{hit:?} highlighted\n{screen}"
    );
}

fn with_lines(mut lines: Vec<String>, set: &[(usize, &str)]) -> Vec<String> {
    for &(at, text) in set {
        lines[at] = text.to_owned();
    }
    lines
}

fn log_search(lines: Vec<String>, query: &str, first: &str) -> Transcript {
    let mut t = log_of(&lines);
    assert_eq!(search(&mut t, query).as_deref(), Some(first));
    t
}

#[test]
fn a_hash_line_in_tool_output_is_not_taken_for_the_plain_line_after_it() {
    let lines = with_lines(filler(150, 100, "done"), &[(4, "# done"), (139, "done")]);
    let mut t = log_search(lines, "done", "1/3");
    ch(&mut t, 'n');
    near(&mut t, "2/3", 100, "done");
}

#[test]
fn a_star_line_in_tool_output_is_not_taken_for_the_plain_line_after_it() {
    let lines = with_lines(filler(150, 100, "done"), &[(4, "* done"), (139, "done")]);
    let mut t = log_search(lines, "done", "1/3");
    ch(&mut t, 'n');
    near(&mut t, "2/3", 100, "done");
}

#[test]
fn a_hash_line_before_a_plain_one_beyond_the_limit_is_expanded_and_shown() {
    let lines = with_lines(filler(500, 350, "done"), &[(4, "# done"), (399, "done")]);
    let mut t = log_search(lines, "done", "1/3");
    ch(&mut t, 'n');
    assert!(t.content.folds.is_expanded(&tool()));
    near(&mut t, "2/3", 350, "done");
}

#[test]
fn a_plain_line_before_a_hash_line_within_the_limit_is_shown() {
    let lines = with_lines(filler(150, 120, "# usb setup"), &[(4, "usb setup")]);
    let mut t = log_search(lines, "usb", "1/2");
    ch(&mut t, 'n');
    near(&mut t, "2/2", 120, "usb");
}

#[test]
fn a_plain_line_before_a_hash_line_beyond_the_limit_is_expanded_and_shown() {
    let lines = with_lines(filler(500, 350, "# usb setup"), &[(4, "usb setup")]);
    let mut t = log_search(lines, "usb", "1/2");
    ch(&mut t, 'n');
    assert!(t.content.folds.is_expanded(&tool()));
    near(&mut t, "2/2", 350, "usb");
}

#[test]
fn an_indented_line_before_a_star_line_is_shown() {
    let lines = with_lines(filler(150, 120, "* usb-fix"), &[(4, "  usb-fix")]);
    let mut t = log_search(lines, "usb", "1/2");
    ch(&mut t, 'n');
    near(&mut t, "2/2", 120, "usb");
}

#[test]
fn a_unique_hash_line_within_the_limit_is_found_without_expanding() {
    let mut t = log_search(filler(500, 21, "# usb here"), "usb", "1/1");
    assert!(!t.content.folds.is_expanded(&tool()));
    assert_hit_shown(&mut t, "1/1", "# usb here", "usb");
}

#[test]
fn a_hash_line_in_a_fenced_code_block_is_not_taken_for_the_plain_lines_after_it() {
    let mut text = String::from("Intro.\n\n```python\n");
    for n in 1..=60 {
        text.push_str(
            match n {
                3 => "# done\n".to_owned(),
                20 | 50 => "done\n".to_owned(),
                n => format!("x{n}\n"),
            }
            .as_str(),
        );
    }
    text.push_str("```\n");
    let mut t = markdown_turn(&text, 80);
    assert_eq!(search(&mut t, "done").as_deref(), Some("1/3"));
    ch(&mut t, 'n');
    assert_eq!(shown(&t).as_deref(), Some("2/3"));
    let screen = rows(&mut t);
    assert!(screen.contains("x19") || screen.contains("x21"), "{screen}");
    assert!(highlighted(&mut t).iter().any(|m| m == "done"), "{screen}");
}

#[test]
fn a_hash_line_in_a_fenced_block_after_a_paragraph_is_found_on_its_own_row() {
    let mut text = String::from("usb setup\n\n");
    for n in 1..=30 {
        text.push_str(&format!("filler {n}\n\n"));
    }
    text.push_str("```sh\n# usb setup\nmake\n```");
    let mut t = markdown_turn(&text, 80);
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/2"));
    ch(&mut t, 'n');
    assert_hit_shown(&mut t, "2/2", "# usb setup", "usb");
    let screen = rows(&mut t);
    assert!(screen.contains("make"), "{screen}");
}

fn filler_paragraphs(tag: &str, count: usize) -> Vec<String> {
    (0..count).map(|n| format!("{tag} {n}")).collect()
}

/// `filler 0..19`, `hit`, `more filler 0..19`, `the second usb hit`,
/// `tail filler 0..19`, as paragraphs.
fn windowed(hit: &str) -> String {
    let mut all = filler_paragraphs("filler", 20);
    all.push(hit.to_owned());
    all.extend(filler_paragraphs("more filler", 20));
    all.push("the second usb hit".to_owned());
    all.extend(filler_paragraphs("tail filler", 20));
    all.join("\n\n")
}

/// Searches `usb`, expects `total` matches, and walks through them
/// checking that `expected[i]` is on screen at the i-th, and, where its flag
/// is set, that the row draws the query, highlighted.
fn walk(text: &str, expected: &[(&str, bool)]) -> Transcript {
    let mut t = markdown_turn(text, 80);
    let total = expected.len();
    assert_eq!(
        search(&mut t, "usb").as_deref(),
        Some(format!("1/{total}").as_str())
    );
    for (i, (row, drawn)) in expected.iter().enumerate() {
        let position = format!("{}/{total}", i + 1);
        if *drawn {
            assert_hit_shown(&mut t, &position, row, "usb");
        } else {
            assert_eq!(shown(&t), Some(position.clone()));
            let screen = rows(&mut t);
            assert!(screen.contains(row), "{position}: {row:?}\n{screen}");
        }
        ch(&mut t, 'n');
    }
    t
}

#[test]
fn a_url_hit_before_a_drawn_one_on_the_same_line_stays_on_the_paragraph() {
    walk(
        &windowed("See [the docs](https://usb.org) for the usb port."),
        &[
            ("See the docs for the usb port.", true),
            ("See the docs for the usb port.", true),
            ("the second usb hit", true),
        ],
    );
}

#[test]
fn a_paragraph_with_only_a_url_hit_does_not_borrow_the_next_match() {
    walk(
        &windowed("Read [the spec](https://usb.org/spec) first."),
        &[
            ("Read the spec first.", false),
            ("the second usb hit", true),
        ],
    );
}

#[test]
fn several_undrawn_and_drawn_hits_on_one_line_stay_on_it() {
    walk(
        &windowed(
            "Use `lsusb` from [usbutils](https://github.com/gregkh/usbutils) to list usb devices.",
        ),
        &[
            ("Use lsusb from usbutils to list usb devices.", true),
            ("Use lsusb from usbutils to list usb devices.", true),
            ("Use lsusb from usbutils to list usb devices.", true),
            ("Use lsusb from usbutils to list usb devices.", true),
            ("the second usb hit", true),
        ],
    );
}

#[test]
fn a_link_and_a_fence_info_string_before_the_hit_do_not_shift_it() {
    let mut paragraphs = vec![
        "See [the docs](https://usb.org/spec) and `code`.".to_owned(),
        "```usb\nplain block\n```".to_owned(),
    ];
    paragraphs.extend(filler_paragraphs("filler", 20));
    paragraphs.push("the usb hit you want first".to_owned());
    paragraphs.extend(filler_paragraphs("more filler", 20));
    paragraphs.push("the second usb hit".to_owned());
    paragraphs.extend(filler_paragraphs("tail filler", 20));
    walk(
        &paragraphs.join("\n\n"),
        &[
            ("See the docs and code.", false),
            ("plain block", false),
            ("the usb hit you want first", true),
            ("the second usb hit", true),
        ],
    );
}

#[test]
fn an_undrawn_url_before_a_code_span_stays_on_the_paragraph_and_the_far_line_is_not_reached() {
    let mut paragraphs = vec!["See [docs](https://usb.example.com) for the `usb` port.".to_owned()];
    paragraphs.extend((1..=30).map(|n| format!("filler {n}")));
    paragraphs.push("the far usb line".to_owned());
    let mut t = markdown_turn(&paragraphs.join("\n\n"), 80);
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/3"));
    ch(&mut t, 'n');
    assert_hit_shown(&mut t, "2/3", "for the usb port", "usb");
    let screen = rows(&mut t);
    assert!(!screen.contains("the far usb line"), "{screen}");
}

#[test]
fn a_url_and_a_link_text_before_a_code_span_stay_on_the_paragraph() {
    let mut paragraphs = vec!["Use [serde](https://serde.rs) or `serde_json`.".to_owned()];
    paragraphs.extend((1..=30).map(|n| format!("filler {n}")));
    paragraphs.push("the far serde line".to_owned());
    let mut t = markdown_turn(&paragraphs.join("\n\n"), 80);
    let query = "serde";
    assert_eq!(search(&mut t, query).as_deref(), Some("1/4"));
    ch(&mut t, 'n');
    ch(&mut t, 'n');
    assert_hit_shown(&mut t, "3/4", "or serde_json", "serde");
    let screen = rows(&mut t);
    assert!(!screen.contains("the far serde line"), "{screen}");
}

#[test]
fn the_second_hit_with_nothing_undrawn_between_is_the_second_occurrence() {
    let mut paragraphs = filler_paragraphs("filler", 20);
    paragraphs.push("the `usb` one".to_owned());
    paragraphs.extend((1..=30).map(|n| format!("more `code` {n}")));
    paragraphs.push("the `usb` two".to_owned());
    paragraphs.extend(filler_paragraphs("tail filler", 20));
    let mut t = markdown_turn(&paragraphs.join("\n\n"), 80);
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/2"));
    assert_hit_shown(&mut t, "1/2", "the usb one", "usb");
    let screen = rows(&mut t);
    assert!(!screen.contains("the usb two"), "{screen}");
    ch(&mut t, 'n');
    assert_hit_shown(&mut t, "2/2", "the usb two", "usb");
    let screen = rows(&mut t);
    assert!(!screen.contains("the usb one"), "{screen}");
}

#[test]
fn a_row_that_reads_like_a_line_but_ends_a_longer_one_is_not_taken_for_it() {
    // At 80 columns the output is wrapped at 69 characters: the long line
    // ends in a row that reads exactly `done`.
    let long = format!("{}done", "x".repeat(69));
    let lines = with_lines(filler(100, 60, "done"), &[(4, long.as_str())]);
    let mut t = log_search(lines, "done", "1/2");
    ch(&mut t, 'n');
    near(&mut t, "2/2", 60, "done");
}

#[test]
fn a_repeated_context_line_of_an_edit_lands_on_its_own_hunk() {
    let side = |tag: &str| -> String {
        (0..62)
            .map(|n| match n {
                0 | 57 => "usb ctx\n".to_owned(),
                1..=20 | 56 => format!("{tag} {n}\n"),
                _ => format!("c {n}\n"),
            })
            .collect()
    };
    let mut t = tool_turn(
        crate::model::ToolKind::Edit,
        json!({"file_path": "/a/b.rs", "old_string": side("old"), "new_string": side("new")}),
        "ok",
    );
    assert_eq!(search(&mut t, "usb").as_deref(), Some("1/4"));
    assert_hit_shown(&mut t, "1/4", "@@ -1,24 +1,24 @@", "usb");
    assert!(!rows(&mut t).contains("@@ -54,7 +54,7 @@"));
    ch(&mut t, 'n');
    assert_hit_shown(&mut t, "2/4", "@@ -54,7 +54,7 @@", "usb");
}
