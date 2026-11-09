//! The help overlay in the app (architecture.md#events-focus-and-keys,
//! product §3.3): opening, focus, which keys work while it is open, and the
//! screens. Keys travel through the app's ports and `GlobalEvents`, as they
//! do in the running application.

use super::*;
use crate::components::{HelpContext, INTERIOR_ATTR};
use crate::config::RawKeys;
use crate::keymap::Keymap;

impl Rig {
    /// Presses `?` and waits until the overlay is up.
    pub(super) fn open_help(&mut self) {
        self.press(Key::Char('?'));
        self.pump_until(|app| app.application().mounted(&Id::HelpOverlay));
    }

    fn help_open(&self) -> bool {
        self.app.application().mounted(&Id::HelpOverlay)
    }

    pub(super) fn ctrl(&self, c: char) {
        self.key(Key::Char(c), KeyModifiers::CONTROL);
    }
}

fn rig_with_keymap(keymap: Keymap, mouse: bool) -> Rig {
    let mut init = init(&[CLAUDE], theme("default"));
    init.keymap = keymap;
    init.config.mouse = mouse;
    build_rig_from(init, 120, 40, false)
}

fn copy_on_c() -> Keymap {
    let mut raw = RawKeys::new();
    raw.entry("browser".to_owned())
        .or_default()
        .insert("copy".to_owned(), vec!["c".to_owned()]);
    let (keymap, warnings) = Keymap::from_config(&raw);
    assert!(warnings.is_empty(), "{warnings:?}");
    keymap
}

fn interior(app: &App) -> Size {
    let value = app
        .application()
        .query(&Id::HelpOverlay, INTERIOR_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

fn offset(app: &App) -> usize {
    match app.application().state(&Id::HelpOverlay).unwrap() {
        tuirealm::state::State::Single(tuirealm::state::StateValue::Usize(offset)) => offset,
        other => panic!("{other:?}"),
    }
}

// -- opening and closing

#[test]
fn the_help_key_in_the_browser_opens_the_overlay_and_gives_it_the_focus() {
    let mut rig = rig();
    rig.open_loaded();
    assert_eq!(rig.focus(), Some(Id::Transcript));
    rig.press(Key::Char('?'));
    let seen = rig.pump_until(|app| app.application().mounted(&Id::HelpOverlay));
    assert!(
        seen.contains(&Msg::Overlay(OverlayMsg::OpenHelp(HelpContext::Browser))),
        "{seen:?}"
    );
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));
    assert_eq!(rig.app.screen(), Screen::Browser);
    // The browser stays mounted under it.
    for id in BROWSER_IDS {
        assert!(rig.app.application().mounted(&id), "{id:?}");
    }
}

#[test]
fn esc_closes_the_overlay_and_returns_the_focus_to_the_transcript() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_help();
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!rig.app.should_quit());
}

#[test]
fn the_help_key_closes_the_overlay_too() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_help();
    rig.press(Key::Char('?'));
    let seen = rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    assert!(
        seen.contains(&Msg::Overlay(OverlayMsg::CloseHelp)),
        "{seen:?}"
    );
    assert!(
        !seen
            .iter()
            .any(|m| matches!(m, Msg::Overlay(OverlayMsg::OpenHelp(_)))),
        "the same key must not reopen it: {seen:?}"
    );
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // And once closed it opens again.
    rig.open_help();
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));
}

#[test]
fn quit_and_theme_keys_do_nothing_while_it_is_open() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_help();
    rig.press(Key::Char('q'));
    rig.press(Key::Char('t'));
    rig.ctrl('q');
    rig.ctrl('w');
    rig.ctrl('o');
    rig.press(Key::Char('/'));
    let seen = rig.settle();
    assert!(!rig.app.should_quit(), "{seen:?}");
    assert!(
        !seen.iter().any(|m| matches!(
            m,
            Msg::Quit
                | Msg::Browser(BrowserMsg::OpenSelector)
                | Msg::Transcript(TranscriptMsg::OpenSearch)
        )),
        "{seen:?}"
    );
    assert!(rig.help_open());
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));
    assert_eq!(rig.app.screen(), Screen::Browser);
    // Control: once it is closed the same key quits.
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    rig.press(Key::Char('q'));
    rig.pump_until(App::should_quit);
}

#[test]
fn ctrl_c_quits_from_the_overlay() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_help();
    rig.ctrl('c');
    rig.pump_until(App::should_quit);
}

#[test]
fn a_second_open_request_changes_nothing() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_help();
    rig.app
        .update(Msg::Overlay(OverlayMsg::OpenHelp(HelpContext::Browser)));
    rig.app
        .update(Msg::Overlay(OverlayMsg::OpenHelp(HelpContext::Selector)));
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));
    rig.app.update(Msg::Overlay(OverlayMsg::CloseHelp));
    assert!(!rig.help_open());
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // A close without an overlay is harmless.
    rig.app.update(Msg::Overlay(OverlayMsg::CloseHelp));
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn the_help_key_in_the_startup_selector_opens_the_selectors_help() {
    let mut rig = rig();
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    rig.press(Key::Char('?'));
    let seen = rig.pump_until(|app| app.application().mounted(&Id::HelpOverlay));
    assert!(
        seen.contains(&Msg::Overlay(OverlayMsg::OpenHelp(HelpContext::Selector))),
        "{seen:?}"
    );
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));
    rig.press(Key::Char('q'));
    rig.press(Key::Char('t'));
    assert!(!rig.settle().contains(&Msg::Quit));
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    assert_eq!(rig.app.screen(), Screen::Selector);
}

#[test]
fn the_focus_returns_to_the_pane_that_had_it() {
    let mut rig = rig();
    rig.press(Key::Tab);
    rig.pump_until(|app| app.focused_pane() == crate::app::Pane::Sessions);
    rig.open_help();
    rig.press(Key::Char('?'));
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::SessionList));
}

#[test]
fn the_selector_over_the_browser_opens_the_selectors_help_and_gives_back_both_focuses() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.press(Key::Char('?'));
    let seen = rig.pump_until(|app| app.application().mounted(&Id::HelpOverlay));
    assert!(
        seen.contains(&Msg::Overlay(OverlayMsg::OpenHelp(HelpContext::Selector))),
        "{seen:?}"
    );
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::SessionList));
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    // The next Esc closes the selector, as it did without the overlay.
    rig.press(Key::Esc);
    rig.pump_until(|app| app.screen() == Screen::Browser);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // And the help of the browser is the browser's again.
    rig.press(Key::Char('?'));
    let seen = rig.pump_until(|app| app.application().mounted(&Id::HelpOverlay));
    assert!(
        seen.contains(&Msg::Overlay(OverlayMsg::OpenHelp(HelpContext::Browser))),
        "{seen:?}"
    );
}

#[test]
fn the_question_mark_is_text_in_a_prompt() {
    let mut rig = rig();
    rig.press(Key::Char('/'));
    rig.pump_until(|app| app.application().mounted(&Id::Prompt));
    rig.press(Key::Char('?'));
    rig.settle();
    assert!(!rig.help_open());
    assert_eq!(rig.focus(), Some(Id::Prompt));
}

#[test]
fn opening_another_session_drops_the_overlay() {
    let mut rig = rig();
    rig.open_help();
    rig.app.update(Msg::Browser(BrowserMsg::OpenSession(
        CLAUDE,
        id(BRIO_SESSION),
    )));
    assert!(!rig.help_open());
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

// -- scrolling

#[test]
fn the_interior_follows_the_overlay_and_the_terminal_size() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_help();
    // 40 rows: 38 for the overlay, 36 inside; 72 columns wide, 70 inside.
    assert_eq!(interior(&rig.app), Size::new(70, 36));
    rig.app.update(Msg::Resized(80, 24));
    assert_eq!(interior(&rig.app), Size::new(70, 20));
    rig.app.update(Msg::Resized(200, 60));
    // The overlay wants 63 rows (61 inside), more than the screen has.
    assert_eq!(interior(&rig.app), Size::new(70, 56));
    rig.app.update(Msg::Resized(200, 70));
    assert_eq!(interior(&rig.app), Size::new(70, 61));
}

#[test]
fn the_scroll_keys_move_the_overlay_through_the_app() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_help();
    assert_eq!(offset(&rig.app), 0);
    rig.press(Key::Down);
    rig.press(Key::Char('j'));
    rig.pump_until(|app| offset(app) == 2);
    rig.press(Key::Up);
    rig.press(Key::Char('k'));
    rig.pump_until(|app| offset(app) == 0);
    rig.press(Key::PageDown);
    rig.pump_until(|app| offset(app) == 25);
    rig.press(Key::PageDown);
    rig.settle();
    assert_eq!(offset(&rig.app), 25, "clamped at the end: 61 - 36");
    rig.press(Key::PageUp);
    rig.pump_until(|app| offset(app) == 0);
    // The transcript behind did not scroll or select.
    assert!(rig.help_open());
}

#[test]
fn the_overlay_starts_at_the_top_each_time() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_help();
    rig.press(Key::PageDown);
    rig.pump_until(|app| offset(app) > 0);
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    rig.open_help();
    assert_eq!(offset(&rig.app), 0);
}

// -- screens

fn open_at(width: u16, height: u16) -> Rig {
    let mut rig = rig();
    rig.app.update(Msg::Resized(width, height));
    rig.open_loaded();
    rig.open_help();
    rig
}

fn scroll_to_end(rig: &mut Rig) {
    for _ in 0..4 {
        rig.press(Key::PageDown);
    }
    // 61 rows in a 36 row interior: the last offset.
    rig.pump_until(|app| offset(app) == 25);
}

#[test]
fn the_browser_help_120x40_top_text() {
    let mut rig = open_at(120, 40);
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn the_browser_help_120x40_top_styled() {
    let mut rig = open_at(120, 40);
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

#[test]
fn the_browser_help_120x40_end_text() {
    let mut rig = open_at(120, 40);
    scroll_to_end(&mut rig);
    let text = screen_text(&mut rig, 120, 40);
    let rows: Vec<&str> = text.lines().collect();
    assert!(rows[1].contains(" ↑ more ─╮"), "{}", rows[1]);
    assert!(!rows[38].contains("↓ more"), "{}", rows[38]);
    assert!(
        rows[37].contains("Hold Shift to select text with the mouse"),
        "{}",
        rows[37]
    );
    insta::assert_snapshot!(text);
}

#[test]
fn the_browser_help_80x24_text() {
    let mut rig = open_at(80, 24);
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn the_browser_help_200x60_text() {
    let mut rig = open_at(200, 60);
    let text = screen_text(&mut rig, 200, 60);
    // The 63 rows the help wants do not fit in 58: it scrolls.
    assert!(text.contains("↓ more"), "the end is below");
    assert!(text.contains("Turns sidebar"));
    insta::assert_snapshot!(text);
}

#[test]
fn the_selector_help_120x40_text() {
    let mut rig = rig();
    rig.open_help();
    let text = screen_text(&mut rig, 120, 40);
    assert!(text.contains("Session selector"));
    assert!(text.contains("Switch to Claude"));
    assert!(!text.contains("Switch to OpenCode"));
    assert!(!text.contains("↓ more") && !text.contains("↑ more"));
    insta::assert_snapshot!(text);
}

#[test]
fn the_selector_help_is_drawn_over_the_selector() {
    let mut rig = rig();
    rig.open_help();
    let text = screen_text(&mut rig, 120, 40);
    // The selector's panes, Working Directory and help line are around it.
    for around in [
        "[1] Claude",
        "Sessions in BRIO",
        "Working Directory",
        "to search sessions",
    ] {
        assert!(text.contains(around), "{around}");
    }
    assert!(text.contains("╭─ Keybindings ─"));
}

#[test]
fn a_remapped_copy_key_shows_in_the_drawn_overlay_120x40_text() {
    let mut rig = rig_with_keymap(copy_on_c(), true);
    rig.open_loaded();
    rig.open_help();
    let text = screen_text(&mut rig, 120, 40);
    let copy = text
        .lines()
        .find(|row| row.contains("Copy section as displayed"))
        .unwrap();
    assert!(copy.contains("│    c  "), "{copy}");
    assert!(!copy.contains("│    y "), "{copy}");
    insta::assert_snapshot!(text);
}

#[test]
fn the_shift_note_is_drawn_only_when_the_mouse_is_on() {
    for (mouse, shown) in [(true, true), (false, false)] {
        let mut rig = rig_with_keymap(Keymap::default(), mouse);
        rig.app.update(Msg::Resized(200, 70));
        rig.open_loaded();
        rig.open_help();
        let text = screen_text(&mut rig, 200, 70);
        assert_eq!(text.contains("Hold Shift to select text"), shown, "{mouse}");
    }
}

#[test]
fn a_toast_is_drawn_over_the_help_overlay() {
    let mut rig = rig();
    rig.app.update(Msg::Resized(80, 24));
    rig.open_loaded();
    rig.open_help();
    rig.app.show_toast("copied").unwrap();
    let rows = rows_of(&screen_buffer(&mut rig, 80, 24));
    assert!(rows[1].contains("Keybindings"));
    // The overlay covers columns 4..76; the toast box (10 columns, ending in
    // column 78) is over its right border and edge.
    let toast: String = rows[21].chars().skip(69).take(10).collect();
    assert_eq!(toast, "│ copied │", "{}", rows[21]);
}

#[test]
fn the_help_is_hidden_on_a_screen_that_is_too_small_and_back_when_it_grows() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_help();
    let small = screen_text(&mut rig, 50, 15);
    assert!(small.contains("Terminal too small"));
    assert!(!small.contains("Keybindings"));
    assert!(rig.help_open());
    let big = screen_text(&mut rig, 120, 40);
    assert!(big.contains("Keybindings"));
    assert_eq!(interior(&rig.app), Size::new(70, 36));
}

#[test]
fn the_overlay_is_centred() {
    let mut rig = open_at(120, 40);
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    // 72 wide, 38 high: columns 24..96, rows 1..39.
    assert_eq!(rows[1].chars().nth(24), Some('╭'));
    assert_eq!(rows[1].chars().nth(95), Some('╮'));
    assert_eq!(rows[38].chars().nth(24), Some('╰'));
}
