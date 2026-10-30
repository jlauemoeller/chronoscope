//! Truncated tool output: the `… N more lines` row is a selection stop and
//! `Submit` on it expands the output (task 019).

use serde_json::json;

use super::super::fixtures::*;
use super::super::*;
use super::{down, item_section, section_id, up};
use crate::components::turn_block::SectionKey;
use crate::components::turn_block::fixtures as blocks;
use crate::model::{
    ByteSource, ItemId, LineFormat, OutputSource, ToolOutput, ToolResult, ToolStatus,
};
use crate::test_support::buffer_to_text;
use crate::theme::Token;
use tuirealm::command::Cmd;

fn tool_id() -> SectionId {
    item_section("a1", "c1")
}

fn more_id() -> SectionId {
    section_id("a1", SectionKey::More(ItemId::from("c1")))
}

/// Moves down until `want` is selected.
fn move_to(t: &mut Transcript, want: &SectionId) {
    for _ in 0..200 {
        if t.selected() == Some(want) {
            return;
        }
        t.perform(down());
    }
    panic!("never selected {want:?}, stuck at {:?}", t.selected());
}

fn block_text(t: &Transcript) -> Vec<String> {
    block_rows(t, 0, t.viewport.unwrap().width)
}

fn count_lines_shown(rows: &[String]) -> usize {
    rows.iter()
        .filter(|row| {
            let body = row.trim_matches(['│', '▒', ' ']);
            body.strip_prefix("line ")
                .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
        })
        .count()
}

#[test]
fn a_long_result_shows_200_lines_and_a_more_row() {
    let t = long_output_transcript(80, 24);
    let rows = block_text(&t);
    assert_eq!(t.truncate_output_lines(), 200);
    assert_eq!(count_lines_shown(&rows), 200);
    let more: Vec<&String> = rows.iter().filter(|r| r.contains("more lines")).collect();
    assert_eq!(more.len(), 1);
    assert_eq!(more[0].trim_matches(['│', ' ']), "… 1 234 more lines");
    // Column 7, like the content above it.
    assert_eq!(more[0].chars().nth(7), Some('…'));
    // The row directly follows the last shown line, with no blank between.
    let at = rows.iter().position(|r| r.contains("more lines")).unwrap();
    assert!(rows[at - 1].contains("line 200"));
}

#[test]
fn the_more_row_is_a_stop_of_its_own_after_the_tool_section() {
    let t = long_output_transcript(80, 24);
    let rows = t.content.section_rows(t.turn(0).unwrap(), 80);
    let ids: Vec<&SectionId> = rows.iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [&tool_id(), &more_id()]);
    assert_eq!(rows[0].1.end, rows[1].1.start, "directly after");
    assert_eq!(rows[1].1.len(), 1);
}

#[test]
fn moving_selects_the_more_row_and_only_it_gets_the_marker() {
    let mut t = long_output_transcript(80, 24);
    assert_eq!(t.selected(), Some(&tool_id()));
    move_to(&mut t, &more_id());
    let text = buffer_to_text(&draw(&mut t, 80, 24));
    let marked: Vec<&str> = text.lines().filter(|r| r.starts_with('▒')).collect();
    assert_eq!(marked.len(), 1, "{text}");
    assert!(marked[0].contains("… 1 234 more lines"));
    // Moving up again returns to the tool section.
    t.perform(up());
    assert_eq!(t.selected(), Some(&tool_id()));
}

#[test]
fn submit_on_the_more_row_expands_all_lines_and_selects_the_tool() {
    let mut t = long_output_transcript(80, 24);
    move_to(&mut t, &more_id());
    t.perform(Cmd::Submit);
    assert_eq!(t.selected(), Some(&tool_id()));
    let rows = block_text(&t);
    assert_eq!(count_lines_shown(&rows), 1434);
    assert!(rows.iter().all(|r| !r.contains("more lines")));
    assert!(rows.iter().any(|r| r.contains("line 1434")));
    let sections = t.content.section_rows(t.turn(0).unwrap(), 80);
    assert_eq!(sections.len(), 1, "the more row is gone");
}

#[test]
fn the_expansion_survives_folding_and_unfolding() {
    let mut t = long_output_transcript(80, 24);
    move_to(&mut t, &more_id());
    t.perform(Cmd::Submit);
    // Submit on the tool section folds it, and again unfolds it.
    t.perform(Cmd::Submit);
    assert_eq!(t.is_folded(&tool_id()), Some(true));
    t.perform(Cmd::Submit);
    assert_eq!(t.is_folded(&tool_id()), Some(false));
    let rows = block_text(&t);
    assert_eq!(count_lines_shown(&rows), 1434);
    assert!(rows.iter().all(|r| !r.contains("more lines")));
}

#[test]
fn folding_a_truncated_call_removes_its_stop_and_keeps_the_selection_on_it() {
    let mut t = long_output_transcript(80, 24);
    move_to(&mut t, &more_id());
    t.set_folded(&tool_id(), true);
    assert_eq!(t.selected(), Some(&tool_id()));
    t.set_folded(&tool_id(), false);
    let rows = t.content.section_rows(t.turn(0).unwrap(), 80);
    assert_eq!(rows.len(), 2, "the more row is back after unfolding");
}

#[test]
fn a_result_at_the_limit_is_not_truncated() {
    let mut t = transcript_of(vec![long_output_turn(200)], 80, 24);
    t.set_folded(&tool_id(), false);
    let rows = block_text(&t);
    assert_eq!(count_lines_shown(&rows), 200);
    assert!(rows.iter().all(|r| !r.contains("more lines")));
    let mut t = transcript_of(vec![long_output_turn(201)], 80, 24);
    t.set_folded(&tool_id(), false);
    let rows = block_text(&t);
    assert!(rows.iter().any(|r| r.contains("… 1 more line")));
}

#[test]
fn the_setting_changes_the_cut_and_the_heights() {
    let mut t = long_output_transcript(80, 24);
    let before = t.measured(|cache, m| cache.height(0, m));
    t.attr(TRUNCATE_OUTPUT_LINES_ATTR, AttrValue::Length(10));
    assert_eq!(t.truncate_output_lines(), 10);
    let after = t.measured(|cache, m| cache.height(0, m));
    assert!(after < before);
    let rows = block_text(&t);
    assert_eq!(count_lines_shown(&rows), 10);
    assert!(rows.iter().any(|r| r.contains("… 1 424 more lines")));
    assert_eq!(after, rows.len(), "the cached height is the rendered one");
}

#[test]
fn raising_the_limit_moves_a_selected_more_row_to_its_tool() {
    let mut t = long_output_transcript(80, 24);
    move_to(&mut t, &more_id());
    t.attr(TRUNCATE_OUTPUT_LINES_ATTR, AttrValue::Length(5000));
    assert_eq!(t.selected(), Some(&tool_id()));
    assert_eq!(count_lines_shown(&block_text(&t)), 1434);
}

#[test]
fn an_update_that_shortens_the_output_moves_the_selection_to_the_tool() {
    let mut t = long_output_transcript(80, 24);
    move_to(&mut t, &more_id());
    t.apply_update(&ModelUpdate::Updated(vec![long_output_turn(50)]));
    assert_eq!(t.selected(), Some(&tool_id()));
}

// ---- Lazy outputs ----

/// A Shell call whose result is stored by reference to a transcript line.
fn lazy_call_turn(text: &str, claimed_len: usize) -> Arc<Turn> {
    let line = json!({"message": {"content": [
        {"type": "tool_result", "tool_use_id": "c1", "content": text}
    ]}})
    .to_string();
    let range = 0..line.len() as u64;
    let output = ToolOutput::lazy(
        text.lines().count(),
        claimed_len,
        OutputSource {
            bytes: ByteSource::Memory(Arc::from(line.into_bytes())),
            range,
            format: LineFormat::Claude,
            call_id: "c1".to_owned(),
        },
    );
    let mut turn = long_output_turn(1);
    let Turn::Assistant(a) = Arc::make_mut(&mut turn) else {
        panic!()
    };
    let crate::model::Item::ToolCall(call) = &mut a.items[0] else {
        panic!()
    };
    call.result = Some(ToolResult {
        output,
        is_error: false,
    });
    turn
}

#[test]
fn a_folded_call_never_materialises_its_output_and_unfolding_does() {
    let text = "alpha\nbeta\ngamma";
    let mut t = transcript_of(vec![lazy_call_turn(text, text.len())], 80, 24);
    let start = crate::model::text_calls();
    // Lay out, draw and move around while folded.
    draw(&mut t, 80, 24);
    t.perform(down());
    t.perform(up());
    draw(&mut t, 80, 24);
    assert_eq!(crate::model::text_calls(), start, "folded: never read");
    t.set_folded(&tool_id(), false);
    assert!(crate::model::text_calls() > start, "unfolded: read");
    let rows = text_rows(&draw(&mut t, 80, 24));
    assert!(rows.iter().any(|r| r.contains("gamma")));
}

#[test]
fn an_output_that_cannot_be_read_is_reported_in_error_output() {
    let mut t = transcript_of(vec![lazy_call_turn("alpha", 999)], 80, 24);
    t.set_folded(&tool_id(), false);
    let rows = block_text(&t);
    let at = rows
        .iter()
        .position(|r| r.contains("(output unavailable:"))
        .expect("an unavailable note");
    let buf = draw(&mut t, 80, 24);
    // Block row `at` is screen row `at - offset`; the note starts at column 7.
    let y = u16::try_from(at - t.offset()).unwrap();
    let theme = blocks::theme();
    assert_eq!(
        buf[(8, y)].fg,
        theme.style(Token::ErrorOutput).fg.unwrap(),
        "{rows:?}"
    );
}

// ---- The open_subagent key ----

#[test]
fn the_transcript_passes_the_open_subagent_key_to_subagent_sections() {
    let turn = arc(with_id(
        blocks::assistant(
            None,
            vec![blocks::tool_of(
                crate::model::ToolKind::Subagent,
                "c1",
                "Explore",
                json!({"subagent_type": "Explore", "prompt": "Look around"}),
                Some(("Done.", false)),
                ToolStatus::Ok,
            )],
            None,
        ),
        "a1",
    ));
    let mut t = transcript_of(vec![turn], 80, 24);
    t.set_folded(&tool_id(), false);
    let last = |t: &Transcript| {
        block_text(t)
            .into_iter()
            .rfind(|r| r.contains("to open transcript"))
            .unwrap()
    };
    let hint = last(&t);
    assert_eq!(hint.trim_matches(['│', '▒', ' ']), "a to open transcript");
    assert_eq!(hint.chars().nth(4), Some('a'), "at the label column");
    t.attr(OPEN_SUBAGENT_KEY_ATTR, AttrValue::String("Ctrl-o".into()));
    assert_eq!(
        last(&t).trim_matches(['│', '▒', ' ']),
        "Ctrl-o to open transcript"
    );
    assert_eq!(
        t.query(OPEN_SUBAGENT_KEY_ATTR).map(|q| q.into_attr()),
        Some(AttrValue::String("Ctrl-o".into()))
    );
}

#[test]
fn a_long_subagent_result_is_truncated_with_a_stop_that_expands_it() {
    let text = numbered_lines(30).replace('\n', "\n\n");
    let turn = arc(with_id(
        blocks::assistant(
            None,
            vec![blocks::tool_of(
                crate::model::ToolKind::Subagent,
                "c1",
                "Explore",
                json!({"subagent_type": "Explore", "prompt": "Look around"}),
                Some((&text, false)),
                ToolStatus::Ok,
            )],
            None,
        ),
        "a1",
    ));
    let mut t = transcript_of(vec![turn], 80, 24);
    t.attr(TRUNCATE_OUTPUT_LINES_ATTR, AttrValue::Length(10));
    t.set_folded(&tool_id(), false);
    let ids: Vec<SectionId> = t
        .content
        .section_rows(t.turn(0).unwrap(), 80)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, [tool_id(), more_id()]);
    let rows = block_text(&t);
    assert!(rows.iter().any(|r| r.contains("to open transcript")));
    assert!(rows.iter().any(|r| r.contains("… 49 more lines")));
    move_to(&mut t, &more_id());
    t.perform(Cmd::Submit);
    let rows = block_text(&t);
    assert!(rows.iter().any(|r| r.contains("line 30")));
    assert!(rows.iter().all(|r| !r.contains("more lines")));
}
