//! Snapshots of the turns sidebar at 120x40 and 90x24 (the pane is 30 x 33
//! and 30 x 17 there) in the `default` and `terminal` themes: each state of
//! product §5.3 on each kind of entry, no selection, a list longer than the pane
//! at each end, and the widest durations.

use super::fixtures::*;
use super::*;
use crate::test_support::buffer_to_styled;
use crate::theme::builtin;

const SIZES: [(u16, u16); 2] = [(120, 40), (90, 24)];

fn themes() -> [(&'static str, Theme); 2] {
    [
        ("default", builtin::default_theme()),
        ("terminal", builtin::terminal_theme()),
    ]
}

/// Draws the sidebar `make` builds on every screen size, in every theme, and
/// snapshots each as `<state>_<w>x<h>_<theme>`.
fn snapshot_state(state: &str, make: impl Fn(Theme) -> TurnSidebar, scroll_to_end: bool) {
    for (width, height) in SIZES {
        for (name, theme) in themes() {
            let mut sidebar = make(theme);
            let area = pane_area(width, height);
            sidebar.cursor.set_visible(sidebar_list_rows(area.height));
            if scroll_to_end {
                let rows = sidebar.rows().row_count();
                for _ in 0..rows {
                    sidebar.cursor.scroll(true, sidebar_list_rows(area.height));
                }
            }
            let buffer = draw(&mut sidebar, area.width, area.height);
            insta::assert_snapshot!(
                format!("{state}_{width}x{height}_{name}"),
                buffer_to_styled(&buffer)
            );
        }
    }
}

fn wireframe_with(theme: Theme, entry: Option<usize>, focused: bool) -> TurnSidebar {
    TurnSidebar::new(theme)
        .with_rows(rows_of(&wireframe()))
        .with_selected(entry)
        .focused(focused)
}

// The entries of the wireframe: 0 User, 1 Assistant, 2 User, 3 Assistant,
// 4 Compaction, 5 Assistant.

#[test]
fn focused_selection_on_a_user_turn() {
    snapshot_state("focused_user", |t| wireframe_with(t, Some(2), true), false);
}

#[test]
fn focused_selection_on_an_assistant_turn() {
    snapshot_state(
        "focused_assistant",
        |t| wireframe_with(t, Some(3), true),
        false,
    );
}

#[test]
fn focused_selection_on_a_compaction() {
    snapshot_state(
        "focused_compaction",
        |t| wireframe_with(t, Some(4), true),
        false,
    );
}

#[test]
fn unfocused_selection_on_a_user_turn() {
    snapshot_state(
        "unfocused_user",
        |t| wireframe_with(t, Some(2), false),
        false,
    );
}

#[test]
fn unfocused_selection_on_an_assistant_turn() {
    snapshot_state(
        "unfocused_assistant",
        |t| wireframe_with(t, Some(3), false),
        false,
    );
}

#[test]
fn unfocused_selection_on_a_compaction() {
    snapshot_state(
        "unfocused_compaction",
        |t| wireframe_with(t, Some(4), false),
        false,
    );
}

#[test]
fn no_selection() {
    snapshot_state("no_selection", |t| wireframe_with(t, None, false), false);
}

#[test]
fn a_long_list_at_the_top() {
    snapshot_state(
        "long_top",
        |t| TurnSidebar::new(t).with_rows(rows_of(&long_session(60))),
        false,
    );
}

#[test]
fn a_long_list_scrolled_to_the_end() {
    snapshot_state(
        "long_end",
        |t| TurnSidebar::new(t).with_rows(rows_of(&long_session(60))),
        true,
    );
}

#[test]
fn a_long_list_with_the_selection_in_the_middle() {
    snapshot_state(
        "long_middle",
        |t| {
            TurnSidebar::new(t)
                .with_rows(rows_of(&long_session(60)))
                .with_selected(Some(25))
                .focused(true)
        },
        false,
    );
}

#[test]
fn the_widest_durations() {
    snapshot_state(
        "widest_durations",
        |t| TurnSidebar::new(t).with_rows(rows_of(&widest_durations())),
        false,
    );
}

#[test]
fn the_widest_duration_selected_in_the_focused_pane() {
    snapshot_state(
        "widest_duration_selected",
        |t| {
            TurnSidebar::new(t)
                .with_rows(rows_of(&widest_durations()))
                .with_selected(Some(1))
                .focused(true)
        },
        false,
    );
}
