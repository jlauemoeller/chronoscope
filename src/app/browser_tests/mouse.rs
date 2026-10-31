//! The mouse with a session open: the selector over the browser takes it, the
//! browser under it does not (the transcript's mouse rules come later), and a
//! double-click in the selector opens the session through the real load.

use std::cell::Cell;
use std::rc::Rc;

use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};

use super::*;
use crate::app::layout::Screen;
use crate::app::{Pane, SelectorMsg};

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

#[test]
fn the_browser_alone_takes_no_mouse_yet() {
    let mut rig = rig();
    rig.open_loaded();
    let transcript = rig.app.routing().area(Id::Transcript).unwrap();
    let before = rig.app.screen();
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::ScrollDown,
    ] {
        rig.app.update(Msg::Mouse(MouseEvent {
            kind,
            modifiers: KeyModifiers::NONE,
            column: transcript.x + 3,
            row: transcript.y + 3,
        }));
    }
    assert_eq!(rig.app.screen(), before);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(rig.app.take_queued().is_empty());
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
    // The header is above the overlay, and the transcript under it: no
    // target.
    let header = rig.app.routing().area(Id::BrowserHeader).unwrap();
    rig.app.update(press(header.x + 2, header.y));
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    assert_eq!(selected_session(&rig).as_deref(), Some(NEWER_SESSION));
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
