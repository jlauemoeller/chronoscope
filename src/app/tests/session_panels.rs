//! Tests of the `Session Time` and `Session Tokens` panels in the app (no
//! terminal): the screens with and without them, the heights the lists take
//! from the layout, and the panels' being inert: no focus, no mouse.

use ratatui::layout::Rect;
use tuirealm::event::{Key, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use super::selector::{
    CLAUDE, empty_app, focus_sessions, loaded, long_session_snapshot, press, screen, send,
    session_rows, styled, text, theme,
};
use crate::app::redraw::RedrawRequest;
use crate::app::{App, Id, Msg, Pane};
use crate::components::{THEME_ATTR, VISIBLE_ROWS_ATTR, theme_from_value};
use crate::model::SessionId;
use crate::test_support::{buffer_to_text, render};
use crate::theme::Role;
use tuirealm::props::AttrValue;

// -- helpers

const PANELS: [Id; 2] = [Id::SessionTime, Id::SessionTokens];

/// An app with 40 sessions in the first project and the Sessions pane
/// focused with nothing selected yet.
fn long() -> App {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    app.update(Msg::Resized(120, 40));
    send(&mut app, long_session_snapshot(40));
    app
}

/// The numbers of the sessions drawn in `screen`, top to bottom.
fn drawn_numbers(screen: &str) -> Vec<usize> {
    screen
        .lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once("Session number ")?;
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect()
}

/// The screen as drawn now, at `width` x `height`, without a resize message.
fn drawn(app: &mut App, width: u16, height: u16) -> String {
    buffer_to_text(&render(width, height, |frame| app.view(frame)))
}

fn area(app: &App, id: Id) -> Rect {
    app.routing().area(id).expect("the component is routed")
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Msg {
    Msg::Pointer(MouseEvent {
        kind,
        modifiers: KeyModifiers::NONE,
        column,
        row,
    })
}

const PRESS: MouseEventKind = MouseEventKind::Down(MouseButton::Left);

fn visible(app: &App, id: Id) -> usize {
    match app
        .application()
        .query(&id, VISIBLE_ROWS_ATTR)
        .unwrap()
        .unwrap()
        .into_attr()
    {
        AttrValue::Length(n) => n,
        other => panic!("{other:?}"),
    }
}

// -- screens

#[test]
fn empty_panels_160x50_text() {
    insta::assert_snapshot!(text(&mut loaded(), 160, 50));
}

#[test]
fn empty_panels_160x50_styled() {
    insta::assert_snapshot!(styled(&mut loaded(), 160, 50));
}

#[test]
fn empty_panels_60x33_text() {
    insta::assert_snapshot!(text(&mut loaded(), 60, 33));
}

#[test]
fn empty_panels_60x33_styled() {
    insta::assert_snapshot!(styled(&mut loaded(), 60, 33));
}

#[test]
fn no_panels_60x32_text() {
    insta::assert_snapshot!(text(&mut loaded(), 60, 32));
}

#[test]
fn the_panels_are_drawn_from_33_rows_and_not_below() {
    let mut app = loaded();
    for (height, shown) in [(31, false), (32, false), (33, true), (34, true)] {
        let screen = text(&mut app, 80, height);
        assert_eq!(screen.contains("Session Time"), shown, "{height}\n{screen}");
        assert_eq!(
            screen.contains("Session Tokens"),
            shown,
            "{height}\n{screen}"
        );
    }
}

#[test]
fn the_panels_are_borders_in_border_and_never_ring() {
    let theme = theme("default");
    assert_ne!(theme.role(Role::Border), theme.role(Role::Ring));
    for pane in [Pane::Projects, Pane::Sessions] {
        let mut app = loaded();
        app.update(Msg::Selector(crate::app::SelectorMsg::Focus(pane)));
        let buffer = screen(&mut app, 120, 40);
        for id in PANELS {
            let a = area(&app, id);
            for (x, y) in [
                (a.x, a.y),
                (a.right() - 1, a.y),
                (a.x, a.bottom() - 1),
                (a.right() - 1, a.bottom() - 1),
                (a.x, a.y + 1),
            ] {
                assert_eq!(
                    buffer[(x, y)].fg,
                    theme.role(Role::Border),
                    "{id:?} {pane:?}"
                );
            }
        }
    }
}

// -- the lists' heights

#[test]
fn the_sessions_list_height_follows_its_pane_at_120x40() {
    let app = loaded();
    assert_eq!(visible(&app, Id::ProjectList), 21);
    assert_eq!(visible(&app, Id::SessionList), 10);
}

#[test]
fn at_60x33_the_sessions_list_shows_3_rows() {
    let mut app = loaded();
    app.update(Msg::Resized(60, 33));
    assert_eq!(visible(&app, Id::SessionList), 3);
    assert_eq!(visible(&app, Id::ProjectList), 14);
}

#[test]
fn the_selected_row_stays_visible_when_the_panels_come_and_go() {
    // The last row of a long list, across the sizes where the panels appear
    // and disappear, in both directions and by single steps.
    let sizes = [
        (80, 40, 10),
        (80, 33, 3),
        (80, 32, 13),
        (80, 33, 3),
        (80, 40, 10),
        (100, 32, 13),
        (100, 33, 3),
        (100, 32, 13),
    ];
    let mut app = long();
    focus_sessions(&mut app);
    for _ in 0..39 {
        press(&mut app, Key::Char('j'));
    }
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m39"));
    for (width, height, rows) in sizes {
        let shown = text(&mut app, width, height);
        let numbers = drawn_numbers(&shown);
        assert_eq!(numbers.len(), rows, "{width}x{height}\n{shown}");
        assert_eq!(numbers.last(), Some(&39), "{width}x{height}\n{shown}");
        assert!(
            shown
                .lines()
                .any(|line| line.contains("> 2026-") && line.contains("Session number 39")),
            "the marker is on a drawn row at {width}x{height}\n{shown}"
        );
        assert_eq!(visible(&app, Id::SessionList), rows);
    }
    // A selection in the middle also stays drawn.
    let mut app = long();
    focus_sessions(&mut app);
    for _ in 0..20 {
        press(&mut app, Key::Char('j'));
    }
    for (width, height) in [(80, 40), (80, 33), (80, 32), (80, 33), (80, 40)] {
        let numbers = drawn_numbers(&text(&mut app, width, height));
        assert!(numbers.contains(&20), "{width}x{height}: {numbers:?}");
    }
}

#[test]
fn keyboard_moves_follow_the_new_height() {
    let mut app = long();
    focus_sessions(&mut app);
    for _ in 0..39 {
        press(&mut app, Key::Char('j'));
    }
    // 120x40 shows 10 rows, not the 21 the pane had before the panels.
    let numbers = drawn_numbers(&text(&mut app, 120, 40));
    assert_eq!(numbers, (30..40).collect::<Vec<_>>());
    // Moving up inside the window does not scroll.
    for _ in 0..5 {
        press(&mut app, Key::Char('k'));
    }
    let numbers = drawn_numbers(&text(&mut app, 120, 40));
    assert_eq!(numbers, (30..40).collect::<Vec<_>>());
    // Moving past its top does.
    for _ in 0..5 {
        press(&mut app, Key::Char('k'));
    }
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m29"));
    let numbers = drawn_numbers(&text(&mut app, 120, 40));
    assert_eq!(numbers.first(), Some(&29));
    assert_eq!(numbers.len(), 10);
}

#[test]
fn the_wheel_scrolls_by_the_new_height() {
    for (width, height, rows) in [(120, 40, 10), (100, 33, 3), (120, 32, 13)] {
        let mut app = long();
        app.update(Msg::Resized(width, height));
        assert_eq!(app.selected_session(), None);
        let sessions = area(&app, Id::SessionList);
        let (column, row) = (sessions.x + 6, sessions.y + 3);
        for _ in 0..100 {
            app.update_routed(mouse(MouseEventKind::ScrollDown, column, row));
        }
        let numbers = drawn_numbers(&text(&mut app, width, height));
        assert_eq!(
            numbers,
            (40 - rows..40).collect::<Vec<_>>(),
            "{width}x{height}: the end of the list, no further"
        );
        for _ in 0..100 {
            app.update_routed(mouse(MouseEventKind::ScrollUp, column, row));
        }
        let numbers = drawn_numbers(&text(&mut app, width, height));
        assert_eq!(numbers, (0..rows).collect::<Vec<_>>(), "{width}x{height}");
        assert_eq!(app.selected_session(), None);
    }
}

// -- the mouse

/// The positions of the panels, border corners and text rows included.
fn panel_points(app: &App) -> Vec<(Id, u16, u16)> {
    let mut points = Vec::new();
    for id in PANELS {
        let a = area(app, id);
        for (dx, dy) in [
            (0, 0),
            (1, 0),
            (a.width - 1, 0),
            (0, 1),
            (6, 1),
            (a.width / 2, 2),
            (a.width - 2, a.height - 2),
            (a.width - 1, a.height - 1),
            (3, a.height - 1),
        ] {
            points.push((id, a.x + dx, a.y + dy));
        }
    }
    points
}

#[test]
fn a_click_over_either_panel_changes_neither_selection_nor_scroll_nor_focus() {
    let mut app = long();
    focus_sessions(&mut app);
    for _ in 0..25 {
        press(&mut app, Key::Char('j'));
    }
    let before = drawn_numbers(&text(&mut app, 120, 40));
    let selected = app.selected_session().cloned();
    let project = app.selected_project().cloned();
    let rows = session_rows(&app);
    app.take_redraw();
    for (id, column, row) in panel_points(&app) {
        app.update_routed(mouse(PRESS, column, row));
        assert_eq!(app.selected_session().cloned(), selected, "{id:?}");
        assert_eq!(app.selected_project().cloned(), project, "{id:?}");
        assert_eq!(app.focused_pane(), Pane::Sessions, "{id:?}");
        assert_eq!(app.application().focus(), Some(&Id::SessionList), "{id:?}");
        assert_eq!(session_rows(&app), rows, "{id:?}");
        assert_eq!(drawn_numbers(&drawn(&mut app, 120, 40)), before, "{id:?}");
    }
    assert_eq!(app.take_redraw(), RedrawRequest::None);
}

#[test]
fn a_double_click_over_a_panel_does_not_open_the_session() {
    let mut app = long();
    focus_sessions(&mut app);
    app.take_redraw();
    for (_, column, row) in panel_points(&app) {
        for _ in 0..2 {
            app.update_routed(mouse(PRESS, column, row));
        }
    }
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m00"));
    assert_eq!(app.screen(), crate::app::layout::Screen::Selector);
}

#[test]
fn the_wheel_over_either_panel_changes_neither_selection_nor_scroll() {
    for selected in [false, true] {
        let mut app = long();
        if selected {
            focus_sessions(&mut app);
            for _ in 0..12 {
                press(&mut app, Key::Char('j'));
            }
        } else {
            // Scroll the list a little with the wheel first.
            let sessions = area(&app, Id::SessionList);
            for _ in 0..4 {
                app.update_routed(mouse(
                    MouseEventKind::ScrollDown,
                    sessions.x + 6,
                    sessions.y + 3,
                ));
            }
        }
        let before = drawn_numbers(&text(&mut app, 120, 40));
        let session = app.selected_session().cloned();
        let project = app.selected_project().cloned();
        let focus = app.application().focus().copied();
        app.take_redraw();
        for (id, column, row) in panel_points(&app) {
            for kind in [MouseEventKind::ScrollDown, MouseEventKind::ScrollUp] {
                for _ in 0..30 {
                    app.update_routed(mouse(kind, column, row));
                }
                let now = drawn_numbers(&drawn(&mut app, 120, 40));
                assert_eq!(now, before, "{id:?} {kind:?} selected={selected}");
                assert_eq!(app.selected_session().cloned(), session, "{id:?}");
                assert_eq!(app.selected_project().cloned(), project, "{id:?}");
                assert_eq!(app.application().focus().copied(), focus, "{id:?}");
            }
        }
        assert_eq!(app.take_redraw(), RedrawRequest::None);
    }
}

#[test]
fn the_rows_just_above_the_panels_stay_clickable() {
    let mut app = long();
    let sessions = area(&app, Id::SessionList);
    let time = area(&app, Id::SessionTime);
    assert_eq!(sessions.bottom(), time.y);
    // The pane's last list row is two rows above its bottom border... the
    // border is the row above `Session Time`, the list row above that.
    let last_row_y = sessions.bottom() - 2;
    app.update_routed(mouse(PRESS, sessions.x + 6, last_row_y));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m09"));
    assert_eq!(app.focused_pane(), Pane::Sessions);
    // The same column, one row lower, is the panel: nothing happens.
    app.update_routed(mouse(PRESS, sessions.x + 6, time.y));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m09"));
}

#[test]
fn at_60x33_the_last_list_row_is_clickable_and_the_panel_below_is_not() {
    let mut app = long();
    app.update(Msg::Resized(60, 33));
    let sessions = area(&app, Id::SessionList);
    assert_eq!(sessions.height, 6);
    app.update_routed(mouse(PRESS, sessions.x + 6, sessions.y + 4));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m02"));
    let time = area(&app, Id::SessionTime);
    app.update_routed(mouse(PRESS, time.x + 6, time.y + 1));
    assert_eq!(app.selected_session().map(SessionId::as_str), Some("m02"));
}

// -- focus

#[test]
fn tab_moves_only_between_projects_and_sessions_and_never_to_a_panel() {
    let mut app = loaded();
    let mut seen = Vec::new();
    for key in [
        Key::Tab,
        Key::Tab,
        Key::Tab,
        Key::BackTab,
        Key::BackTab,
        Key::Char('j'),
        Key::Down,
        Key::Up,
        Key::Left,
        Key::Right,
        Key::Enter,
    ] {
        press(&mut app, key);
        let focus = app.application().focus().copied();
        assert!(
            matches!(focus, Some(Id::ProjectList | Id::SessionList)),
            "{key:?}: {focus:?}"
        );
        seen.push(focus);
    }
    assert_eq!(seen[0], Some(Id::SessionList));
    assert_eq!(seen[1], Some(Id::ProjectList));
    assert_eq!(seen[2], Some(Id::SessionList));
    assert_eq!(seen[3], Some(Id::ProjectList));
}

// -- theme

#[test]
fn a_theme_change_recolours_both_panels() {
    let mut app = loaded();
    let before = screen(&mut app, 120, 40);
    let default = theme("default");
    let nord = theme("nord");
    assert_ne!(default.role(Role::Border), nord.role(Role::Border));
    app.apply_theme_id("nord");
    for id in PANELS {
        let value = app
            .application()
            .query(&id, THEME_ATTR)
            .unwrap()
            .unwrap()
            .into_attr();
        assert_eq!(theme_from_value(&value), Some(nord.clone()), "{id:?}");
        let a = area(&app, id);
        assert_eq!(
            before[(a.x, a.y)].fg,
            default.role(Role::Border),
            "{id:?} before"
        );
    }
    let after = screen(&mut app, 120, 40);
    for id in PANELS {
        let a = area(&app, id);
        assert_eq!(after[(a.x, a.y)].fg, nord.role(Role::Border), "{id:?}");
        assert_eq!(after[(a.x + 3, a.y)].fg, nord.role(Role::Foreground));
    }
}
