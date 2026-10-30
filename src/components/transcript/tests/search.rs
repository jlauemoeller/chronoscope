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
    let buf = draw(t, WIDTH, HEIGHT);
    let mut found = Vec::new();
    for y in 0..HEIGHT {
        let mut run = String::new();
        for x in 0..WIDTH {
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
    text_rows(&draw(t, WIDTH, HEIGHT)).join("\n")
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
