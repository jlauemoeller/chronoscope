//! The turns sidebar and the mouse (product §5.9): the wheel selects the
//! previous or next entry, a click selects a row and focuses the pane, a
//! double-click does what `Enter` does, and a click on the transcript
//! focuses it.
//!
//! The events go through `App::update_routed(Msg::Pointer(..))`, which
//! handles the router's answer as the running application does; what they
//! must do is what the keys do (`j` / `k` / `Enter`), which the tests compare
//! with.

use ratatui::layout::Rect;
use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};

use super::sidebar::{pane_rows, sidebar_rows};
use super::sidebar_follow::{long_rig, long_turns, section, sidebar_entry};
use super::sidebar_keys::hidden;
use super::*;
use crate::components::transcript::Presentation;
use crate::components::transcript::fixtures::{arc, with_id};
use crate::components::turn_block::fixtures as blocks;
use crate::components::{HelpContext, PRESENTATION_ATTR};
use crate::test_support::TestClock;

const PRESS: MouseEventKind = MouseEventKind::Down(MouseButton::Left);
const WHEEL_DOWN: MouseEventKind = MouseEventKind::ScrollDown;
const WHEEL_UP: MouseEventKind = MouseEventKind::ScrollUp;

fn at(kind: MouseEventKind, column: u16, row: u16) -> Msg {
    Msg::Pointer(MouseEvent {
        kind,
        modifiers: KeyModifiers::NONE,
        column,
        row,
    })
}

fn presentation(app: &App) -> Presentation {
    let value = app
        .application()
        .query(&app.top_transcript(), PRESENTATION_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

fn sidebar_focused(rig: &Rig) -> bool {
    rig.focus() == Some(Id::TurnSidebar)
}

fn turn_of(app: &App) -> Option<String> {
    section(app).map(|s| s.turn.as_str().to_owned())
}

/// The turn id of the entry `entry` of [`long_turns`].
fn long_turn_id(entry: usize) -> String {
    format!(
        "{}{}",
        if entry.is_multiple_of(2) { "u" } else { "a" },
        entry / 2
    )
}

impl Rig {
    fn pane(&self) -> Rect {
        let pane = self.app.routing().area(Id::TurnSidebar).unwrap();
        assert_eq!(pane.y, 5, "the pane starts under the header");
        pane
    }

    /// A press at (`dx`, `dy`) of the pane.
    fn press_pane(&mut self, dx: u16, dy: u16) {
        let pane = self.pane();
        self.app.update_routed(at(PRESS, pane.x + dx, pane.y + dy));
    }

    /// A wheel step over the middle of the pane.
    fn wheel_pane(&mut self, kind: MouseEventKind) {
        let pane = self.pane();
        self.app.update_routed(at(kind, pane.x + 10, pane.y + 6));
    }

    /// The row of the pane (from its top) that shows the selected entry,
    /// read from the screen.
    fn marker_row(&mut self) -> u16 {
        let (width, height) = self.app.size();
        let row = pane_rows(self, width, height)
            .iter()
            .position(|row| row.chars().nth(3) == Some('▒'))
            .expect("the selected row is on screen");
        u16::try_from(row).unwrap()
    }

    /// The row of the pane that shows `entry` of an alternating list, which
    /// must be on screen (two rows per entry from the selected one).
    fn row_of(&mut self, entry: usize) -> u16 {
        let selected = sidebar_entry(&self.app).expect("a selected entry");
        let marker = i32::from(self.marker_row());
        let row = marker + 2 * (i32::try_from(entry).unwrap() - i32::try_from(selected).unwrap());
        u16::try_from(row).unwrap_or_else(|_| panic!("entry {entry} is not on screen"))
    }

    /// A left press on the row of `entry`.
    fn click_entry(&mut self, entry: usize) {
        let row = self.row_of(entry);
        self.press_pane(10, row);
    }

    /// Gives the app a click clock that shows the returned time.
    fn click_clock(&mut self) -> TestClock {
        let elapsed = TestClock::new();
        self.app.set_click_clock(elapsed.reader());
        elapsed
    }
}

/// A rig whose sidebar is as high as the list: `u0 a0 [compaction] u1 a1`.
/// Rows: 0 Start, 1 connector, 2 `u0`, 3 connector, 4 `a0`, 5 connector, 6
/// Compaction, 7 connector, 8 `u1`, 9 connector, 10 `a1`, 11 connector, 12
/// End.
fn compaction_rig() -> Rig {
    let mut turns = long_turns(2);
    turns.insert(2, arc(with_id(blocks::compaction("A summary."), "c1")));
    let mut rig = rig_with(theme("default"), 120, 30);
    rig.open_turns(turns);
    assert_eq!(sidebar_rows(&rig.app).row_count(), 13);
    rig
}

// -- clicking a row

#[test]
fn a_click_on_a_row_focuses_the_sidebar_selects_it_and_scrolls_like_j() {
    for entry in 1..=3 {
        let mut clicked = long_rig(10, 120, 20);
        assert_eq!(clicked.focus(), Some(Id::Transcript));
        let start = presentation(&clicked.app);
        clicked.click_entry(entry);
        assert!(sidebar_focused(&clicked), "entry {entry}");
        assert_eq!(sidebar_entry(&clicked.app), Some(entry));
        assert_eq!(turn_of(&clicked.app), Some(long_turn_id(entry)));
        let after = presentation(&clicked.app);
        assert_ne!(after.offset, start.offset, "entry {entry}: it scrolled");
        // The same result as `j` in the sidebar.
        let mut keys = long_rig(10, 120, 20);
        keys.focus_sidebar();
        keys.select_down(entry);
        assert_eq!(after, presentation(&keys.app), "entry {entry}");
        assert_eq!(clicked.focus(), keys.focus());
    }
}

#[test]
fn a_click_moves_the_selection_up_too_and_leaves_the_pane_focused() {
    let mut rig = long_rig(10, 120, 20);
    rig.click_entry(3);
    let at_three = presentation(&rig.app);
    rig.click_entry(2);
    assert!(sidebar_focused(&rig));
    assert_eq!(sidebar_entry(&rig.app), Some(2));
    assert_eq!(turn_of(&rig.app), Some("u1".to_owned()));
    // And `k` from there is the same as it would be after `k` from 3.
    let mut keys = long_rig(10, 120, 20);
    keys.focus_sidebar();
    keys.select_down(3);
    assert_eq!(presentation(&keys.app), at_three);
    keys.press(Key::Char('k'));
    keys.pump_until(|app| sidebar_entry(app) == Some(2));
    assert_eq!(presentation(&rig.app), presentation(&keys.app));
}

#[test]
fn a_click_selects_the_row_wherever_in_the_row_it_lands() {
    for column in [1, 2, 3, 10, 27, 28] {
        let mut rig = long_rig(10, 120, 20);
        let row = rig.row_of(0) + 4;
        rig.press_pane(column, row);
        assert_eq!(sidebar_entry(&rig.app), Some(2), "column {column}");
        assert!(sidebar_focused(&rig));
    }
}

#[test]
fn a_click_on_a_compaction_row_selects_it() {
    let mut rig = compaction_rig();
    rig.press_pane(10, 2 + 6);
    assert_eq!(sidebar_entry(&rig.app), Some(2));
    assert_eq!(turn_of(&rig.app), Some("c1".to_owned()));
    assert!(sidebar_focused(&rig));
}

#[test]
fn a_click_on_an_entry_with_nothing_selectable_scrolls_and_leaves_the_transcripts_selection() {
    let mut turns = long_turns(6);
    turns.insert(1, hidden("hidden"));
    let mut rig = rig_with(theme("default"), 120, 20);
    rig.open_turns(turns);
    let before = presentation(&rig.app);
    rig.click_entry(1);
    assert!(sidebar_focused(&rig));
    assert_eq!(sidebar_entry(&rig.app), Some(1));
    let after = presentation(&rig.app);
    assert_eq!(after.selected, before.selected, "nothing to select there");
    assert_ne!(after.offset, before.offset, "but the view is at the block");
    // `Enter` gives the transcript the focus, on the section it had.
    rig.press(Key::Enter);
    rig.pump_until(|app| app.application().focus() == Some(&Id::Transcript));
    assert_eq!(presentation(&rig.app).selected, before.selected);
}

#[test]
fn the_pane_is_focused_by_the_click_and_the_hints_follow() {
    let mut rig = long_rig(10, 120, 20);
    rig.click_entry(1);
    let text = screen_text(&mut rig, 120, 20);
    assert!(text.contains("ENTER to go to block"), "{text}");
}

// -- double-clicking a row

#[test]
fn a_double_click_does_what_enter_does() {
    let mut rig = long_rig(10, 120, 20);
    let clock = rig.click_clock();
    rig.click_entry(3);
    assert!(sidebar_focused(&rig));
    clock.set(Duration::from_millis(250));
    rig.click_entry(3);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(sidebar_entry(&rig.app), Some(3));
    assert_eq!(turn_of(&rig.app), Some("a1".to_owned()));
    // The same as `j` three times and `Enter`.
    let mut keys = long_rig(10, 120, 20);
    keys.focus_sidebar();
    keys.select_down(3);
    keys.press(Key::Enter);
    keys.pump_until(|app| app.application().focus() == Some(&Id::Transcript));
    assert_eq!(presentation(&rig.app), presentation(&keys.app));
    // The transcript's selection is the block's first selectable section,
    // and the keys are the transcript's now.
    rig.press(Key::Char('j'));
    rig.settle();
    assert_eq!(turn_of(&rig.app), Some("u2".to_owned()));
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn a_double_click_on_a_row_the_transcript_cannot_select_in_leaves_its_selection() {
    let mut turns = long_turns(6);
    turns.insert(1, hidden("hidden"));
    let mut rig = rig_with(theme("default"), 120, 20);
    rig.open_turns(turns);
    let before = presentation(&rig.app);
    let clock = rig.click_clock();
    rig.click_entry(1);
    clock.set(Duration::from_millis(100));
    rig.click_entry(1);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(presentation(&rig.app).selected, before.selected);
}

#[test]
fn a_third_click_starts_a_new_double_click() {
    let mut rig = long_rig(10, 120, 20);
    let clock = rig.click_clock();
    rig.click_entry(3);
    clock.set(Duration::from_millis(100));
    rig.click_entry(3);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // The sidebar has no focus now; the third click focuses it only.
    clock.set(Duration::from_millis(200));
    rig.click_entry(3);
    assert!(sidebar_focused(&rig));
    clock.set(Duration::from_millis(300));
    rig.click_entry(3);
    assert_eq!(rig.focus(), Some(Id::Transcript), "the fourth completes");
}

#[test]
fn two_clicks_on_different_rows_are_not_a_double_click() {
    let mut rig = long_rig(10, 120, 20);
    let clock = rig.click_clock();
    rig.click_entry(2);
    clock.set(Duration::from_millis(100));
    rig.click_entry(3);
    assert!(sidebar_focused(&rig));
    assert_eq!(sidebar_entry(&rig.app), Some(3));
    // The second row clicked again soon after is a double-click.
    clock.set(Duration::from_millis(200));
    rig.click_entry(3);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn two_clicks_further_apart_than_the_double_click_time_are_not_a_double_click() {
    for (ms, double) in [(400, true), (401, false), (1000, false)] {
        let mut rig = long_rig(10, 120, 20);
        let clock = rig.click_clock();
        rig.click_entry(3);
        clock.set(Duration::from_millis(ms));
        rig.click_entry(3);
        let expected = if double {
            Id::Transcript
        } else {
            Id::TurnSidebar
        };
        assert_eq!(rig.focus(), Some(expected), "{ms} ms");
    }
}

// -- presses that are not on a row

/// Where the presses that are on no entry land, in [`compaction_rig`]'s pane
/// (from its top left): the border, the blank row above the list, `Start`,
/// a connector, a connector next to the Compaction, `End`, below the list,
/// and the gap after the pane.
fn dead_spots() -> Vec<(&'static str, u16, u16)> {
    vec![
        ("top border", 10, 0),
        ("blank row above the list", 10, 1),
        ("Start", 10, 2),
        ("Start, its marker", 3, 2),
        ("connector", 3, 3),
        ("connector, to the side", 20, 3),
        ("connector before the Compaction", 10, 2 + 5),
        ("connector after the Compaction", 10, 2 + 7),
        ("End", 10, 2 + 12),
        ("below the list", 10, 2 + 13),
        ("bottom border", 10, 29),
        ("left border", 0, 2 + 2),
        ("right border", 29, 2 + 2),
        ("left border, on Start", 0, 2),
        ("the gap after the pane", 30, 2 + 2),
    ]
}

#[test]
fn presses_on_no_entry_change_neither_the_selection_nor_the_focus() {
    for sidebar_has_focus in [false, true] {
        let mut rig = compaction_rig();
        if sidebar_has_focus {
            rig.click_entry(1);
            assert!(sidebar_focused(&rig));
        }
        let focus = rig.focus();
        let entry = sidebar_entry(&rig.app);
        let shown = presentation(&rig.app);
        let turn = turn_of(&rig.app);
        for (what, dx, dy) in dead_spots() {
            rig.press_pane(dx, dy);
            assert_eq!(
                rig.focus(),
                focus,
                "{what}: focus (sidebar {sidebar_has_focus})"
            );
            assert_eq!(sidebar_entry(&rig.app), entry, "{what}: selection");
            assert_eq!(presentation(&rig.app), shown, "{what}: transcript");
            assert_eq!(turn_of(&rig.app), turn, "{what}: section");
            assert!(rig.app.take_queued().is_empty(), "{what}: messages");
        }
    }
}

#[test]
fn presses_on_no_entry_do_not_focus_the_sidebar_from_the_transcript() {
    let mut rig = compaction_rig();
    for (what, dx, dy) in dead_spots() {
        rig.press_pane(dx, dy);
        assert_eq!(rig.focus(), Some(Id::Transcript), "{what}");
    }
}

#[test]
fn a_press_on_no_entry_between_two_clicks_on_a_row_ends_the_double_click() {
    // Without the press in between, the second click is a double-click.
    let mut rig = compaction_rig();
    let clock = rig.click_clock();
    rig.click_entry(2);
    clock.set(Duration::from_millis(100));
    rig.click_entry(2);
    assert_eq!(rig.focus(), Some(Id::Transcript), "the control");
    for (what, dx, dy) in dead_spots() {
        let mut rig = compaction_rig();
        let clock = rig.click_clock();
        rig.click_entry(2);
        clock.set(Duration::from_millis(100));
        rig.press_pane(dx, dy);
        clock.set(Duration::from_millis(200));
        rig.click_entry(2);
        assert_eq!(
            rig.focus(),
            Some(Id::TurnSidebar),
            "{what}: not a double-click"
        );
        // The click after that, soon enough, is the double-click.
        clock.set(Duration::from_millis(300));
        rig.click_entry(2);
        assert_eq!(rig.focus(), Some(Id::Transcript), "{what}: then it is");
    }
}

#[test]
fn a_press_elsewhere_on_the_screen_between_two_clicks_ends_the_double_click_too() {
    let mut rig = compaction_rig();
    let clock = rig.click_clock();
    rig.click_entry(2);
    clock.set(Duration::from_millis(100));
    // The transcript's far corner.
    let area = rig.app.routing().area(Id::Transcript).unwrap();
    rig.app.update_routed(at(PRESS, area.x + 60, area.y + 5));
    rig.focus_sidebar();
    clock.set(Duration::from_millis(200));
    rig.click_entry(2);
    assert_eq!(rig.focus(), Some(Id::TurnSidebar));
}

// -- a scrolled list

#[test]
fn a_row_is_found_when_the_list_is_scrolled() {
    let mut rig = long_rig(30, 120, 20);
    rig.focus_sidebar();
    rig.select_down(40);
    let shown = pane_rows(&mut rig, 120, 20);
    assert!(
        shown.iter().all(|row| !row.contains("Start")),
        "the list is scrolled: {shown:#?}"
    );
    let selected = sidebar_entry(&rig.app).unwrap();
    // The list is scrolled to show the selected entry on its last row.
    for entry in [
        selected - 3,
        selected - 1,
        selected - 4,
        selected,
        selected - 2,
    ] {
        rig.click_entry(entry);
        assert_eq!(sidebar_entry(&rig.app), Some(entry));
        assert_eq!(turn_of(&rig.app), Some(long_turn_id(entry)));
        assert!(sidebar_focused(&rig));
    }
}

#[test]
fn a_connector_row_is_found_when_the_list_is_scrolled() {
    let mut rig = long_rig(30, 120, 20);
    rig.focus_sidebar();
    rig.select_down(40);
    let entry = sidebar_entry(&rig.app).unwrap();
    let shown = presentation(&rig.app);
    let marker = rig.marker_row();
    for row in [marker - 1, marker + 1, marker - 3] {
        rig.press_pane(10, row);
        assert_eq!(sidebar_entry(&rig.app), Some(entry), "row {row}");
        assert_eq!(presentation(&rig.app), shown);
    }
}

#[test]
fn end_is_found_at_the_end_of_a_scrolled_list_and_is_not_selectable() {
    let mut rig = long_rig(30, 120, 20);
    rig.focus_sidebar();
    rig.select_down(59);
    // A taller pane shows the end of the list, scrolled.
    rig.app.update(Msg::Resized(120, 30));
    let rows = pane_rows(&mut rig, 120, 30);
    assert!(rows.iter().all(|row| !row.contains("Start")), "{rows:#?}");
    let end = rows.iter().position(|row| row.contains("End")).unwrap();
    let shown = presentation(&rig.app);
    rig.press_pane(10, u16::try_from(end).unwrap());
    rig.press_pane(10, u16::try_from(end).unwrap() - 1);
    assert_eq!(sidebar_entry(&rig.app), Some(59));
    assert_eq!(presentation(&rig.app), shown);
    rig.click_entry(58);
    assert_eq!(sidebar_entry(&rig.app), Some(58));
    assert_eq!(turn_of(&rig.app), Some(long_turn_id(58)));
}

// -- the wheel

#[test]
fn the_wheel_selects_the_next_and_previous_entry_and_scrolls_like_the_keys() {
    let mut rig = long_rig(10, 120, 20);
    let mut keys = long_rig(10, 120, 20);
    keys.focus_sidebar();
    for entry in 1..=4 {
        rig.wheel_pane(WHEEL_DOWN);
        keys.select_down(1);
        assert_eq!(sidebar_entry(&rig.app), Some(entry));
        assert_eq!(turn_of(&rig.app), Some(long_turn_id(entry)));
        assert_eq!(presentation(&rig.app), presentation(&keys.app), "{entry}");
    }
    for entry in (0..4).rev() {
        rig.wheel_pane(WHEEL_UP);
        keys.press(Key::Char('k'));
        keys.pump_until(|app| sidebar_entry(app) == Some(entry));
        assert_eq!(sidebar_entry(&rig.app), Some(entry));
        assert_eq!(presentation(&rig.app), presentation(&keys.app), "{entry}");
    }
}

#[test]
fn the_wheel_does_not_change_the_focus() {
    for focused in [Id::Transcript, Id::TurnSidebar] {
        let mut rig = long_rig(10, 120, 20);
        if focused == Id::TurnSidebar {
            rig.focus_sidebar();
        }
        for _ in 0..3 {
            rig.wheel_pane(WHEEL_DOWN);
            assert_eq!(rig.focus(), Some(focused));
        }
        rig.wheel_pane(WHEEL_UP);
        assert_eq!(rig.focus(), Some(focused));
        assert_eq!(sidebar_entry(&rig.app), Some(2));
        assert_eq!(turn_of(&rig.app), Some("u1".to_owned()));
    }
}

#[test]
fn the_wheel_does_not_wrap_around_at_either_end() {
    let mut rig = long_rig(4, 120, 20);
    let start = presentation(&rig.app);
    rig.wheel_pane(WHEEL_UP);
    rig.wheel_pane(WHEEL_UP);
    assert_eq!(sidebar_entry(&rig.app), Some(0), "no wrap to the last");
    assert_eq!(presentation(&rig.app), start);
    for _ in 0..12 {
        rig.wheel_pane(WHEEL_DOWN);
    }
    let last = sidebar_rows(&rig.app).entry_count() - 1;
    assert_eq!(sidebar_entry(&rig.app), Some(last), "no wrap to the first");
    let end = presentation(&rig.app);
    rig.wheel_pane(WHEEL_DOWN);
    assert_eq!(sidebar_entry(&rig.app), Some(last));
    assert_eq!(presentation(&rig.app), end);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // And the same with the sidebar focused.
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.wheel_pane(WHEEL_UP);
    assert_eq!(sidebar_entry(&rig.app), Some(0));
    for _ in 0..12 {
        rig.wheel_pane(WHEEL_DOWN);
    }
    assert_eq!(sidebar_entry(&rig.app), Some(last));
    assert_eq!(rig.focus(), Some(Id::TurnSidebar));
}

#[test]
fn the_wheel_works_anywhere_over_the_pane_and_not_in_the_gap() {
    let mut rig = long_rig(10, 120, 20);
    for (dx, dy) in [(0, 0), (29, 10), (15, 1), (10, 12)] {
        let pane = rig.pane();
        let before = sidebar_entry(&rig.app).unwrap();
        rig.app
            .update_routed(at(WHEEL_DOWN, pane.x + dx, pane.y + dy));
        assert_eq!(sidebar_entry(&rig.app), Some(before + 1), "({dx}, {dy})");
    }
    let pane = rig.pane();
    let before = presentation(&rig.app);
    rig.app
        .update_routed(at(WHEEL_DOWN, pane.x + 30, pane.y + 6));
    assert_eq!(sidebar_entry(&rig.app), Some(4));
    assert_eq!(presentation(&rig.app), before);
}

#[test]
fn the_wheel_scrolls_the_pane_to_keep_the_selection_visible() {
    let mut rig = long_rig(30, 120, 20);
    for _ in 0..30 {
        rig.wheel_pane(WHEEL_DOWN);
    }
    assert_eq!(sidebar_entry(&rig.app), Some(30));
    let marked: Vec<String> = pane_rows(&mut rig, 120, 20)
        .into_iter()
        .filter(|row| row.chars().nth(3) == Some('▒'))
        .collect();
    assert_eq!(marked.len(), 1, "{marked:?}");
}

#[test]
fn the_wheel_moves_past_an_entry_with_nothing_selectable_while_the_transcript_has_the_focus() {
    let mut turns = long_turns(6);
    turns.insert(1, hidden("hidden"));
    let mut rig = rig_with(theme("default"), 120, 20);
    rig.open_turns(turns);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(turn_of(&rig.app), Some("u0".to_owned()));
    // Onto the entry: the transcript keeps its selection, and the sidebar
    // its row (the transcript's selection did not change, so nothing is
    // pushed back).
    rig.wheel_pane(WHEEL_DOWN);
    assert_eq!(sidebar_entry(&rig.app), Some(1));
    assert_eq!(turn_of(&rig.app), Some("u0".to_owned()));
    let there = presentation(&rig.app);
    // And past it.
    rig.wheel_pane(WHEEL_DOWN);
    assert_eq!(sidebar_entry(&rig.app), Some(2));
    assert_eq!(turn_of(&rig.app), Some("a0".to_owned()));
    assert_ne!(presentation(&rig.app).offset, there.offset);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // Back the other way: onto it and past it.
    rig.wheel_pane(WHEEL_UP);
    assert_eq!(sidebar_entry(&rig.app), Some(1));
    rig.wheel_pane(WHEEL_UP);
    assert_eq!(sidebar_entry(&rig.app), Some(0));
    assert_eq!(turn_of(&rig.app), Some("u0".to_owned()));
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn the_wheel_over_the_transcript_still_scrolls_it_and_the_sidebar_follows() {
    let mut rig = long_rig(10, 120, 20);
    rig.wheel(WHEEL_DOWN);
    rig.wheel(WHEEL_DOWN);
    assert!(presentation(&rig.app).offset > 0);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    let turn = turn_of(&rig.app).unwrap();
    let entry = sidebar_rows(&rig.app)
        .entry_index(&crate::model::TurnId::from(turn.as_str()))
        .unwrap();
    assert_eq!(sidebar_entry(&rig.app), Some(entry));
}

// -- clicking the transcript

#[test]
fn a_click_on_the_transcript_gives_it_the_focus_while_the_sidebar_has_it() {
    let mut rig = long_rig(10, 120, 20);
    rig.click_entry(3);
    assert!(sidebar_focused(&rig));
    let area = rig.app.routing().area(Id::Transcript).unwrap();
    rig.app.update_routed(at(PRESS, area.x + 10, area.y + 2));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    let text = screen_text(&mut rig, 120, 20);
    assert!(!text.contains("ENTER to go to block"), "the hints follow");
    // The click's selection syncs the sidebar.
    let turn = turn_of(&rig.app).unwrap();
    let entry = sidebar_rows(&rig.app)
        .entry_index(&crate::model::TurnId::from(turn.as_str()))
        .unwrap();
    assert_eq!(sidebar_entry(&rig.app), Some(entry));
}

#[test]
fn a_click_on_the_transcript_after_the_wheel_moved_the_sidebar_on_syncs_it() {
    let mut turns = long_turns(6);
    turns.insert(1, hidden("hidden"));
    let mut rig = rig_with(theme("default"), 120, 20);
    rig.open_turns(turns);
    rig.click_entry(1);
    assert!(sidebar_focused(&rig));
    let area = rig.app.routing().area(Id::Transcript).unwrap();
    rig.app.update_routed(at(PRESS, area.x + 10, area.y + 1));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    let turn = turn_of(&rig.app).unwrap();
    let entry = sidebar_rows(&rig.app)
        .entry_index(&crate::model::TurnId::from(turn.as_str()))
        .unwrap();
    assert_eq!(sidebar_entry(&rig.app), Some(entry));
    assert_ne!(entry, 1, "the hidden entry has no selectable section");
}

// -- when the sidebar gets no mouse

/// A press on a row and a wheel step over the pane, at positions found
/// before something covered the pane.
struct Aim {
    pane: Rect,
    row: u16,
}

fn aim(rig: &mut Rig) -> Aim {
    Aim {
        pane: rig.pane(),
        row: rig.row_of(3),
    }
}

impl Aim {
    fn press(&self, rig: &mut Rig) {
        rig.app
            .update_routed(at(PRESS, self.pane.x + 10, self.pane.y + self.row));
    }

    fn wheel(&self, rig: &mut Rig) {
        rig.app
            .update_routed(at(WHEEL_DOWN, self.pane.x + 10, self.pane.y + 6));
    }

    fn fire(&self, rig: &mut Rig) {
        self.press(rig);
        self.wheel(rig);
    }
}

#[test]
fn with_a_prompt_open_the_sidebar_gets_no_mouse() {
    let mut rig = long_rig(10, 120, 20);
    let aim = aim(&mut rig);
    rig.press(Key::Char('/'));
    rig.pump_until(|app| app.application().focus() == Some(&Id::Prompt));
    let entry = sidebar_entry(&rig.app);
    let shown = presentation(&rig.app);
    aim.fire(&mut rig);
    assert_eq!(rig.focus(), Some(Id::Prompt));
    assert_eq!(sidebar_entry(&rig.app), entry);
    assert_eq!(presentation(&rig.app), shown);
}

#[test]
fn with_an_overlay_open_the_sidebar_gets_no_mouse() {
    type Open = fn(&mut Rig);
    let overlays: [(&str, Id, Open); 3] = [
        ("help", Id::HelpOverlay, |rig| {
            rig.app
                .update(Msg::Overlay(OverlayMsg::OpenHelp(HelpContext::Browser)));
        }),
        ("themes", Id::ThemeSelector, Rig::open_themes),
        ("selector", Id::SelectorFrame, |rig| {
            rig.ctrl_o();
            rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
        }),
    ];
    for (what, overlay, open) in overlays {
        let mut rig = long_rig(10, 120, 40);
        let aim = aim(&mut rig);
        open(&mut rig);
        let area = rig.app.routing().area(overlay).unwrap();
        let entry = sidebar_entry(&rig.app);
        let shown = presentation(&rig.app);
        // The wheel step and the press land on the pane, which the overlay
        // may or may not cover; either way the sidebar gets neither. (A
        // press outside the overlay closes it, so the wheel goes first.)
        aim.wheel(&mut rig);
        assert!(rig.app.application().mounted(&overlay), "{what}: open");
        assert_ne!(rig.focus(), Some(Id::TurnSidebar), "{what}");
        assert_eq!(sidebar_entry(&rig.app), entry, "{what}: wheel");
        assert_eq!(presentation(&rig.app), shown, "{what}: wheel");
        aim.press(&mut rig);
        assert_ne!(rig.focus(), Some(Id::TurnSidebar), "{what}");
        assert_eq!(sidebar_entry(&rig.app), entry, "{what}: press");
        assert_eq!(presentation(&rig.app), shown, "{what}: press");
        assert!(area.width > 0);
    }
}

#[test]
fn a_press_on_the_sidebar_under_an_overlay_that_covers_it_does_not_reach_it() {
    // The theme selector is drawn over the middle of the screen; the
    // selector over the browser covers the whole of it below the header.
    let mut rig = long_rig(10, 120, 40);
    let aim = aim(&mut rig);
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    let overlay = rig.app.routing().area(Id::SelectorFrame).unwrap();
    let point = (aim.pane.x + 10, aim.pane.y + aim.row);
    assert!(
        overlay.contains(point.into()),
        "the press lands in the overlay: {overlay:?} {point:?}"
    );
    let entry = sidebar_entry(&rig.app);
    aim.fire(&mut rig);
    assert_eq!(rig.app.screen(), Screen::BrowserSelector, "still open");
    assert_eq!(sidebar_entry(&rig.app), entry);
    assert_eq!(rig.focus(), Some(Id::ProjectList));
}

#[test]
fn with_mouse_off_the_sidebar_gets_no_mouse() {
    let mut init = init(&[CLAUDE], theme("default"));
    init.config.mouse = false;
    let mut rig = build_rig_from(init, 120, 20, false);
    rig.open_turns(long_turns(10));
    let aim = aim(&mut rig);
    let shown = presentation(&rig.app);
    aim.fire(&mut rig);
    aim.fire(&mut rig);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(sidebar_entry(&rig.app), Some(0));
    assert_eq!(presentation(&rig.app), shown);
}

#[test]
fn the_sidebar_gets_no_mouse_when_it_is_hidden() {
    let mut rig = long_rig(10, 89, 20);
    assert!(rig.app.routing().area(Id::TurnSidebar).is_none());
    // Where the pane would be at 120 columns: the transcript is there.
    rig.app.update_routed(at(PRESS, 5, 9));
    rig.app.update_routed(at(WHEEL_DOWN, 5, 9));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!rig.app.take_queued().iter().any(|msg| matches!(
        msg,
        Msg::Browser(BrowserMsg::TurnSelected(_))
            | Msg::Browser(BrowserMsg::ChooseTurn { .. })
            | Msg::Browser(BrowserMsg::OpenTurn(_))
    )));
}

// -- the press-elsewhere notice

#[test]
fn a_press_on_the_header_the_toast_or_the_transcript_between_two_clicks_on_an_entry_ends_it() {
    // The control: without the press between, the second click enters.
    let mut rig = compaction_rig();
    let clock = rig.click_clock();
    rig.click_entry(2);
    clock.set(Duration::from_millis(100));
    rig.click_entry(2);
    assert_eq!(rig.focus(), Some(Id::Transcript), "the control");
    // The header and the toast take no pointer; the transcript does.
    let elsewhere = [
        (Id::BrowserHeader, 10, 0),
        (Id::Toast, 2, 1),
        (Id::Transcript, 60, 5),
    ];
    for (what, dx, dy) in elsewhere {
        let mut rig = compaction_rig();
        rig.app.show_toast("A toast to press").unwrap();
        let clock = rig.click_clock();
        rig.click_entry(2);
        let area = rig.app.routing().area(what).unwrap();
        rig.app.update_routed(at(PRESS, area.x + dx, area.y + dy));
        // A press on the transcript gives it the focus; back to the sidebar.
        rig.focus_sidebar();
        clock.set(Duration::from_millis(100));
        rig.click_entry(2);
        assert_eq!(rig.focus(), Some(Id::TurnSidebar), "{what:?}: not entered");
        clock.set(Duration::from_millis(200));
        rig.click_entry(2);
        assert_eq!(rig.focus(), Some(Id::Transcript), "{what:?}: then it is");
    }
}

#[test]
fn the_click_clock_reaches_a_sidebar_mounted_after_it_was_set() {
    for (ms, enters) in [(400, true), (401, false)] {
        let mut rig = rig_with(theme("default"), 120, 30);
        let clock = rig.click_clock();
        rig.open_turns(long_turns(4));
        rig.click_entry(1);
        clock.set(Duration::from_millis(ms));
        rig.click_entry(1);
        assert_eq!(rig.focus() == Some(Id::Transcript), enters, "{ms} ms");
    }
}
