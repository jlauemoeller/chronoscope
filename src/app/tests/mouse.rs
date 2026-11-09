//! Tests of the mouse routing and of what the selector does with the mouse
//! (no terminal): hit-testing over the layout, the routing areas, the wheel,
//! clicks, double-clicks and the tabs.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::listener::{EventListenerCfg, Poll, PortResult};

use super::search::searching;
use super::selector::{
    CLAUDE, OPENCODE, cwd, empty_app_in, init, loaded, project_rows, send, session_rows, tabs,
    theme, two_harnesses,
};
use crate::app::fixtures::{self, fixture_snapshot};
use crate::app::layout::{LayoutInput, Screen, layout};
use crate::app::mouse::{DOUBLE_CLICK, hit_test};
use crate::app::{App, Id, Msg, Pane, UserEvent};
use crate::components::HelpContext;
use crate::index::SessionMap;
use crate::model::SessionId;
use crate::test_support::{buffer_to_text, render};

// -- helpers

const PRESS: MouseEventKind = MouseEventKind::Down(MouseButton::Left);

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        modifiers: KeyModifiers::NONE,
        column,
        row,
    }
}

/// The screen position of (`column`, `row`) of the component `id`.
fn pos(app: &App, id: Id, column: u16, row: u16) -> (u16, u16) {
    let area = app.routing().area(id).expect("the component is routed");
    (area.x + column, area.y + row)
}

/// A left press at (`column`, `row`) of `id`.
fn click(app: &mut App, id: Id, column: u16, row: u16) {
    let (column, row) = pos(app, id, column, row);
    app.update(Msg::Mouse(mouse(PRESS, column, row)));
}

/// A press on the list row `line` (0 is the first row drawn) of a pane.
fn click_row(app: &mut App, id: Id, line: u16) {
    click(app, id, 6, 2 + line);
}

fn wheel(app: &mut App, id: Id, down: bool) {
    let kind = if down {
        MouseEventKind::ScrollDown
    } else {
        MouseEventKind::ScrollUp
    };
    let (column, row) = pos(app, id, 6, 3);
    app.update(Msg::Mouse(mouse(kind, column, row)));
}

/// Gives the app a click clock that shows the returned time.
fn click_clock(app: &mut App) -> Rc<Cell<Duration>> {
    let start = Instant::now();
    let elapsed = Rc::new(Cell::new(Duration::ZERO));
    let shared = Rc::clone(&elapsed);
    app.set_click_clock(move || start + shared.get());
    elapsed
}

fn focus_of(app: &App) -> Option<Id> {
    app.application().focus().copied()
}

fn project(app: &App) -> String {
    app.selected_project().unwrap().as_str().to_owned()
}

fn session(app: &App) -> Option<&str> {
    app.selected_session().map(SessionId::as_str)
}

/// The Sessions pane of `app` as drawn at its size.
fn drawn(app: &mut App, width: u16, height: u16) -> String {
    buffer_to_text(&render(width, height, |frame| app.view(frame)))
}

fn at_size(width: u16, height: u16) -> crate::app::layout::ScreenLayout {
    layout(&LayoutInput::default(), Rect::new(0, 0, width, height))
}

fn hit(layout: &crate::app::layout::ScreenLayout, column: u16, row: u16) -> Option<Id> {
    hit_test(layout, column, row).map(|(id, _)| id)
}

// -- hit-testing

#[test]
fn hit_testing_maps_points_to_components_at_80x24() {
    let l = at_size(80, 24);
    // The overlay is (0, 1, 80, 22); Projects is x 4..28, the gap is column
    // 28, Sessions is x 29..76, and the panes are rows 6..14.
    assert_eq!(hit(&l, 4, 6), Some(Id::ProjectList));
    assert_eq!(hit(&l, 27, 13), Some(Id::ProjectList));
    assert_eq!(hit(&l, 28, 8), Some(Id::SelectorFrame), "the gap");
    assert_eq!(hit(&l, 29, 6), Some(Id::SessionList));
    assert_eq!(hit(&l, 75, 13), Some(Id::SessionList));
    assert_eq!(hit(&l, 76, 8), Some(Id::SelectorFrame), "the margin");
    assert_eq!(hit(&l, 5, 3), Some(Id::HarnessTabs));
    assert_eq!(hit(&l, 5, 4), Some(Id::HarnessTabs), "the underline row");
    assert_eq!(hit(&l, 10, 15), Some(Id::WorkingDir));
    assert_eq!(hit(&l, 10, 14), Some(Id::WorkingDir));
    assert_eq!(hit(&l, 10, 5), Some(Id::SelectorFrame), "above the panes");
    assert_eq!(hit(&l, 10, 19), Some(Id::SelectorFrame), "under the box");
    assert_eq!(hit(&l, 10, 20), Some(Id::HelpLine));
    assert_eq!(hit(&l, 0, 0), None, "above the overlay");
    assert_eq!(hit(&l, 0, 23), None, "under the overlay");
    assert_eq!(hit(&l, 80, 8), None, "off the screen");
}

#[test]
fn hit_testing_maps_points_to_components_at_200x50() {
    let l = at_size(200, 50);
    // The overlay is centred: (20, 1, 160, 48). Projects is x 24..60 (at
    // most 36 columns), the gap is column 60, Sessions starts at column 61;
    // the panes are rows 6..40.
    assert_eq!(hit(&l, 24, 6), Some(Id::ProjectList));
    assert_eq!(hit(&l, 59, 39), Some(Id::ProjectList));
    assert_eq!(hit(&l, 60, 20), Some(Id::SelectorFrame), "the gap");
    assert_eq!(hit(&l, 61, 20), Some(Id::SessionList));
    assert_eq!(hit(&l, 175, 28), Some(Id::SessionList), "last Sessions row");
    // The panels: Session Time rows 29..33, Session Tokens rows 33..40.
    assert_eq!(hit(&l, 61, 29), Some(Id::SessionTime));
    assert_eq!(hit(&l, 175, 32), Some(Id::SessionTime));
    assert_eq!(hit(&l, 61, 33), Some(Id::SessionTokens));
    assert_eq!(hit(&l, 175, 39), Some(Id::SessionTokens));
    assert_eq!(
        hit(&l, 60, 35),
        Some(Id::SelectorFrame),
        "the gap beside them"
    );
    assert_eq!(
        hit(&l, 176, 35),
        Some(Id::SelectorFrame),
        "the margin beside them"
    );
    assert_eq!(hit(&l, 25, 3), Some(Id::HarnessTabs));
    assert_eq!(hit(&l, 30, 40), Some(Id::WorkingDir));
    assert_eq!(hit(&l, 30, 46), Some(Id::HelpLine));
    assert_eq!(hit(&l, 20, 10), Some(Id::SelectorFrame), "the frame");
    assert_eq!(hit(&l, 10, 10), None, "left of the centred overlay");
    assert_eq!(hit(&l, 190, 10), None, "right of the centred overlay");
}

fn browser_at(width: u16, height: u16, session_empty: bool) -> crate::app::layout::ScreenLayout {
    let input = LayoutInput {
        screen: crate::app::layout::Screen::Browser,
        session_empty,
        ..LayoutInput::default()
    };
    layout(&input, Rect::new(0, 0, width, height))
}

#[test]
fn hit_testing_finds_the_sidebar_in_the_pane_nothing_in_the_gap_and_the_transcript_beyond() {
    let l = browser_at(120, 40, false);
    // The sidebar is columns 0..30 of rows 5..38, the gap is column 30 and
    // the transcript is columns 31..120.
    assert_eq!(hit(&l, 0, 5), Some(Id::TurnSidebar));
    assert_eq!(hit(&l, 15, 20), Some(Id::TurnSidebar));
    assert_eq!(hit(&l, 29, 37), Some(Id::TurnSidebar));
    assert_eq!(hit(&l, 30, 5), None, "the gap");
    assert_eq!(hit(&l, 30, 20), None, "the gap");
    assert_eq!(hit(&l, 30, 37), None, "the gap");
    assert_eq!(hit(&l, 31, 5), Some(Id::Transcript));
    assert_eq!(hit(&l, 119, 37), Some(Id::Transcript));
    assert_eq!(hit(&l, 15, 38), Some(Id::BrowserHelpLine));
    assert_eq!(hit(&l, 15, 4), Some(Id::BrowserHeader));
    assert_eq!(hit(&l, 120, 20), None, "off the screen");
}

#[test]
fn hit_testing_at_89_and_90_columns_has_the_sidebar_only_from_90() {
    assert_eq!(
        hit(&browser_at(89, 24, false), 10, 10),
        Some(Id::Transcript)
    );
    assert_eq!(
        hit(&browser_at(89, 24, false), 30, 10),
        Some(Id::Transcript)
    );
    let l = browser_at(90, 24, false);
    assert_eq!(hit(&l, 10, 10), Some(Id::TurnSidebar));
    assert_eq!(hit(&l, 30, 10), None);
    assert_eq!(hit(&l, 31, 10), Some(Id::Transcript));
    assert_eq!(hit(&l, 89, 10), Some(Id::Transcript));
}

#[test]
fn an_empty_session_has_nothing_at_the_sidebar_and_the_message_everywhere() {
    let l = browser_at(120, 40, true);
    assert_eq!(hit(&l, 10, 20), Some(Id::EmptySession));
    assert_eq!(hit(&l, 30, 20), Some(Id::EmptySession));
}

#[test]
fn a_press_on_the_base_selector_screen_never_finds_its_background() {
    // `SelectorFrame` is an overlay, so what is drawn before it is never a
    // candidate here; `a_component_that_only_paints_is_never_found` in
    // `app/mouse.rs` checks the exclusion itself, with no overlay.
    let selector = [
        Id::SelectorFrame,
        Id::HarnessTabs,
        Id::ProjectList,
        Id::SessionList,
        Id::SessionTime,
        Id::SessionTokens,
        Id::WorkingDir,
        Id::HelpLine,
    ];
    for (width, height) in [(60, 20), (80, 24), (120, 40), (200, 50)] {
        let l = at_size(width, height);
        assert_eq!(
            l.area(Id::ScreenBackground),
            Some(Rect::new(0, 0, width, height))
        );
        for row in 0..height {
            for column in 0..width {
                if let Some(id) = hit(&l, column, row) {
                    assert!(selector.contains(&id), "{id:?} at ({column}, {row})");
                }
            }
        }
        assert_eq!(hit(&l, 0, 0), None, "{width}x{height}: outside the overlay");
    }
}

#[test]
fn the_gap_between_the_panes_is_the_frame_at_every_size() {
    for (width, height) in [(60, 20), (80, 24), (120, 40), (200, 50), (201, 24)] {
        let l = at_size(width, height);
        let projects = l.area(Id::ProjectList).unwrap();
        let sessions = l.area(Id::SessionList).unwrap();
        let gap = projects.right();
        assert_eq!(sessions.x, gap + 1, "{width}x{height}");
        assert_eq!(hit(&l, gap, projects.y + 2), Some(Id::SelectorFrame));
        assert_eq!(hit(&l, gap - 1, projects.y + 2), Some(Id::ProjectList));
        assert_eq!(hit(&l, gap + 1, projects.y + 2), Some(Id::SessionList));
    }
}

#[test]
fn the_topmost_component_wins() {
    // The toast is drawn over the selector, in its bottom-right corner.
    let input = LayoutInput {
        toast_text_width: Some(10),
        ..LayoutInput::default()
    };
    let l = layout(&input, Rect::new(0, 0, 80, 24));
    let toast = l.area(Id::Toast).unwrap();
    assert_eq!(hit(&l, toast.x + 1, toast.y), Some(Id::Toast));
    assert_eq!(hit(&l, toast.x - 1, toast.y), Some(Id::HelpLine));
}

#[test]
fn nothing_under_an_overlay_is_a_target() {
    let area = Rect::new(0, 0, 80, 24);
    let help = LayoutInput {
        help: Some(ratatui::layout::Size::new(40, 10)),
        ..LayoutInput::default()
    };
    let l = layout(&help, area);
    let overlay = l.area(Id::HelpOverlay).unwrap();
    assert_eq!(hit(&l, overlay.x + 1, overlay.y + 1), Some(Id::HelpOverlay));
    // Where the panes are, but the overlay is not.
    assert_eq!(hit(&l, 5, 7), None);
    assert_eq!(hit(&l, 70, 10), None);
    let details = LayoutInput {
        details: Some(ratatui::layout::Size::new(40, 10)),
        ..LayoutInput::default()
    };
    let l = layout(&details, area);
    let overlay = l.area(Id::DetailsOverlay).unwrap();
    assert_eq!(
        hit(&l, overlay.x + 1, overlay.y + 1),
        Some(Id::DetailsOverlay)
    );
    assert_eq!(hit(&l, 5, 7), None);
    assert_eq!(hit(&l, 70, 10), None);
    let themes = LayoutInput {
        theme_selector: Some(ratatui::layout::Size::new(40, 10)),
        ..LayoutInput::default()
    };
    let l = layout(&themes, area);
    assert_eq!(hit(&l, 5, 7), None);
    let theme = l.area(Id::ThemeSelector).unwrap();
    assert_eq!(hit(&l, theme.x, theme.y), Some(Id::ThemeSelector));
}

#[test]
fn the_browser_under_the_selector_is_not_a_target() {
    let input = LayoutInput {
        screen: Screen::BrowserSelector,
        ..LayoutInput::default()
    };
    let l = layout(&input, Rect::new(0, 0, 80, 24));
    // The header's first row is above the overlay.
    assert!(l.area(Id::BrowserHeader).is_some());
    assert_eq!(hit(&l, 10, 0), None);
    assert_eq!(hit(&l, 5, 3), Some(Id::HarnessTabs));
    let browser = LayoutInput {
        screen: Screen::Browser,
        ..LayoutInput::default()
    };
    let l = layout(&browser, Rect::new(0, 0, 80, 24));
    assert_eq!(hit(&l, 10, 0), Some(Id::BrowserHeader));
    assert_eq!(hit(&l, 10, 10), Some(Id::Transcript));
}

#[test]
fn a_terminal_that_is_too_small_routes_to_the_empty_state_only() {
    let l = at_size(59, 24);
    assert_eq!(hit(&l, 10, 10), Some(Id::EmptyState));
}

// -- routing areas

#[test]
fn the_routing_areas_follow_the_size() {
    let mut app = loaded();
    app.update(Msg::Resized(80, 24));
    assert_eq!(
        app.routing().area(Id::ProjectList),
        Some(Rect::new(4, 6, 24, 8))
    );
    assert_eq!(
        app.routing().area(Id::SessionList),
        Some(Rect::new(29, 6, 47, 8))
    );
    app.update(Msg::Resized(200, 50));
    assert_eq!(
        app.routing().area(Id::ProjectList),
        Some(Rect::new(24, 6, 36, 34))
    );
    assert_eq!(
        app.routing().area(Id::SessionList),
        Some(Rect::new(61, 6, 115, 23))
    );
    assert_eq!(
        app.routing().area(Id::SessionTime),
        Some(Rect::new(61, 29, 115, 4))
    );
    assert_eq!(
        app.routing().area(Id::SessionTokens),
        Some(Rect::new(61, 33, 115, 7))
    );
    app.update(Msg::Resized(40, 10));
    assert_eq!(app.routing().area(Id::ProjectList), None);
    // A click on the old position hits nothing now.
    let before = (project(&app), app.focused_pane());
    app.update(Msg::Mouse(mouse(PRESS, 6, 8)));
    assert_eq!((project(&app), app.focused_pane()), before);
}

#[test]
fn a_click_lands_where_the_new_size_puts_the_row() {
    let mut app = loaded();
    app.update(Msg::Resized(200, 50));
    // Row 1 (BRIO) of the Projects pane at 200x50.
    app.update(Msg::Mouse(mouse(PRESS, 30, 6 + 2 + 1)));
    assert!(project(&app).ends_with("/BRIO"));
}

#[test]
fn the_routing_areas_are_not_computed_by_view() {
    let mut app = loaded();
    let routing = app.routing().clone();
    // Drawing at another size changes nothing about the routing.
    drawn(&mut app, 80, 24);
    drawn(&mut app, 200, 50);
    assert_eq!(app.routing(), &routing);
    assert_eq!(app.size(), (120, 40));
}

#[test]
fn the_routing_areas_follow_the_screen_and_the_overlays() {
    let mut app = loaded();
    assert!(app.routing().area(Id::HelpOverlay).is_none());
    app.update(Msg::OpenHelp(HelpContext::Selector));
    assert!(app.routing().area(Id::HelpOverlay).is_some());
    app.update(Msg::CloseHelp);
    assert!(app.routing().area(Id::HelpOverlay).is_none());
    app.update(Msg::OpenThemeSelector);
    assert!(app.routing().area(Id::ThemeSelector).is_some());
    app.update(Msg::CancelTheme);
    assert!(app.routing().area(Id::ThemeSelector).is_none());
    // A toast is routed: it is on top.
    app.show_toast("Hello").unwrap();
    assert!(app.routing().area(Id::Toast).is_some());
}

// -- the wheel

#[test]
fn the_wheel_over_projects_changes_the_project_and_clears_the_session() {
    let mut app = loaded();
    app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
        Pane::Sessions,
    )));
    assert_eq!(session(&app), Some("s1"));
    wheel(&mut app, Id::ProjectList, true);
    assert!(project(&app).ends_with("/BRIO"));
    assert_eq!(session(&app), None);
    assert_eq!(project_rows(&app).selected, Some(1));
    assert_eq!(cwd(&app), "/home/jacob/Development/projects/BRIO");
    // The focus did not move.
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(focus_of(&app), Some(Id::SessionList));
    wheel(&mut app, Id::ProjectList, false);
    assert!(project(&app).ends_with("/Agentmon"));
    // No wrap around at the top.
    wheel(&mut app, Id::ProjectList, false);
    assert!(project(&app).ends_with("/Agentmon"));
}

#[test]
fn the_wheel_over_projects_works_with_projects_focused_too() {
    let mut app = loaded();
    wheel(&mut app, Id::ProjectList, true);
    wheel(&mut app, Id::ProjectList, true);
    assert!(project(&app).ends_with("/Calira"));
    assert_eq!(app.focused_pane(), Pane::Projects);
}

#[test]
fn the_wheel_over_sessions_moves_the_selection_when_there_is_one() {
    let mut app = loaded();
    app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
        Pane::Sessions,
    )));
    wheel(&mut app, Id::SessionList, true);
    assert_eq!(session(&app), Some("s2"));
    assert_eq!(session_rows(&app).selected, Some(1));
    wheel(&mut app, Id::SessionList, true);
    assert_eq!(session(&app), Some("s3"));
    wheel(&mut app, Id::SessionList, false);
    wheel(&mut app, Id::SessionList, false);
    assert_eq!(session(&app), Some("s1"));
    wheel(&mut app, Id::SessionList, false);
    assert_eq!(session(&app), Some("s1"), "no wrap around");
    assert_eq!(project_rows(&app).selected, Some(0));
}

#[test]
fn the_wheel_over_sessions_does_not_move_the_focus() {
    let mut app = loaded();
    app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
        Pane::Sessions,
    )));
    app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
        Pane::Projects,
    )));
    // Projects has the focus and a session is selected.
    assert_eq!(session(&app), Some("s1"));
    wheel(&mut app, Id::SessionList, true);
    assert_eq!(session(&app), Some("s2"));
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(focus_of(&app), Some(Id::ProjectList));
}

#[test]
fn the_wheel_over_sessions_only_scrolls_when_nothing_is_selected() {
    let mut app = loaded();
    // 60x20 shows one session row.
    app.update(Msg::Resized(60, 20));
    assert_eq!(session(&app), None);
    let first = "2026-10-30 11:40Z";
    let second = "2026-10-28 09:15Z";
    let last = "2026-06-12 08:35Z";
    assert!(drawn(&mut app, 60, 20).contains(first));
    app.take_redraw();
    wheel(&mut app, Id::SessionList, true);
    let text = drawn(&mut app, 60, 20);
    assert!(text.contains(second) && !text.contains(first), "{text}");
    assert_ne!(app.take_redraw(), crate::app::redraw::RedrawRequest::None);
    assert_eq!(session(&app), None);
    assert_eq!(session_rows(&app).selected, None);
    assert_eq!(app.focused_pane(), Pane::Projects);
    for _ in 0..10 {
        wheel(&mut app, Id::SessionList, true);
    }
    let text = drawn(&mut app, 60, 20);
    assert!(text.contains(last) && !text.contains(second), "{text}");
    wheel(&mut app, Id::SessionList, false);
    assert!(drawn(&mut app, 60, 20).contains("2026-09-22 14:12Z"));
    for _ in 0..10 {
        wheel(&mut app, Id::SessionList, false);
    }
    assert!(drawn(&mut app, 60, 20).contains(first));
    assert_eq!(session(&app), None);
}

#[test]
fn a_wheel_with_nothing_to_do_asks_for_no_redraw() {
    let mut app = loaded();
    app.take_redraw();
    // Nothing is selected and everything fits: nothing scrolls.
    wheel(&mut app, Id::SessionList, true);
    assert_eq!(app.take_redraw(), crate::app::redraw::RedrawRequest::None);
}

// -- clicks

#[test]
fn a_click_on_a_project_row_focuses_projects_and_selects_it() {
    let mut app = loaded();
    app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
        Pane::Sessions,
    )));
    assert_eq!(session(&app), Some("s1"));
    click_row(&mut app, Id::ProjectList, 2);
    assert!(project(&app).ends_with("/Calira"));
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(focus_of(&app), Some(Id::ProjectList));
    assert_eq!(session(&app), None, "another project clears the session");
    assert_eq!(project_rows(&app).selected, Some(2));
    assert_eq!(session_rows(&app).project, "Calira");
    assert_eq!(cwd(&app), "/home/jacob/Development/projects/Calira");
}

#[test]
fn a_click_on_the_selected_project_keeps_the_session_and_focuses_projects() {
    let mut app = loaded();
    app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
        Pane::Sessions,
    )));
    click_row(&mut app, Id::ProjectList, 0);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert!(project(&app).ends_with("/Agentmon"));
    assert_eq!(session(&app), Some("s1"));
}

#[test]
fn a_click_on_a_session_row_focuses_sessions_and_selects_that_row() {
    let mut app = loaded();
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(session(&app), None);
    // Not the newest: the selection ends on the row, not on the newest.
    click_row(&mut app, Id::SessionList, 2);
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(focus_of(&app), Some(Id::SessionList));
    assert_eq!(session(&app), Some("s3"));
    assert_eq!(session_rows(&app).selected, Some(2));
    click_row(&mut app, Id::SessionList, 4);
    assert_eq!(session(&app), Some("s5"));
    assert_eq!(session_rows(&app).selected, Some(4));
    assert!(project(&app).ends_with("/Agentmon"));
}

#[test]
fn a_click_on_a_session_row_of_a_scrolled_list_hits_the_row_drawn_there() {
    let mut app = loaded();
    app.update(Msg::Resized(60, 20));
    wheel(&mut app, Id::SessionList, true);
    wheel(&mut app, Id::SessionList, true);
    // The one row drawn is the third session.
    click_row(&mut app, Id::SessionList, 0);
    assert_eq!(session(&app), Some("s3"));
}

#[test]
fn a_click_in_the_empty_pane_area_only_focuses_the_pane() {
    let mut app = loaded();
    // Under the last session row of Sessions (5 rows).
    click(&mut app, Id::SessionList, 6, 12);
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(focus_of(&app), Some(Id::SessionList));
    assert!(project(&app).ends_with("/Agentmon"));
    // Gaining the focus with nothing selected selects the newest session
    // (product §3.1), as it does for any way of focusing the pane.
    assert_eq!(session(&app), Some("s1"));
    // Under the last project row, on the blank row and on the border.
    for (column, row) in [(6, 12), (6, 1), (0, 5), (3, 0)] {
        click(&mut app, Id::ProjectList, column, row);
        assert_eq!(app.focused_pane(), Pane::Projects, "{column}, {row}");
        assert!(project(&app).ends_with("/Agentmon"), "{column}, {row}");
        click(&mut app, Id::SessionList, column, row);
        assert_eq!(app.focused_pane(), Pane::Sessions, "{column}, {row}");
        assert!(project(&app).ends_with("/Agentmon"), "{column}, {row}");
    }
}

#[test]
fn a_double_click_in_a_list_scrolled_by_the_wheel_opens_the_row_under_the_pointer() {
    let mut app = loaded();
    app.update(Msg::Resized(80, 22));
    let time = click_clock(&mut app);
    // Three rows are drawn; nothing is selected, so the wheel only scrolls.
    wheel(&mut app, Id::SessionList, true);
    wheel(&mut app, Id::SessionList, true);
    assert_eq!(session(&app), None);
    let before = drawn(&mut app, 80, 22);
    assert!(before.contains("2026-10-12 17:02Z"), "{before}");
    click_row(&mut app, Id::SessionList, 0);
    assert_eq!(session(&app), Some("s3"));
    // The list did not jump under the pointer.
    assert_eq!(
        drawn(&mut app, 80, 22).replace('>', " "),
        before.replace('>', " ")
    );
    time.set(Duration::from_millis(100));
    click_row(&mut app, Id::SessionList, 0);
    assert_eq!(session(&app), Some("s3"));
    assert_eq!(
        app.take_queued(),
        [Msg::OpenSession(CLAUDE, SessionId::from("s3"))]
    );
}

#[test]
fn a_press_that_is_not_on_a_row_ends_a_double_click_under_way() {
    for (id, column, row) in [
        (Id::HarnessTabs, 3, 0),
        (Id::SessionList, 6, 12),
        (Id::WorkingDir, 3, 2),
        (Id::SelectorFrame, 0, 5),
    ] {
        let mut app = loaded();
        let time = click_clock(&mut app);
        click_row(&mut app, Id::SessionList, 1);
        time.set(Duration::from_millis(100));
        click(&mut app, id, column, row);
        time.set(Duration::from_millis(200));
        click_row(&mut app, Id::SessionList, 1);
        assert!(app.take_queued().is_empty(), "{id:?}");
    }
    // A press on nothing at all does too.
    let mut app = loaded();
    let time = click_clock(&mut app);
    click_row(&mut app, Id::SessionList, 1);
    app.update(Msg::Mouse(mouse(PRESS, 0, 0)));
    time.set(Duration::from_millis(100));
    click_row(&mut app, Id::SessionList, 1);
    assert!(app.take_queued().is_empty());
}

#[test]
fn the_wheel_over_projects_leaves_the_filtered_selector_alone() {
    let mut app = loaded();
    let matches = vec![(CLAUDE, SessionId::from("s2"))];
    app.show_filtered("s2", &matches);
    let before = (project(&app), cwd(&app), project_rows(&app).selected);
    wheel(&mut app, Id::ProjectList, true);
    wheel(&mut app, Id::ProjectList, false);
    assert_eq!(app.filter_prefix(), Some("s2"));
    assert_eq!(
        (project(&app), cwd(&app), project_rows(&app).selected),
        before
    );
    assert_eq!(app.focused_pane(), Pane::Sessions);
    // The wheel over the matches still works.
    wheel(&mut app, Id::SessionList, true);
    assert_eq!(app.filter_prefix(), Some("s2"));
}

#[test]
fn a_click_in_the_empty_pane_area_changes_nothing_but_the_focus() {
    let mut app = loaded();
    click_row(&mut app, Id::SessionList, 2);
    assert_eq!(session(&app), Some("s3"));
    app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
        Pane::Projects,
    )));
    let project_before = project(&app);
    // Under the last session row: Sessions gets the focus, s3 stays.
    click(&mut app, Id::SessionList, 6, 12);
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(focus_of(&app), Some(Id::SessionList));
    assert_eq!(session(&app), Some("s3"));
    assert_eq!(project(&app), project_before);
    // Under the last project row: Projects gets the focus, nothing else
    // changes.
    click(&mut app, Id::ProjectList, 6, 12);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(focus_of(&app), Some(Id::ProjectList));
    assert_eq!(session(&app), Some("s3"));
    assert_eq!(project(&app), project_before);
    assert!(app.take_queued().is_empty());
}

#[test]
fn a_click_on_a_row_but_outside_its_text_columns_is_a_pane_click() {
    let mut app = loaded();
    app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
        Pane::Sessions,
    )));
    // The border column of row 1 of Projects does not select BRIO.
    click(&mut app, Id::ProjectList, 0, 3);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert!(project(&app).ends_with("/Agentmon"));
}

#[test]
fn a_click_elsewhere_does_nothing() {
    let mut app = loaded();
    app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
        Pane::Sessions,
    )));
    app.update(Msg::Selector(crate::app::SelectorMsg::SessionSelected(
        SessionId::from("s3"),
    )));
    let projects = app.routing().area(Id::ProjectList).unwrap();
    let working_dir = pos(&app, Id::WorkingDir, 3, 2);
    let help_line = pos(&app, Id::HelpLine, 3, 0);
    let frame = pos(&app, Id::SelectorFrame, 0, 5);
    let gap = (projects.right(), projects.y + 3);
    for (column, row) in [working_dir, help_line, frame, gap, (0, 0), (119, 39)] {
        app.update(Msg::Mouse(mouse(PRESS, column, row)));
        app.update(Msg::Mouse(mouse(MouseEventKind::ScrollDown, column, row)));
        assert_eq!(app.focused_pane(), Pane::Sessions, "{column}, {row}");
        assert_eq!(session(&app), Some("s3"), "{column}, {row}");
        assert!(project(&app).ends_with("/Agentmon"), "{column}, {row}");
    }
    assert!(app.take_queued().is_empty());
}

#[test]
fn other_buttons_and_kinds_do_nothing() {
    let mut app = loaded();
    let (column, row) = pos(&app, Id::SessionList, 6, 3);
    for kind in [
        MouseEventKind::Down(MouseButton::Right),
        MouseEventKind::Down(MouseButton::Middle),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Moved,
        MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight,
    ] {
        app.update(Msg::Mouse(mouse(kind, column, row)));
        assert_eq!(app.focused_pane(), Pane::Projects, "{kind:?}");
        assert_eq!(session(&app), None, "{kind:?}");
    }
}

// -- double-clicks

#[test]
fn a_double_click_on_a_session_row_within_400_ms_opens_it() {
    let mut app = loaded();
    let time = click_clock(&mut app);
    click_row(&mut app, Id::SessionList, 1);
    assert_eq!(session(&app), Some("s2"));
    assert!(app.take_queued().is_empty());
    time.set(Duration::from_millis(300));
    click_row(&mut app, Id::SessionList, 1);
    assert_eq!(
        app.take_queued(),
        [Msg::OpenSession(CLAUDE, SessionId::from("s2"))]
    );
    assert_eq!(session(&app), Some("s2"));
}

#[test]
fn two_clicks_500_ms_apart_do_not_open_the_session() {
    let mut app = loaded();
    let time = click_clock(&mut app);
    click_row(&mut app, Id::SessionList, 1);
    time.set(Duration::from_millis(500));
    click_row(&mut app, Id::SessionList, 1);
    assert!(app.take_queued().is_empty());
    assert_eq!(session(&app), Some("s2"));
    // The second click starts a new pair.
    time.set(Duration::from_millis(600));
    click_row(&mut app, Id::SessionList, 1);
    assert_eq!(app.take_queued().len(), 1);
}

#[test]
fn the_double_click_time_is_400_ms_inclusive() {
    assert_eq!(DOUBLE_CLICK, Duration::from_millis(400));
    for (millis, opens) in [(400, true), (401, false)] {
        let mut app = loaded();
        let time = click_clock(&mut app);
        click_row(&mut app, Id::SessionList, 0);
        time.set(Duration::from_millis(millis));
        click_row(&mut app, Id::SessionList, 0);
        assert_eq!(!app.take_queued().is_empty(), opens, "{millis} ms");
    }
}

#[test]
fn two_clicks_on_different_rows_are_not_a_double_click() {
    let mut app = loaded();
    let time = click_clock(&mut app);
    click_row(&mut app, Id::SessionList, 1);
    time.set(Duration::from_millis(100));
    click_row(&mut app, Id::SessionList, 2);
    assert!(app.take_queued().is_empty());
    assert_eq!(session(&app), Some("s3"));
}

#[test]
fn a_click_in_between_breaks_the_double_click() {
    let mut app = loaded();
    let time = click_clock(&mut app);
    click_row(&mut app, Id::SessionList, 1);
    time.set(Duration::from_millis(100));
    click_row(&mut app, Id::ProjectList, 0);
    time.set(Duration::from_millis(200));
    click_row(&mut app, Id::SessionList, 1);
    assert!(app.take_queued().is_empty());
}

#[test]
fn a_third_click_does_not_open_the_session_again() {
    let mut app = loaded();
    let time = click_clock(&mut app);
    click_row(&mut app, Id::SessionList, 1);
    time.set(Duration::from_millis(100));
    click_row(&mut app, Id::SessionList, 1);
    assert_eq!(app.take_queued().len(), 1);
    time.set(Duration::from_millis(200));
    click_row(&mut app, Id::SessionList, 1);
    assert!(app.take_queued().is_empty());
}

#[test]
fn a_double_click_on_a_project_row_acts_like_enter_on_it() {
    let mut app = loaded();
    let time = click_clock(&mut app);
    click_row(&mut app, Id::ProjectList, 1);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert_eq!(session(&app), None);
    time.set(Duration::from_millis(200));
    click_row(&mut app, Id::ProjectList, 1);
    assert!(project(&app).ends_with("/BRIO"));
    assert_eq!(app.focused_pane(), Pane::Sessions);
    assert_eq!(focus_of(&app), Some(Id::SessionList));
    assert!(session(&app).is_some(), "the newest session is selected");
    assert!(app.take_queued().is_empty(), "nothing is opened");
}

// -- tabs

#[test]
fn a_click_on_an_available_tab_switches_to_it() {
    let mut app = two_harnesses();
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    // `[1] Claude` is 10 cells, then two spaces: `[2] OpenCode` starts at 12.
    click(&mut app, Id::HarnessTabs, 12, 0);
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(tabs(&app).selected, Some(OPENCODE));
    assert_eq!(project_rows(&app).rows[0].name, "Fake");
    assert_eq!(session(&app), None);
    // Any cell of the label, then back to the first.
    click(&mut app, Id::HarnessTabs, 0, 0);
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    click(&mut app, Id::HarnessTabs, 23, 0);
    assert_eq!(app.selected_harness(), Some(OPENCODE));
}

#[test]
fn a_tab_switch_by_click_selects_the_newest_session_only_with_sessions_focused() {
    let mut app = two_harnesses();
    click(&mut app, Id::HarnessTabs, 12, 0);
    assert_eq!(session(&app), None);
    click(&mut app, Id::HarnessTabs, 3, 0);
    app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
        Pane::Sessions,
    )));
    click(&mut app, Id::HarnessTabs, 12, 0);
    assert!(session(&app).is_some());
    assert_eq!(app.focused_pane(), Pane::Sessions);
}

#[test]
fn a_click_on_a_muted_tab_does_nothing() {
    let mut app = empty_app_in(&[CLAUDE, OPENCODE], theme("default"), "/nowhere");
    send(&mut app, fixture_snapshot(CLAUDE));
    send(
        &mut app,
        fixtures::snapshot_event(OPENCODE, false, Vec::new(), SessionMap::new()),
    );
    assert!(!tabs(&app).tabs[1].available);
    let before = (tabs(&app), project(&app), app.focused_pane());
    click(&mut app, Id::HarnessTabs, 12, 0);
    assert_eq!(app.selected_harness(), Some(CLAUDE));
    assert_eq!((tabs(&app), project(&app), app.focused_pane()), before);
}

#[test]
fn a_click_between_the_tabs_or_on_the_underline_does_nothing() {
    let mut app = two_harnesses();
    for (column, row) in [(10, 0), (11, 0), (12, 1), (3, 1), (40, 0)] {
        click(&mut app, Id::HarnessTabs, column, row);
        assert_eq!(app.selected_harness(), Some(CLAUDE), "{column}, {row}");
    }
}

// -- overlays and prompts

#[test]
fn nothing_under_the_help_overlay_or_the_theme_selector_gets_the_mouse() {
    for open in [Msg::OpenHelp(HelpContext::Selector), Msg::OpenThemeSelector] {
        let mut app = loaded();
        let overlay = if open == Msg::OpenThemeSelector {
            Id::ThemeSelector
        } else {
            Id::HelpOverlay
        };
        app.update(open);
        let focus = focus_of(&app);
        assert_eq!(focus, Some(overlay));
        let before = (project(&app), session(&app).map(str::to_owned));
        // The points of the panes that the overlay covers.
        let covered = app.routing().area(overlay).unwrap();
        let mut tried = 0;
        for pane in [Id::ProjectList, Id::SessionList] {
            let area = covered.intersection(app.routing().area(pane).unwrap());
            for (column, row) in [(area.x, area.y), (area.x + 1, area.y + 2)] {
                if !area.contains((column, row).into()) {
                    continue;
                }
                tried += 1;
                // The overlay may act on the click itself (a theme row); the
                // pane under it must not.
                app.update(Msg::Mouse(mouse(PRESS, column, row)));
                assert_eq!(app.routing().area(overlay), Some(covered), "{overlay:?}");
            }
        }
        assert!(tried > 0, "the overlay covers part of a pane");
        assert_eq!(focus_of(&app), focus, "{overlay:?}");
        assert_eq!(
            (project(&app), session(&app).map(str::to_owned)),
            before,
            "{overlay:?}"
        );
        assert_eq!(app.focused_pane(), Pane::Projects);
    }
}

#[test]
fn a_click_on_a_session_while_searching_selects_that_result() {
    let mut app = searching("brio");
    let ids: Vec<String> = app
        .search_results()
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect();
    assert!(ids.len() >= 3, "{ids:?}");
    let project_before = project(&app);
    click_row(&mut app, Id::SessionList, 2);
    assert_eq!(
        app.selected_result().map(|id| id.as_str().to_owned()),
        Some(ids[2].clone())
    );
    assert_eq!(session_rows(&app).selected, Some(2));
    // The search goes on: the prompt keeps the input.
    assert_eq!(focus_of(&app), Some(Id::Prompt));
    assert_eq!(app.search_query(), Some("brio"));
    assert_eq!(project(&app), project_before);
    assert_eq!(app.search_results().len(), ids.len());
}

#[test]
fn the_wheel_moves_the_selected_search_result() {
    let mut app = searching("brio");
    let ids: Vec<String> = app
        .search_results()
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect();
    wheel(&mut app, Id::SessionList, true);
    assert_eq!(
        app.selected_result().map(|id| id.as_str().to_owned()),
        Some(ids[1].clone())
    );
    wheel(&mut app, Id::SessionList, false);
    assert_eq!(
        app.selected_result().map(|id| id.as_str().to_owned()),
        Some(ids[0].clone())
    );
    assert_eq!(focus_of(&app), Some(Id::Prompt));
}

#[test]
fn a_double_click_on_a_search_result_opens_it() {
    let mut app = searching("brio");
    let time = click_clock(&mut app);
    let id = app.search_results()[1].clone();
    click_row(&mut app, Id::SessionList, 1);
    time.set(Duration::from_millis(250));
    click_row(&mut app, Id::SessionList, 1);
    assert_eq!(app.take_queued(), [Msg::OpenSession(CLAUDE, id)]);
}

#[test]
fn the_projects_and_the_tabs_are_off_while_searching() {
    let mut app = searching("brio");
    let before = (
        project(&app),
        app.focused_pane(),
        app.search_results().len(),
    );
    click_row(&mut app, Id::ProjectList, 3);
    wheel(&mut app, Id::ProjectList, true);
    click(&mut app, Id::HarnessTabs, 3, 0);
    assert_eq!(
        (
            project(&app),
            app.focused_pane(),
            app.search_results().len()
        ),
        before
    );
    assert_eq!(focus_of(&app), Some(Id::Prompt));
    assert!(app.focus_stack.len() == 1);
}

// -- the filtered selector

#[test]
fn a_click_in_the_filtered_selector_selects_the_clicked_match() {
    let mut app = loaded();
    let matches: Vec<_> = ["s1", "s2", "s3"]
        .iter()
        .map(|id| (CLAUDE, SessionId::from(*id)))
        .collect();
    app.show_filtered("s", &matches);
    assert_eq!(app.filtered_sessions().len(), 3);
    let time = click_clock(&mut app);
    click_row(&mut app, Id::SessionList, 2);
    assert_eq!(session_rows(&app).selected, Some(2));
    assert_eq!(app.filter_prefix(), Some("s"));
    let id = app.filtered_sessions()[2].clone();
    time.set(Duration::from_millis(100));
    click_row(&mut app, Id::SessionList, 2);
    assert_eq!(app.take_queued(), [Msg::OpenSession(CLAUDE, id)]);
}

#[test]
fn a_click_on_projects_ends_the_filter() {
    let mut app = loaded();
    let matches = vec![(CLAUDE, SessionId::from("s2"))];
    app.show_filtered("s2", &matches);
    click_row(&mut app, Id::ProjectList, 1);
    assert_eq!(app.filter_prefix(), None);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert!(project(&app).ends_with("/BRIO"));
    assert_eq!(session(&app), None);
}

// -- mouse = false

#[test]
fn with_mouse_off_mouse_events_have_no_effect() {
    let mut init = init(&[CLAUDE], theme("default"));
    init.config.mouse = false;
    let mut app = App::new(init, EventListenerCfg::default(), PathBuf::from("/nowhere")).unwrap();
    app.set_clock(fixtures::now);
    app.update(Msg::Resized(120, 40));
    send(&mut app, fixture_snapshot(CLAUDE));
    let time = click_clock(&mut app);
    assert!(app.routing().area(Id::SessionList).is_some());
    click_row(&mut app, Id::SessionList, 2);
    time.set(Duration::from_millis(10));
    click_row(&mut app, Id::SessionList, 2);
    wheel(&mut app, Id::ProjectList, true);
    click_row(&mut app, Id::ProjectList, 3);
    assert_eq!(app.focused_pane(), Pane::Projects);
    assert!(project(&app).ends_with("/Agentmon"));
    assert_eq!(session(&app), None);
    assert!(app.take_queued().is_empty());
}

// -- through the event loop

/// A port that delivers the events sent through its channel.
struct EventPort(Receiver<Event<UserEvent>>);

impl Poll<UserEvent> for EventPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        Ok(self.0.try_recv().ok())
    }
}

fn app_with_events(mouse: bool) -> (App, Sender<Event<UserEvent>>) {
    let (tx, rx) = channel();
    let listener =
        EventListenerCfg::default().add_port(Box::new(EventPort(rx)), Duration::from_millis(2), 8);
    let mut init = init(&[CLAUDE], theme("default"));
    init.config.mouse = mouse;
    let mut app = App::new(init, listener, PathBuf::from("/nowhere")).unwrap();
    app.set_clock(fixtures::now);
    app.update(Msg::Resized(120, 40));
    send(&mut app, fixture_snapshot(CLAUDE));
    (app, tx)
}

/// Ticks until the app returned something, then a few times more.
fn collect(app: &mut App) -> Vec<Msg> {
    let mut msgs = Vec::new();
    for _ in 0..200 {
        msgs.extend(app.tick().unwrap());
        if !msgs.is_empty() {
            break;
        }
    }
    for _ in 0..5 {
        msgs.extend(app.tick().unwrap());
    }
    msgs
}

fn event(kind: MouseEventKind, modifiers: KeyModifiers, column: u16, row: u16) -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        modifiers,
        column,
        row,
    })
}

#[test]
fn a_click_travels_from_the_terminal_once_whichever_component_has_focus() {
    for focus_sessions in [false, true] {
        let (mut app, tx) = app_with_events(true);
        if focus_sessions {
            app.update(Msg::Selector(crate::app::SelectorMsg::Focus(
                Pane::Sessions,
            )));
        }
        // Row 1 (BRIO) of the Projects pane.
        let (column, row) = pos(&app, Id::ProjectList, 6, 3);
        tx.send(event(PRESS, KeyModifiers::NONE, column, row))
            .unwrap();
        let msgs = collect(&mut app);
        assert_eq!(
            msgs,
            [Msg::Mouse(mouse(PRESS, column, row))],
            "the focused component must not act on the raw event too"
        );
        for msg in msgs {
            app.update(msg);
        }
        assert!(project(&app).ends_with("/BRIO"));
        assert_eq!(app.focused_pane(), Pane::Projects);
    }
}

#[test]
fn the_wheel_travels_from_the_terminal() {
    let (mut app, tx) = app_with_events(true);
    let (column, row) = pos(&app, Id::ProjectList, 6, 3);
    tx.send(event(
        MouseEventKind::ScrollDown,
        KeyModifiers::NONE,
        column,
        row,
    ))
    .unwrap();
    let msgs = collect(&mut app);
    assert_eq!(msgs.len(), 1, "{msgs:?}");
    for msg in msgs {
        app.update(msg);
    }
    assert!(project(&app).ends_with("/BRIO"));
}

#[test]
fn only_presses_and_wheel_steps_without_modifiers_reach_the_app() {
    let (mut app, tx) = app_with_events(true);
    let (column, row) = pos(&app, Id::ProjectList, 6, 3);
    for (kind, modifiers) in [
        (MouseEventKind::Moved, KeyModifiers::NONE),
        (MouseEventKind::Up(MouseButton::Left), KeyModifiers::NONE),
        (MouseEventKind::Drag(MouseButton::Left), KeyModifiers::NONE),
        (MouseEventKind::Down(MouseButton::Right), KeyModifiers::NONE),
        (PRESS, KeyModifiers::SHIFT),
        (PRESS, KeyModifiers::CONTROL),
    ] {
        tx.send(event(kind, modifiers, column, row)).unwrap();
    }
    assert_eq!(collect(&mut app), []);
}

#[test]
fn with_mouse_off_the_app_is_not_subscribed_to_the_mouse() {
    let (mut app, tx) = app_with_events(false);
    let (column, row) = pos(&app, Id::ProjectList, 6, 3);
    tx.send(event(PRESS, KeyModifiers::NONE, column, row))
        .unwrap();
    tx.send(event(
        MouseEventKind::ScrollDown,
        KeyModifiers::NONE,
        column,
        row,
    ))
    .unwrap();
    assert_eq!(collect(&mut app), []);
    // Keys still arrive.
    tx.send(Event::Keyboard(KeyEvent::new(
        Key::Char('q'),
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert_eq!(collect(&mut app), [Msg::Quit]);
}
