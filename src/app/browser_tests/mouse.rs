//! The mouse with a session open: the transcript takes the wheel and clicks,
//! the selector over the browser takes it and the browser under it does not,
//! and a double-click in the selector opens the session through the real
//! load.

use std::cell::Cell;
use std::rc::Rc;

use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};

use super::*;
use crate::app::layout::Screen;
use crate::app::{Pane, SelectorMsg};
use crate::components::transcript::Presentation;
use crate::components::{FOLLOWING_ATTR, PRESENTATION_ATTR};

fn press(column: u16, row: u16) -> Msg {
    Msg::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        modifiers: KeyModifiers::NONE,
        column,
        row,
    })
}

/// A press on list row `line` of the pane `pane`.
fn click_row(rig: &mut Rig, pane: Id, line: u16) {
    let area = rig.app.routing().area(pane).unwrap();
    rig.app.update(press(area.x + 6, area.y + 2 + line));
}

fn selected_session(rig: &Rig) -> Option<String> {
    rig.app.selected_session().map(|s| s.as_str().to_owned())
}

#[test]
fn a_double_click_in_the_selector_opens_the_session() {
    let mut rig = rig();
    let start = Instant::now();
    let elapsed = Rc::new(Cell::new(Duration::ZERO));
    let shared = Rc::clone(&elapsed);
    rig.app.set_click_clock(move || start + shared.get());
    // BRIO's sessions, newest first: NEWER_SESSION, BRIO_SESSION.
    rig.harness.script(
        BRIO_SESSION,
        vec![ModelUpdate::Appended(turns()), ModelUpdate::Loaded],
    );
    click_row(&mut rig, Id::SessionList, 1);
    assert_eq!(selected_session(&rig).as_deref(), Some(BRIO_SESSION));
    assert!(rig.app.take_queued().is_empty());
    elapsed.set(Duration::from_millis(250));
    click_row(&mut rig, Id::SessionList, 1);
    for msg in rig.app.take_queued() {
        rig.app.update(msg);
    }
    rig.pump_until(App::session_loaded);
    assert_eq!(rig.app.screen(), Screen::Browser);
    let open = rig.app.open_session_id().unwrap();
    assert_eq!((open.0, open.1.as_str()), (CLAUDE, BRIO_SESSION));
    // The routing follows the screen: the selector's panes are gone.
    assert!(rig.app.routing().area(Id::SessionList).is_none());
    assert!(rig.app.routing().area(Id::BrowserHeader).is_some());
}

// -- the transcript

const WHEEL_DOWN: MouseEventKind = MouseEventKind::ScrollDown;
const WHEEL_UP: MouseEventKind = MouseEventKind::ScrollUp;

fn mouse_at(kind: MouseEventKind, column: u16, row: u16) -> Msg {
    Msg::Mouse(MouseEvent {
        kind,
        modifiers: KeyModifiers::NONE,
        column,
        row,
    })
}

/// A mouse event at (`column`, `row`) of the transcript's area.
fn on_transcript(rig: &mut Rig, kind: MouseEventKind, column: u16, row: u16) {
    let area = rig.app.routing().area(Id::Transcript).unwrap();
    rig.app
        .update(mouse_at(kind, area.x + column, area.y + row));
}

fn presentation_of(rig: &Rig, id: Id) -> Presentation {
    let value = rig
        .app
        .application()
        .query(&id, PRESENTATION_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

fn offset_of(rig: &Rig, id: Id) -> usize {
    presentation_of(rig, id).offset
}

fn following_of(rig: &Rig, id: Id) -> bool {
    rig.app
        .application()
        .query(&id, FOLLOWING_ATTR)
        .unwrap()
        .unwrap()
        .into_attr()
        == AttrValue::Flag(true)
}

/// The rows of the transcript's area as drawn.
fn transcript_rows(rig: &mut Rig) -> Vec<String> {
    let area = rig.app.routing().area(Id::Transcript).unwrap();
    let screen = screen_text(rig, 120, 40);
    screen
        .lines()
        .skip(usize::from(area.y))
        .take(usize::from(area.height))
        .map(str::to_owned)
        .collect()
}

/// The row of the transcript's area that shows `text`.
fn row_with(rig: &mut Rig, text: &str) -> u16 {
    let rows = transcript_rows(rig);
    let at = rows
        .iter()
        .position(|row| row.contains(text))
        .unwrap_or_else(|| panic!("no row shows {text:?}:\n{}", rows.join("\n")));
    u16::try_from(at).unwrap()
}

#[test]
fn the_wheel_over_the_transcript_scrolls_three_rows_per_event() {
    let mut rig = rig();
    rig.open_loaded();
    assert_eq!(offset_of(&rig, Id::Transcript), 0);
    on_transcript(&mut rig, WHEEL_DOWN, 10, 5);
    assert_eq!(offset_of(&rig, Id::Transcript), 3);
    on_transcript(&mut rig, WHEEL_DOWN, 10, 5);
    assert_eq!(offset_of(&rig, Id::Transcript), 6);
    on_transcript(&mut rig, WHEEL_UP, 10, 5);
    assert_eq!(offset_of(&rig, Id::Transcript), 3);
    assert!(rig.app.take_queued().is_empty());
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn the_wheel_moves_the_selection_to_the_topmost_fully_visible_section() {
    let mut rig = rig();
    rig.open_loaded();
    let first = presentation_of(&rig, Id::Transcript).selected.unwrap();
    for _ in 0..3 {
        on_transcript(&mut rig, WHEEL_DOWN, 10, 5);
    }
    let after = presentation_of(&rig, Id::Transcript);
    assert_eq!(after.offset, 9);
    assert_ne!(
        after.selected,
        Some(first),
        "the first prompt scrolled away"
    );
    // The section selected is drawn: its marker is on screen.
    let rows = transcript_rows(&mut rig);
    assert!(rows.iter().any(|row| row.starts_with('▒')), "{rows:?}");
}

#[test]
fn a_wheel_up_stops_following_and_the_wheel_down_to_the_end_resumes_it() {
    let mut rig = rig();
    rig.open_loaded();
    assert!(!following_of(&rig, Id::Transcript));
    for _ in 0..40 {
        on_transcript(&mut rig, WHEEL_DOWN, 10, 5);
    }
    assert!(following_of(&rig, Id::Transcript), "at the end");
    on_transcript(&mut rig, WHEEL_UP, 10, 5);
    assert!(!following_of(&rig, Id::Transcript));
    on_transcript(&mut rig, WHEEL_DOWN, 10, 5);
    assert!(following_of(&rig, Id::Transcript));
}

#[test]
fn new_turns_after_a_wheel_up_are_counted_in_the_help_line() {
    let mut rig = rig();
    rig.open_loaded();
    for _ in 0..40 {
        on_transcript(&mut rig, WHEEL_DOWN, 10, 5);
    }
    on_transcript(&mut rig, WHEEL_UP, 10, 5);
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![crate::components::transcript::fixtures::arc(
            crate::components::transcript::fixtures::reply("t9", "A reply."),
        )]),
    ));
    assert_eq!(rig.app.new_sections(), 1);
    assert_eq!(status(&rig.app), ["↓ 1 new"]);
    for _ in 0..5 {
        on_transcript(&mut rig, WHEEL_DOWN, 10, 5);
    }
    assert!(following_of(&rig, Id::Transcript));
    assert_eq!(rig.app.new_sections(), 0);
    assert!(status(&rig.app).is_empty());
}

#[test]
fn a_click_on_a_summary_line_toggles_its_fold_and_selects_it() {
    let mut rig = rig();
    rig.open_loaded();
    let row = row_with(&mut rig, "▶ Thinking:");
    let before = presentation_of(&rig, Id::Transcript);
    on_transcript(&mut rig, MouseEventKind::Down(MouseButton::Left), 10, row);
    let rows = transcript_rows(&mut rig);
    assert!(rows[usize::from(row)].contains("▼ Thinking"), "{rows:?}");
    assert!(rows[usize::from(row)].starts_with('▒'), "selected");
    assert_ne!(
        presentation_of(&rig, Id::Transcript).selected,
        before.selected
    );
    assert_eq!(offset_of(&rig, Id::Transcript), before.offset, "no scroll");
    // A second click folds it again.
    on_transcript(&mut rig, MouseEventKind::Down(MouseButton::Left), 10, row);
    let rows = transcript_rows(&mut rig);
    assert!(rows[usize::from(row)].contains("▶ Thinking"), "{rows:?}");
}

#[test]
fn a_click_on_a_body_row_selects_without_toggling() {
    let mut rig = rig();
    rig.open_loaded();
    let row = row_with(&mut rig, "I'll start by looking at what the camera exposes");
    let before = presentation_of(&rig, Id::Transcript);
    on_transcript(&mut rig, MouseEventKind::Down(MouseButton::Left), 10, row);
    let after = presentation_of(&rig, Id::Transcript);
    assert_ne!(after.selected, before.selected);
    assert_eq!(after.folds, before.folds);
    assert_eq!(after.offset, before.offset);
}

#[test]
fn a_click_on_a_border_or_a_gap_selects_nothing() {
    let mut rig = rig();
    rig.open_loaded();
    let before = presentation_of(&rig, Id::Transcript);
    let area = rig.app.routing().area(Id::Transcript).unwrap();
    on_transcript(&mut rig, MouseEventKind::Down(MouseButton::Left), 10, 0);
    on_transcript(&mut rig, MouseEventKind::Down(MouseButton::Left), 0, 2);
    on_transcript(
        &mut rig,
        MouseEventKind::Down(MouseButton::Left),
        area.width - 1,
        2,
    );
    assert_eq!(presentation_of(&rig, Id::Transcript), before);
}

#[test]
fn a_click_on_the_transcript_focuses_it() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.application.blur().unwrap();
    assert_eq!(rig.focus(), None);
    let row = row_with(&mut rig, "▶ Thinking:");
    on_transcript(&mut rig, MouseEventKind::Down(MouseButton::Left), 10, row);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // Not on a section: the click still gives it the focus.
    rig.app.application.blur().unwrap();
    on_transcript(&mut rig, MouseEventKind::Down(MouseButton::Left), 0, 0);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn the_wheel_does_not_change_the_focus() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.application.blur().unwrap();
    on_transcript(&mut rig, WHEEL_DOWN, 10, 5);
    assert_eq!(
        offset_of(&rig, Id::Transcript),
        3,
        "it scrolls all the same"
    );
    assert_eq!(rig.focus(), None);
}

#[test]
fn the_header_and_the_help_line_ignore_clicks() {
    let mut rig = rig();
    rig.open_loaded();
    let before = presentation_of(&rig, Id::Transcript);
    for id in [Id::BrowserHeader, Id::BrowserHelpLine] {
        let area = rig.app.routing().area(id).unwrap();
        for kind in [MouseEventKind::Down(MouseButton::Left), WHEEL_DOWN] {
            rig.app
                .update(mouse_at(kind, area.x + 4, area.y + area.height - 1));
        }
    }
    assert_eq!(presentation_of(&rig, Id::Transcript), before);
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert!(rig.app.take_queued().is_empty());
}

#[test]
fn an_open_prompt_keeps_the_transcript_from_the_mouse() {
    let mut rig = rig();
    rig.open_loaded();
    rig.press(Key::Char('/'));
    rig.settle();
    assert_eq!(rig.focus(), Some(Id::Prompt));
    let before = presentation_of(&rig, Id::Transcript);
    let row = row_with(&mut rig, "▶ Thinking:");
    on_transcript(&mut rig, MouseEventKind::Down(MouseButton::Left), 10, row);
    on_transcript(&mut rig, WHEEL_DOWN, 10, 5);
    assert_eq!(presentation_of(&rig, Id::Transcript), before);
    assert_eq!(rig.focus(), Some(Id::Prompt));
}

#[test]
fn with_mouse_off_the_transcript_ignores_the_mouse() {
    let mut init = init(&[CLAUDE], theme("default"));
    init.config.mouse = false;
    let mut rig = build_rig_from(init, 120, 40, false);
    rig.open_loaded();
    let before = presentation_of(&rig, Id::Transcript);
    let row = row_with(&mut rig, "▶ Thinking:");
    on_transcript(&mut rig, MouseEventKind::Down(MouseButton::Left), 10, row);
    on_transcript(&mut rig, WHEEL_DOWN, 10, 5);
    assert_eq!(presentation_of(&rig, Id::Transcript), before);
}

#[test]
fn the_wheel_and_clicks_go_to_the_subagent_on_top() {
    let mut rig = super::subagent::subagent_rig(120, 40);
    rig.open_subagent(1);
    let area = rig.app.routing().area(Id::Transcript).unwrap();
    let parent = presentation_of(&rig, Id::Transcript);
    let child = presentation_of(&rig, Id::Subagent(1));
    // Click on the prompt of the subagent's first turn.
    let row = row_with(&mut rig, "Write a short story");
    rig.app.update(mouse_at(
        MouseEventKind::Down(MouseButton::Left),
        area.x + 10,
        area.y + row,
    ));
    let after = presentation_of(&rig, Id::Subagent(1));
    assert_eq!(
        after.selected.as_ref().map(|s| s.turn.to_string()),
        Some("su1".into())
    );
    assert_ne!(after, child, "the subagent's fold changed");
    assert_eq!(
        presentation_of(&rig, Id::Transcript),
        parent,
        "the parent did not"
    );
    assert_eq!(rig.focus(), Some(Id::Subagent(1)));
}

#[test]
fn the_selector_over_the_browser_takes_the_mouse_and_the_browser_under_it_does_not() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.settle();
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    // Both are routed, the overlay on top.
    assert!(rig.app.routing().area(Id::BrowserHeader).is_some());
    assert!(rig.app.routing().area(Id::Transcript).is_some());
    click_row(&mut rig, Id::SessionList, 0);
    assert_eq!(selected_session(&rig).as_deref(), Some(NEWER_SESSION));
    assert_eq!(rig.focus(), Some(Id::SessionList));
    // The overlay's frame, over the transcript, is no target: the transcript
    // under it gets nothing.
    let transcript = presentation_of(&rig, Id::Transcript);
    let projects = rig.app.routing().area(Id::ProjectList).unwrap();
    rig.app.update(press(projects.right(), projects.y + 2));
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    assert_eq!(selected_session(&rig).as_deref(), Some(NEWER_SESSION));
    assert_eq!(presentation_of(&rig, Id::Transcript), transcript);
    // The wheel moves the selection over the list.
    let list = rig.app.routing().area(Id::SessionList).unwrap();
    rig.app.update(Msg::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        modifiers: KeyModifiers::NONE,
        column: list.x + 6,
        row: list.y + 3,
    }));
    assert_eq!(selected_session(&rig).as_deref(), Some(BRIO_SESSION));
}

#[test]
fn a_click_during_the_resume_confirmation_does_nothing() {
    let mut rig = rig();
    rig.harness.set_resume(Some("claude --resume {id}"));
    rig.app.set_dir_exists(|_| true);
    rig.app
        .update(Msg::Selector(SelectorMsg::Focus(Pane::Sessions)));
    rig.key(Key::Char('R'), KeyModifiers::SHIFT);
    rig.settle();
    assert!(rig.app.resume_prompt_open());
    assert_eq!(rig.focus(), Some(Id::Prompt));
    let before = (
        rig.app.selected_session().cloned(),
        rig.app.selected_project().cloned(),
        rig.app.selected_harness(),
    );
    click_row(&mut rig, Id::SessionList, 1);
    click_row(&mut rig, Id::ProjectList, 1);
    let tabs = rig.app.routing().area(Id::HarnessTabs).unwrap();
    rig.app.update(press(tabs.x + 2, tabs.y));
    let list = rig.app.routing().area(Id::SessionList).unwrap();
    rig.app.update(Msg::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        modifiers: KeyModifiers::NONE,
        column: list.x + 6,
        row: list.y + 3,
    }));
    assert_eq!(
        (
            rig.app.selected_session().cloned(),
            rig.app.selected_project().cloned(),
            rig.app.selected_harness(),
        ),
        before
    );
    assert!(rig.app.resume_prompt_open());
    assert_eq!(rig.focus(), Some(Id::Prompt));
    assert!(rig.app.take_queued().is_empty());
}
