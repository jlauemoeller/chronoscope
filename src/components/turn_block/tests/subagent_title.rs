//! The rule and the open hint on a subagent's title row (cycle 4 SPEC §2):
//! where they are drawn, how they are styled, and that they are chrome for
//! search, highlights and copied text.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::widgets::Widget;
use serde_json::json;

use super::super::fixtures::*;
use super::super::plain::{displayed_text, full_text};
use super::super::*;
use crate::model::{Item, ToolKind, ToolStatus, Turn};
use crate::test_support::{buffer_to_styled, buffer_to_text};
use crate::theme::Role;
use crate::transcript_search::SearchQuery;

const HINT: &str = "a to open transcript";

/// A finished subagent call `c1`. `agent` is the input's `subagent_type`.
fn call(description: &str, agent: Option<&str>, millis: Option<u64>, result: &str) -> Item {
    let mut input = json!({"prompt": "List the crates."});
    if let Some(agent) = agent {
        input["subagent_type"] = json!(agent);
    }
    let item = tool_of(
        ToolKind::Subagent,
        "c1",
        description,
        input,
        Some((result, false)),
        ToolStatus::Ok,
    );
    match millis {
        Some(millis) => with_duration(item, millis),
        None => item,
    }
}

fn turn_of(item: Item) -> Turn {
    assistant(Some("high"), vec![item], None)
}

/// The wireframes' subagent: `Explore the repository (story-writer) · 5m 2s`.
fn repository() -> Turn {
    turn_of(call(
        "Explore the repository",
        Some("story-writer"),
        Some(302_000),
        "Found three crates.",
    ))
}

fn unfolded(turn: &Turn) -> Opts {
    Opts::default().unfold(item_id(turn, "c1"))
}

fn grid(buf: &Buffer) -> Vec<Vec<char>> {
    rows(buf).iter().map(|r| r.chars().collect()).collect()
}

fn row_of(turn: &Turn, opts: &Opts, width: u16, y: usize) -> String {
    rows(&draw(turn, opts, width))[y].clone()
}

// ---- Geometry ----

#[test]
fn folded_at_100_the_hint_ends_at_column_w_minus_3_then_a_blank_and_the_border() {
    let turn = turn_of(call(
        "Explore the UVC descriptors",
        Some("Explore"),
        None,
        "ok",
    ));
    let row: Vec<char> = row_of(&turn, &Opts::default(), 100, 2).chars().collect();
    assert_eq!(row.len(), 100);
    assert_eq!(row[97], 't', "the hint's last cell is column W - 3");
    assert_eq!(row[98], ' ', "column W - 2 is blank");
    assert_eq!(row[99], '│');
    assert_eq!(
        row[78..98].iter().collect::<String>(),
        HINT,
        "the hint is columns 78 to 97"
    );
    // The rule is 33 cells, columns 43 to 75, between two gaps of 2.
    assert!(row[43..76].iter().all(|&c| c == '─'));
    assert_eq!(row[41..43], [' ', ' ']);
    assert_eq!(row[76..78], [' ', ' ']);
    assert_eq!(row[42], ' ');
}

#[test]
fn folded_at_80_the_rule_is_5_cells_and_the_hint_fits_exactly() {
    let row: Vec<char> = row_of(&repository(), &Opts::default(), 80, 2)
        .chars()
        .collect();
    assert!(row[51..56].iter().all(|&c| c == '─'));
    assert_eq!(row[50], ' ');
    assert_eq!(row[56], ' ');
    assert_eq!(row[58..78].iter().collect::<String>(), HINT);
    assert_eq!(row[78], ' ');
    assert_eq!(row[79], '│');
}

#[test]
fn with_a_wider_key_the_hint_is_left_out_and_the_rule_runs_to_column_w_minus_3() {
    let opts = Opts::default().open_key("Ctrl-o");
    let row: Vec<char> = row_of(&repository(), &opts, 80, 2).chars().collect();
    assert!(row[51..78].iter().all(|&c| c == '─'), "27 cells, 51 to 77");
    assert_eq!(row[50], ' ');
    assert_eq!(row[78], ' ');
    assert_eq!(row[79], '│');
    // A folded row shows the hint nowhere.
    assert!(
        !rows(&draw(&repository(), &opts, 80))
            .iter()
            .any(|r| r.contains("transcript"))
    );
}

#[test]
fn unfolded_with_a_wider_key_the_hint_has_a_row_below_the_description() {
    let turn = repository();
    let opts = unfolded(&turn).open_key("Ctrl-o");
    let all = rows(&draw(&turn, &opts, 80));
    assert_eq!(
        all[2],
        "│ ▼ Explore the repository (story-writer) · 5m 2s                              │"
    );
    let hint: Vec<char> = all[3].chars().collect();
    // The rule is 47 cells from column 4 to 50; the hint runs from 53 to 77.
    assert!(hint[4..51].iter().all(|&c| c == '─'));
    assert_eq!(hint[51..53], [' ', ' ']);
    assert_eq!(
        hint[53..78].iter().collect::<String>(),
        "Ctrl-o to open transcript"
    );
    assert_eq!(hint[2..4], [' ', ' ']);
    assert_eq!(hint[78], ' ');
    assert_eq!(hint[79], '│');
    // The blank row that follows the description follows the hint's row.
    assert_eq!(all[4], format!("│{}│", " ".repeat(78)));
    assert_eq!(all[5], format!("│   Prompt:{}│", " ".repeat(68)));
}

#[test]
fn only_a_title_rows_content_columns_reach_into_the_padding() {
    let turn = turn_of(call(
        "Explore the UVC descriptors",
        Some("Explore"),
        None,
        "ok",
    ));
    with_block(&turn, &unfolded(&turn), |block| {
        let layout = block.layout(100);
        // The hint of the title row ends at column W - 3.
        assert_eq!(layout.content_columns(2), 2..98);
        // Every other row, the borders and a row past the end: the text
        // area, which ends at column W - 5.
        let last = layout.height() - 1;
        for row in (0..2).chain(3..=last + 1) {
            assert_eq!(layout.content_columns(row), 2..96, "row {row}");
        }
    });
    // A title row that draws nothing into the padding keeps to the text area.
    let full = turn_of(call(&"d".repeat(62), Some("Explore"), None, "ok"));
    let columns = with_block(&full, &Opts::default(), |block| {
        block.layout(80).content_columns(2)
    });
    assert_eq!(columns, 2..76);
}

#[test]
fn only_title_rows_use_the_padding() {
    let turn = repository();
    let cases = [
        (Opts::default(), 2..3),
        (unfolded(&turn), 2..3),
        (unfolded(&turn).open_key("Ctrl-o"), 2..4),
    ];
    for (opts, title) in cases {
        let all = grid(&draw(&turn, &opts, 80));
        for (y, row) in all.iter().enumerate().take(all.len() - 1).skip(1) {
            if !title.contains(&y) {
                assert_eq!(row[76..79], [' ', ' ', ' '], "row {y}: {row:?}");
            }
        }
    }
}

#[test]
fn a_description_that_fills_the_text_area_leaves_the_padding_blank() {
    // t = 72 at 80 columns: 62 + " (Explore)" = 72.
    let turn = turn_of(call(&"d".repeat(62), Some("Explore"), None, "ok"));
    let row: Vec<char> = row_of(&turn, &Opts::default(), 80, 2).chars().collect();
    assert_eq!(row[76..79], [' ', ' ', ' ']);
    assert!(!row.contains(&'─'));
}

#[test]
fn a_cut_description_has_neither_rule_nor_hint() {
    let turn = turn_of(call(&"d".repeat(120), Some("Explore"), Some(1_200), "ok"));
    let row = row_of(&turn, &Opts::default(), 80, 2);
    assert!(row.contains("… (Explore) · 1.2s"), "{row}");
    assert!(!row.contains('─') && !row.contains("transcript"), "{row}");
}

#[test]
fn the_row_follows_the_narrow_cases_at_every_width() {
    // d = 45 for the wireframe's row; t = W - 8, r = t + 2 = W - 6.
    let d = 45;
    let h = HINT.len();
    let turn = repository();
    for width in 40_u16..=100 {
        let r = usize::from(width) - 6;
        let row: Vec<char> = row_of(&turn, &Opts::default(), width, 2).chars().collect();
        let w = usize::from(width);
        let rules = row.iter().filter(|&&c| c == '─').count();
        let text: String = row.iter().collect();
        if r >= d + 7 + h {
            assert_eq!(rules, r - d - 4 - h, "{width}");
            assert!(text.contains(HINT), "{width}");
            assert_eq!(row[w - 3], 't', "{width}");
        } else if r >= d + 5 {
            assert_eq!(rules, r - d - 2, "{width}");
            assert!(!text.contains("transcript"), "{width}");
            assert_eq!(row[w - 3], '─', "{width}");
        } else {
            assert_eq!(rules, 0, "{width}");
            assert_eq!(row[w - 4..w - 1], [' ', ' ', ' '], "{width}");
        }
        assert_eq!(row[w - 1], '│', "{width}");
        assert_eq!(row[w - 2], ' ', "{width}");
        assert_eq!(row.len(), w, "{width}");
    }
}

#[test]
fn the_unfolded_hint_is_on_the_title_row_or_a_row_of_its_own_at_every_width() {
    let turn = repository();
    let opts = unfolded(&turn);
    for width in 30_u16..=100 {
        let all = rows(&draw(&turn, &opts, width));
        let hints = all.iter().filter(|r| r.contains("to open")).count();
        assert_eq!(hints, 1, "{width}: {all:#?}");
        // The body never ends with the hint.
        let last = &all[all.len() - 2];
        assert!(!last.contains("to open"), "{width}");
    }
}

#[test]
fn a_hint_wider_than_the_row_is_cut_with_an_ellipsis_on_its_own_row() {
    let turn = repository();
    let all = rows(&draw(&turn, &unfolded(&turn), 20));
    // t = 12, r = 14: only the hint, cut to 14 cells, from the text column.
    let at = all.iter().position(|r| r.contains("a to open")).unwrap();
    assert_eq!(all[at], "│   a to open tra… │");
}

#[test]
fn the_layout_counts_the_hints_own_row() {
    let turn = repository();
    // The hint's own row is one row more than a hint on the title row.
    let narrow = unfolded(&turn).open_key("Ctrl-o");
    assert_eq!(
        height(&turn, &narrow, 80),
        height(&turn, &unfolded(&turn), 80) + 1
    );
}

#[test]
fn a_subagent_without_an_agent_type_and_a_pending_one_get_the_rule_and_the_hint() {
    let plain = turn_of(call("Do the thing", None, None, "ok"));
    let row = row_of(&plain, &Opts::default(), 80, 2);
    assert!(row.starts_with("│ ▶ Do the thing  ───"), "{row}");
    assert!(row.ends_with("  a to open transcript │"), "{row}");
    let pending = turn_of(tool_of(
        ToolKind::Subagent,
        "c1",
        "Do the thing",
        json!({"subagent_type": "Explore", "prompt": "p"}),
        None,
        ToolStatus::Pending,
    ));
    let row = row_of(&pending, &Opts::default(), 80, 2);
    assert!(row.starts_with("│ ▶ Do the thing (Explore)  ───"), "{row}");
    assert!(row.ends_with("  a to open transcript │"), "{row}");
}

#[test]
fn a_shell_call_and_an_other_call_get_neither() {
    let shell = turn_of(tool(
        "c1",
        "List the files",
        json!({"command": "ls"}),
        Some(("a", false)),
        ToolStatus::Ok,
    ));
    let other = turn_of(tool_of(
        ToolKind::Other("mcp__x__y".into()),
        "c1",
        "Call it",
        json!({}),
        Some(("a", false)),
        ToolStatus::Ok,
    ));
    for turn in [&shell, &other] {
        for opts in [Opts::default(), unfolded(turn)] {
            let text = buffer_to_text(&draw(turn, &opts, 80));
            assert!(!text.contains("transcript"), "{text}");
            let interior = text.lines().filter(|l| l.starts_with('│'));
            assert!(interior.clone().all(|l| !l.contains('─')), "{text}");
        }
    }
}

// ---- Style ----

/// The cell of `row` whose symbol is the first one of `needle`.
fn cell_of<'b>(buf: &'b Buffer, y: u16, needle: &str) -> &'b ratatui::buffer::Cell {
    let row = rows(buf)[usize::from(y)].clone();
    let x = row[..row.find(needle).unwrap()].chars().count();
    &buf[(x as u16, y)]
}

#[test]
fn the_rule_is_border_and_never_bold_and_the_hint_is_muted_and_bold_when_selected() {
    let turn = repository();
    let id = item_id(&turn, "c1");
    let theme = distinct_theme();
    let border = theme.role(Role::Border);
    let muted = theme.role(Role::MutedForeground);
    // (selected, dim_unselected)
    for (selected, dim) in [(true, true), (false, true), (false, false)] {
        for folded in [true, false] {
            let mut opts = Opts::default().themed(theme.clone()).dim(dim);
            if !folded {
                opts = opts.unfold(id.clone());
            }
            if selected {
                opts = opts.select(id.clone());
            }
            let buf = draw(&turn, &opts, 80);
            let rule = cell_of(&buf, 2, "─");
            assert_eq!(rule.fg, border, "selected {selected}, dim {dim}");
            assert!(!rule.modifier.contains(Modifier::BOLD));
            let hint = cell_of(&buf, 2, "open");
            assert_eq!(hint.fg, muted, "selected {selected}, dim {dim}");
            assert_eq!(hint.modifier.contains(Modifier::BOLD), selected);
            // No background on the row but the default one.
            let bg = buf[(0, 0)].bg;
            for x in 2..79 {
                assert_eq!(buf[(x, 2)].bg, bg, "column {x}");
            }
        }
    }
}

#[test]
fn the_rule_of_the_hints_own_row_is_border_too() {
    let turn = repository();
    let id = item_id(&turn, "c1");
    let theme = distinct_theme();
    for selected in [true, false] {
        let mut opts = unfolded(&turn).open_key("Ctrl-o").themed(theme.clone());
        if selected {
            opts = opts.select(id.clone());
        }
        let buf = draw(&turn, &opts, 80);
        let rule = cell_of(&buf, 3, "─");
        assert_eq!(rule.fg, theme.role(Role::Border));
        assert!(!rule.modifier.contains(Modifier::BOLD));
        let hint = cell_of(&buf, 3, "Ctrl-o");
        assert_eq!(hint.fg, theme.role(Role::MutedForeground));
        assert_eq!(hint.modifier.contains(Modifier::BOLD), selected);
    }
}

#[test]
fn changing_the_theme_recolours_the_rule_and_the_hint() {
    let turn = repository();
    let other = crate::theme::builtin::get("nord").unwrap();
    assert_ne!(other.role(Role::Border), theme().role(Role::Border));
    assert_ne!(
        other.role(Role::MutedForeground),
        theme().role(Role::MutedForeground)
    );
    let id = item_id(&turn, "c1");
    for theme in [theme(), other] {
        let opts = Opts::default().themed(theme.clone()).select(id.clone());
        let buf = draw(&turn, &opts, 80);
        assert_eq!(cell_of(&buf, 2, "─").fg, theme.role(Role::Border));
        assert_eq!(
            cell_of(&buf, 2, "open").fg,
            theme.role(Role::MutedForeground)
        );
    }
}

// ---- Snapshots ----

#[test]
fn folded_with_ctrl_o_at_80() {
    let opts = Opts::default().open_key("Ctrl-o");
    insta::assert_snapshot!(buffer_to_text(&draw(&repository(), &opts, 80)));
}

#[test]
fn unfolded_with_ctrl_o_at_80() {
    let turn = repository();
    let opts = unfolded(&turn).open_key("Ctrl-o");
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &opts, 80)));
}

#[test]
fn folded_selected_at_80_styled() {
    let turn = repository();
    let opts = Opts::default()
        .themed(distinct_theme())
        .select(item_id(&turn, "c1"));
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &opts, 80)));
}

#[test]
fn folded_unselected_at_80_styled() {
    let opts = Opts::default().themed(distinct_theme());
    insta::assert_snapshot!(buffer_to_styled(&draw(&repository(), &opts, 80)));
}

#[test]
fn unfolded_with_ctrl_o_selected_at_80_styled() {
    let turn = repository();
    let opts = unfolded(&turn)
        .open_key("Ctrl-o")
        .themed(distinct_theme())
        .select(item_id(&turn, "c1"));
    insta::assert_snapshot!(buffer_to_styled(&draw(&turn, &opts, 80)));
}

// ---- Text ----

fn shown(turn: &Turn, folded: bool, key: &str) -> String {
    let id = item_id(turn, "c1");
    let mut folds = FoldState::new();
    folds.set_folded(id.clone(), folded);
    displayed_text(turn, &id, &folds, &theme(), 200, key, Some(80)).unwrap()
}

#[test]
fn the_displayed_text_of_a_folded_subagent_is_its_description_row() {
    for key in ["a", "Ctrl-o"] {
        assert_eq!(
            shown(&repository(), true, key),
            "Explore the repository (story-writer) · 5m 2s"
        );
    }
}

#[test]
fn the_displayed_text_of_an_unfolded_subagent_ends_with_the_result() {
    let text = shown(&repository(), false, "a");
    assert_eq!(
        text,
        "Explore the repository (story-writer) · 5m 2s\n\nPrompt:\n\nList the crates.\n\nResult:\n\nFound three crates."
    );
    assert!(!text.contains("to open transcript") && !text.contains('─'));
}

#[test]
fn a_truncated_result_ends_the_displayed_text_with_the_more_row() {
    let result: String = (1..=30).map(|n| format!("line {n}\n\n")).collect();
    let turn = turn_of(call("Explore", Some("Explore"), None, &result));
    let id = item_id(&turn, "c1");
    let mut folds = FoldState::new();
    folds.set_folded(id.clone(), false);
    let text = displayed_text(&turn, &id, &folds, &theme(), 10, "a", Some(80)).unwrap();
    assert!(text.ends_with("… 50 more lines"), "{text}");
    assert!(!text.contains("to open transcript"));
}

#[test]
fn the_full_text_has_neither_rule_nor_hint() {
    let turn = repository();
    let text = full_text(&turn, &item_id(&turn, "c1"), &theme(), "a").unwrap();
    assert_eq!(
        text,
        "Explore the repository (story-writer) · 5m 2s\n\nPrompt:\n\nList the crates.\n\nResult:\n\nFound three crates."
    );
}

#[test]
fn the_rows_text_has_no_rule_and_no_hint() {
    let turn = repository();
    for opts in [
        Opts::default(),
        unfolded(&turn),
        unfolded(&turn).open_key("Ctrl-o"),
    ] {
        let all: Vec<String> = with_block(&turn, &opts, |block| {
            let layout = block.layout(80);
            (0..layout.height())
                .filter_map(|row| layout.row_text(row))
                .collect()
        });
        assert!(
            all.iter()
                .all(|r| !r.contains('─') && !r.contains("transcript")),
            "{all:#?}"
        );
        let marker = if opts.folds == FoldState::new() {
            '▶'
        } else {
            '▼'
        };
        assert_eq!(
            all[1],
            format!("{marker} Explore the repository (story-writer) · 5m 2s")
        );
    }
}

#[test]
fn both_exports_have_neither_rule_nor_hint() {
    use super::super::export::{ExportSettings, export_markdown};
    let turn = repository();
    let theme = theme();
    let formatter = formatter();
    let mut folds = FoldState::new();
    folds.set_folded(item_id(&turn, "c1"), false);
    for (folds, full) in [
        (FoldState::new(), false),
        (folds.clone(), false),
        (folds, true),
    ] {
        let settings = ExportSettings {
            theme: &theme,
            formatter: &formatter,
            folds: &folds,
            truncate: 20,
            open_key: "a",
            show_system: true,
            block_width: Some(80),
        };
        let text = export_markdown(None, &[std::sync::Arc::new(turn.clone())], &settings, full);
        assert!(text.contains("Explore the repository"), "{text}");
        assert!(
            !text.contains("to open transcript") && !text.contains('─'),
            "{text}"
        );
    }
}

#[test]
fn a_zero_width_character_ending_the_description_stays_in_the_rows_text() {
    // `cafe` + U+0301 is drawn `café`; with no agent type and no duration it
    // ends the text of the title row, right before the chrome.
    let turn = turn_of(call("Check the cafe\u{301}", None, None, "ok"));
    let all: Vec<String> = with_block(&turn, &Opts::default(), |block| {
        let layout = block.layout(80);
        (0..layout.height())
            .filter_map(|row| layout.row_text(row))
            .collect()
    });
    assert_eq!(all[1], "▶ Check the cafe\u{301}");
    // And the match lights the cells of `café`.
    let cells = highlighted(&turn, &Opts::default(), "cafe\u{301}", 80, 2);
    assert_eq!(cells, (14..18).collect::<Vec<_>>());
}

#[test]
fn the_hints_own_rows_text_is_empty() {
    let turn = repository();
    let opts = unfolded(&turn).open_key("Ctrl-o");
    let text = with_block(&turn, &opts, |block| block.layout(80).row_text(3));
    assert_eq!(text.as_deref(), Some(""));
}

#[test]
fn a_pending_subagent_without_an_agent_type_at_80() {
    let turn = turn_of(tool_of(
        ToolKind::Subagent,
        "c1",
        "Do the thing",
        json!({"prompt": "p"}),
        None,
        ToolStatus::Pending,
    ));
    insta::assert_snapshot!(buffer_to_text(&draw(&turn, &Opts::default(), 80)));
}

/// The `row_text` of the title row (layout row 2) of `turn` at 80 columns.
fn title_text(turn: &Turn) -> String {
    with_block(turn, &Opts::default(), |block| {
        block.layout(80).row_text(2).unwrap()
    })
}

fn pending(description: &str) -> Turn {
    turn_of(tool_of(
        ToolKind::Subagent,
        "c1",
        description,
        json!({"prompt": "p"}),
        None,
        ToolStatus::Pending,
    ))
}

#[test]
fn a_zwj_sequence_before_the_duration_leaves_the_rows_text_whole() {
    let description = "Family 👨\u{200D}👩\u{200D}👧 report";
    let turn = turn_of(call(description, Some("Explore"), Some(302_000), "ok"));
    assert_eq!(
        title_text(&turn),
        format!("▶ {description} (Explore) · 5m 2s")
    );
    // `5m 2s` is the last five cells of the description; no wider than the
    // line, the highlight is on exactly them.
    let cells = highlighted(&turn, &Opts::default(), "5m 2s", 80, 2);
    assert_eq!(cells.len(), 5, "{cells:?}");
    assert!(cells.windows(2).all(|w| w[1] == w[0] + 1));
}

#[test]
fn lam_alef_before_the_duration_leaves_the_rows_text_whole() {
    let description = "Arabic \u{644}\u{627} text";
    let turn = turn_of(call(description, Some("Explore"), Some(302_000), "ok"));
    assert_eq!(
        title_text(&turn),
        format!("▶ {description} (Explore) · 5m 2s")
    );
    // The widths of lam-alef differ between the layout and the drawing, so
    // only check that the duration is lit.
    assert!(!highlighted(&turn, &Opts::default(), "5m 2s", 80, 2).is_empty());
}

#[test]
fn a_pending_row_ending_in_a_sequence_keeps_all_of_it() {
    let zwj = "Family 👨\u{200D}👩\u{200D}👧";
    assert_eq!(title_text(&pending(zwj)), format!("▶ {zwj}"));
    assert_eq!(
        title_text(&pending("Love ❤\u{FE0F}")),
        "▶ Love ❤\u{FE0F}",
        "no gap cell after a variation selector"
    );
}

#[test]
fn a_description_of_zero_width_is_not_taken_for_the_hints_own_row() {
    assert_eq!(title_text(&pending("\u{301}")), "▶ \u{301}");
}

// ---- Highlights ----

/// The columns of row `y` drawn with the `search_match` background.
fn highlighted(turn: &Turn, opts: &Opts, query: &str, width: u16, y: usize) -> Vec<usize> {
    let query = SearchQuery::new(query).unwrap();
    let bg: Color = opts.theme.role(Role::Primary);
    let h = height(turn, opts, width);
    let buf = with_block(turn, opts, |block| {
        crate::test_support::render(width, h as u16, |frame| {
            block
                .search(Some(&query))
                .render(frame.area(), frame.buffer_mut());
        })
    });
    (0..usize::from(width))
        .filter(|&x| buf[(x as u16, y as u16)].bg == bg)
        .collect()
}

#[test]
fn a_match_in_the_description_highlights_exactly_its_cells() {
    let turn = repository();
    // `Explore the repository`, columns 4 to 25.
    let cells = highlighted(&turn, &Opts::default(), "the repo", 80, 2);
    assert_eq!(cells, (12..20).collect::<Vec<_>>());
    let cells = highlighted(&turn, &unfolded(&turn), "story-writer", 80, 2);
    assert_eq!(cells, (28..40).collect::<Vec<_>>());
}

#[test]
fn a_query_in_the_hint_highlights_no_cell_of_it_folded_or_unfolded() {
    // The result also contains `transcript`.
    let turn = turn_of(call(
        "Explore the repository",
        Some("story-writer"),
        Some(302_000),
        "Read the transcript.",
    ));
    for query in ["transcript", "open", "a to open"] {
        let folded = highlighted(&turn, &Opts::default(), query, 80, 2);
        assert_eq!(folded, Vec::<usize>::new(), "folded, {query}");
        let open = highlighted(&turn, &unfolded(&turn), query, 80, 2);
        assert_eq!(open, Vec::<usize>::new(), "unfolded, {query}");
    }
    // The result is highlighted when the section is unfolded.
    let opts = unfolded(&turn);
    let body: Vec<usize> = (0..height(&turn, &opts, 80))
        .flat_map(|y| highlighted(&turn, &opts, "transcript", 80, y))
        .collect();
    assert_eq!(body.len(), "transcript".len());
}

#[test]
fn a_query_in_the_hints_own_row_highlights_nothing_on_it() {
    let turn = turn_of(call(
        "Explore the repository",
        Some("story-writer"),
        Some(302_000),
        "Read the transcript.",
    ));
    let opts = unfolded(&turn).open_key("Ctrl-o");
    for query in ["transcript", "Ctrl-o", "─"] {
        let cells = highlighted(&turn, &opts, query, 80, 3);
        assert_eq!(cells, Vec::<usize>::new(), "{query}");
    }
}

#[test]
fn a_query_for_the_rule_highlights_no_cell_of_it() {
    let turn = repository();
    for opts in [Opts::default(), unfolded(&turn)] {
        assert_eq!(highlighted(&turn, &opts, "─", 80, 2), Vec::<usize>::new());
    }
}
