//! Update tests (no terminal), loop tests (a scripted event source and a
//! `TestBackend`) and screen snapshots of `App::view`.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use ratatui::layout::Size;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::listener::{EventListenerCfg, Poll, PortResult};
use tuirealm::terminal::{TerminalAdapter, TestTerminalAdapter};

use super::redraw::RedrawRequest;
use super::runtime::run_loop;
use super::{App, AppInit, Id, Msg, UserEvent};
use crate::config::{Config, ConfigWarning};
use crate::keymap::Keymap;
use crate::model::HarnessId;
use crate::test_support::{buffer_to_styled, buffer_to_text, render};
use crate::theme::{TerminalEnv, Theme, ThemeRegistry, resolve_by_id};

mod browser;
mod mouse;
mod search;
mod selection;
mod selector;
mod selector_totals;
mod session_panels;
mod session_panels_values;

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
        theme_id: selector::id_of(&theme),
        env: env(),
        config_path: None,
        theme,
        keymap: Keymap::default(),
        harnesses: vec![HarnessId::Claude],
        warnings: Vec::new(),
    }
}

fn warning(message: &str) -> ConfigWarning {
    ConfigWarning {
        key: "k".to_owned(),
        message: message.to_owned(),
    }
}

/// An app with startup warnings and no event ports.
fn app_with_warnings(messages: &[&str]) -> App {
    let mut init = init_with(theme("default"));
    init.warnings = messages.iter().map(|m| warning(m)).collect();
    App::new(init, EventListenerCfg::default(), PathBuf::from("/nowhere")).unwrap()
}

/// An app without event ports: nothing reads the terminal.
fn app_with(theme: Theme) -> App {
    App::new(
        init_with(theme),
        EventListenerCfg::default(),
        PathBuf::from("/nowhere"),
    )
    .unwrap()
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
fn a_transcript_change_asks_for_a_redraw() {
    let mut app = app();
    app.take_redraw();
    app.update(Msg::TranscriptChanged);
    assert_eq!(app.take_redraw(), RedrawRequest::Dirty);
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
    let app = App::new(init, EventListenerCfg::default(), PathBuf::from("/nowhere")).unwrap();
    assert_eq!(app.config(), &Config::default());
    assert_eq!(app.theme(), &theme("default"));
    assert_eq!(app.keymap(), &Keymap::default());
    assert_eq!(app.theme_registry(), &ThemeRegistry::builtin());
    assert_eq!(app.warnings().len(), 1);
}

#[test]
fn startup_mounts_the_components_and_focuses_the_projects_pane() {
    let app = app();
    for id in [
        Id::GlobalEvents,
        Id::ScreenBackground,
        Id::SelectorFrame,
        Id::HarnessTabs,
        Id::ProjectList,
        Id::SessionList,
        Id::SessionTime,
        Id::SessionTokens,
        Id::WorkingDir,
        Id::EmptyState,
        Id::HelpLine,
    ] {
        assert!(app.application().mounted(&id), "{id:?}");
    }
    assert_eq!(app.application().focus(), Some(&Id::ProjectList));
}

// -- toast

#[test]
fn no_warnings_mount_no_toast() {
    let app = app();
    assert!(!app.application().mounted(&Id::Toast));
    assert_eq!(app.toast_text(), None);
}

#[test]
fn one_startup_warning_mounts_a_toast_with_its_text() {
    let app = app_with_warnings(&["unknown key \"x\""]);
    assert!(app.application().mounted(&Id::Toast));
    assert_eq!(app.toast_text(), Some("Config: unknown key \"x\""));
}

#[test]
fn three_startup_warnings_end_the_toast_with_two_more() {
    let app = app_with_warnings(&["first", "second", "third"]);
    let text = app.toast_text().unwrap();
    assert_eq!(text, "Config: first (+2 more, see log)");
    assert!(text.ends_with("(+2 more, see log)"));
}

#[test]
fn the_expiry_message_unmounts_the_toast() {
    let mut app = app_with_warnings(&["w"]);
    app.take_redraw();
    app.update(Msg::ToastExpired(1));
    assert!(!app.application().mounted(&Id::Toast));
    assert_eq!(app.toast_text(), None);
    assert_eq!(app.take_redraw(), RedrawRequest::Dirty);
}

#[test]
fn the_startup_toast_does_not_reappear_on_later_ticks() {
    let mut app = app_with_warnings(&["w"]);
    app.update(Msg::ToastExpired(1));
    app.take_redraw();
    for _ in 0..3 {
        app.update(Msg::Tick);
    }
    assert!(!app.application().mounted(&Id::Toast));
    assert_eq!(app.warnings().len(), 1);
    assert_eq!(app.take_redraw(), RedrawRequest::None);
}

#[test]
fn a_second_expiry_message_is_harmless() {
    let mut app = app_with_warnings(&["w"]);
    app.update(Msg::ToastExpired(1));
    app.take_redraw();
    app.update(Msg::ToastExpired(1));
    assert_eq!(app.take_redraw(), RedrawRequest::None);
}

#[test]
fn the_expiry_of_a_replaced_toast_does_not_remove_the_new_one() {
    let mut app = app_with_warnings(&["old"]);
    // The startup toast has id 1; the new one gets id 2.
    app.show_toast("new").unwrap();
    app.take_redraw();
    app.update(Msg::ToastExpired(1));
    assert!(app.application().mounted(&Id::Toast));
    assert_eq!(app.toast_text(), Some("new"));
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    app.update(Msg::ToastExpired(2));
    assert!(!app.application().mounted(&Id::Toast));
}

#[test]
fn a_new_toast_replaces_the_current_one() {
    let mut app = app_with_warnings(&["old"]);
    app.show_toast("new").unwrap();
    assert_eq!(app.toast_text(), Some("new"));
    assert!(app.application().mounted(&Id::Toast));
    let text = buffer_to_text(&screen(&mut app, 80, 24));
    assert!(text.contains("new"));
    assert!(!text.contains("Config: old"));
}

#[test]
fn a_toast_expires_on_a_tick_event_through_the_application() {
    let mut app = scripted_app(Vec::new());
    app.show_toast_for("copied", Duration::ZERO).unwrap();
    let mut msgs = Vec::new();
    for _ in 0..50 {
        msgs = app.tick().unwrap();
        if msgs.contains(&Msg::ToastExpired(1)) {
            break;
        }
    }
    assert!(msgs.contains(&Msg::ToastExpired(1)), "{msgs:?}");
    for msg in msgs {
        app.update(msg);
    }
    assert!(!app.application().mounted(&Id::Toast));
}

#[test]
fn a_toast_that_has_not_expired_stays_through_ticks() {
    let mut app = scripted_app(Vec::new());
    app.show_toast_for("copied", Duration::from_secs(3600))
        .unwrap();
    for _ in 0..5 {
        for msg in app.tick().unwrap() {
            assert!(!matches!(msg, Msg::ToastExpired(_)));
            app.update(msg);
        }
    }
    assert!(app.application().mounted(&Id::Toast));
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
    App::new(
        init_with(theme("default")),
        listener,
        PathBuf::from("/nowhere"),
    )
    .unwrap()
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

#[test]
fn selector_help_line_120x40_styled() {
    insta::assert_snapshot!(buffer_to_styled(&screen(&mut app(), 120, 40)));
}

#[test]
fn help_line_is_on_row_20_at_80x24_and_36_at_120x40_with_default_keys() {
    let a = buffer_to_text(&screen(&mut app(), 80, 24));
    assert!(
        a.lines()
            .nth(20)
            .unwrap()
            .starts_with("│    / to search sessions | ENTER to open")
    );
    let b = buffer_to_text(&screen(&mut app(), 120, 40));
    assert!(b.lines().nth(36).unwrap().contains(
        "/ to search sessions | ENTER to open | TAB to switch between projects and sessions | ESC to Cancel"
    ));
}

#[test]
fn a_remapped_search_key_shows_in_the_drawn_help_line() {
    let mut raw = crate::config::RawKeys::new();
    raw.entry("selector".to_owned())
        .or_default()
        .insert("search".to_owned(), vec!["f".to_owned()]);
    let (keymap, warnings) = Keymap::from_config(&raw);
    assert!(warnings.is_empty());
    let mut init = init_with(theme("default"));
    init.keymap = keymap;
    let mut app = App::new(init, EventListenerCfg::default(), PathBuf::from("/nowhere")).unwrap();
    let text = buffer_to_text(&screen(&mut app, 120, 40));
    assert!(
        text.lines()
            .nth(36)
            .unwrap()
            .contains("│    f to search sessions | ENTER")
    );
}

#[test]
fn selector_help_line_80x24_text() {
    insta::assert_snapshot!(buffer_to_text(&screen(&mut app(), 80, 24)));
}

#[test]
fn selector_help_line_120x40_text() {
    insta::assert_snapshot!(buffer_to_text(&screen(&mut app(), 120, 40)));
}

#[test]
fn selector_with_toast_80x24_text() {
    let mut app = app_with_warnings(&["unknown key \"x\""]);
    insta::assert_snapshot!(buffer_to_text(&screen(&mut app, 80, 24)));
}

#[test]
fn selector_with_toast_120x40_text() {
    let mut app = app_with_warnings(&["unknown key \"x\""]);
    insta::assert_snapshot!(buffer_to_text(&screen(&mut app, 120, 40)));
}

#[test]
fn selector_with_toast_120x40_styled() {
    let mut app = app_with_warnings(&["unknown key \"x\""]);
    insta::assert_snapshot!(buffer_to_styled(&screen(&mut app, 120, 40)));
}

#[test]
fn the_toast_is_drawn_on_top_of_the_overlay_at_the_planned_place() {
    let mut app = app_with_warnings(&["unknown key \"x\""]);
    let buffer = screen(&mut app, 80, 24);
    let text = buffer_to_text(&buffer);
    let rows: Vec<&str> = text.lines().collect();
    // "Config: unknown key "x"" is 23 columns; the box is 27 wide, ends in
    // column 78 and its bottom border is on row 22.
    assert!(rows[20].contains("╭") && rows[22].chars().nth(78) == Some('╯'));
    assert_eq!(rows[21].chars().nth(52), Some('│'));
    assert_eq!(rows[21].chars().nth(78), Some('│'));
    let theme = theme("default");
    use crate::theme::Role;
    assert_eq!(buffer[(60, 21)].bg, theme.role(Role::Surface));
    assert_eq!(buffer[(60, 21)].fg, theme.role(Role::Foreground));
    assert_eq!(
        buffer[(79, 21)].symbol(),
        "│",
        "the overlay's border is behind column 79"
    );
}

#[test]
fn a_toast_over_the_too_small_screen_is_not_drawn() {
    let mut app = app_with_warnings(&["w"]);
    let text = buffer_to_text(&screen(&mut app, 59, 24));
    assert!(!text.contains("Config"));
}
