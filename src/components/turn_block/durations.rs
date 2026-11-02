//! The duration of a finished tool call after its description (cycle 2
//! SPEC §3): the rows, the text copied, saved and exported, and the
//! snapshots.

use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::style::Color;
use serde_json::json;

use super::export::{ExportSettings, export_markdown};
use super::fixtures::*;
use super::plain::{displayed_text, full_text};
use super::*;
use crate::model::{Item, ToolKind, ToolStatus, Turn};
use crate::test_support::{buffer_to_styled, buffer_to_text};

const KEY: &str = "a";

fn shell(description: &str, millis: Option<u64>, status: ToolStatus) -> Item {
    let result = (status != ToolStatus::Pending).then_some(("lsusb output", false));
    let item = tool(
        "c1",
        description,
        json!({"command": "lsusb"}),
        result,
        status,
    );
    match millis {
        Some(millis) => with_duration(item, millis),
        None => item,
    }
}

fn finished(description: &str, millis: u64) -> Turn {
    assistant(
        Some("high"),
        vec![shell(description, Some(millis), ToolStatus::Ok)],
        None,
    )
}

fn subagent(millis: Option<u64>) -> Turn {
    let item = tool_of(
        ToolKind::Subagent,
        "c1",
        "Explore the repository",
        json!({
            "subagent_type": "story-writer",
            "description": "Explore the repository",
            "prompt": "List the crates.",
        }),
        Some(("Found three crates.", false)),
        ToolStatus::Ok,
    );
    let item = match millis {
        Some(millis) => with_duration(item, millis),
        None => item,
    };
    assistant(Some("high"), vec![item], None)
}

fn unfolded(turn: &Turn) -> Opts {
    Opts::default().unfold(item_id(turn, "c1")).dim(false)
}

/// The text of the screen row that holds the description of the call (after
/// the title and a blank row).
fn description_row(turn: &Turn, opts: &Opts, width: u16) -> String {
    let text = buffer_to_text(&draw(turn, opts, width));
    text.lines().nth(2).unwrap().to_owned()
}

// ---- Snapshots ----

#[test]
fn folded_call_with_a_duration_at_80() {
    let turn = finished("Dump full USB descriptors", 1_200);
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 80)));
}

#[test]
fn folded_call_with_a_duration_at_120() {
    let turn = finished("Dump full USB descriptors", 1_200);
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 120)));
}

#[test]
fn unfolded_call_with_a_duration_at_80() {
    let turn = finished("Dump full USB descriptors", 1_200);
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 80)));
}

#[test]
fn unfolded_call_with_a_duration_at_120() {
    let turn = finished("Dump full USB descriptors", 1_200);
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 120)));
}

#[test]
fn folded_subagent_with_a_duration_at_80() {
    let turn = subagent(Some(302_000));
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 80)));
}

#[test]
fn folded_subagent_with_a_duration_at_120() {
    let turn = subagent(Some(302_000));
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 120)));
}

#[test]
fn unfolded_subagent_with_a_duration_at_80() {
    let turn = subagent(Some(302_000));
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 80)));
}

#[test]
fn unfolded_subagent_with_a_duration_at_120() {
    let turn = subagent(Some(302_000));
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &unfolded(&turn), 120)));
}

#[test]
fn folded_call_too_narrow_for_its_description_at_40() {
    let turn = finished("Investigate the descriptors of the camera", 340);
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 40)));
}

#[test]
fn folded_subagent_too_narrow_for_its_description_at_40() {
    let turn = subagent(Some(302_000));
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 40)));
}

#[test]
fn unfolded_call_wrapped_at_40_keeps_the_duration_muted_styled() {
    let turn = finished("Investigate the descriptors of the camera", 340);
    let opts = unfolded(&turn).themed(distinct_theme());
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &opts, 40)));
}

#[test]
fn selected_folded_call_keeps_the_duration_muted_styled() {
    let turn = finished("Dump full USB descriptors", 1_200);
    let opts = Opts::default()
        .select(item_id(&turn, "c1"))
        .themed(distinct_theme());
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &opts, 80)));
}

// ---- Rows ----

#[test]
fn the_row_reads_description_then_duration() {
    let turn = finished("Dump full USB descriptors", 1_200);
    let row = description_row(&turn, &Opts::default(), 80);
    assert!(row.contains("▶ Dump full USB descriptors · 1.2s "), "{row}");
    let row = description_row(&turn, &unfolded(&turn), 80);
    assert!(row.contains("▼ Dump full USB descriptors · 1.2s "), "{row}");
}

#[test]
fn a_subagent_shows_the_duration_after_the_agent_type() {
    let turn = subagent(Some(302_000));
    let row = description_row(&turn, &Opts::default(), 100);
    assert!(
        row.contains("Explore the repository (story-writer) · 5m 2s "),
        "{row}"
    );
}

#[test]
fn a_pending_call_shows_no_duration_and_no_separator() {
    // Even a pending call that carries a duration shows none.
    let item = with_duration(shell("Dump", Some(0), ToolStatus::Pending), 1_200);
    let turn = assistant(Some("high"), vec![item], None);
    for opts in [Opts::default(), unfolded(&turn)] {
        let row = description_row(&turn, &opts, 80);
        assert!(!row.contains('·') && !row.contains("1.2s"), "{row}");
        assert!(row.contains("Dump "), "{row}");
    }
}

#[test]
fn a_call_without_a_known_duration_shows_no_duration_and_no_separator() {
    let turn = assistant(
        Some("high"),
        vec![shell("Dump", None, ToolStatus::Ok)],
        None,
    );
    for opts in [Opts::default(), unfolded(&turn)] {
        let row = description_row(&turn, &opts, 80);
        assert!(!row.contains('·'), "{row}");
        assert!(row.contains("Dump "), "{row}");
    }
    let turn = subagent(None);
    let row = description_row(&turn, &Opts::default(), 100);
    assert!(!row.contains('·'), "{row}");
    assert!(row.contains("(story-writer) "), "{row}");
}

#[test]
fn a_failed_call_with_a_duration_shows_it() {
    let item = with_duration(
        tool(
            "c1",
            "Run the tests",
            json!({"command": "cargo test"}),
            Some(("error[E0432]", true)),
            ToolStatus::Error,
        ),
        2_500,
    );
    let turn = assistant(Some("high"), vec![item], None);
    let row = description_row(&turn, &Opts::default(), 80);
    assert!(row.contains("▶ Run the tests · 2.5s "), "{row}");
    let row = description_row(&turn, &unfolded(&turn), 80);
    assert!(row.contains("▼ Run the tests · 2.5s "), "{row}");
}

#[test]
fn the_narrow_row_cuts_the_description_and_keeps_the_duration() {
    let turn = finished("Investigate the descriptors of the camera", 340);
    let row = description_row(&turn, &Opts::default(), 40);
    assert!(row.contains("… · 340ms "), "{row}");
    assert!(row.starts_with("│ ▶ Investigate"), "{row}");
}

#[test]
fn the_narrow_subagent_row_cuts_the_description_and_keeps_both_suffixes() {
    let turn = subagent(Some(302_000));
    let row = description_row(&turn, &Opts::default(), 40);
    assert!(row.contains("… (story-writer) · 5m 2s "), "{row}");
}

#[test]
fn a_row_that_leaves_no_room_for_the_description_is_cut_as_a_whole() {
    // The agent type and the duration alone take 23 cells, and the text is
    // 18 wide: no room for the description, so the existing fallback applies.
    let turn = subagent(Some(302_000));
    let row = description_row(&turn, &Opts::default(), 26);
    assert!(row.contains("Explore the r"), "{row}");
    assert!(row.contains("…"), "{row}");
    assert!(
        !row.contains("5m 2s") && !row.contains("(story-writer)"),
        "{row}"
    );
}

/// The foreground of the cell at `column` of row `row`.
fn fg(buffer: &Buffer, column: usize, row: u16) -> Color {
    buffer[(u16::try_from(column).unwrap(), row)].fg
}

/// The column the text `needle` starts at in row `row` of `buffer`.
fn column_of(buffer: &Buffer, row: u16, needle: &str) -> usize {
    let text = buffer_to_text(buffer);
    let line = text.lines().nth(usize::from(row)).unwrap();
    line.find(needle)
        .map(|byte| line[..byte].chars().count())
        .unwrap_or_else(|| panic!("{needle:?} not in {line:?}"))
}

#[test]
fn the_duration_is_muted_and_the_description_keeps_its_token() {
    let theme = distinct_theme();
    let muted = theme.role(crate::theme::Role::MutedForeground);
    let description = theme
        .style(crate::theme::Token::ToolDescription)
        .fg
        .unwrap();
    assert_ne!(muted, description);
    let turn = subagent(Some(302_000));
    for (opts, width) in [
        (Opts::default(), 100),
        (unfolded(&turn), 100),
        (Opts::default(), 40),
        (unfolded(&turn), 40),
    ] {
        let opts = opts.dim(false).themed(theme.clone());
        let buffer = draw(&turn, &opts, width);
        let text = buffer_to_text(&buffer);
        let row = u16::try_from(
            text.lines()
                .position(|l| l.contains("5m"))
                .expect("the duration is drawn"),
        )
        .unwrap();
        let at = column_of(&buffer, row, "5m 2s");
        for column in at..at + 5 {
            assert_eq!(
                fg(&buffer, column, row),
                muted,
                "width {width} col {column}"
            );
        }
        let dot = column_of(&buffer, row, "·");
        assert_eq!(fg(&buffer, dot, row), muted, "the separator is muted");
        // The agent type is drawn like the description.
        if text.contains("(story-writer)") && width >= 100 {
            let agent = column_of(&buffer, row, "(story-writer)");
            assert_eq!(fg(&buffer, agent, row), description);
        }
        let start = column_of(&buffer, 2, "Explore");
        assert_eq!(fg(&buffer, start, 2), description, "width {width}");
    }
}

// ---- Copy, save, export ----

fn shown(turn: &Turn, folded: bool, width: Option<u16>) -> String {
    let mut folds = FoldState::new();
    folds.set_folded(item_id(turn, "c1"), folded);
    displayed_text(turn, &item_id(turn, "c1"), &folds, &theme(), 20, KEY, width).unwrap()
}

#[test]
fn the_displayed_copy_of_a_folded_call_has_the_duration() {
    let turn = finished("Dump full USB descriptors", 1_200);
    assert_eq!(shown(&turn, true, None), "Dump full USB descriptors · 1.2s");
    // Whatever width the block has: a folded call is copied whole.
    assert_eq!(
        shown(&turn, true, Some(40)),
        "Dump full USB descriptors · 1.2s"
    );
}

#[test]
fn the_displayed_copy_of_a_subagent_has_the_duration_after_the_agent_type() {
    let turn = subagent(Some(302_000));
    assert_eq!(
        shown(&turn, true, Some(40)),
        "Explore the repository (story-writer) · 5m 2s"
    );
}

#[test]
fn the_displayed_copy_of_an_unfolded_call_has_the_duration() {
    let turn = finished("Dump full USB descriptors", 1_200);
    let text = shown(&turn, false, None);
    assert!(
        text.starts_with("Dump full USB descriptors · 1.2s\n\nCommand:\n"),
        "{text}"
    );
}

#[test]
fn the_full_copy_has_the_duration() {
    let turn = finished("Dump full USB descriptors", 1_200);
    let text = full_text(&turn, &item_id(&turn, "c1"), &theme(), KEY).unwrap();
    assert!(
        text.starts_with("Dump full USB descriptors · 1.2s\n\nCommand:\n"),
        "{text}"
    );
    let turn = finished("Dump full USB descriptors", 340);
    let text = full_text(&turn, &item_id(&turn, "c1"), &theme(), KEY).unwrap();
    assert!(
        text.starts_with("Dump full USB descriptors · 340ms\n"),
        "{text}"
    );
}

#[test]
fn the_copy_of_a_pending_call_has_no_duration() {
    let item = with_duration(shell("Dump", Some(0), ToolStatus::Pending), 1_200);
    let turn = assistant(Some("high"), vec![item], None);
    assert_eq!(shown(&turn, true, None), "Dump");
    let full = full_text(&turn, &item_id(&turn, "c1"), &theme(), KEY).unwrap();
    assert!(full.starts_with("Dump\n"), "{full}");
}

fn export(turn: Turn, folds: &FoldState, full: bool) -> String {
    let theme = theme();
    let formatter = formatter();
    let settings = ExportSettings {
        theme: &theme,
        formatter: &formatter,
        folds,
        truncate: 20,
        open_key: KEY,
        show_system: true,
        block_width: Some(80),
    };
    export_markdown(None, &[Arc::new(turn)], &settings, full)
}

#[test]
fn the_folded_export_has_the_duration_folded_and_unfolded() {
    let turn = finished("Dump full USB descriptors", 1_200);
    let folded = export(turn.clone(), &FoldState::new(), false);
    assert!(
        folded.contains("Dump full USB descriptors · 1.2s"),
        "{folded}"
    );
    let mut folds = FoldState::new();
    folds.set_folded(item_id(&turn, "c1"), false);
    let open = export(turn, &folds, false);
    assert!(
        open.contains("**Dump full USB descriptors · 1.2s**"),
        "{open}"
    );
}

#[test]
fn the_full_export_summary_has_the_duration() {
    let turn = finished("Dump full USB descriptors", 1_200);
    let full = export(turn, &FoldState::new(), true);
    assert!(
        full.contains("<details><summary>Dump full USB descriptors · 1.2s</summary>"),
        "{full}"
    );
}

#[test]
fn a_call_without_a_duration_exports_as_it_did() {
    let turn = assistant(
        Some("high"),
        vec![shell("Dump", None, ToolStatus::Ok)],
        None,
    );
    let full = export(turn.clone(), &FoldState::new(), true);
    assert!(full.contains("<details><summary>Dump</summary>"), "{full}");
    let folded = export(turn, &FoldState::new(), false);
    assert!(!folded.contains('·'), "{folded}");
}
