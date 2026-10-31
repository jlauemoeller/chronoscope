//! The mouse and the overlays (task 035): a click outside the selector over
//! the browser, the help overlay or the theme selector closes it as `Esc`
//! would, the components under an overlay get no mouse events, and the
//! overlays take the wheel and clicks inside them.

use std::cell::Cell;
use std::rc::Rc;

use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};
use tuirealm::state::{State, StateValue};

use super::theme_selector::mounted_themes;
use super::*;
use crate::app::layout::Screen;
use crate::components::HelpContext;
use crate::components::transcript::Presentation;
use crate::components::{FOLLOWING_ATTR, PRESENTATION_ATTR};

const PRESS: MouseEventKind = MouseEventKind::Down(MouseButton::Left);
const WHEEL_DOWN: MouseEventKind = MouseEventKind::ScrollDown;
const WHEEL_UP: MouseEventKind = MouseEventKind::ScrollUp;

fn at(kind: MouseEventKind, column: u16, row: u16) -> Msg {
    Msg::Mouse(MouseEvent {
        kind,
        modifiers: KeyModifiers::NONE,
        column,
        row,
    })
}

/// A mouse event at (`column`, `row`) of the component `id`.
fn on(rig: &mut Rig, id: Id, kind: MouseEventKind, column: u16, row: u16) {
    let area = rig.app.routing().area(id).unwrap();
    rig.app.update(at(kind, area.x + column, area.y + row));
}

/// A press in the top-left corner of the screen: outside every overlay.
fn press_outside(rig: &mut Rig) {
    rig.app.update(at(PRESS, 0, 0));
}

/// A press at the bottom-right corner of the screen.
fn press_far_corner(rig: &mut Rig) {
    let (width, height) = rig.app.size();
    rig.app.update(at(PRESS, width - 1, height - 1));
}

fn wheel_outside(rig: &mut Rig, kind: MouseEventKind) {
    rig.app.update(at(kind, 0, 0));
}

/// The screen position of a `▶ Thinking:` summary row of the transcript at
/// the left edge of the screen (column 3), which a centred overlay does not
/// cover. Call it before the overlay opens: the overlay hides rows.
fn summary_point(rig: &mut Rig, width: u16, height: u16) -> (u16, u16) {
    let area = rig.app.routing().area(Id::Transcript).unwrap();
    let screen = screen_text(rig, width, height);
    let row = screen
        .lines()
        .enumerate()
        .skip(usize::from(area.y))
        .take(usize::from(area.height))
        .find(|(_, row)| row.contains("▶ Thinking:"))
        .map(|(row, _)| u16::try_from(row).unwrap())
        .expect("a folded thinking section is on screen");
    (area.x + 3, row)
}

/// Asserts that `point` is outside the overlay `id`, and on the transcript.
fn assert_outside(rig: &Rig, id: Id, point: (u16, u16)) {
    let overlay = rig.app.routing().area(id).unwrap();
    assert!(
        !overlay.contains(point.into()),
        "{point:?} is in {overlay:?}"
    );
    let transcript = rig.app.routing().area(Id::Transcript).unwrap();
    assert!(
        transcript.contains(point.into()),
        "{point:?} is not on the transcript"
    );
}

fn click_clock(rig: &mut Rig) -> Rc<Cell<Duration>> {
    let start = Instant::now();
    let elapsed = Rc::new(Cell::new(Duration::ZERO));
    let shared = Rc::clone(&elapsed);
    rig.app.set_click_clock(move || start + shared.get());
    elapsed
}

fn presentation(rig: &Rig) -> Presentation {
    let value = rig
        .app
        .application()
        .query(&Id::Transcript, PRESENTATION_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

fn following(rig: &Rig) -> bool {
    rig.app
        .application()
        .query(&Id::Transcript, FOLLOWING_ATTR)
        .unwrap()
        .unwrap()
        .into_attr()
        == AttrValue::Flag(true)
}

fn mounted(rig: &Rig, id: Id) -> bool {
    rig.app.application().mounted(&id)
}

fn help_offset(rig: &Rig) -> usize {
    match rig.app.application().state(&Id::HelpOverlay).unwrap() {
        State::Single(StateValue::Usize(offset)) => offset,
        other => panic!("{other:?}"),
    }
}

fn cursor(rig: &Rig) -> Option<String> {
    match rig.app.application().state(&Id::ThemeSelector).unwrap() {
        State::Single(StateValue::String(id)) => Some(id),
        State::None => None,
        other => panic!("{other:?}"),
    }
}

/// A press on theme row `line` of the open theme selector.
fn click_theme(rig: &mut Rig, line: u16) {
    on(rig, Id::ThemeSelector, PRESS, 8, 2 + line);
}

// -- the selector over the browser

/// A 200x50 rig with the session open and the selector over it: the overlay
/// is 160 columns wide, so the transcript shows in the 20 columns either side.
/// Returns the screen position of a summary row at the left edge.
fn selector_over_wide_browser() -> (Rig, (u16, u16)) {
    let mut rig = rig_with(theme("default"), 200, 50);
    rig.open_loaded();
    let point = summary_point(&mut rig, 200, 50);
    rig.ctrl_o();
    rig.settle();
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    assert_eq!(rig.focus(), Some(Id::SessionList));
    assert_outside(&rig, Id::SelectorFrame, point);
    (rig, point)
}

#[test]
fn a_click_outside_the_selector_over_the_browser_closes_it_and_focuses_the_transcript() {
    let (mut rig, point) = selector_over_wide_browser();
    let transcript = presentation(&rig);
    rig.app.update(at(PRESS, point.0, point.1));
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert!(!mounted(&rig, Id::SelectorFrame));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(
        presentation(&rig),
        transcript,
        "the click was on a summary row but toggled nothing and selected nothing"
    );
    assert!(rig.app.take_queued().is_empty());
}

#[test]
fn a_click_above_the_selector_over_the_browser_closes_it_too() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.settle();
    let header = rig.app.routing().area(Id::BrowserHeader).unwrap();
    rig.app.update(at(PRESS, header.x + 3, header.y));
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn a_click_outside_the_selector_during_its_search_closes_it() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.settle();
    rig.press(Key::Char('/'));
    rig.settle();
    assert_eq!(rig.focus(), Some(Id::Prompt));
    rig.app.update(at(PRESS, 0, 0));
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert!(!mounted(&rig, Id::Prompt), "the search ended with it");
    assert!(!mounted(&rig, Id::SelectorFrame));
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn the_resume_confirmation_is_modal_to_a_click_outside_the_selector() {
    let mut rig = rig();
    rig.harness.set_resume(Some("claude --resume {id}"));
    rig.app.set_dir_exists(|_| true);
    rig.open_loaded();
    rig.ctrl_o();
    rig.settle();
    rig.key(Key::Char('R'), KeyModifiers::SHIFT);
    rig.settle();
    assert!(rig.app.resume_prompt_open());
    press_outside(&mut rig);
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    assert!(rig.app.resume_prompt_open());
    assert_eq!(rig.focus(), Some(Id::Prompt));
}

#[test]
fn a_click_inside_the_selector_acts_on_it_and_does_not_close_it() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.settle();
    let transcript = presentation(&rig);
    // On the frame between the panes, on a session row, and on the tabs.
    let projects = rig.app.routing().area(Id::ProjectList).unwrap();
    rig.app.update(at(PRESS, projects.right(), projects.y + 2));
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    on(&mut rig, Id::SessionList, PRESS, 6, 2);
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    assert_eq!(
        rig.app.selected_session().map(|s| s.as_str().to_owned()),
        Some(NEWER_SESSION.to_owned())
    );
    assert_eq!(presentation(&rig), transcript);
}

#[test]
fn the_wheel_outside_the_selector_over_the_browser_is_ignored() {
    let (mut rig, point) = selector_over_wide_browser();
    let transcript = presentation(&rig);
    let session = rig.app.selected_session().cloned();
    for kind in [WHEEL_DOWN, WHEEL_UP, WHEEL_DOWN] {
        rig.app.update(at(kind, point.0, point.1));
    }
    assert_eq!(rig.app.screen(), Screen::BrowserSelector, "still open");
    assert_eq!(rig.focus(), Some(Id::SessionList));
    assert_eq!(
        presentation(&rig),
        transcript,
        "the transcript did not scroll"
    );
    assert_eq!(rig.app.selected_session().cloned(), session);
}

#[test]
fn the_transcript_under_the_selector_gets_no_wheel_or_click() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.settle();
    let before = presentation(&rig);
    // Over the overlay's frame, where the transcript is drawn under it.
    let frame = rig.app.routing().area(Id::SelectorFrame).unwrap();
    let projects = rig.app.routing().area(Id::ProjectList).unwrap();
    for kind in [PRESS, WHEEL_DOWN] {
        rig.app.update(at(kind, frame.x + 1, frame.y + 6));
        rig.app.update(at(kind, projects.right(), projects.y + 3));
    }
    assert_eq!(presentation(&rig), before);
}

#[test]
fn at_startup_a_click_outside_the_selector_does_nothing() {
    for outside in [press_outside, press_far_corner] {
        let mut rig = rig();
        assert_eq!(rig.app.screen(), Screen::Selector);
        let focus = rig.focus();
        let project = rig.app.selected_project().cloned();
        outside(&mut rig);
        assert_eq!(rig.app.screen(), Screen::Selector);
        assert!(mounted(&rig, Id::SelectorFrame));
        assert!(!rig.app.should_quit(), "closing would quit");
        assert_eq!(rig.focus(), focus);
        assert_eq!(rig.app.selected_project().cloned(), project);
        assert!(rig.app.take_queued().is_empty());
    }
}

// -- the help overlay

#[test]
fn a_click_outside_the_help_overlay_closes_it_and_returns_the_focus() {
    let mut rig = rig();
    rig.open_loaded();
    let point = summary_point(&mut rig, 120, 40);
    let transcript = presentation(&rig);
    rig.app.update(Msg::OpenHelp(HelpContext::Browser));
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));
    assert_outside(&rig, Id::HelpOverlay, point);
    rig.app.update(at(PRESS, point.0, point.1));
    assert!(!mounted(&rig, Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert_eq!(
        presentation(&rig),
        transcript,
        "the press on a summary row did not reach the transcript"
    );
}

#[test]
fn a_click_outside_the_help_overlay_over_the_startup_selector_closes_only_the_overlay() {
    let mut rig = rig();
    rig.app.update(Msg::OpenHelp(HelpContext::Selector));
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));
    let project = rig.app.selected_project().cloned();
    // Over a project row of the selector under it.
    on(&mut rig, Id::ProjectList, PRESS, 6, 3);
    assert!(!mounted(&rig, Id::HelpOverlay));
    assert!(mounted(&rig, Id::SelectorFrame));
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    assert_eq!(rig.app.selected_project().cloned(), project);
    assert!(!rig.app.should_quit());
}

#[test]
fn a_click_inside_the_help_overlay_does_nothing() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.update(Msg::OpenHelp(HelpContext::Browser));
    let area = rig.app.routing().area(Id::HelpOverlay).unwrap();
    for (dx, dy) in [(0, 0), (3, 3), (area.width - 1, area.height - 1)] {
        rig.app.update(at(PRESS, area.x + dx, area.y + dy));
    }
    assert!(mounted(&rig, Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));
    assert_eq!(help_offset(&rig), 0);
}

#[test]
fn the_wheel_scrolls_the_help_overlay_and_outside_it_is_ignored() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_loaded();
    let transcript = presentation(&rig);
    rig.app.update(Msg::OpenHelp(HelpContext::Browser));
    assert_eq!(help_offset(&rig), 0);
    on(&mut rig, Id::HelpOverlay, WHEEL_DOWN, 5, 5);
    assert_eq!(help_offset(&rig), 3);
    on(&mut rig, Id::HelpOverlay, WHEEL_DOWN, 5, 5);
    assert_eq!(help_offset(&rig), 6);
    on(&mut rig, Id::HelpOverlay, WHEEL_UP, 5, 5);
    assert_eq!(help_offset(&rig), 3);
    assert_eq!(presentation(&rig), transcript);
}

#[test]
fn the_wheel_outside_an_overlay_over_the_browser_is_ignored() {
    // On the transcript, beside the overlay, and while it is following.
    for open in [Msg::OpenHelp(HelpContext::Browser), Msg::OpenThemeSelector] {
        let mut rig = rig();
        rig.open_loaded();
        let point = summary_point(&mut rig, 120, 40);
        let overlay = if open == Msg::OpenThemeSelector {
            Id::ThemeSelector
        } else {
            Id::HelpOverlay
        };
        rig.app.update(open);
        assert_outside(&rig, overlay, point);
        let transcript = presentation(&rig);
        for kind in [WHEEL_DOWN, WHEEL_UP, WHEEL_DOWN] {
            rig.app.update(at(kind, point.0, point.1));
        }
        assert!(mounted(&rig, overlay), "{overlay:?} stays open");
        assert_eq!(presentation(&rig), transcript, "{overlay:?}");
        assert_eq!(rig.focus(), Some(overlay));
        if overlay == Id::HelpOverlay {
            assert_eq!(help_offset(&rig), 0);
        } else {
            assert_eq!(cursor(&rig).as_deref(), Some("default"));
            assert_eq!(rig.app.theme_id(), "default");
        }
    }
}

#[test]
fn the_wheel_outside_an_overlay_does_not_stop_following() {
    let mut rig = rig();
    rig.open_loaded();
    for _ in 0..40 {
        on(&mut rig, Id::Transcript, WHEEL_DOWN, 10, 5);
    }
    assert!(following(&rig));
    let point = {
        let area = rig.app.routing().area(Id::Transcript).unwrap();
        (area.x + 3, area.y + 4)
    };
    rig.app.update(Msg::OpenHelp(HelpContext::Browser));
    assert_outside(&rig, Id::HelpOverlay, point);
    rig.app.update(at(WHEEL_UP, point.0, point.1));
    assert!(following(&rig));
}

// -- the theme selector

#[test]
fn a_click_outside_the_theme_selector_cancels_and_restores_the_theme() {
    let mut rig = rig();
    rig.open_loaded();
    let original = rig.app.theme().clone();
    let point = summary_point(&mut rig, 120, 40);
    let transcript = presentation(&rig);
    rig.open_themes();
    rig.move_to(Key::Down, "terminal");
    assert_ne!(rig.app.theme(), &original, "previewed");
    assert_outside(&rig, Id::ThemeSelector, point);
    rig.app.update(at(PRESS, point.0, point.1));
    assert!(!mounted(&rig, Id::ThemeSelector));
    assert_eq!(rig.app.theme_id(), "default");
    assert_eq!(rig.app.theme(), &original);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // Every component is back on the original theme.
    for (id, theme) in mounted_themes(&rig.app) {
        assert_eq!(theme, original, "{id:?}");
    }
    assert_eq!(rig.app.config().theme, "default", "nothing was kept");
    assert_eq!(presentation(&rig), transcript, "the press stopped there");
}

#[test]
fn a_click_outside_the_theme_selector_at_startup_returns_to_the_pane() {
    let mut rig = rig();
    rig.open_themes();
    press_far_corner(&mut rig);
    assert!(!mounted(&rig, Id::ThemeSelector));
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    assert!(!rig.app.should_quit());
}

#[test]
fn a_click_on_a_theme_previews_it_and_leaves_the_selector_open() {
    let mut rig = rig();
    rig.open_themes();
    let original = rig.app.theme().clone();
    click_theme(&mut rig, 3);
    assert_eq!(rig.app.theme_id(), "gruvbox");
    assert_ne!(rig.app.theme(), &original);
    assert!(mounted(&rig, Id::ThemeSelector));
    assert_eq!(cursor(&rig).as_deref(), Some("gruvbox"));
    assert_eq!(rig.focus(), Some(Id::ThemeSelector));
    click_theme(&mut rig, 5);
    assert_eq!(rig.app.theme_id(), "tokyo-night");
    assert_eq!(cursor(&rig).as_deref(), Some("tokyo-night"));
    // The click did not keep the theme.
    assert_eq!(rig.app.config().theme, "default");
    // Esc still gives the first one back.
    rig.close_themes();
    assert_eq!(rig.app.theme_id(), "default");
    assert_eq!(rig.app.theme(), &original);
}

#[test]
fn a_double_click_on_a_theme_keeps_it_like_enter() {
    let mut rig = rig();
    rig.open_themes();
    let time = click_clock(&mut rig);
    click_theme(&mut rig, 3);
    assert!(mounted(&rig, Id::ThemeSelector));
    time.set(Duration::from_millis(300));
    click_theme(&mut rig, 3);
    assert!(!mounted(&rig, Id::ThemeSelector), "selected");
    assert_eq!(rig.app.theme_id(), "gruvbox");
    assert_eq!(rig.app.config().theme, "gruvbox", "kept for the config");
    assert_eq!(rig.focus(), Some(Id::ProjectList));
}

#[test]
fn the_theme_double_click_is_400_ms_inclusive() {
    for (ms, selects) in [(400, true), (401, false), (500, false)] {
        let mut rig = rig();
        rig.open_themes();
        let time = click_clock(&mut rig);
        click_theme(&mut rig, 2);
        time.set(Duration::from_millis(ms));
        click_theme(&mut rig, 2);
        assert_eq!(!mounted(&rig, Id::ThemeSelector), selects, "{ms} ms");
        assert_eq!(rig.app.theme_id(), "catppuccin");
    }
}

#[test]
fn two_clicks_on_different_themes_are_not_a_double_click() {
    let mut rig = rig();
    rig.open_themes();
    let time = click_clock(&mut rig);
    click_theme(&mut rig, 2);
    time.set(Duration::from_millis(100));
    click_theme(&mut rig, 3);
    assert!(mounted(&rig, Id::ThemeSelector));
    assert_eq!(rig.app.theme_id(), "gruvbox");
    // And a third press on that one, soon after, is the double-click.
    time.set(Duration::from_millis(200));
    click_theme(&mut rig, 3);
    assert!(!mounted(&rig, Id::ThemeSelector));
}

#[test]
fn a_click_outside_a_theme_row_inside_the_selector_does_nothing() {
    let mut rig = rig();
    rig.open_themes();
    let area = rig.app.routing().area(Id::ThemeSelector).unwrap();
    let cursor_before = cursor(&rig);
    for (dx, dy) in [
        (8, 0),
        (8, 1),
        (0, 3),
        (area.width - 1, 3),
        (8, area.height - 1),
        (8, area.height - 2),
    ] {
        rig.app.update(at(PRESS, area.x + dx, area.y + dy));
    }
    assert!(mounted(&rig, Id::ThemeSelector));
    assert_eq!(cursor(&rig), cursor_before);
    assert_eq!(rig.app.theme_id(), "default");
}

#[test]
fn the_wheel_moves_the_theme_cursor_and_previews_the_theme() {
    let mut rig = rig();
    rig.open_themes();
    on(&mut rig, Id::ThemeSelector, WHEEL_DOWN, 8, 4);
    assert_eq!(cursor(&rig).as_deref(), Some("terminal"));
    assert_eq!(rig.app.theme_id(), "terminal");
    on(&mut rig, Id::ThemeSelector, WHEEL_DOWN, 8, 4);
    assert_eq!(rig.app.theme_id(), "catppuccin");
    on(&mut rig, Id::ThemeSelector, WHEEL_UP, 8, 4);
    assert_eq!(rig.app.theme_id(), "terminal");
    // Outside the overlay the wheel is ignored.
    wheel_outside(&mut rig, WHEEL_DOWN);
    assert_eq!(rig.app.theme_id(), "terminal");
    assert!(mounted(&rig, Id::ThemeSelector));
}

#[test]
fn nothing_under_the_theme_selector_gets_a_click_that_lands_inside_it() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_themes();
    let transcript = presentation(&rig);
    click_theme(&mut rig, 1);
    assert_eq!(presentation(&rig), transcript);
    assert_eq!(rig.focus(), Some(Id::ThemeSelector));
}

// -- mouse = false

#[test]
fn with_mouse_off_the_overlays_take_no_mouse() {
    let mut init = init(&[CLAUDE], theme("default"));
    init.config.mouse = false;
    let mut rig = build_rig_from(init, 120, 40, false);
    rig.open_loaded();
    // Help: neither the wheel nor an outside click.
    rig.app.update(Msg::OpenHelp(HelpContext::Browser));
    on(&mut rig, Id::HelpOverlay, WHEEL_DOWN, 5, 5);
    assert_eq!(help_offset(&rig), 0);
    press_outside(&mut rig);
    assert!(mounted(&rig, Id::HelpOverlay));
    rig.app.update(Msg::CloseHelp);
    // Themes: no click, wheel or outside click.
    rig.open_themes();
    click_theme(&mut rig, 3);
    on(&mut rig, Id::ThemeSelector, WHEEL_DOWN, 8, 4);
    assert_eq!(rig.app.theme_id(), "default");
    press_outside(&mut rig);
    assert!(mounted(&rig, Id::ThemeSelector));
    rig.close_themes();
    // The selector over the browser.
    rig.ctrl_o();
    rig.settle();
    press_outside(&mut rig);
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    assert!(rig.app.take_queued().is_empty());
}
