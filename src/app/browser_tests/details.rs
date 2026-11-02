//! The details overlay in the app (SPEC §1.4, §5): opening, focus, the keys
//! that work while it is open, scrolling, the mouse and the screens. Keys
//! travel through the app's ports and `GlobalEvents`, as they do in the
//! running application.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use serde_json::json;
use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};
use tuirealm::state::{State, StateValue};

use super::theme_selector::{assert_all_have, mounted_themes};
use super::*;
use crate::clipboard::{Clipboard, CopyOutcome};
use crate::components::transcript::Presentation;
use crate::components::transcript::fixtures::{arc, long_output_turn, with_id};
use crate::components::turn_block::fixtures as blocks;
use crate::components::{HelpContext, INTERIOR_ATTR, PRESENTATION_ATTR};
use crate::config::RawKeys;
use crate::keymap::Keymap;
use crate::model::{TokenUsage, ToolStatus, TurnStats};

const PRESS: MouseEventKind = MouseEventKind::Down(MouseButton::Left);
const WHEEL_DOWN: MouseEventKind = MouseEventKind::ScrollDown;

fn at(kind: MouseEventKind, column: u16, row: u16) -> Msg {
    Msg::Mouse(MouseEvent {
        kind,
        modifiers: KeyModifiers::NONE,
        column,
        row,
    })
}

fn open(app: &App) -> bool {
    app.application().mounted(&Id::DetailsOverlay)
}

fn offset(app: &App) -> usize {
    match app.application().state(&Id::DetailsOverlay).unwrap() {
        State::Single(StateValue::Usize(offset)) => offset,
        other => panic!("{other:?}"),
    }
}

fn interior(app: &App) -> Size {
    let value = app
        .application()
        .query(&Id::DetailsOverlay, INTERIOR_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

fn presentation(app: &App) -> Presentation {
    let value = app
        .application()
        .query(&Id::Transcript, PRESENTATION_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

/// The rows of the overlay as the app draws them at `width` x `height`.
fn overlay_rows(rig: &mut Rig, width: u16, height: u16) -> Vec<String> {
    let area = rig.app.routing().area(Id::DetailsOverlay).unwrap();
    screen_text(rig, width, height)
        .lines()
        .skip(usize::from(area.y))
        .take(usize::from(area.height))
        .map(|row| {
            row.chars()
                .skip(usize::from(area.x))
                .take(usize::from(area.width))
                .collect()
        })
        .collect()
}

impl Rig {
    /// Presses the details key and waits until the overlay is up.
    fn open_details(&mut self) {
        self.press(Key::Char('i'));
        self.pump_until(open);
    }

    fn close_details_by(&mut self, code: Key) {
        self.press(code);
        self.pump_until(|app| !open(app));
    }

    /// Moves the selection down `n` sections.
    fn down(&self, n: usize) {
        for _ in 0..n {
            self.press(Key::Char('j'));
        }
    }

    /// Opens the BRIO session with `turns`, fully loaded.
    fn open_with(&mut self, turns: Vec<Arc<Turn>>) {
        self.harness.script(
            BRIO_SESSION,
            vec![
                ModelUpdate::Appended(turns),
                ModelUpdate::Progress(1.0),
                ModelUpdate::ParseWarnings(0),
                ModelUpdate::Loaded,
            ],
        );
        self.open(BRIO_SESSION);
    }
}

/// A clipboard that records what it is given.
struct Recorder(Rc<RefCell<Vec<String>>>);

impl Clipboard for Recorder {
    fn copy(&mut self, text: &str) -> CopyOutcome {
        self.0.borrow_mut().push(text.to_owned());
        CopyOutcome {
            system: Ok(()),
            osc52: None,
        }
    }
}

fn keymap_with(context: &str, action: &str, key: &str) -> Keymap {
    let mut raw = RawKeys::new();
    raw.entry(context.to_owned())
        .or_default()
        .insert(action.to_owned(), vec![key.to_owned()]);
    let (keymap, warnings) = Keymap::from_config(&raw);
    assert!(warnings.is_empty(), "{warnings:?}");
    keymap
}

fn rig_with_keymap(keymap: Keymap) -> Rig {
    let mut init = init(&[CLAUDE], theme("default"));
    init.keymap = keymap;
    build_rig_from(init, 120, 40, false)
}

/// An Assistant turn with every token figure and a cost, holding a text, a
/// timed tool call and a failing one: its details are taller than a small
/// terminal.
fn rich_turn() -> Arc<Turn> {
    let stats = TurnStats {
        duration: Duration::from_secs(243),
        message_count: 12,
        tokens: Some(TokenUsage {
            input: 2_800,
            output: 42_400,
            cache_read: 359_400_000,
            cache_write: 1_200,
            reasoning: 9_000,
        }),
        cost: Some(0.42),
    };
    arc(with_id(
        blocks::assistant(
            Some("high"),
            vec![
                blocks::text("x1", "Running it."),
                blocks::with_duration(
                    blocks::tool(
                        "c1",
                        "Run the tests",
                        json!({"command": "cargo test"}),
                        Some(("ok", false)),
                        ToolStatus::Ok,
                    ),
                    62_000,
                ),
            ],
            Some(stats),
        ),
        "a1",
    ))
}

// -- opening and closing

#[test]
fn the_details_key_opens_the_overlay_for_the_selected_section_and_gives_it_the_focus() {
    let mut rig = rig();
    rig.open_loaded();
    assert_eq!(rig.focus(), Some(Id::Transcript));
    rig.press(Key::Char('i'));
    let seen = rig.pump_until(open);
    assert!(
        seen.iter().any(|m| matches!(m, Msg::OpenDetails(_))),
        "{seen:?}"
    );
    assert_eq!(rig.focus(), Some(Id::DetailsOverlay));
    assert_eq!(rig.app.screen(), Screen::Browser);
    for id in BROWSER_IDS {
        assert!(rig.app.application().mounted(&id), "{id:?}");
    }
    // The first selected section of the fixture is the first prompt.
    let rows = overlay_rows(&mut rig, 120, 40).join("\n");
    assert!(rows.contains("╭─ Details"), "{rows}");
    assert!(rows.contains("Prompt"), "{rows}");
    assert!(rows.contains("timestamp"), "{rows}");
    assert!(!rows.contains("Tool call"), "{rows}");
}

#[test]
fn the_overlay_shows_the_section_that_was_selected() {
    let mut rig = rig();
    rig.open_loaded();
    // Down to the first tool call of the second turn.
    rig.down(3);
    rig.open_details();
    let rows = overlay_rows(&mut rig, 120, 40).join("\n");
    assert!(rows.contains("Turn"), "{rows}");
    assert!(rows.contains("claude-opus-5.5"), "{rows}");
    assert!(rows.contains("Tool call"), "{rows}");
    assert!(rows.contains("status"), "{rows}");
}

#[test]
fn esc_closes_the_overlay_and_returns_the_focus_to_the_transcript() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_details();
    rig.close_details_by(Key::Esc);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!rig.app.should_quit());
    assert_eq!(rig.app.screen(), Screen::Browser);
}

#[test]
fn the_details_key_closes_it_too_and_does_not_reopen_it() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_details();
    rig.press(Key::Char('i'));
    let seen = rig.pump_until(|app| !open(app));
    assert!(seen.contains(&Msg::CloseDetails), "{seen:?}");
    assert!(
        !seen.iter().any(|m| matches!(m, Msg::OpenDetails(_))),
        "the same key must not reopen it: {seen:?}"
    );
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // Once closed it opens again.
    rig.open_details();
    assert_eq!(rig.focus(), Some(Id::DetailsOverlay));
}

#[test]
fn the_selection_and_the_scroll_of_the_transcript_are_unchanged_by_the_overlay() {
    let mut rig = rig();
    rig.open_loaded();
    rig.down(2);
    rig.settle();
    let before = presentation(&rig.app);
    rig.open_details();
    for key in [Key::Char('j'), Key::Down, Key::PageDown, Key::Char('k')] {
        rig.press(key);
    }
    rig.settle();
    rig.close_details_by(Key::Esc);
    assert_eq!(presentation(&rig.app), before);
}

#[test]
fn a_more_row_opens_the_details_of_its_tool_call() {
    let mut rig = rig();
    rig.open_with(vec![long_output_turn(1434)]);
    // The only section is the folded tool call: unfold it, then down to the
    // `… N more lines` row.
    let mut details = Vec::new();
    rig.open_details();
    details.push(overlay_rows(&mut rig, 120, 40));
    rig.close_details_by(Key::Esc);
    rig.press(Key::Char('l'));
    rig.press(Key::Char('j'));
    rig.settle();
    rig.open_details();
    details.push(overlay_rows(&mut rig, 120, 40));
    assert_eq!(details[0], details[1]);
    let rows = details[1].join("\n");
    assert!(rows.contains("Tool call"), "{rows}");
}

#[test]
fn with_no_selection_the_key_does_nothing() {
    let mut rig = rig();
    rig.open_with(Vec::new());
    assert_eq!(rig.turn_count(), 0);
    rig.press(Key::Char('i'));
    let seen = rig.settle();
    assert!(!open(&rig.app));
    assert!(
        !seen.iter().any(|m| matches!(m, Msg::OpenDetails(_))),
        "{seen:?}"
    );
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn an_open_request_while_open_or_over_another_overlay_changes_nothing() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_details();
    let before = overlay_rows(&mut rig, 120, 40);
    rig.down(0);
    rig.app.update(Msg::OpenHelp(HelpContext::Browser));
    rig.app.update(Msg::OpenThemeSelector);
    assert!(!rig.app.application().mounted(&Id::HelpOverlay));
    assert!(!rig.app.application().mounted(&Id::ThemeSelector));
    assert_eq!(rig.focus(), Some(Id::DetailsOverlay));
    assert_eq!(overlay_rows(&mut rig, 120, 40), before);
    rig.app.update(Msg::CloseDetails);
    assert!(!open(&rig.app));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // A close without an overlay is harmless.
    rig.app.update(Msg::CloseDetails);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // And the help overlay refuses to open over it the other way round.
    rig.app.update(Msg::OpenHelp(HelpContext::Browser));
    let content = crate::components::DetailsContent::default();
    rig.app.update(Msg::OpenDetails(content));
    assert!(!open(&rig.app));
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));
}

#[test]
fn opening_another_session_drops_the_overlay() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_details();
    rig.app.update(Msg::OpenSession(CLAUDE, id(NEWER_SESSION)));
    assert!(!open(&rig.app));
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

// -- remapping

#[test]
fn a_remapped_details_key_opens_and_closes_the_overlay() {
    let mut rig = rig_with_keymap(keymap_with("browser", "details", "x"));
    rig.open_loaded();
    rig.press(Key::Char('i'));
    let seen = rig.settle();
    assert!(!open(&rig.app), "the old key does nothing: {seen:?}");
    rig.press(Key::Char('x'));
    rig.pump_until(open);
    // The old key does not close it.
    rig.press(Key::Char('i'));
    rig.settle();
    assert!(open(&rig.app));
    rig.close_details_by(Key::Char('x'));
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn the_help_overlay_shows_the_remapped_details_key() {
    let mut rig = rig_with_keymap(keymap_with("browser", "details", "x"));
    rig.open_loaded();
    rig.press(Key::Char('?'));
    rig.pump_until(|app| app.application().mounted(&Id::HelpOverlay));
    // The overlay is taller than the screen: scroll to the Sections group.
    let mut found = None;
    for _ in 0..4 {
        let screen = screen_text(&mut rig, 120, 40);
        found = screen
            .lines()
            .find(|row| row.contains("Show section details"))
            .map(str::to_owned);
        if found.is_some() {
            break;
        }
        rig.press(Key::PageDown);
        rig.settle();
    }
    let row = found.expect("the help lists the action");
    let keys = row.split("Show section details").next().unwrap();
    assert!(keys.trim_end().ends_with(" x"), "{row}");
    assert!(!keys.contains(" i"), "{row}");
}

#[test]
fn the_help_overlay_lists_the_default_details_key() {
    let mut rig = rig();
    rig.open_loaded();
    rig.press(Key::Char('?'));
    rig.pump_until(|app| app.application().mounted(&Id::HelpOverlay));
    let screen = screen_text(&mut rig, 200, 60);
    let row = screen
        .lines()
        .find(|row| row.contains("Show section details"))
        .expect("the help lists the action");
    assert!(row.contains("│    i  "), "{row}");
}

// -- a snapshot

#[test]
fn what_it_shows_does_not_change_when_the_turn_does() {
    let mut rig = rig();
    rig.open_loaded();
    // The first section of the second turn: its text.
    rig.down(1);
    rig.settle();
    rig.open_details();
    let before = overlay_rows(&mut rig, 120, 40);
    assert!(before.iter().any(|row| row.contains("5m 2s")), "{before:?}");
    // The turn changes while the overlay is open: a longer duration.
    let mut changed = (*turns()[1]).clone();
    let Turn::Assistant(assistant) = &mut changed else {
        panic!("the second turn is the assistant's");
    };
    assistant.stats.as_mut().unwrap().duration = Duration::from_secs(10 * 60);
    let update = session_msg(&rig.app, ModelUpdate::Updated(vec![Arc::new(changed)]));
    rig.app.update(update);
    assert_eq!(overlay_rows(&mut rig, 120, 40), before);
    // Control: the transcript did take the update, and a new overlay shows
    // it.
    rig.close_details_by(Key::Esc);
    rig.open_details();
    let after = overlay_rows(&mut rig, 120, 40);
    assert_ne!(after, before);
    assert!(after.iter().any(|row| row.contains("10m 0s")), "{after:?}");
}

// -- copy, save and export

#[test]
fn copy_save_and_export_do_nothing_while_it_is_open() {
    let mut rig = rig();
    rig.open_loaded();
    let copies = Rc::new(RefCell::new(Vec::new()));
    rig.app
        .set_clipboard(Box::new(Recorder(Rc::clone(&copies))), None);
    rig.down(3);
    rig.settle();
    // Control: with the overlay closed `y` copies the section.
    rig.press(Key::Char('y'));
    rig.pump_until(|app| app.toast_text().is_some());
    let copied = copies.borrow().clone();
    assert_eq!(copied.len(), 1);
    for label in ["status", "model", "input tokens", "Tool call"] {
        assert!(!copied[0].contains(label), "{label:?} in {}", copied[0]);
    }
    copies.borrow_mut().clear();

    rig.open_details();
    for (code, modifiers) in [
        (Key::Char('y'), KeyModifiers::NONE),
        (Key::Char('Y'), KeyModifiers::SHIFT),
        (Key::Char('w'), KeyModifiers::NONE),
        (Key::Char('w'), KeyModifiers::CONTROL),
        (Key::Char('W'), KeyModifiers::SHIFT),
        (Key::Char('e'), KeyModifiers::NONE),
        (Key::Char('e'), KeyModifiers::CONTROL),
        (Key::Char('E'), KeyModifiers::SHIFT),
    ] {
        rig.key(code, modifiers);
    }
    let seen = rig.settle();
    assert!(
        !seen.iter().any(|m| matches!(
            m,
            Msg::CopyText(_) | Msg::SaveSection { .. } | Msg::Export { .. }
        )),
        "{seen:?}"
    );
    assert!(copies.borrow().is_empty());
    assert!(!rig.app.write_prompt_open());
    assert!(open(&rig.app));
    // And what is copied afterwards is the section, not the overlay.
    rig.close_details_by(Key::Esc);
    rig.press(Key::Char('y'));
    rig.pump_until(|_| !copies.borrow().is_empty());
    assert_eq!(copies.borrow()[0], copied[0]);
}

// -- the keys of the app

#[test]
fn the_global_keys_do_nothing_while_it_is_open() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_details();
    rig.press(Key::Char('q'));
    rig.press(Key::Char('t'));
    rig.press(Key::Char('?'));
    rig.press(Key::Char('/'));
    rig.ctrl('q');
    rig.ctrl('w');
    rig.ctrl('o');
    let seen = rig.settle();
    assert!(!rig.app.should_quit(), "{seen:?}");
    assert!(
        !seen.iter().any(|m| matches!(
            m,
            Msg::Quit | Msg::OpenSelector | Msg::OpenSearch | Msg::OpenHelp(_)
        ) || matches!(m, Msg::OpenThemeSelector)),
        "{seen:?}"
    );
    assert!(open(&rig.app));
    assert!(!rig.app.application().mounted(&Id::HelpOverlay));
    assert!(!rig.app.application().mounted(&Id::ThemeSelector));
    assert_eq!(rig.focus(), Some(Id::DetailsOverlay));
    assert_eq!(rig.app.screen(), Screen::Browser);
    // Control: once it is closed the same key quits.
    rig.press(Key::Esc);
    rig.pump_until(|app| !open(app));
    rig.press(Key::Char('q'));
    rig.pump_until(App::should_quit);
}

#[test]
fn ctrl_c_quits_from_the_overlay() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_details();
    rig.ctrl('c');
    rig.pump_until(App::should_quit);
}

#[test]
fn the_scroll_keys_move_the_overlay_through_the_app() {
    let mut rig = rig_with(theme("default"), 60, 20);
    rig.open_with(vec![rich_turn()]);
    rig.down(1);
    rig.settle();
    rig.open_details();
    // The details of a tool call with every figure: 20 rows in 16.
    assert_eq!(interior(&rig.app).height, 16);
    assert_eq!(offset(&rig.app), 0);
    rig.press(Key::Down);
    rig.press(Key::Char('j'));
    rig.pump_until(|app| offset(app) == 2);
    rig.press(Key::Up);
    rig.press(Key::Char('k'));
    rig.pump_until(|app| offset(app) == 0);
    rig.press(Key::PageDown);
    rig.pump_until(|app| offset(app) == 4);
    rig.press(Key::PageDown);
    rig.settle();
    assert_eq!(offset(&rig.app), 4, "clamped at the end: 20 - 16");
    rig.press(Key::PageUp);
    rig.pump_until(|app| offset(app) == 0);
    assert!(open(&rig.app));
}

#[test]
fn the_overlay_starts_at_the_top_each_time() {
    let mut rig = rig_with(theme("default"), 60, 20);
    rig.open_with(vec![rich_turn()]);
    rig.down(1);
    rig.open_details();
    rig.press(Key::PageDown);
    rig.pump_until(|app| offset(app) > 0);
    rig.close_details_by(Key::Esc);
    rig.open_details();
    assert_eq!(offset(&rig.app), 0);
}

#[test]
fn the_interior_follows_the_overlay_and_the_terminal_size() {
    let mut rig = rig_with(theme("default"), 60, 20);
    rig.open_with(vec![rich_turn()]);
    rig.down(1);
    rig.open_details();
    // 20 rows: 18 for the overlay, 16 inside.
    assert_eq!(interior(&rig.app).height, 16);
    rig.app.update(Msg::Resized(100, 40));
    assert_eq!(interior(&rig.app).height, 20, "all 20 rows fit");
    rig.app.update(Msg::Resized(60, 20));
    assert_eq!(interior(&rig.app).height, 16);
}

// -- the mouse

#[test]
fn the_wheel_scrolls_the_overlay_and_a_press_outside_closes_it() {
    let mut rig = rig_with(theme("default"), 60, 20);
    rig.open_with(vec![rich_turn()]);
    rig.down(1);
    rig.settle();
    let transcript = presentation(&rig.app);
    rig.open_details();
    let area = rig.app.routing().area(Id::DetailsOverlay).unwrap();
    let inside = (area.x + 5, area.y + 5);
    rig.app.update(at(WHEEL_DOWN, inside.0, inside.1));
    assert_eq!(offset(&rig.app), 3);
    rig.app.update(at(WHEEL_DOWN, inside.0, inside.1));
    assert_eq!(offset(&rig.app), 4, "clamped to the last row that fits");
    rig.app
        .update(at(MouseEventKind::ScrollUp, inside.0, inside.1));
    assert_eq!(offset(&rig.app), 1);
    // A press inside does nothing.
    rig.app.update(at(PRESS, inside.0, inside.1));
    assert!(open(&rig.app));
    assert_eq!(rig.focus(), Some(Id::DetailsOverlay));
    // The wheel outside is ignored.
    rig.app.update(at(WHEEL_DOWN, 0, 0));
    assert_eq!(offset(&rig.app), 1);
    assert_eq!(presentation(&rig.app), transcript);
    // A press outside closes it and returns the focus.
    assert!(!area.contains((0, 0).into()));
    rig.app.update(at(PRESS, 0, 0));
    assert!(!open(&rig.app));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.app.screen(), Screen::Browser);
}

#[test]
fn nothing_under_the_overlay_gets_the_mouse() {
    let mut rig = rig();
    rig.open_loaded();
    rig.down(1);
    rig.settle();
    let transcript = presentation(&rig.app);
    rig.open_details();
    let area = rig.app.routing().area(Id::DetailsOverlay).unwrap();
    let covered = rig.app.routing().area(Id::Transcript).unwrap();
    assert!(covered.intersects(area));
    for (dx, dy) in [(0, 0), (3, 3), (area.width - 1, area.height - 1)] {
        rig.app.update(at(PRESS, area.x + dx, area.y + dy));
        rig.app.update(at(WHEEL_DOWN, area.x + dx, area.y + dy));
    }
    assert!(open(&rig.app));
    assert_eq!(rig.focus(), Some(Id::DetailsOverlay));
    assert_eq!(presentation(&rig.app), transcript);
}

#[test]
fn with_mouse_off_the_overlay_takes_no_mouse() {
    let mut init = init(&[CLAUDE], theme("default"));
    init.config.mouse = false;
    let mut rig = build_rig_from(init, 60, 20, false);
    rig.open_with(vec![rich_turn()]);
    rig.down(1);
    rig.open_details();
    let area = rig.app.routing().area(Id::DetailsOverlay).unwrap();
    rig.app.update(at(WHEEL_DOWN, area.x + 5, area.y + 5));
    assert_eq!(offset(&rig.app), 0);
    rig.app.update(at(PRESS, 0, 0));
    assert!(open(&rig.app));
}

// -- the theme

#[test]
fn the_overlay_gets_the_theme_that_is_chosen_while_it_is_open() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_details();
    rig.app.apply_theme_id("nord");
    assert_all_have(&rig.app, &theme("nord"), 4);
    assert!(
        mounted_themes(&rig.app)
            .iter()
            .any(|(id, _)| *id == Id::DetailsOverlay)
    );
}

// -- screens

fn screen_with_details(width: u16, height: u16, theme_name: &str, down: usize) -> String {
    let mut rig = rig_with(theme(theme_name), width, height);
    rig.open_loaded();
    rig.down(down);
    rig.settle();
    rig.open_details();
    screen_text(&mut rig, width, height)
}

#[test]
fn the_details_over_the_browser_120x40_text() {
    // The first tool call of the second turn.
    insta::assert_snapshot!(screen_with_details(120, 40, "default", 3));
}

#[test]
fn the_details_over_the_browser_80x24_text() {
    insta::assert_snapshot!(screen_with_details(80, 24, "default", 3));
}
