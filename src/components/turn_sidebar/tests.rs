//! Tests of the turns sidebar: the rows it gets from a transcript (product
//! §5.2, §6.5, §6.6), its look (§6.3) and the way it takes updates. How the
//! rows are built from turns is tested in `turn_rows`.

use std::sync::Arc;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Size;
use ratatui::style::{Color, Modifier};

use super::fixtures::*;
use super::*;
use crate::components::transcript::fixtures::{arc, long_output_turn, transcript_of, with_id};
use crate::components::turn_block::fixtures::block_work;
use crate::components::turn_rows::entry_builds;
use crate::components::{THEME_ATTR, Transcript};
use crate::model::{Item, Turn, TurnId, source_reads, text_calls};
use crate::test_support::buffer_to_text;
use crate::theme::{Role, Theme, Token, builtin};

mod snapshots;

fn update(update: ModelUpdate) -> AttrValue {
    payload_value(Arc::new(update))
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
fn rows_built_from_a_transcript_match_those_built_from_its_turns() {
    let turns = wireframe();
    let transcript = transcript_of(shared(&turns), 100, 30);
    let value = transcript.query(TURN_ROWS_ATTR).unwrap().into_attr();
    let from_transcript: TurnRows = payload_from_value(&value).unwrap();
    assert_eq!(from_transcript, rows_of(&turns));
}

// -- what building rows must not do (product §5.6)

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
    // 40 rows: the border, a blank row, 37 list rows and the border.
    assert_eq!(line::lines_built() - before, 37);
    assert!(row_text(&buffer, 2).contains("Start"));
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
    let row = row_text(&before, 6);
    assert_eq!(row, "│  ├   Assistant             │");
    sidebar.attr(
        MODEL_UPDATE_ATTR,
        update(updated(vec![assistant("a1", 4 * 60 + 3)])),
    );
    let after = draw(&mut sidebar, 30, 20);
    assert_eq!(row_text(&after, 6), "│  ├   Assistant      4m 3s  │");
    // Nothing else moved.
    for y in (0..20).filter(|&y| y != 6) {
        assert_eq!(row_text(&before, y), row_text(&after, y), "row {y}");
    }
}

#[test]
fn a_duration_ends_at_the_right_margin_in_the_shared_format() {
    let mut sidebar = sidebar(&widest_durations());
    let buffer = draw(&mut sidebar, 30, 30);
    let mut seen = Vec::new();
    for y in 2..30 {
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
        ["9999h 59m", "100h 30m", "26h 30m", "59m 59s", "59s", "0s"]
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
    assert_eq!(row_text(&buffer, 2), "│  ┬   Start                 │");
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
        "│  ┬   Start                 │",
        "│  │                         │",
        "│  ├   User                  │",
        "│  │                         │",
        "│  ├   Assistant      4m 3s  │",
        "│  │                         │",
        "│  ├   User                  │",
        "│  │                         │",
        "│  ├   Assistant      3h 4m  │",
        "│  │                         │",
        "│  ├╶╶ Compaction ╶╶╶╶╶╶╶╶╶  │",
        "│  │                         │",
        "│  ├   Assistant     2h 45m  │",
        "│  │                         │",
        "│  ┴   End                   │",
        "│                            │",
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
    // `┬` on Start, `│`, a User label, a duration, the Compaction row.
    for (x, y) in [(3, 2), (3, 3), (7, 4), (22, 6), (3, 12), (10, 12), (26, 12)] {
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
    for (entry, row, label) in [(0, 4, "User"), (1, 6, "Assistant"), (4, 12, "Compaction")] {
        let mut sidebar = selected(entry, true);
        let buffer = draw(&mut sidebar, 30, 20);
        let text = row_text(&buffer, row);
        assert_eq!(text.chars().nth(3), Some('▒'), "{text}");
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
    assert_eq!(cols(&buffer, 6, 22, 27), "4m 3s");
    for x in 22..=26 {
        assert_eq!(modifier(&buffer, x, 6), Modifier::REVERSED);
    }
}

#[test]
fn a_selected_compaction_has_the_marker_before_the_rule() {
    let mut sidebar = selected(4, true);
    let buffer = draw(&mut sidebar, 30, 20);
    assert_eq!(row_text(&buffer, 12), "│  ▒╶╶ Compaction ╶╶╶╶╶╶╶╶╶  │");
}

#[test]
fn a_selected_row_of_the_unfocused_pane_is_bold_secondary_foreground_not_inverted() {
    for (entry, row) in [(0, 4), (1, 6), (4, 12)] {
        let mut sidebar = selected(entry, false);
        let buffer = draw(&mut sidebar, 30, 20);
        let secondary = theme().role(Role::SecondaryForeground);
        let text = row_text(&buffer, row);
        assert_eq!(text.chars().nth(3), Some('▒'), "{text}");
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
    for y in (0..20).filter(|&y| y != 8) {
        assert_eq!(row_text(&plain, y), row_text(&selected, y), "row {y}");
    }
    assert_eq!(row_text(&selected, 8), "│  ▒   User                  │");
}

#[test]
fn the_background_is_painted_over_the_pane_and_only_the_pane() {
    let mut sidebar = sidebar(&wireframe());
    // A frame one column wider than the pane: the column after it is the
    // gap's, which `SidebarGap` paints, not the pane.
    let buffer = crate::test_support::render(31, 20, |frame| {
        sidebar.view(frame, ratatui::layout::Rect::new(0, 0, 30, 20));
    });
    let background = theme().role(Role::Background);
    for (x, y) in [(1, 1), (15, 10), (28, 18), (29, 10)] {
        assert_eq!(buffer[(x, y)].bg, background, "({x}, {y})");
    }
    for y in [0, 10, 19] {
        assert_eq!(buffer[(30, y)].bg, Color::Reset, "the gap column, row {y}");
    }
}

#[test]
fn a_transparent_theme_leaves_the_pane_unpainted() {
    let transparent = theme().with_transparent_background(true);
    let mut sidebar = TurnSidebar::new(transparent).with_rows(rows_of(&wireframe()));
    let buffer = draw(&mut sidebar, 30, 20);
    for (x, y) in [(1, 1), (15, 10), (28, 18), (5, 4), (10, 12)] {
        assert_eq!(buffer[(x, y)].bg, Color::Reset, "({x}, {y})");
    }
    // The text is still drawn, and a band is still inverted.
    assert_eq!(row_text(&buffer, 4), "│  ├   User                  │");
    let mut selected = TurnSidebar::new(theme().with_transparent_background(true))
        .with_rows(rows_of(&wireframe()))
        .with_selected(Some(0))
        .focused(true);
    let buffer = draw(&mut selected, 30, 20);
    assert_eq!(modifier(&buffer, 10, 4), Modifier::REVERSED);
    assert_eq!(buffer[(10, 4)].bg, Color::Reset);
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
    // `x0` is now first; the selected turn, `u2`, is entry 3.
    let rows = rows_of(&[user("x0"), user("u1"), assistant("a1", 5), user("u2")]);
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows));
    assert_eq!(sidebar.selected_entry(), Some(3));
}

#[test]
fn a_selected_turn_that_is_not_in_new_rows_is_deselected() {
    let mut sidebar = sidebar(&wireframe());
    sidebar.attr(SELECTED_ATTR, selected_value(Some(1)));
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&[user("other")])));
    assert_eq!(sidebar.selected_entry(), None);
    // Rows that hold the turn again do not bring the selection back.
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&wireframe())));
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
    // The entry comes back; the selection does not.
    sidebar.attr(MODEL_UPDATE_ATTR, update(updated(vec![assistant("a1", 9)])));
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
    assert_eq!(fg(&buffer, 3, 2), other.role(Role::MutedForeground));
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

// -- keys

fn key(code: tuirealm::event::Key) -> Event<UserEvent> {
    Event::Keyboard(tuirealm::event::KeyEvent::new(
        code,
        tuirealm::event::KeyModifiers::NONE,
    ))
}

fn ch(c: char) -> Event<UserEvent> {
    key(tuirealm::event::Key::Char(c))
}

/// The turn of entry `entry` of the wireframe.
fn turn_of(entry: usize) -> TurnId {
    id(["u1", "a1", "u2", "a2", "c1", "a3"][entry])
}

#[test]
fn down_and_up_select_the_next_and_previous_entry_and_name_its_turn() {
    use tuirealm::event::Key;
    let mut sidebar = selected(1, true);
    for event in [key(Key::Down), ch('j')] {
        let before = sidebar.selected_entry().unwrap();
        assert_eq!(
            sidebar.on(&event),
            Some(Msg::TurnSelected(turn_of(before + 1)))
        );
        assert_eq!(sidebar.selected_entry(), Some(before + 1));
    }
    assert_eq!(sidebar.selected_entry(), Some(3));
    for event in [key(Key::Up), ch('k')] {
        let before = sidebar.selected_entry().unwrap();
        assert_eq!(
            sidebar.on(&event),
            Some(Msg::TurnSelected(turn_of(before - 1)))
        );
    }
    assert_eq!(sidebar.selected_entry(), Some(1));
}

#[test]
fn the_list_never_wraps_around() {
    use tuirealm::event::Key;
    let mut sidebar = selected(0, true);
    assert_eq!(sidebar.on(&key(Key::Up)), None);
    assert_eq!(sidebar.on(&ch('k')), None);
    assert_eq!(sidebar.selected_entry(), Some(0));
    let mut sidebar = selected(5, true);
    assert_eq!(sidebar.on(&key(Key::Down)), None);
    assert_eq!(sidebar.on(&ch('j')), None);
    assert_eq!(sidebar.selected_entry(), Some(5));
}

#[test]
fn with_no_selection_the_first_move_in_either_direction_selects_the_first_entry() {
    for event in [ch('j'), ch('k')] {
        let mut sidebar = sidebar(&wireframe()).focused(true);
        assert_eq!(sidebar.selected_entry(), None);
        assert_eq!(sidebar.on(&event), Some(Msg::TurnSelected(turn_of(0))));
        assert_eq!(sidebar.selected_entry(), Some(0));
    }
    // The second move goes on from there.
    let mut sidebar = sidebar(&wireframe()).focused(true);
    sidebar.on(&ch('k'));
    assert_eq!(
        sidebar.on(&ch('k')),
        None,
        "the first entry has no previous"
    );
    assert_eq!(sidebar.on(&ch('j')), Some(Msg::TurnSelected(turn_of(1))));
}

#[test]
fn the_keys_do_nothing_on_an_empty_list() {
    use tuirealm::event::Key;
    let mut sidebar = TurnSidebar::new(theme()).focused(true);
    for event in [
        key(Key::Down),
        key(Key::Up),
        ch('j'),
        ch('k'),
        key(Key::Enter),
    ] {
        assert_eq!(sidebar.on(&event), None);
    }
    assert_eq!(sidebar.selected_entry(), None);
    // A list of turns with no entry is empty too.
    let mut sidebar = sidebar_of(&[system_only("s1")]);
    assert_eq!(sidebar.on(&ch('j')), None);
    assert_eq!(sidebar.on(&key(Key::Enter)), None);
}

fn sidebar_of(turns: &[Turn]) -> TurnSidebar {
    sidebar(turns).focused(true)
}

#[test]
fn enter_names_the_selected_turn_and_does_not_move_the_selection() {
    use tuirealm::event::Key;
    let mut sidebar = selected(4, true);
    assert_eq!(
        sidebar.on(&key(Key::Enter)),
        Some(Msg::OpenTurn(turn_of(4)))
    );
    assert_eq!(sidebar.selected_entry(), Some(4));
    // With nothing selected it has no block to go to.
    let mut sidebar = sidebar_of(&wireframe());
    assert_eq!(sidebar.on(&key(Key::Enter)), None);
}

#[test]
fn the_pane_keys_ask_for_the_transcripts_focus_and_leave_the_selection() {
    use tuirealm::event::{Key, KeyEvent, KeyModifiers};
    let mut sidebar = selected(2, true);
    let shift_tab = Event::Keyboard(KeyEvent::new(Key::BackTab, KeyModifiers::SHIFT));
    for event in [key(Key::Tab), shift_tab, key(Key::Right), ch('l')] {
        assert_eq!(sidebar.on(&event), Some(Msg::FocusTranscript));
    }
    assert_eq!(sidebar.selected_entry(), Some(2));
}

#[test]
fn the_other_keys_do_nothing_and_neither_does_a_raw_mouse_event() {
    use tuirealm::event::{Key, MouseEvent, MouseEventKind};
    let mut sidebar = selected(2, true);
    for event in [
        ch('1'),
        key(Key::Left),
        ch('h'),
        ch('/'),
        ch('y'),
        ch('g'),
        key(Key::Esc),
        key(Key::PageDown),
        ch(' '),
        ch('q'),
    ] {
        assert_eq!(sidebar.on(&event), None, "{event:?}");
    }
    // Raw mouse events (screen coordinates, for the focused component) are
    // not the `App`'s routed ones: they do nothing, wherever they land.
    for kind in [
        MouseEventKind::Down(tuirealm::event::MouseButton::Left),
        MouseEventKind::ScrollDown,
        MouseEventKind::ScrollUp,
    ] {
        for (column, row) in [(3, 6), (10, 5), (10, 3), (1, 1)] {
            let raw = Event::Mouse(MouseEvent {
                kind,
                modifiers: tuirealm::event::KeyModifiers::NONE,
                column,
                row,
            });
            assert_eq!(sidebar.on(&raw), None, "{kind:?} at ({column}, {row})");
        }
    }
    assert_eq!(sidebar.on(&Event::Tick), None);
    assert_eq!(sidebar.selected_entry(), Some(2));
}

#[test]
fn the_keys_come_from_the_keymap_it_is_given() {
    use tuirealm::event::Key;
    let mut raw = crate::config::RawKeys::new();
    let turns = raw.entry("turns".to_owned()).or_default();
    turns.insert("down".to_owned(), vec!["s".to_owned()]);
    turns.insert("open".to_owned(), vec!["o".to_owned()]);
    turns.insert("focus_transcript".to_owned(), vec!["x".to_owned()]);
    let (keymap, warnings) = Keymap::from_config(&raw);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut sidebar = selected(1, true).keymap(keymap);
    assert_eq!(sidebar.on(&ch('j')), None, "the default is replaced");
    assert_eq!(sidebar.on(&key(Key::Down)), None);
    assert_eq!(sidebar.on(&ch('s')), Some(Msg::TurnSelected(turn_of(2))));
    assert_eq!(sidebar.on(&ch('o')), Some(Msg::OpenTurn(turn_of(2))));
    assert_eq!(sidebar.on(&ch('x')), Some(Msg::FocusTranscript));
    assert_eq!(sidebar.on(&ch('l')), None);
    // Untouched actions keep their keys.
    assert_eq!(sidebar.on(&ch('k')), Some(Msg::TurnSelected(turn_of(1))));
}

#[test]
fn a_chord_in_the_turns_context_works() {
    let mut raw = crate::config::RawKeys::new();
    raw.entry("turns".to_owned())
        .or_default()
        .insert("down".to_owned(), vec!["g g".to_owned()]);
    let (keymap, warnings) = Keymap::from_config(&raw);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut sidebar = selected(0, true).keymap(keymap);
    assert_eq!(sidebar.on(&ch('g')), None, "the prefix waits");
    assert_eq!(sidebar.selected_entry(), Some(0));
    assert_eq!(sidebar.on(&ch('g')), Some(Msg::TurnSelected(turn_of(1))));
}

#[test]
fn the_cmds_move_and_submit() {
    let mut sidebar = selected(1, true);
    assert_eq!(
        sidebar.perform(Cmd::Move(Direction::Down)),
        CmdResult::Changed(State::Single(StateValue::Usize(2)))
    );
    assert_eq!(
        sidebar.perform(Cmd::Submit),
        CmdResult::Submit(State::Single(StateValue::Usize(2)))
    );
    assert_eq!(
        sidebar.perform(Cmd::Scroll(Direction::Down)),
        CmdResult::Invalid(Cmd::Scroll(Direction::Down))
    );
    let mut sidebar = selected(5, true);
    assert_eq!(
        sidebar.perform(Cmd::Move(Direction::Down)),
        CmdResult::NoChange
    );
}

#[test]
fn a_move_keeps_the_selected_row_visible() {
    use tuirealm::event::Key;
    // A 20-row pane shows 17 list rows.
    let mut sidebar = sidebar(&long_session(40)).focused(true);
    sidebar.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(17));
    for _ in 0..30 {
        sidebar.on(&key(Key::Down));
    }
    let entry = sidebar.selected_entry().unwrap();
    assert_eq!(entry, 29);
    let row = TurnRows::row_of_entry(entry);
    let offset = sidebar.offset(20);
    assert!((offset..offset + 17).contains(&row), "{offset} for {row}");
}

#[test]
fn a_key_that_moves_the_selection_lays_out_and_renders_no_block() {
    use tuirealm::event::Key;
    let mut sidebar = sidebar(&long_session(60)).focused(true);
    let before = (block_work(), entry_builds());
    for _ in 0..20 {
        sidebar.on(&key(Key::Down));
    }
    draw(&mut sidebar, 30, 20);
    assert_eq!((block_work(), entry_builds()), before);
}

#[test]
fn the_title_reads_just_turns_when_the_action_has_no_key() {
    let mut sidebar = TurnSidebar::new(theme()).key_label("");
    let buffer = draw(&mut sidebar, 30, 8);
    assert_eq!(row_text(&buffer, 0), "╭─ Turns  ───────────────────╮");
    sidebar.attr(KEY_LABEL_ATTR, AttrValue::String("F2".into()));
    let buffer = draw(&mut sidebar, 30, 8);
    assert_eq!(row_text(&buffer, 0), "╭─ [F2] Turns  ──────────────╮");
}

// -- scrolling

#[test]
fn the_list_scrolls_only_as_far_as_needed_to_keep_the_selection_visible() {
    // A 20-row pane shows 17 list rows.
    let mut sidebar = sidebar(&long_session(40));
    sidebar.cursor.set_visible(17);
    assert_eq!(sidebar.offset(20), 0);
    // The entry on list row 16 is visible without scrolling.
    sidebar.attr(SELECTED_ATTR, selected_value(Some(7)));
    assert_eq!(sidebar.offset(20), 0);
    // Entry 8 is on list row 18, just below the 17 rows shown.
    sidebar.attr(SELECTED_ATTR, selected_value(Some(8)));
    assert_eq!(sidebar.offset(20), 2);
    // Moving back up inside the window does not scroll.
    sidebar.attr(SELECTED_ATTR, selected_value(Some(5)));
    assert_eq!(sidebar.offset(20), 2);
    sidebar.attr(SELECTED_ATTR, selected_value(Some(0)));
    assert_eq!(sidebar.offset(20), 2, "row 2 is still in the window");
    // Far down, the selected row is the last one shown.
    sidebar.attr(SELECTED_ATTR, selected_value(Some(15)));
    assert_eq!(sidebar.offset(20), 2 * 16 + 1 - 17);
    // Above the window it scrolls up to the selected row.
    sidebar.attr(SELECTED_ATTR, selected_value(Some(5)));
    assert_eq!(sidebar.offset(20), 2 * 6);
}

#[test]
fn start_and_end_scroll_with_the_list() {
    let mut sidebar = sidebar(&long_session(40));
    let top = draw(&mut sidebar, 30, 20);
    assert!(row_text(&top, 2).contains("Start"));
    assert!(!(0..20).any(|y| row_text(&top, y).contains("End")));
    let last_entry = sidebar.rows().entry_count() - 1;
    sidebar.attr(SELECTED_ATTR, selected_value(Some(last_entry)));
    let end = draw(&mut sidebar, 30, 20);
    assert!(!(0..20).any(|y| row_text(&end, y).contains("Start")));
    // The least scrolling shows the last entry (a Compaction, selected) on the
    // last row; End is a connector and a row further.
    assert_eq!(row_text(&end, 18), "│  ▒╶╶ Compaction ╶╶╶╶╶╶╶╶╶  │");
}

#[test]
fn the_list_can_be_scrolled_to_show_end_on_the_last_row() {
    let mut sidebar = sidebar(&long_session(40));
    let rows = sidebar.rows().row_count();
    for _ in 0..rows {
        sidebar.cursor.scroll(true, 17);
    }
    let buffer = draw(&mut sidebar, 30, 20);
    assert_eq!(row_text(&buffer, 18), "│  ┴   End                   │");
    assert_eq!(row_text(&buffer, 19), format!("╰{}╯", "─".repeat(28)));
}

#[test]
fn a_selection_is_never_off_screen_after_updates() {
    let mut sidebar = sidebar(&long_session(40));
    sidebar.cursor.set_visible(17);
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
            (2..19).any(|y| row_text(&buffer, y).chars().nth(3) == Some('▒')),
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

// -- selecting a turn (the sidebar follows the transcript)

#[test]
fn a_turn_with_an_entry_is_selected_through_an_attribute() {
    let mut sidebar = sidebar(&wireframe());
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u2")));
    assert_eq!(sidebar.selected_entry(), Some(2));
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("c1")));
    assert_eq!(sidebar.selected_entry(), Some(4));
    assert_eq!(sidebar.state(), State::Single(StateValue::Usize(4)));
}

#[test]
fn a_turn_without_an_entry_or_unknown_leaves_the_selection_where_it_is() {
    let mut sidebar = sidebar(&[user("u1"), system_only("s1"), user("u2")]);
    // Nothing selected stays nothing selected.
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("s1")));
    assert_eq!(sidebar.selected_entry(), None);
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u2")));
    assert_eq!(sidebar.selected_entry(), Some(1));
    // A turn with no entry neither clears nor moves it, nor an unknown one.
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("s1")));
    assert_eq!(sidebar.selected_entry(), Some(1));
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("nope")));
    assert_eq!(sidebar.selected_entry(), Some(1));
    // Something that is not a turn is ignored.
    sidebar.attr(SELECTED_TURN_ATTR, AttrValue::Flag(true));
    assert_eq!(sidebar.selected_entry(), Some(1));
}

#[test]
fn selecting_a_turn_scrolls_the_list_by_the_least_needed() {
    let mut sidebar = TurnSidebar::new(theme());
    sidebar.attr(
        TURN_ROWS_ATTR,
        payload_value(rows_of(
            &(0..30).map(|i| user(&format!("u{i}"))).collect::<Vec<_>>(),
        )),
    );
    sidebar.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(9));
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u0")));
    assert_eq!(sidebar.offset(12), 0);
    // Entry 3 is on row 8, the last visible row: no scroll yet.
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u3")));
    assert_eq!(sidebar.offset(12), 0);
    // Entry 4 is on row 10: the list scrolls two rows.
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u4")));
    assert_eq!(sidebar.offset(12), 2);
    // Back up to the first visible row: no scroll; one above it: scrolls.
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u1")));
    assert_eq!(sidebar.offset(12), 2);
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u0")));
    assert_eq!(sidebar.offset(12), 2, "row 2 is the first visible row");
}

#[test]
fn rows_set_after_an_uncover_bring_back_the_selection_and_offset_of_the_covered_pane() {
    let turns: Vec<_> = (0..30).map(|i| user(&format!("u{i}"))).collect();
    let mut sidebar = TurnSidebar::new(theme());
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&turns)));
    sidebar.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(9));
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u10")));
    // Row 22 is the last visible row: the offset is 14.
    assert_eq!(sidebar.offset(12), 14);
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u7")));
    assert_eq!(sidebar.offset(12), 14, "row 16: still visible");
    // A transcript of two turns covers it.
    sidebar.attr(COVER_ATTR, AttrValue::Flag(true));
    sidebar.attr(
        TURN_ROWS_ATTR,
        payload_value(rows_of(&[user("s1"), assistant("s2", 1)])),
    );
    assert_eq!(sidebar.selected_entry(), None);
    assert_eq!(sidebar.offset(12), 0);
    // It is closed: the parent's rows, as they are now, with its pane.
    sidebar.attr(COVER_ATTR, AttrValue::Flag(false));
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&turns)));
    assert_eq!(sidebar.selected_entry(), Some(7));
    assert_eq!(sidebar.offset(12), 14);
}

#[test]
fn a_restored_selection_scrolls_the_least_needed_and_covers_nest() {
    let turns: Vec<_> = (0..30).map(|i| user(&format!("u{i}"))).collect();
    let mut sidebar = TurnSidebar::new(theme());
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&turns)));
    sidebar.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(9));
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u10")));
    sidebar.attr(COVER_ATTR, AttrValue::Flag(true));
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&turns[..5])));
    sidebar.attr(SELECTED_TURN_ATTR, payload_value(id("u3")));
    sidebar.attr(COVER_ATTR, AttrValue::Flag(true));
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&turns[..2])));
    sidebar.attr(COVER_ATTR, AttrValue::Flag(false));
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&turns[..5])));
    assert_eq!(sidebar.selected_entry(), Some(3));
    // The pane shrank meanwhile: three rows fewer are visible.
    sidebar.attr(COVER_ATTR, AttrValue::Flag(false));
    sidebar.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(5));
    sidebar.attr(TURN_ROWS_ATTR, payload_value(rows_of(&turns)));
    assert_eq!(sidebar.selected_entry(), Some(10));
    assert_eq!(
        sidebar.offset(8),
        18,
        "row 22 on the last of 5 visible rows"
    );
}

// -- the mouse

mod mouse {
    use tuirealm::event::{MouseButton, MouseEventKind};

    use super::*;
    use crate::test_support::TestClock;

    /// The size of the pane the events are routed in: 30 x 14 shows eleven
    /// list rows (rows 2..13 of the pane).
    const PANE: Size = Size::new(30, 14);

    fn routed(kind: MouseEventKind, column: u16, row: u16, area: Size) -> Event<UserEvent> {
        Event::User(UserEvent::Pointer(RoutedMouse {
            kind,
            column,
            row,
            area,
        }))
    }

    fn press(column: u16, row: u16) -> Event<UserEvent> {
        routed(MouseEventKind::Down(MouseButton::Left), column, row, PANE)
    }

    fn wheel(down: bool) -> Event<UserEvent> {
        let kind = if down {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        };
        routed(kind, 10, 5, PANE)
    }

    /// The answer to a press on the entry of `turn`.
    fn chosen(turn: &str, enter: bool) -> Option<Msg> {
        Some(Msg::ChooseTurn {
            id: id(turn),
            enter,
        })
    }

    /// The wireframe's list rows: 0 Start, 1 |, 2 u1, 3 |, 4 a1, 5 |, 6 u2,
    /// 7 |, 8 a2, 9 |, 10 c1, 11 |, 12 a3, 13 |, 14 End.
    fn clicked(turn: &str) -> Option<Msg> {
        chosen(turn, false)
    }

    #[test]
    fn a_left_press_on_an_entrys_row_names_its_turn_wherever_in_the_row() {
        // A second apart, so that no two presses are a double-click.
        let clock = TestClock::new();
        let mut sidebar = sidebar_of(&wireframe()).with_click_clock(clock.click_clock());
        // The list starts at pane row 2 and has no offset: the entries are on
        // rows 4, 6, 8 and 10 (list rows 2, 4, 6 and 8).
        for (seconds, column) in (0..).zip([1, 2, 3, 7, 15, 28]) {
            clock.set(Duration::from_secs(seconds));
            assert_eq!(sidebar.on(&press(column, 4)), clicked("u1"), "{column}");
        }
        assert_eq!(sidebar.on(&press(10, 6)), clicked("a1"));
        assert_eq!(sidebar.on(&press(10, 8)), clicked("u2"));
        assert_eq!(sidebar.on(&press(10, 10)), clicked("a2"));
        // The pane does not move its selection: the `App` decides.
        assert_eq!(sidebar.selected_entry(), None);
    }

    #[test]
    fn a_press_elsewhere_in_the_pane_answers_nothing() {
        let mut sidebar = sidebar_of(&wireframe());
        let nothing = [
            (10, 0, "top border"),
            (10, 1, "blank row"),
            (10, 2, "Start"),
            (10, 3, "connector"),
            (10, 5, "connector"),
            (0, 4, "left border"),
            (29, 4, "right border"),
            (10, 13, "the bottom border"),
            (10, 100, "far below"),
        ];
        for (column, row, what) in nothing {
            assert_eq!(sidebar.on(&press(column, row)), None, "{what}");
        }
        // The bottom border of a pane that shows all the rows.
        let tall = Size::new(30, 40);
        let bottom = routed(MouseEventKind::Down(MouseButton::Left), 10, 39, tall);
        assert_eq!(sidebar.on(&bottom), None);
        // `End` and the row above it.
        let end = routed(MouseEventKind::Down(MouseButton::Left), 10, 2 + 14, tall);
        assert_eq!(sidebar.on(&end), None, "End");
        let above = routed(MouseEventKind::Down(MouseButton::Left), 10, 2 + 13, tall);
        assert_eq!(sidebar.on(&above), None, "the connector above End");
        let entry = routed(MouseEventKind::Down(MouseButton::Left), 10, 2 + 12, tall);
        assert_eq!(sidebar.on(&entry), clicked("a3"));
    }

    #[test]
    fn a_connector_next_to_a_compaction_is_not_an_entry_and_the_compaction_is() {
        let mut sidebar = sidebar_of(&wireframe());
        let tall = Size::new(30, 40);
        let at = |row: u16| routed(MouseEventKind::Down(MouseButton::Left), 10, 2 + row, tall);
        assert_eq!(sidebar.on(&at(9)), None, "connector above the Compaction");
        assert_eq!(sidebar.on(&at(10)), clicked("c1"));
        assert_eq!(sidebar.on(&at(11)), None, "connector below the Compaction");
    }

    #[test]
    fn the_other_buttons_and_mouse_events_answer_nothing() {
        let mut sidebar = sidebar_of(&wireframe());
        for kind in [
            MouseEventKind::Down(MouseButton::Right),
            MouseEventKind::Down(MouseButton::Middle),
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::Moved,
            MouseEventKind::ScrollLeft,
            MouseEventKind::ScrollRight,
        ] {
            assert_eq!(sidebar.on(&routed(kind, 10, 5, PANE)), None, "{kind:?}");
        }
        assert_eq!(sidebar.selected_entry(), None);
    }

    #[test]
    fn a_row_is_found_by_the_scroll_offset() {
        // Eleven rows are shown in a pane of 14; selecting entry 20 of a long
        // list scrolls it.
        let turns = long_session(30);
        let mut sidebar = sidebar_of(&turns).with_selected(Some(20));
        let offset = sidebar.offset(PANE.height);
        assert!(offset > 0, "the list is scrolled");
        // Entry `e` is on list row `2 (e + 1)`, at pane row `2 + row - offset`.
        let pane_row =
            |entry: usize| u16::try_from(2 + TurnRows::row_of_entry(entry) - offset).unwrap();
        for entry in [18, 19, 20] {
            assert_eq!(
                sidebar.on(&press(10, pane_row(entry))),
                Some(Msg::ChooseTurn {
                    id: sidebar.rows().entry(entry).unwrap().0.clone(),
                    enter: false,
                }),
                "entry {entry}"
            );
        }
        // The connector between two of them is not an entry.
        assert_eq!(sidebar.on(&press(10, pane_row(19) + 1)), None);
        assert_eq!(sidebar.on(&press(10, pane_row(19) - 1)), None);
        // And the rows that are scrolled away are not under the pointer.
        assert_eq!(sidebar.selected_entry(), Some(20));
    }

    #[test]
    fn the_pane_row_of_the_first_list_row_is_the_third() {
        // `Start` is list row 0 at pane row 2, the first entry two rows
        // below, whatever the pane's height.
        for height in [8, 12, 40] {
            let mut sidebar = sidebar_of(&wireframe());
            let size = Size::new(30, height);
            let press = |row| routed(MouseEventKind::Down(MouseButton::Left), 10, row, size);
            assert_eq!(sidebar.on(&press(2)), None, "{height}: Start");
            assert_eq!(sidebar.on(&press(3)), None, "{height}: connector");
            let shown = height.saturating_sub(3);
            let first = sidebar.on(&press(4));
            if shown >= 3 {
                assert_eq!(first, clicked("u1"), "{height}");
            } else {
                assert_eq!(first, None, "{height}");
            }
        }
    }

    #[test]
    fn a_pane_too_small_to_show_a_row_gives_no_click() {
        let mut sidebar = sidebar_of(&wireframe());
        for size in [
            Size::new(0, 0),
            Size::new(2, 10),
            Size::new(30, 3),
            Size::new(1, 1),
        ] {
            for (column, row) in [(0, 0), (1, 2), (1, 4), (5, 4)] {
                let event = routed(MouseEventKind::Down(MouseButton::Left), column, row, size);
                assert_eq!(sidebar.on(&event), None, "{size:?} ({column}, {row})");
            }
        }
    }

    #[test]
    fn the_wheel_selects_the_next_and_previous_entry_like_the_keys() {
        let mut sidebar = selected(1, false);
        assert_eq!(
            sidebar.on(&wheel(true)),
            Some(Msg::TurnSelected(turn_of(2)))
        );
        assert_eq!(sidebar.selected_entry(), Some(2));
        assert_eq!(
            sidebar.on(&wheel(true)),
            Some(Msg::TurnSelected(turn_of(3)))
        );
        assert_eq!(
            sidebar.on(&wheel(false)),
            Some(Msg::TurnSelected(turn_of(2)))
        );
        assert_eq!(sidebar.selected_entry(), Some(2));
        // It does not need the focus, and does not take it.
        assert_eq!(
            sidebar.query(Attribute::Focus).map(|q| q.into_attr()),
            Some(AttrValue::Flag(false))
        );
    }

    #[test]
    fn the_wheel_selects_the_first_entry_when_nothing_is_selected() {
        for down in [true, false] {
            let mut sidebar = sidebar_of(&wireframe());
            assert_eq!(
                sidebar.on(&wheel(down)),
                Some(Msg::TurnSelected(turn_of(0)))
            );
        }
    }

    #[test]
    fn the_wheel_does_not_wrap_around() {
        let mut sidebar = selected(0, false);
        assert_eq!(sidebar.on(&wheel(false)), None);
        assert_eq!(sidebar.selected_entry(), Some(0));
        let last = sidebar.rows().entry_count() - 1;
        let mut sidebar = selected(last, false);
        assert_eq!(sidebar.on(&wheel(true)), None);
        assert_eq!(sidebar.selected_entry(), Some(last));
    }

    #[test]
    fn the_wheel_over_an_empty_list_answers_nothing() {
        let mut sidebar = sidebar_of(&[]);
        assert_eq!(sidebar.on(&wheel(true)), None);
        assert_eq!(sidebar.on(&wheel(false)), None);
    }

    // -- double-clicks

    /// The wireframe's sidebar, double-clicks measured with the returned
    /// clock. Pane row 4 is `u1`'s entry, 6 `a1`'s, 5 the connector between.
    fn timed() -> (TurnSidebar, TestClock) {
        let clock = TestClock::new();
        let sidebar = sidebar_of(&wireframe()).with_click_clock(clock.click_clock());
        (sidebar, clock)
    }

    const ELSEWHERE: Event<UserEvent> = Event::User(UserEvent::PointerElsewhere);

    #[test]
    fn two_presses_on_an_entry_at_most_400_ms_apart_enter_it() {
        for (ms, enter) in [(0, true), (400, true), (401, false)] {
            let (mut sidebar, clock) = timed();
            assert_eq!(sidebar.on(&press(10, 4)), chosen("u1", false));
            clock.set_ms(ms);
            assert_eq!(sidebar.on(&press(3, 4)), chosen("u1", enter), "{ms} ms");
        }
    }

    #[test]
    fn a_third_press_starts_a_new_double_click() {
        let (mut sidebar, clock) = timed();
        sidebar.on(&press(10, 4));
        clock.set_ms(100);
        assert_eq!(sidebar.on(&press(10, 4)), chosen("u1", true));
        clock.set_ms(200);
        assert_eq!(sidebar.on(&press(10, 4)), chosen("u1", false));
        clock.set_ms(300);
        assert_eq!(sidebar.on(&press(10, 4)), chosen("u1", true));
    }

    #[test]
    fn a_press_on_another_entry_ends_a_double_click() {
        let (mut sidebar, clock) = timed();
        sidebar.on(&press(10, 4));
        clock.set_ms(100);
        assert_eq!(sidebar.on(&press(10, 6)), chosen("a1", false));
        clock.set_ms(200);
        assert_eq!(sidebar.on(&press(10, 4)), chosen("u1", false));
    }

    #[test]
    fn a_press_off_the_entries_ends_a_double_click() {
        // A connector, `Start`, the blank row, the border.
        for (column, row) in [(10, 5), (10, 2), (10, 1), (0, 4)] {
            let (mut sidebar, clock) = timed();
            sidebar.on(&press(10, 4));
            clock.set_ms(100);
            assert_eq!(sidebar.on(&press(column, row)), None);
            clock.set_ms(200);
            assert_eq!(
                sidebar.on(&press(10, 4)),
                chosen("u1", false),
                "{column}, {row}"
            );
        }
    }

    #[test]
    fn a_press_elsewhere_ends_a_double_click_and_answers_nothing() {
        let (mut sidebar, clock) = timed();
        sidebar.on(&press(10, 4));
        clock.set_ms(100);
        assert_eq!(sidebar.on(&ELSEWHERE), None);
        assert_eq!(sidebar.selected_entry(), None);
        clock.set_ms(200);
        assert_eq!(sidebar.on(&press(10, 4)), chosen("u1", false));
    }

    #[test]
    fn the_wheel_does_not_end_a_double_click() {
        let (mut sidebar, clock) = timed();
        sidebar.on(&press(10, 4));
        clock.set_ms(100);
        assert_eq!(
            sidebar.on(&wheel(true)),
            Some(Msg::TurnSelected(turn_of(0)))
        );
        clock.set_ms(200);
        assert_eq!(sidebar.on(&press(10, 4)), chosen("u1", true));
    }

    #[test]
    fn the_click_clock_can_be_replaced_through_an_attribute() {
        let mut sidebar = sidebar_of(&wireframe());
        let clock = TestClock::new();
        sidebar.attr(CLICK_CLOCK_ATTR, clock.click_clock().value());
        sidebar.on(&press(10, 4));
        clock.set_ms(401);
        assert_eq!(sidebar.on(&press(10, 4)), chosen("u1", false));
    }

    #[test]
    fn the_system_clock_measures_double_clicks_by_default() {
        let mut sidebar = sidebar_of(&wireframe());
        sidebar.on(&press(10, 4));
        assert_eq!(sidebar.on(&press(10, 4)), chosen("u1", true));
    }
}
