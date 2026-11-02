//! Tests of the turns sidebar: the rows it builds from turns (SPEC §6.2,
//! §6.5, §6.6), its look (§6.3) and the way it takes updates.

use std::sync::Arc;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

use super::fixtures::*;
use super::rows::entry_builds;
use super::*;
use crate::components::THEME_ATTR;
use crate::components::transcript::Transcript;
use crate::components::transcript::fixtures::{arc, long_output_turn, transcript_of, with_id};
use crate::components::turn_block::block_work;
use crate::components::turn_block::fixtures as blocks;
use crate::model::{Item, Thinking, Turn, TurnId, source_reads, text_calls};
use crate::test_support::buffer_to_text;
use crate::theme::{Role, Theme, Token, builtin};

fn id(id: &str) -> TurnId {
    TurnId::from(id)
}

fn update(update: ModelUpdate) -> AttrValue {
    payload_value(Arc::new(update))
}

fn appended(turns: Vec<Turn>) -> ModelUpdate {
    ModelUpdate::Appended(shared(&turns))
}

fn updated(turns: Vec<Turn>) -> ModelUpdate {
    ModelUpdate::Updated(shared(&turns))
}

/// The text of row `y` of `buffer`.
fn row_text(buffer: &Buffer, y: u16) -> String {
    buffer_to_text(buffer)
        .lines()
        .nth(usize::from(y))
        .unwrap()
        .to_owned()
}

/// The text of the cells `from..to` of row `y`.
fn cols(buffer: &Buffer, y: u16, from: usize, to: usize) -> String {
    row_text(buffer, y)
        .chars()
        .skip(from)
        .take(to - from)
        .collect()
}

// -- the rows

#[test]
fn each_kind_is_listed_in_transcript_order_between_start_and_end() {
    let rows = rows_of(&wireframe());
    assert_eq!(
        kinds(&rows),
        [
            "Start",
            "|",
            "User",
            "|",
            "Assistant",
            "|",
            "User",
            "|",
            "Assistant",
            " ",
            "Compaction",
            " ",
            "Assistant",
            "|",
            "End"
        ]
    );
    assert_eq!(rows.entry_count(), 6);
    assert_eq!(rows.row_count(), 15);
}

#[test]
fn start_and_end_are_not_selectable_and_have_no_entry() {
    let rows = rows_of(&wireframe());
    assert_eq!(rows.row(0), Some(Row::Start));
    assert_eq!(rows.row(rows.row_count() - 1), Some(Row::End));
    for row in 0..rows.row_count() {
        let selectable = rows.row(row).unwrap().selectable();
        assert_eq!(selectable, rows.entry_at_row(row).is_some(), "row {row}");
        let is_entry = matches!(rows.row(row), Some(Row::Entry(_)));
        assert_eq!(selectable, is_entry, "row {row}");
    }
    assert_eq!(rows.entry_at_row(0), None);
    assert_eq!(rows.entry_at_row(rows.row_count() - 1), None);
    assert_eq!(rows.entry_at_row(1), None, "a connector");
    assert_eq!(rows.entry_at_row(rows.row_count()), None);
    // The entry and its row map both ways.
    for entry in 0..rows.entry_count() {
        assert_eq!(
            rows.entry_at_row(TurnRows::row_of_entry(entry)),
            Some(entry)
        );
    }
}

#[test]
fn an_empty_list_is_start_a_connector_and_end() {
    let rows = rows_of(&[]);
    assert_eq!(kinds(&rows), ["Start", "|", "End"]);
    assert_eq!(rows.entry_count(), 0);
}

#[test]
fn connector_rows_join_dots_and_a_compaction_has_blank_rows_around_it() {
    let rows = rows_of(&[user("u1"), compaction("c1"), assistant("a1", 5)]);
    assert_eq!(
        kinds(&rows),
        [
            "Start",
            "|",
            "User",
            " ",
            "Compaction",
            " ",
            "Assistant",
            "|",
            "End"
        ]
    );
}

#[test]
fn two_adjacent_compactions_have_one_blank_row_between_them() {
    let rows = rows_of(&[user("u1"), compaction("c1"), compaction("c2"), user("u2")]);
    assert_eq!(
        kinds(&rows),
        [
            "Start",
            "|",
            "User",
            " ",
            "Compaction",
            " ",
            "Compaction",
            " ",
            "User",
            "|",
            "End"
        ]
    );
}

#[test]
fn a_compaction_right_after_start_and_right_before_end() {
    let first = rows_of(&[compaction("c1"), user("u1")]);
    assert_eq!(
        kinds(&first),
        ["Start", " ", "Compaction", " ", "User", "|", "End"]
    );
    let last = rows_of(&[user("u1"), compaction("c1")]);
    assert_eq!(
        kinds(&last),
        ["Start", "|", "User", " ", "Compaction", " ", "End"]
    );
    let only = rows_of(&[compaction("c1")]);
    assert_eq!(kinds(&only), ["Start", " ", "Compaction", " ", "End"]);
}

#[test]
fn a_turn_with_only_redacted_thinking_or_only_system_content_has_no_entry() {
    assert_eq!(entry_for(&redacted_only("a1")), None);
    assert_eq!(entry_for(&system_only("u1")), None);
    let rows = rows_of(&[redacted_only("a1"), system_only("u1"), user("u2")]);
    assert_eq!(kinds(&rows), ["Start", "|", "User", "|", "End"]);
    // Their slots are kept: an update can give them an entry later.
    assert_eq!(rows.turn_count(), 3);
    assert_eq!(rows.entry_index(&id("a1")), None);
    assert_eq!(rows.entry_index(&id("u2")), Some(0));
}

#[test]
fn redacted_thinking_with_a_duration_has_an_entry_because_of_its_footer() {
    let turn = with_stats(redacted_only("a1"), 90);
    assert!(entry_for(&turn).is_some());
    let rows = rows_of(&[turn]);
    let (_, entry) = rows.entry(0).unwrap();
    assert_eq!(entry.duration, Some(Duration::from_secs(90)));
}

fn with_stats(mut turn: Turn, secs: u64) -> Turn {
    if let Turn::Assistant(assistant) = &mut turn {
        assistant.stats = Some(stats(secs));
    }
    turn
}

#[test]
fn a_turn_with_only_a_notice_has_an_entry() {
    let rows = rows_of(&[notice_only("a1")]);
    assert_eq!(kinds(&rows), ["Start", "|", "Assistant", "|", "End"]);
    assert_eq!(rows.entry(0).unwrap().1.duration, None);
}

#[test]
fn system_content_next_to_a_prompt_does_not_change_the_entry() {
    let with_prompt = with_id_user_items("u1");
    assert_eq!(
        entry_for(&with_prompt).map(|e| e.kind),
        Some(EntryKind::User)
    );
}

fn with_id_user_items(id: &str) -> Turn {
    crate::components::transcript::fixtures::with_id(
        blocks::user(
            "A question.",
            vec![blocks::injected("i1", &[("file", "/notes.md\nUse it.")])],
        ),
        id,
    )
}

#[test]
fn toggling_system_content_leaves_the_entries_unchanged() {
    let turns = vec![
        arc(system_only("u0")),
        arc(with_id_user_items("u1")),
        arc(assistant("a1", 12)),
        arc(system_only("u2")),
    ];
    let mut transcript = transcript_of(turns, 100, 30);
    let rows = |transcript: &Transcript| -> TurnRows {
        let value = transcript.query(TURN_ROWS_ATTR).unwrap().into_attr();
        payload_from_value(&value).unwrap()
    };
    let hidden = {
        transcript.attr(crate::components::SHOW_SYSTEM_ATTR, AttrValue::Flag(false));
        rows(&transcript)
    };
    let shown = {
        transcript.attr(crate::components::SHOW_SYSTEM_ATTR, AttrValue::Flag(true));
        rows(&transcript)
    };
    assert_eq!(hidden, shown);
    assert_eq!(
        kinds(&shown),
        ["Start", "|", "User", "|", "Assistant", "|", "End"]
    );
}

#[test]
fn an_assistant_turn_gaining_a_duration_shows_it_in_place() {
    let mut rows = rows_of(&[user("u1"), running("a1"), user("u2")]);
    assert_eq!(rows.entry(1).unwrap().1.duration, None);
    rows.apply(&updated(vec![assistant("a1", 4 * 60 + 3)]));
    assert_eq!(
        rows.entry(1).unwrap().1.duration,
        Some(Duration::from_secs(243))
    );
    assert_eq!(
        kinds(&rows),
        kinds(&rows_of(&[user("u1"), running("a1"), user("u2")]))
    );
}

#[test]
fn an_updated_turn_gaining_its_first_section_appears_in_its_transcript_position() {
    // `a1` has no section yet (redacted thinking, no stats).
    let mut rows = rows_of(&[
        user("u1"),
        redacted_only("a1"),
        user("u2"),
        compaction("c1"),
    ]);
    assert_eq!(
        kinds(&rows),
        [
            "Start",
            "|",
            "User",
            "|",
            "User",
            " ",
            "Compaction",
            " ",
            "End"
        ]
    );
    rows.apply(&updated(vec![assistant("a1", 7)]));
    // Between the two User turns, where the turn is, not at the end.
    assert_eq!(
        kinds(&rows),
        [
            "Start",
            "|",
            "User",
            "|",
            "Assistant",
            "|",
            "User",
            " ",
            "Compaction",
            " ",
            "End"
        ]
    );
    assert_eq!(rows.entry_index(&id("a1")), Some(1));
    assert_eq!(rows.entry_index(&id("u2")), Some(2));
}

#[test]
fn an_updated_turn_losing_its_entry_leaves_the_list_and_keeps_its_place() {
    let mut rows = rows_of(&[user("u1"), assistant("a1", 7), user("u2")]);
    rows.apply(&updated(vec![redacted_only("a1")]));
    assert_eq!(
        kinds(&rows),
        ["Start", "|", "User", "|", "User", "|", "End"]
    );
    assert_eq!(rows.turn_count(), 3);
    assert_eq!(rows.entry_index(&id("a1")), None);
    assert_eq!(rows.entry_index(&id("u2")), Some(1));
    rows.apply(&updated(vec![assistant("a1", 9)]));
    assert_eq!(rows.entry_index(&id("a1")), Some(1), "back in its place");
    assert_eq!(rows.entry_index(&id("u2")), Some(2));
}

#[test]
fn an_appended_turn_with_a_known_id_updates_its_row_instead_of_adding_one() {
    let mut rows = rows_of(&[user("u1"), running("a1")]);
    rows.apply(&appended(vec![assistant("a1", 61)]));
    assert_eq!(rows.turn_count(), 2);
    assert_eq!(rows.entry_count(), 2);
    assert_eq!(
        rows.entry(1).unwrap().1.duration,
        Some(Duration::from_secs(61))
    );
    // A new turn is appended, with the same batch holding an update.
    rows.apply(&appended(vec![assistant("a1", 62), user("u2")]));
    assert_eq!(rows.turn_count(), 3);
    assert_eq!(
        rows.entry(1).unwrap().1.duration,
        Some(Duration::from_secs(62))
    );
    assert_eq!(rows.entry_index(&id("u2")), Some(2));
}

#[test]
fn an_updated_turn_with_an_unknown_id_is_ignored() {
    let mut rows = rows_of(&[user("u1")]);
    let before = rows.clone();
    rows.apply(&updated(vec![assistant("zz", 5)]));
    assert_eq!(rows, before);
}

#[test]
fn reset_clears_the_rows_and_other_updates_change_nothing() {
    let mut rows = rows_of(&wireframe());
    let before = rows.clone();
    for other in [
        ModelUpdate::Loaded,
        ModelUpdate::Progress(0.5),
        ModelUpdate::ParseWarnings(2),
    ] {
        rows.apply(&other);
        assert_eq!(rows, before);
    }
    rows.apply(&ModelUpdate::Reset);
    assert_eq!(kinds(&rows), ["Start", "|", "End"]);
    assert_eq!(rows.turn_count(), 0);
    assert_eq!(rows.entry_index(&id("u1")), None, "the map is cleared too");
}

#[test]
fn rows_built_from_a_transcript_match_those_built_from_its_turns() {
    let turns = wireframe();
    let transcript = transcript_of(shared(&turns), 100, 30);
    let value = transcript.query(TURN_ROWS_ATTR).unwrap().into_attr();
    let from_transcript: TurnRows = payload_from_value(&value).unwrap();
    assert_eq!(from_transcript, rows_of(&turns));
}

// -- what building rows must not do (SPEC §6.6)

#[test]
fn building_rows_neither_lays_out_nor_renders_nor_reads_output() {
    let mut turns = wireframe();
    // Large results are stored by reference to a transcript line; reading one
    // moves both counters (checked below).
    turns.push(lazy_output_turn("big"));
    turns.push(with_id(long_output_turn(1434).as_ref().clone(), "long"));
    let transcript = transcript_of(shared(&turns), 100, 30);
    let (work, texts, reads) = (block_work(), text_calls(), source_reads());
    let value = transcript.query(TURN_ROWS_ATTR).unwrap().into_attr();
    let rows: TurnRows = payload_from_value(&value).unwrap();
    assert_eq!(rows.entry_count(), 8);
    let mut sidebar = TurnSidebar::new(builtin::default_theme());
    sidebar.attr(TURN_ROWS_ATTR, value);
    sidebar.attr(
        MODEL_UPDATE_ATTR,
        update(updated(vec![assistant("a1", 99), lazy_output_turn("big")])),
    );
    draw(&mut sidebar, 30, 20);
    // `turn_stop` renders the block: it goes through `sections_of`,
    // `section_rows` and `TurnBlock::layout`, which `block_work` counts. So
    // `block_work` unchanged also shows that `turn_stop` was not reached.
    assert_eq!(block_work(), work, "a block was laid out or rendered");
    assert_eq!(text_calls(), texts, "an output was materialised");
    assert_eq!(source_reads(), reads, "the transcript was read again");
}

#[test]
fn the_counters_of_the_test_above_move_when_work_is_done() {
    // A control: the same fixtures do move the counters when a block is
    // laid out and an output is read.
    let turns = vec![lazy_output_turn("big")];
    let mut transcript = transcript_of(shared(&turns), 100, 30);
    let (work, texts, reads) = (block_work(), text_calls(), source_reads());
    transcript.attr(
        crate::components::VIEWPORT_ATTR,
        crate::components::viewport_value(ratatui::layout::Size::new(90, 30)),
    );
    assert!(block_work() > work, "laying out is counted");
    let Turn::Assistant(assistant) = &turns[0] else {
        unreachable!()
    };
    let Some(Item::ToolCall(call)) = assistant.items.first() else {
        unreachable!()
    };
    let output = &call.result.as_ref().unwrap().output;
    let _ = output.text();
    assert!(text_calls() > texts, "reading the output is counted");
    assert!(source_reads() > reads, "re-reading the line is counted");
}

#[test]
fn a_turn_update_rebuilds_only_the_turn_that_changed() {
    let turns = long_session(5_000);
    let mut rows = rows_of(&turns);
    let others: Vec<_> = (0..rows.entry_count())
        .map(|e| rows.entry(e).unwrap().1)
        .collect();
    let builds = entry_builds();
    rows.apply(&updated(vec![assistant("a4001", 12_345)]));
    assert_eq!(entry_builds() - builds, 1, "one entry derived for one turn");
    for (e, before) in others.iter().enumerate() {
        let (turn, now) = rows.entry(e).unwrap();
        if turn.as_str() == "a4001" {
            assert_eq!(now.duration, Some(Duration::from_secs(12_345)));
        } else {
            assert_eq!(now, *before, "entry {e}");
        }
    }
    let builds = entry_builds();
    rows.apply(&appended(vec![user("new")]));
    assert_eq!(entry_builds() - builds, 1);
}

#[test]
fn the_sidebar_applies_an_update_by_touching_only_the_turn_named() {
    let mut sidebar = sidebar(&long_session(5_000));
    let builds = entry_builds();
    sidebar.attr(
        MODEL_UPDATE_ATTR,
        update(updated(vec![assistant("a1001", 3)])),
    );
    assert_eq!(entry_builds() - builds, 1);
}

#[test]
fn the_sidebar_builds_only_the_rows_it_shows() {
    let mut sidebar = sidebar(&long_session(10_000));
    assert!(sidebar.rows().row_count() > 20_000);
    let before = line::lines_built();
    let buffer = draw(&mut sidebar, 30, 40);
    // 40 rows: the border, two blank rows, 36 list rows and the border.
    assert_eq!(line::lines_built() - before, 36);
    assert!(row_text(&buffer, 3).contains("Start"));
}

#[test]
fn a_list_of_10_000_turns_is_drawn_and_updated_quickly() {
    let turns = long_session(10_000);
    let mut sidebar = sidebar(&turns);
    sidebar.attr(SELECTED_ATTR, selected_value(Some(5_000)));
    let start = std::time::Instant::now();
    for i in 0..20 {
        sidebar.attr(
            MODEL_UPDATE_ATTR,
            update(updated(vec![assistant("a5001", 100 + i)])),
        );
        draw(&mut sidebar, 30, 40);
    }
    // Generous for a debug build; the release budget is in tests/budgets.rs.
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn the_sidebar_keeps_no_turns() {
    let turn = Arc::new(assistant("a1", 5));
    let mut sidebar = TurnSidebar::new(builtin::default_theme());
    sidebar.attr(
        MODEL_UPDATE_ATTR,
        update(ModelUpdate::Appended(vec![Arc::clone(&turn)])),
    );
    sidebar.attr(
        MODEL_UPDATE_ATTR,
        update(ModelUpdate::Updated(vec![Arc::clone(&turn)])),
    );
    assert_eq!(sidebar.rows().entry_count(), 1);
    assert_eq!(
        Arc::strong_count(&turn),
        1,
        "the sidebar holds no Arc<Turn>"
    );
}

// -- durations

#[test]
fn an_assistant_entry_without_stats_shows_no_duration_and_gets_it_in_place() {
    let mut sidebar = sidebar(&[user("u1"), running("a1"), user("u2")]);
    let before = draw(&mut sidebar, 30, 20);
    let row = row_text(&before, 7);
    assert_eq!(row, "│  ●   Assistant             │");
    sidebar.attr(
        MODEL_UPDATE_ATTR,
        update(updated(vec![assistant("a1", 4 * 60 + 3)])),
    );
    let after = draw(&mut sidebar, 30, 20);
    assert_eq!(row_text(&after, 7), "│  ●   Assistant      4m 3s  │");
    // Nothing else moved.
    for y in (0..20).filter(|&y| y != 7) {
        assert_eq!(row_text(&before, y), row_text(&after, y), "row {y}");
    }
}

#[test]
fn a_duration_ends_at_the_right_margin_in_the_shared_format() {
    let mut sidebar = sidebar(&widest_durations());
    let buffer = draw(&mut sidebar, 30, 30);
    let mut seen = Vec::new();
    for y in 3..30 {
        let row = row_text(&buffer, y);
        if row.contains("Assistant") {
            let text: String = row.chars().skip(16).take(11).collect();
            // Columns 16..27: the duration ends in column 26 and columns 27
            // and 28 are blank.
            assert_eq!(row.chars().nth(27), Some(' '), "{row}");
            assert_eq!(row.chars().nth(28), Some(' '), "{row}");
            assert_eq!(row.chars().nth(29), Some('│'), "{row}");
            seen.push(text.trim().to_owned());
        }
    }
    assert_eq!(
        seen,
        ["0s", "59s", "59m 59s", "100h 30m", "26h 30m", "9999h 0m"]
    );
}

// -- the look

fn theme() -> Theme {
    builtin::default_theme()
}

fn fg(buffer: &Buffer, x: u16, y: u16) -> Color {
    buffer[(x, y)].fg
}

fn modifier(buffer: &Buffer, x: u16, y: u16) -> Modifier {
    buffer[(x, y)].modifier
}

/// A sidebar of the wireframe with entry `entry` selected.
fn selected(entry: usize, focused: bool) -> TurnSidebar {
    sidebar(&wireframe())
        .with_selected(Some(entry))
        .focused(focused)
}

#[test]
fn the_title_reads_the_key_and_has_two_spaces_before_the_rule() {
    let mut sidebar = sidebar(&wireframe());
    let buffer = draw(&mut sidebar, 30, 20);
    assert_eq!(row_text(&buffer, 0), "╭─ [1] Turns  ───────────────╮");
    assert_eq!(row_text(&buffer, 1), format!("│{}│", " ".repeat(28)));
    assert_eq!(row_text(&buffer, 2), format!("│{}│", " ".repeat(28)));
    assert_eq!(row_text(&buffer, 3), "│  ●   Start                 │");
    assert_eq!(row_text(&buffer, 19), format!("╰{}╯", "─".repeat(28)));
    let relabelled = TurnSidebar::new(theme()).key_label("2");
    let mut relabelled = relabelled;
    let buffer = draw(&mut relabelled, 30, 8);
    assert!(row_text(&buffer, 0).starts_with("╭─ [2] Turns  ─"));
}

#[test]
fn the_wireframe_is_drawn_as_specified() {
    let mut sidebar = sidebar(&wireframe());
    let rows: Vec<String> = buffer_to_text(&draw(&mut sidebar, 30, 20))
        .lines()
        .map(str::to_owned)
        .collect();
    let expected = [
        "╭─ [1] Turns  ───────────────╮",
        "│                            │",
        "│                            │",
        "│  ●   Start                 │",
        "│  │                         │",
        "│  ●   User                  │",
        "│  │                         │",
        "│  ●   Assistant      4m 3s  │",
        "│  │                         │",
        "│  ●   User                  │",
        "│  │                         │",
        "│  ●   Assistant      3h 4m  │",
        "│                            │",
        "│  ═══ Compaction ═════════  │",
        "│                            │",
        "│  ●   Assistant     2h 45m  │",
        "│  │                         │",
        "│  ●   End                   │",
        "│                            │",
        "╰────────────────────────────╯",
    ];
    assert_eq!(rows, expected);
}

#[test]
fn unselected_rows_are_muted_foreground() {
    let mut sidebar = sidebar(&wireframe());
    let buffer = draw(&mut sidebar, 30, 20);
    let muted = theme().role(Role::MutedForeground);
    // `●` on Start, `│`, a User label, a duration, the Compaction rule.
    for (x, y) in [(3, 3), (3, 4), (7, 5), (22, 7), (3, 13), (10, 13), (26, 13)] {
        assert_eq!(fg(&buffer, x, y), muted, "({x}, {y})");
        assert_eq!(modifier(&buffer, x, y), Modifier::empty(), "({x}, {y})");
    }
}

#[test]
fn the_border_is_ring_when_focused_and_border_otherwise() {
    let mut unfocused = sidebar(&wireframe());
    let buffer = draw(&mut unfocused, 30, 20);
    assert_eq!(fg(&buffer, 0, 5), theme().role(Role::Border));
    assert_eq!(fg(&buffer, 29, 5), theme().role(Role::Border));
    assert_eq!(fg(&buffer, 0, 0), theme().role(Role::Border));
    let mut focused = sidebar(&wireframe()).focused(true);
    let buffer = draw(&mut focused, 30, 20);
    for (x, y) in [(0, 5), (29, 5), (0, 0), (10, 19)] {
        assert_eq!(fg(&buffer, x, y), theme().role(Role::Ring), "({x}, {y})");
    }
    // The title text is foreground either way.
    assert_eq!(fg(&buffer, 4, 0), theme().role(Role::Foreground));
}

#[test]
fn a_selected_row_of_the_focused_pane_is_an_inverted_band_after_a_primary_marker() {
    for (entry, row, label) in [(0, 5, "User"), (1, 7, "Assistant"), (4, 13, "Compaction")] {
        let mut sidebar = selected(entry, true);
        let buffer = draw(&mut sidebar, 30, 20);
        let text = row_text(&buffer, row);
        assert_eq!(text.chars().nth(3), Some('>'), "{text}");
        assert!(text.contains(label), "{text}");
        let marker = &buffer[(3, row)];
        assert_eq!(marker.fg, theme().style(Token::SelectionMarker).fg.unwrap());
        assert_eq!(marker.fg, theme().role(Role::Primary));
        assert_eq!(marker.modifier, Modifier::empty(), "never inverted");
        // From column 4 to column 26: foreground, reversed, not bold.
        for x in 4..=26 {
            let cell = &buffer[(x, row)];
            assert_eq!(cell.fg, theme().role(Role::Foreground), "col {x}");
            assert_eq!(cell.modifier, Modifier::REVERSED, "col {x}");
        }
        // The margin and the border are not part of the band.
        for x in [27, 28] {
            assert_eq!(buffer[(x, row)].modifier, Modifier::empty(), "col {x}");
        }
        assert_eq!(buffer[(2, row)].modifier, Modifier::empty());
    }
}

#[test]
fn a_selected_assistant_row_gives_its_duration_the_bands_style() {
    let mut sidebar = selected(1, true);
    let buffer = draw(&mut sidebar, 30, 20);
    assert_eq!(cols(&buffer, 7, 22, 27), "4m 3s");
    for x in 22..=26 {
        assert_eq!(modifier(&buffer, x, 7), Modifier::REVERSED);
    }
}

#[test]
fn a_selected_compaction_has_the_marker_in_place_of_the_first_rule_cell() {
    let mut sidebar = selected(4, true);
    let buffer = draw(&mut sidebar, 30, 20);
    assert_eq!(row_text(&buffer, 13), "│  >══ Compaction ═════════  │");
}

#[test]
fn a_selected_row_of_the_unfocused_pane_is_bold_secondary_foreground_not_inverted() {
    for (entry, row) in [(0, 5), (1, 7), (4, 13)] {
        let mut sidebar = selected(entry, false);
        let buffer = draw(&mut sidebar, 30, 20);
        let secondary = theme().role(Role::SecondaryForeground);
        let text = row_text(&buffer, row);
        assert_eq!(text.chars().nth(3), Some('>'), "{text}");
        let marker = &buffer[(3, row)];
        assert_eq!((marker.fg, marker.modifier), (secondary, Modifier::empty()));
        for x in 4..=26 {
            let cell = &buffer[(x, row)];
            assert_eq!(cell.fg, secondary, "col {x}");
            assert_eq!(cell.modifier, Modifier::BOLD, "col {x}");
        }
    }
}

#[test]
fn only_the_selected_row_changes_with_the_selection() {
    let mut plain = sidebar(&wireframe());
    let plain = draw(&mut plain, 30, 20);
    let mut selected = selected(2, true);
    let selected = draw(&mut selected, 30, 20);
    for y in (0..20).filter(|&y| y != 9) {
        assert_eq!(row_text(&plain, y), row_text(&selected, y), "row {y}");
    }
    assert_eq!(row_text(&selected, 9), "│  >   User                  │");
}

#[test]
fn the_background_is_painted_over_the_pane_and_the_gap_unless_transparent() {
    let mut sidebar = sidebar(&wireframe());
    let buffer = crate::test_support::render(31, 20, |frame| {
        sidebar.view(frame, ratatui::layout::Rect::new(0, 0, 30, 20));
    });
    let background = theme().role(Role::Background);
    assert_eq!(buffer[(15, 10)].bg, background);
    assert_eq!(buffer[(30, 10)].bg, background, "the gap column");
}

// -- attributes

#[test]
fn the_selection_is_an_entry_and_pushing_it_produces_no_message() {
    let mut sidebar = sidebar(&wireframe());
    assert_eq!(sidebar.selected_entry(), None);
    assert_eq!(sidebar.state(), State::None);
    sidebar.attr(SELECTED_ATTR, selected_value(Some(3)));
    assert_eq!(sidebar.selected_entry(), Some(3));
    assert_eq!(sidebar.state(), State::Single(StateValue::Usize(3)));
    let queried = sidebar.query(SELECTED_ATTR).unwrap().into_attr();
    assert_eq!(selected_from_value(&queried), Some(Some(3)));
    sidebar.attr(SELECTED_ATTR, selected_value(Some(99)));
    assert_eq!(sidebar.selected_entry(), None, "no such entry");
    sidebar.attr(SELECTED_ATTR, selected_value(Some(2)));
    sidebar.attr(SELECTED_ATTR, selected_value(None));
    assert_eq!(sidebar.selected_entry(), None);
}

#[test]
fn the_selection_stays_on_its_turn_when_turns_are_inserted_before_it() {
    let mut sidebar = sidebar(&[user("u1"), assistant("a1", 5), user("u2")]);
    sidebar.attr(SELECTED_ATTR, selected_value(Some(2)));
    // `a0` is at the start; the selected turn is entry 3 now.
    sidebar.attr(MODEL_UPDATE_ATTR, update(updated(vec![user("u1")])));
    let mut rows = rows_of(&[user("x0"), user("u1"), assistant("a1", 5), user("u2")]);
    rows.apply(&ModelUpdate::Loaded);
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows));
    assert_eq!(sidebar.selected_entry(), Some(3));
}

#[test]
fn a_selected_turn_that_is_not_in_new_rows_is_deselected() {
    let mut sidebar = sidebar(&wireframe());
    sidebar.attr(SELECTED_ATTR, selected_value(Some(1)));
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&[user("other")])));
    assert_eq!(sidebar.selected_entry(), None);
    sidebar.attr(MODEL_UPDATE_ATTR, update(ModelUpdate::Reset));
    assert_eq!(kinds(sidebar.rows()), ["Start", "|", "End"]);
}

#[test]
fn a_selected_turn_that_loses_its_entry_is_deselected() {
    let mut sidebar = selected(1, true);
    sidebar.attr(
        MODEL_UPDATE_ATTR,
        update(updated(vec![redacted_only("a1")])),
    );
    assert_eq!(sidebar.selected_entry(), None);
}

#[test]
fn rows_are_replaced_through_an_attribute_and_can_be_queried() {
    let mut sidebar = TurnSidebar::new(theme());
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&wireframe())));
    let value = sidebar.query(TURN_ROWS_ATTR).unwrap().into_attr();
    assert_eq!(
        payload_from_value::<TurnRows>(&value),
        Some(rows_of(&wireframe()))
    );
}

#[test]
fn the_theme_focus_and_key_are_set_and_queried_through_attributes() {
    let mut sidebar = sidebar(&wireframe());
    let other = builtin::terminal_theme();
    sidebar.attr(THEME_ATTR, theme_value(&other));
    let value = sidebar.query(THEME_ATTR).unwrap().into_attr();
    assert_eq!(theme_from_value(&value), Some(other.clone()));
    let buffer = draw(&mut sidebar, 30, 20);
    assert_eq!(fg(&buffer, 3, 3), other.role(Role::MutedForeground));
    sidebar.attr(Attribute::Focus, AttrValue::Flag(true));
    assert_eq!(
        sidebar.query(Attribute::Focus).unwrap().into_attr(),
        AttrValue::Flag(true)
    );
    sidebar.attr(KEY_LABEL_ATTR, AttrValue::String("3".into()));
    let buffer = draw(&mut sidebar, 30, 20);
    assert!(row_text(&buffer, 0).starts_with("╭─ [3] Turns"));
    assert_eq!(fg(&buffer, 0, 5), other.role(Role::Ring));
}

#[test]
fn the_sidebar_takes_no_keys_and_no_mouse_yet() {
    use tuirealm::event::{Key, KeyEvent, KeyModifiers};
    let mut sidebar = selected(1, true);
    for code in [Key::Down, Key::Up, Key::Char('j'), Key::Enter] {
        let key = Event::Keyboard(KeyEvent::new(code, KeyModifiers::NONE));
        assert_eq!(sidebar.on(&key), None);
    }
    assert_eq!(sidebar.selected_entry(), Some(1));
}

// -- scrolling

#[test]
fn the_list_scrolls_only_as_far_as_needed_to_keep_the_selection_visible() {
    // A 20-row pane shows 16 list rows.
    let mut sidebar = sidebar(&long_session(40));
    sidebar.cursor.set_visible(16);
    assert_eq!(sidebar.offset(20), 0);
    // The entry on list row 14 is visible without scrolling.
    sidebar.attr(SELECTED_ATTR, selected_value(Some(6)));
    assert_eq!(sidebar.offset(20), 0);
    // Entry 7 is on list row 16, just below the 16 rows shown.
    sidebar.attr(SELECTED_ATTR, selected_value(Some(7)));
    assert_eq!(sidebar.offset(20), 1);
    // Moving back up inside the window does not scroll.
    sidebar.attr(SELECTED_ATTR, selected_value(Some(5)));
    assert_eq!(sidebar.offset(20), 1);
    sidebar.attr(SELECTED_ATTR, selected_value(Some(0)));
    assert_eq!(sidebar.offset(20), 1, "row 2 is still in the window");
    // Far down, the selected row is the last one shown.
    sidebar.attr(SELECTED_ATTR, selected_value(Some(15)));
    assert_eq!(sidebar.offset(20), 2 * 16 + 1 - 16);
    // Above the window it scrolls up to the selected row.
    sidebar.attr(SELECTED_ATTR, selected_value(Some(5)));
    assert_eq!(sidebar.offset(20), 2 * 6);
}

#[test]
fn start_and_end_scroll_with_the_list() {
    let mut sidebar = sidebar(&long_session(40));
    let top = draw(&mut sidebar, 30, 20);
    assert!(row_text(&top, 3).contains("Start"));
    assert!(!(0..20).any(|y| row_text(&top, y).contains("End")));
    let last_entry = sidebar.rows().entry_count() - 1;
    sidebar.attr(SELECTED_ATTR, selected_value(Some(last_entry)));
    let end = draw(&mut sidebar, 30, 20);
    assert!(!(0..20).any(|y| row_text(&end, y).contains("Start")));
    // The least scrolling shows the last entry on the last row; End is a
    // connector and a row further.
    assert!(row_text(&end, 18).contains('>') || row_text(&end, 18).contains("Compaction"));
}

#[test]
fn the_list_can_be_scrolled_to_show_end_on_the_last_row() {
    let mut sidebar = sidebar(&long_session(40));
    let rows = sidebar.rows().row_count();
    for _ in 0..rows {
        sidebar.cursor.scroll(true, 16);
    }
    let buffer = draw(&mut sidebar, 30, 20);
    assert_eq!(row_text(&buffer, 18), "│  ●   End                   │");
    assert_eq!(row_text(&buffer, 19), format!("╰{}╯", "─".repeat(28)));
}

#[test]
fn a_selection_is_never_off_screen_after_updates() {
    let mut sidebar = sidebar(&long_session(40));
    sidebar.cursor.set_visible(16);
    sidebar.attr(SELECTED_ATTR, selected_value(Some(30)));
    for i in 0..10 {
        sidebar.attr(
            MODEL_UPDATE_ATTR,
            update(appended(vec![
                user(&format!("n{i}")),
                assistant(&format!("m{i}"), 4),
            ])),
        );
        let buffer = draw(&mut sidebar, 30, 20);
        assert!(
            (3..19).any(|y| row_text(&buffer, y).chars().nth(3) == Some('>')),
            "{}",
            buffer_to_text(&buffer)
        );
    }
}

#[test]
fn a_tiny_area_draws_without_panicking() {
    let mut sidebar = selected(1, true);
    for (w, h) in [
        (0, 0),
        (1, 1),
        (2, 2),
        (5, 3),
        (11, 10),
        (12, 5),
        (30, 4),
        (30, 3),
    ] {
        draw(&mut sidebar, w, h);
    }
    let mut empty = TurnSidebar::new(theme());
    draw(&mut empty, 30, 5);
}

#[test]
fn thinking_alone_is_not_a_section() {
    // A guard for the fixtures: redacted thinking is no section, text is.
    let readable = with_stats(
        crate::components::transcript::fixtures::with_id(
            blocks::assistant(
                None,
                vec![blocks::thinking("th", Thinking::Text("hm".into()))],
                None,
            ),
            "a1",
        ),
        1,
    );
    assert!(entry_for(&readable).is_some());
}
