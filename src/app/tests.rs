//! Update tests (no terminal), loop tests (a scripted event source and a
//! `TestBackend`) and screen snapshots of `App::view`.

use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use ratatui::layout::Size;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::listener::{EventListenerCfg, Poll, PortResult};
use tuirealm::terminal::{TerminalAdapter, TestTerminalAdapter};

use super::redraw::RedrawRequest;
use super::runtime::run_loop;
use super::{App, AppInit, Id, Msg, UserEvent};
use crate::config::Config;
use crate::keymap::Keymap;
use crate::test_support::{buffer_to_styled, buffer_to_text, render};
use crate::theme::{TerminalEnv, Theme, ThemeRegistry, resolve_by_id};

fn env() -> TerminalEnv {
    TerminalEnv {
        colorterm: Some("truecolor".to_owned()),
        no_color: false,
    }
}

fn theme(id: &str) -> Theme {
    resolve_by_id(&ThemeRegistry::builtin(), id, &env(), None).unwrap()
}

fn init_with(theme: Theme) -> AppInit {
    AppInit {
        config: Config::default(),
        registry: ThemeRegistry::builtin(),
        theme,
        keymap: Keymap::default(),
        warnings: Vec::new(),
    }
}

/// An app without event ports: nothing reads the terminal.
fn app_with(theme: Theme) -> App {
    App::new(init_with(theme), EventListenerCfg::default()).unwrap()
}

fn app() -> App {
    app_with(theme("default"))
}

// -- update

#[test]
fn quit_message_sets_the_quit_state() {
    let mut app = app();
    assert!(!app.should_quit());
    app.update(Msg::Quit);
    assert!(app.should_quit());
}

#[test]
fn resized_message_marks_the_state_dirty_and_records_the_size() {
    let mut app = app();
    assert_eq!(app.take_redraw(), RedrawRequest::Dirty);
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    app.update(Msg::Resized(100, 30));
    assert_eq!(app.size(), (100, 30));
    assert_eq!(app.take_redraw(), RedrawRequest::Resized);
    assert_eq!(app.take_redraw(), RedrawRequest::None);
}

#[test]
fn a_tick_changes_nothing_visible() {
    let mut app = app();
    app.take_redraw();
    app.update(Msg::Tick);
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    assert!(!app.should_quit());
}

#[test]
fn a_resize_request_is_not_downgraded_by_later_requests() {
    let mut app = app();
    app.update(Msg::Resized(80, 24));
    app.update(Msg::Tick);
    assert_eq!(app.take_redraw(), RedrawRequest::Resized);
}

#[test]
fn the_app_owns_the_domain_state_it_was_built_with() {
    let mut init = init_with(theme("default"));
    init.warnings.push(crate::config::ConfigWarning {
        key: "k".to_owned(),
        message: "m".to_owned(),
    });
    let app = App::new(init, EventListenerCfg::default()).unwrap();
    assert_eq!(app.config(), &Config::default());
    assert_eq!(app.theme(), &theme("default"));
    assert_eq!(app.keymap(), &Keymap::default());
    assert_eq!(app.theme_registry(), &ThemeRegistry::builtin());
    assert_eq!(app.warnings().len(), 1);
}

#[test]
fn startup_mounts_the_components_and_nothing_has_focus() {
    let app = app();
    for id in [Id::GlobalEvents, Id::SelectorFrame, Id::EmptyState] {
        assert!(app.application().mounted(&id), "{id:?}");
    }
    assert_eq!(app.application().focus(), None);
}

// -- loop

/// A port that delivers a fixed list of events, one per poll.
struct Script(VecDeque<Event<UserEvent>>);

impl Poll<UserEvent> for Script {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        Ok(self.0.pop_front())
    }
}

fn scripted_app(events: Vec<Event<UserEvent>>) -> App {
    let listener = EventListenerCfg::default()
        .add_port(Box::new(Script(events.into())), Duration::from_millis(5), 1)
        .tick_interval(Duration::from_millis(20));
    App::new(init_with(theme("default")), listener).unwrap()
}

fn key(code: Key, modifiers: KeyModifiers) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent { code, modifiers })
}

fn test_terminal(width: u16, height: u16) -> TestTerminalAdapter {
    TestTerminalAdapter::new(Size { width, height }).unwrap()
}

#[test]
fn every_quit_key_ends_the_loop() {
    let keys = [
        key(Key::Char('q'), KeyModifiers::NONE),
        key(Key::Char('q'), KeyModifiers::CONTROL),
        key(Key::Char('w'), KeyModifiers::CONTROL),
        key(Key::Char('c'), KeyModifiers::CONTROL),
    ];
    for quit_key in keys {
        let mut app = scripted_app(vec![quit_key.clone()]);
        let mut terminal = test_terminal(80, 24);
        run_loop(&mut app, &mut terminal, &AtomicBool::new(false)).unwrap();
        assert!(app.should_quit(), "{quit_key:?}");
    }
}

#[test]
fn other_keys_do_not_end_the_loop() {
    let mut app = scripted_app(vec![
        key(Key::Char('a'), KeyModifiers::NONE),
        key(Key::Enter, KeyModifiers::NONE),
        key(Key::Char('d'), KeyModifiers::CONTROL),
        key(Key::Char('x'), KeyModifiers::NONE),
        key(Key::Char('q'), KeyModifiers::NONE),
    ]);
    let mut terminal = test_terminal(80, 24);
    run_loop(&mut app, &mut terminal, &AtomicBool::new(false)).unwrap();
    assert!(app.should_quit());
}

#[test]
fn the_loop_draws_the_startup_frame_before_it_ends() {
    let mut app = scripted_app(vec![Event::Tick, key(Key::Char('q'), KeyModifiers::NONE)]);
    let mut terminal = test_terminal(80, 24);
    run_loop(&mut app, &mut terminal, &AtomicBool::new(false)).unwrap();
    let text = buffer_to_text(terminal.raw().backend().buffer());
    assert_eq!(text.lines().nth(1).unwrap().chars().next(), Some('╭'));
}

#[test]
fn a_resize_event_updates_the_size() {
    let mut app = scripted_app(vec![
        Event::WindowResize(90, 30),
        key(Key::Char('q'), KeyModifiers::NONE),
    ]);
    let mut terminal = test_terminal(80, 24);
    run_loop(&mut app, &mut terminal, &AtomicBool::new(false)).unwrap();
    assert_eq!(app.size(), (90, 30));
}

#[test]
fn the_loop_starts_with_the_terminal_size() {
    let mut app = scripted_app(vec![key(Key::Char('q'), KeyModifiers::NONE)]);
    let mut terminal = test_terminal(77, 33);
    run_loop(&mut app, &mut terminal, &AtomicBool::new(false)).unwrap();
    assert_eq!(app.size(), (77, 33));
}

#[test]
fn the_shutdown_flag_ends_the_loop() {
    let mut app = scripted_app(Vec::new());
    let mut terminal = test_terminal(80, 24);
    run_loop(&mut app, &mut terminal, &AtomicBool::new(true)).unwrap();
    assert!(!app.should_quit());
}

// -- screens

fn screen(app: &mut App, width: u16, height: u16) -> ratatui::buffer::Buffer {
    render(width, height, |frame| app.view(frame))
}

#[test]
fn startup_frame_80x24_text() {
    insta::assert_snapshot!(buffer_to_text(&screen(&mut app(), 80, 24)));
}

#[test]
fn startup_frame_120x40_text() {
    insta::assert_snapshot!(buffer_to_text(&screen(&mut app(), 120, 40)));
}

#[test]
fn startup_frame_200x50_text() {
    insta::assert_snapshot!(buffer_to_text(&screen(&mut app(), 200, 50)));
}

#[test]
fn startup_frame_80x24_styled() {
    insta::assert_snapshot!(buffer_to_styled(&screen(&mut app(), 80, 24)));
}

#[test]
fn startup_frame_200x50_spans_columns_20_to_179() {
    let theme = theme("default");
    let buffer = screen(&mut app(), 200, 50);
    use crate::theme::Role;
    for (x, y, symbol, bg) in [
        (19, 10, " ", theme.role(Role::Background)),
        (20, 1, "╭", theme.role(Role::Surface)),
        (179, 48, "╯", theme.role(Role::Surface)),
        (180, 10, " ", theme.role(Role::Background)),
        (100, 0, " ", theme.role(Role::Background)),
        (100, 49, " ", theme.role(Role::Background)),
    ] {
        assert_eq!(buffer[(x, y)].symbol(), symbol, "({x},{y})");
        assert_eq!(buffer[(x, y)].bg, bg, "({x},{y})");
    }
}

#[test]
fn startup_frame_with_a_transparent_theme_80x24_styled() {
    let transparent = theme("default").with_transparent_background(true);
    let buffer = screen(&mut app_with(transparent), 80, 24);
    let theme = theme("default");
    use crate::theme::Role;
    for cell in buffer.content() {
        assert_ne!(cell.bg, theme.role(Role::Surface));
        assert_ne!(cell.bg, theme.role(Role::Background));
    }
    assert_eq!(buffer[(0, 1)].fg, theme.role(Role::Ring));
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}

#[test]
fn too_small_59x24_text() {
    insta::assert_snapshot!(buffer_to_text(&screen(&mut app(), 59, 24)));
}

#[test]
fn too_small_80x19_text() {
    insta::assert_snapshot!(buffer_to_text(&screen(&mut app(), 80, 19)));
}

#[test]
fn too_small_screen_shows_only_the_message() {
    let text = buffer_to_text(&screen(&mut app(), 59, 24));
    assert_eq!(text.matches("Terminal too small").count(), 1);
    assert!(!text.contains('╭'));
}

#[test]
fn view_does_not_panic_on_degenerate_sizes() {
    let mut app = app();
    for (w, h) in [(0, 0), (1, 1), (60, 0), (60, 20), (500, 200)] {
        let _ = screen(&mut app, w, h);
    }
}

#[test]
fn every_builtin_theme_draws_the_startup_frame() {
    for entry in ThemeRegistry::builtin().entries() {
        let mut app = app_with(theme(entry.id()));
        let text = buffer_to_text(&screen(&mut app, 80, 24));
        assert!(text.contains('╭'), "{}", entry.id());
    }
}
