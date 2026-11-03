//! The turns sidebar's keys and focus (SPEC §6.1, §6.3, §6.4): the keys that
//! hand the focus to it and back, what selecting an entry does to the
//! transcript, the focus while overlays are open and when the sidebar is
//! hidden, the help line's hints, and the screens with the sidebar focused.
//!
//! Keys travel through the app's ports and `GlobalEvents`, as they do in the
//! running application.

use ratatui::layout::Size;
use tuirealm::command::{Cmd, Direction};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::Attribute;

use super::sidebar::sidebar_rows;
use super::sidebar_follow::{long_rig, long_turns, section, sidebar_entry};
use super::*;
use crate::components::transcript::fixtures::{arc, prompt, reply, transcript_of};
use crate::components::transcript::{GO_TO_TURN_ATTR, Presentation, Transcript};
use crate::components::{FOLLOWING_ATTR, HINTS_ATTR, Hint, PRESENTATION_ATTR, payload_value};
use crate::config::RawKeys;
use crate::keymap::Keymap;
use crate::model::TurnId;

// -- what the tests read

fn presentation(app: &App) -> Presentation {
    let value = app
        .application()
        .query(&app.top_transcript(), PRESENTATION_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

fn following(app: &App) -> bool {
    let value = app
        .application()
        .query(&app.top_transcript(), FOLLOWING_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    value == AttrValue::Flag(true)
}

/// The hints in the browser's help line, read the way they are drawn.
fn hint_line(app: &App) -> String {
    let value = app
        .application()
        .query(&Id::BrowserHelpLine, HINTS_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    let hints: Vec<Hint> = payload_from_value(&value).unwrap();
    hints
        .iter()
        .map(|h| format!("{} {}", h.key, h.text))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// What the pane is told about its focus.
fn sidebar_flag(app: &App) -> bool {
    let value = app
        .application()
        .query(&Id::TurnSidebar, Attribute::Focus)
        .unwrap()
        .unwrap()
        .into_attr();
    value == AttrValue::Flag(true)
}

const BROWSER_HINTS: &str =
    "/ to search | ENTER to fold or unfold sections | y to copy current | ? for keybindings";
const SIDEBAR_HINTS: &str = "ENTER to go to block | TAB to switch to transcript";

impl Rig {
    /// Presses `1` and waits until the sidebar has the focus.
    pub(super) fn focus_sidebar(&mut self) {
        self.press(Key::Char('1'));
        self.pump_until(|app| app.application().focus() == Some(&Id::TurnSidebar));
    }

    /// Presses `code` and waits until the transcript has the focus.
    fn leave_sidebar_by(&mut self, code: Key, modifiers: KeyModifiers) {
        self.key(code, modifiers);
        self.pump_until(|app| app.application().focus() == Some(&app.top_transcript()));
    }

    /// Presses `j` `n` times, each time waiting for the sidebar's selection.
    pub(super) fn select_down(&mut self, n: usize) {
        for _ in 0..n {
            let before = sidebar_entry(&self.app);
            self.press(Key::Char('j'));
            self.pump_until(|app| sidebar_entry(app) != before);
        }
    }

    fn click(&mut self, column: u16, row: u16) {
        self.app.update(Msg::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            modifiers: KeyModifiers::NONE,
            column,
            row,
        }));
    }
}

/// The messages `settle` saw that came from the sidebar's keys.
fn sidebar_msgs(seen: &[Msg]) -> Vec<&Msg> {
    seen.iter()
        .filter(|m| {
            matches!(
                m,
                Msg::TurnSelected(_) | Msg::OpenTurn(_) | Msg::FocusTranscript | Msg::FocusTurns
            )
        })
        .collect()
}

/// A standalone transcript of `turns` in `size`: what `]` and `[` do to it
/// is what the sidebar must do to the transcript in the app.
fn reference(turns: &[Arc<Turn>], size: Size) -> Transcript {
    transcript_of(turns.to_vec(), size.width, size.height)
}

fn press_in(t: &mut Transcript, c: char) {
    use tuirealm::event::KeyEvent;
    t.on(&Event::Keyboard(KeyEvent::new(
        Key::Char(c),
        KeyModifiers::NONE,
    )));
}

/// The first row of the transcript on screen, from the transcript's first
/// column on.
fn first_transcript_row(rig: &mut Rig, width: u16, height: u16) -> String {
    let text = screen_text(rig, width, height);
    from_transcript(text.lines().nth(5).unwrap())
}

/// A prompt whose text renders no rows (a link reference definition): the
/// sidebar lists an entry for the turn, but the transcript has nothing to
/// select in it.
pub(super) fn hidden(id: &str) -> Arc<Turn> {
    arc(prompt(id, "[//]: # (nothing to see here)"))
}

/// The transcript's viewport at 120x20 (the sidebar and its gap take 31
/// columns; the header and the help line 7 rows).
const SMALL: Size = Size::new(89, 13);

// -- focusing the sidebar

#[test]
fn one_tab_and_shift_tab_focus_the_sidebar_and_nothing_scrolls() {
    let keys = [
        (Key::Char('1'), KeyModifiers::NONE),
        (Key::Tab, KeyModifiers::NONE),
        (Key::BackTab, KeyModifiers::SHIFT),
    ];
    for (code, modifiers) in keys {
        let mut rig = long_rig(10, 120, 20);
        assert_eq!(rig.focus(), Some(Id::Transcript));
        assert!(!sidebar_flag(&rig.app));
        // The transcript is somewhere in the middle, as a user leaves it.
        rig.press(Key::PageDown);
        rig.settle();
        let before = presentation(&rig.app);
        assert!(before.offset > 0);
        let entry = sidebar_entry(&rig.app);
        rig.key(code, modifiers);
        rig.pump_until(|app| app.application().focus() == Some(&Id::TurnSidebar));
        assert!(
            sidebar_flag(&rig.app),
            "{code:?}: the pane is drawn focused"
        );
        assert_eq!(
            presentation(&rig.app),
            before,
            "{code:?}: focusing does not scroll or select"
        );
        assert_eq!(sidebar_entry(&rig.app), entry, "{code:?}: nor move the row");
    }
}

#[test]
fn the_focus_keys_do_not_touch_the_selector_which_keeps_one_and_tab() {
    // In the selector `1` still switches harness and `Tab` the pane.
    let mut rig = rig();
    rig.press(Key::Tab);
    rig.pump_until(|app| app.application().focus() == Some(&Id::SessionList));
    rig.press(Key::Tab);
    rig.pump_until(|app| app.application().focus() == Some(&Id::ProjectList));
    rig.press(Key::Char('1'));
    rig.settle();
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    assert!(!rig.app.application().mounted(&Id::TurnSidebar));
}

#[test]
fn tab_shift_tab_right_and_l_return_to_the_transcript_leaving_it_as_it_is() {
    let keys = [
        (Key::Tab, KeyModifiers::NONE),
        (Key::BackTab, KeyModifiers::SHIFT),
        (Key::Right, KeyModifiers::NONE),
        (Key::Char('l'), KeyModifiers::NONE),
    ];
    for (code, modifiers) in keys {
        let mut rig = long_rig(10, 120, 20);
        rig.focus_sidebar();
        rig.select_down(2);
        let moved = presentation(&rig.app);
        assert!(moved.offset > 0, "the selection scrolled the transcript");
        rig.leave_sidebar_by(code, modifiers);
        assert!(!sidebar_flag(&rig.app));
        assert_eq!(rig.focus(), Some(Id::Transcript));
        assert_eq!(presentation(&rig.app), moved, "{code:?}: left as it is");
        // And the sidebar is on the turn of the transcript's selection.
        assert_eq!(sidebar_entry(&rig.app), Some(2), "{code:?}");
    }
}

#[test]
fn right_and_l_in_the_transcript_still_unfold_and_do_not_focus_the_sidebar() {
    let mut rig = long_rig(10, 120, 20);
    rig.press(Key::Right);
    rig.press(Key::Char('l'));
    rig.settle();
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn enter_scrolls_to_the_selected_entrys_block_and_moves_the_focus_there() {
    let mut rig = long_rig(10, 120, 20);
    let turns = long_turns(10);
    rig.focus_sidebar();
    rig.select_down(3);
    // The wheel moves the transcript away; the sidebar, which has the focus,
    // does not follow it.
    for _ in 0..2 {
        rig.wheel(MouseEventKind::ScrollDown);
    }
    assert_eq!(sidebar_entry(&rig.app), Some(3));
    let away = presentation(&rig.app);
    let mut expected = reference(&turns, SMALL);
    for _ in 0..3 {
        press_in(&mut expected, ']');
    }
    assert_ne!(away.offset, expected.offset(), "the view is elsewhere");
    rig.press(Key::Enter);
    rig.pump_until(|app| app.application().focus() == Some(&Id::Transcript));
    let after = presentation(&rig.app);
    assert_eq!(after.offset, expected.offset());
    assert_eq!(after.selected.as_ref(), expected.selected());
    assert_eq!(section(&rig.app).unwrap().turn.as_str(), "a1");
    assert_eq!(sidebar_entry(&rig.app), Some(3));
    assert!(!sidebar_flag(&rig.app));
    // The keys are the transcript's again.
    rig.press(Key::Char('j'));
    rig.settle();
    assert_eq!(section(&rig.app).unwrap().turn.as_str(), "u2");
}

#[test]
fn enter_with_the_first_entry_selected_scrolls_to_its_block_too() {
    let mut rig = long_rig(10, 120, 20);
    rig.focus_sidebar();
    for _ in 0..4 {
        rig.wheel(MouseEventKind::ScrollDown);
    }
    assert!(presentation(&rig.app).offset > 0);
    rig.press(Key::Enter);
    rig.pump_until(|app| app.application().focus() == Some(&Id::Transcript));
    let after = presentation(&rig.app);
    assert_eq!(after.offset, 0);
    assert_eq!(section(&rig.app).unwrap().turn.as_str(), "u0");
}

// -- selecting entries

/// Presses `j` through every entry of `turns` and `k` back, and checks each
/// step against what `]` and `[` do to a transcript of the same size.
fn assert_the_keys_do_what_the_turn_keys_do(turns: Vec<Arc<Turn>>, (w, h): (u16, u16)) {
    let viewport = Size::new(w - 31, h - 7);
    let mut rig = rig_with(theme("default"), w, h);
    rig.open_turns(turns.clone());
    let mut expected = reference(&turns, viewport);
    assert_eq!(presentation(&rig.app), expected.presentation(), "the start");
    rig.focus_sidebar();
    let entries = sidebar_rows(&rig.app).entry_count();
    assert_eq!(entries, turns.len());
    let mut states = vec![expected.presentation()];
    for entry in 1..entries {
        rig.press(Key::Char('j'));
        rig.pump_until(|app| sidebar_entry(app) == Some(entry));
        press_in(&mut expected, ']');
        let got = presentation(&rig.app);
        assert_eq!(got.offset, expected.offset(), "j to entry {entry}: offset");
        assert_eq!(
            got.selected.as_ref(),
            expected.selected(),
            "j to entry {entry}: the selection is the block's first section"
        );
        assert_eq!(
            got.selected.as_ref().map(|s| s.turn.clone()),
            sidebar_rows(&rig.app)
                .entry(entry)
                .map(|(id, _)| id.clone()),
            "the selected section is in the entry's turn"
        );
        states.push(expected.presentation());
    }
    for entry in (0..entries - 1).rev() {
        rig.press(Key::Char('k'));
        rig.pump_until(|app| sidebar_entry(app) == Some(entry));
        press_in(&mut expected, '[');
        let got = presentation(&rig.app);
        assert_eq!(got.offset, expected.offset(), "k to entry {entry}: offset");
        assert_eq!(got.selected.as_ref(), expected.selected(), "k to {entry}");
    }
    assert_eq!(presentation(&rig.app).offset, states[0].offset);
}

#[test]
fn j_and_k_scroll_the_transcript_as_the_turn_keys_do_for_every_entry_of_a_fixture() {
    assert_the_keys_do_what_the_turn_keys_do(turns(), (120, 24));
    assert_the_keys_do_what_the_turn_keys_do(turns(), (90, 24));
    assert_the_keys_do_what_the_turn_keys_do(long_turns(8), (120, 20));
}

#[test]
fn down_and_up_do_the_same_as_j_and_k() {
    let mut rig = long_rig(10, 120, 20);
    rig.focus_sidebar();
    rig.press(Key::Down);
    rig.pump_until(|app| sidebar_entry(app) == Some(1));
    let down = presentation(&rig.app);
    rig.press(Key::Down);
    rig.pump_until(|app| sidebar_entry(app) == Some(2));
    rig.press(Key::Up);
    rig.pump_until(|app| sidebar_entry(app) == Some(1));
    assert_eq!(presentation(&rig.app), down);
}

#[test]
fn a_block_is_at_the_top_of_the_viewport_or_as_near_as_the_end_allows() {
    let mut rig = rig_with(theme("default"), 120, 24);
    rig.open_turns(turns());
    rig.focus_sidebar();
    let entries = sidebar_rows(&rig.app).entry_count();
    let mut at_the_top = 0;
    for entry in 1..entries {
        rig.select_down(1);
        assert_eq!(sidebar_entry(&rig.app), Some(entry));
        let row = first_transcript_row(&mut rig, 120, 24);
        if row.starts_with('╭') {
            at_the_top += 1;
        } else {
            assert!(following(&rig.app), "entry {entry}: {row}");
        }
    }
    assert!(at_the_top >= 4, "{at_the_top} blocks at the top");
}

#[test]
fn the_list_does_not_wrap_around_at_either_end() {
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    let start = presentation(&rig.app);
    rig.press(Key::Char('k'));
    rig.press(Key::Up);
    let seen = rig.settle();
    assert!(sidebar_msgs(&seen).is_empty(), "{seen:?}");
    assert_eq!(sidebar_entry(&rig.app), Some(0));
    assert_eq!(presentation(&rig.app), start);

    let last = sidebar_rows(&rig.app).entry_count() - 1;
    rig.select_down(last);
    let end = presentation(&rig.app);
    assert_eq!(sidebar_entry(&rig.app), Some(last));
    rig.press(Key::Char('j'));
    rig.press(Key::Down);
    let seen = rig.settle();
    assert!(sidebar_msgs(&seen).is_empty(), "{seen:?}");
    assert_eq!(sidebar_entry(&rig.app), Some(last), "no wrap to the first");
    assert_eq!(presentation(&rig.app), end);
}

#[test]
fn the_selected_row_stays_visible_while_moving_through_a_long_list() {
    let mut rig = long_rig(30, 120, 20);
    rig.focus_sidebar();
    rig.select_down(40);
    assert_eq!(sidebar_entry(&rig.app), Some(40));
    let text = screen_text(&mut rig, 120, 20);
    let marked: Vec<&str> = text
        .lines()
        .skip(5)
        .filter(|row| row.chars().nth(3) == Some('▒'))
        .collect();
    assert_eq!(marked.len(), 1, "{text}");
}

#[test]
fn an_entry_whose_text_renders_no_rows_scrolls_to_the_block_and_leaves_the_selection() {
    let mut turns = long_turns(6);
    turns.insert(6, hidden("hidden"));
    let mut rig = rig_with(theme("default"), 120, 20);
    rig.open_turns(turns.clone());
    let rows = sidebar_rows(&rig.app);
    assert_eq!(rows.entry_count(), 13, "the turn has an entry");
    assert_eq!(rows.entry_index(&TurnId::from("hidden")), Some(6));
    rig.focus_sidebar();
    rig.select_down(5);
    let before = presentation(&rig.app);
    assert_eq!(before.selected.as_ref().unwrap().turn.as_str(), "a2");
    rig.select_down(1);
    assert_eq!(sidebar_entry(&rig.app), Some(6));
    let after = presentation(&rig.app);
    assert_eq!(after.selected, before.selected, "nothing to select there");
    assert_ne!(after.offset, before.offset, "but the view is at the block");
    let mut expected = reference(&turns, SMALL);
    expected.attr(GO_TO_TURN_ATTR, payload_value(TurnId::from("hidden")));
    assert_eq!(after.offset, expected.offset());
    // `Enter` there moves the focus to the transcript, still on `a2`.
    rig.press(Key::Enter);
    rig.pump_until(|app| app.application().focus() == Some(&Id::Transcript));
    let entered = presentation(&rig.app);
    assert_eq!(entered.selected, before.selected);
    assert_eq!(entered.offset, after.offset);
    // The sidebar shows the turn of the transcript's selection again.
    assert_eq!(sidebar_entry(&rig.app), Some(5));
}

#[test]
fn leaving_an_entry_with_no_rows_by_tab_puts_the_row_back_on_the_transcripts_turn() {
    // The transcript's selection is the one the sidebar followed last: only
    // pushing again when the focus returns puts the row back.
    let mut turns = long_turns(3);
    turns.insert(1, hidden("hidden"));
    let mut rig = rig_with(theme("default"), 120, 20);
    rig.open_turns(turns);
    rig.focus_sidebar();
    rig.select_down(1);
    assert_eq!(sidebar_entry(&rig.app), Some(1));
    assert_eq!(section(&rig.app).unwrap().turn.as_str(), "u0");
    rig.leave_sidebar_by(Key::Tab, KeyModifiers::NONE);
    assert_eq!(
        sidebar_entry(&rig.app),
        Some(0),
        "the transcript's selection is in the turn before"
    );
}

#[test]
fn with_no_selection_the_first_move_in_either_direction_selects_the_first_entry() {
    for key in ['j', 'k'] {
        // Every turn renders no rows: the transcript selects nothing, and
        // neither does the sidebar.
        let mut rig = rig_with(theme("default"), 120, 40);
        rig.open_turns(vec![hidden("h1"), hidden("h2"), hidden("h3")]);
        assert_eq!(sidebar_entry(&rig.app), None);
        assert_eq!(presentation(&rig.app).selected, None);
        rig.focus_sidebar();
        assert_eq!(sidebar_entry(&rig.app), None, "focusing selects nothing");
        rig.press(Key::Char(key));
        rig.pump_until(|app| sidebar_entry(app).is_some());
        assert_eq!(sidebar_entry(&rig.app), Some(0), "{key}");
        assert_eq!(presentation(&rig.app).selected, None);
        // From there it goes on, and does not wrap.
        rig.press(Key::Char('k'));
        let seen = rig.settle();
        assert!(sidebar_msgs(&seen).is_empty(), "{seen:?}");
        assert_eq!(sidebar_entry(&rig.app), Some(0));
    }
}

#[test]
fn on_an_empty_list_the_keys_do_nothing() {
    // A turn that holds system content only has no entry.
    let system_only = arc(crate::components::transcript::fixtures::with_id(
        crate::components::turn_block::fixtures::user(
            "",
            vec![crate::components::turn_block::fixtures::injected(
                "i1",
                &[("file", "/notes.md\nUse it.")],
            )],
        ),
        "sys",
    ));
    let mut rig = rig_with(theme("default"), 120, 40);
    rig.open_turns(vec![system_only]);
    assert_eq!(sidebar_rows(&rig.app).entry_count(), 0);
    rig.focus_sidebar();
    let before = presentation(&rig.app);
    for code in [
        Key::Char('j'),
        Key::Char('k'),
        Key::Down,
        Key::Up,
        Key::Enter,
    ] {
        rig.press(code);
    }
    let seen = rig.settle();
    assert!(sidebar_msgs(&seen).is_empty(), "{seen:?}");
    assert_eq!(sidebar_entry(&rig.app), None);
    assert_eq!(rig.focus(), Some(Id::TurnSidebar));
    assert_eq!(presentation(&rig.app), before);
}

// -- following

#[test]
fn moving_away_from_the_bottom_stops_following_and_a_new_turn_is_counted() {
    let mut rig = long_rig(10, 120, 20);
    rig.press(Key::Char('G'));
    rig.settle();
    assert!(following(&rig.app));
    rig.focus_sidebar();
    let last = sidebar_entry(&rig.app).unwrap();
    // Block by block up: the first moves still show the end of the
    // transcript, then the view leaves it.
    let mut steps = 0;
    while following(&rig.app) {
        let before = sidebar_entry(&rig.app);
        rig.press(Key::Char('k'));
        rig.pump_until(|app| sidebar_entry(app) != before);
        steps += 1;
        assert!(steps < last, "the view never left the end");
    }
    assert!(!following(&rig.app));
    let away = presentation(&rig.app);
    let entry = sidebar_entry(&rig.app);
    let entries = sidebar_rows(&rig.app).entry_count();
    assert_eq!(rig.app.new_sections(), 0);

    // A turn arrives: counted, and nothing moves.
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![arc(reply("late", "Late."))]),
    ));
    assert_eq!(rig.app.new_sections(), 1);
    assert_eq!(status(&rig.app), ["↓ 1 new"]);
    assert_eq!(presentation(&rig.app), away, "the transcript stays");
    assert_eq!(sidebar_entry(&rig.app), entry, "and so does the selection");
    assert_eq!(sidebar_rows(&rig.app).entry_count(), entries + 1, "a row");
    assert_eq!(rig.focus(), Some(Id::TurnSidebar));

    // Back at the end of the transcript following resumes.
    let remaining = entries - entry.unwrap();
    rig.select_down(remaining);
    assert_eq!(sidebar_entry(&rig.app), Some(entries));
    assert!(following(&rig.app));
    assert_eq!(rig.app.new_sections(), 0);
    assert!(status(&rig.app).is_empty());
}

#[test]
fn a_turn_that_arrives_while_the_sidebar_has_focus_adds_a_row_and_moves_nothing() {
    let mut rig = long_rig(10, 120, 20);
    rig.focus_sidebar();
    rig.select_down(4);
    let before = presentation(&rig.app);
    let entries = sidebar_rows(&rig.app).entry_count();
    for n in 0..3 {
        rig.app.update(session_msg(
            &rig.app,
            ModelUpdate::Appended(vec![arc(reply(&format!("late{n}"), "Late."))]),
        ));
        assert_eq!(sidebar_rows(&rig.app).entry_count(), entries + n + 1);
        assert_eq!(sidebar_entry(&rig.app), Some(4));
        assert_eq!(presentation(&rig.app), before);
        assert_eq!(rig.focus(), Some(Id::TurnSidebar));
    }
    assert_eq!(rig.app.new_sections(), 3);
}

// -- keys that do nothing while the sidebar has the focus

#[test]
fn the_other_browser_keys_do_nothing_while_the_sidebar_has_the_focus() {
    let mut rig = long_rig(10, 120, 20);
    rig.focus_sidebar();
    rig.select_down(2);
    let before = presentation(&rig.app);
    let hints = hint_line(&rig.app);
    let plain = [
        '/', 'y', 'Y', 'w', 'W', 'i', 's', '-', '+', '=', ' ', 'h', '1', 'n', 'N', 'a', 'R', 'e',
        'E', 'g', 'G', '[', ']', 'z',
    ];
    for c in plain {
        rig.press(Key::Char(c));
    }
    for code in [
        Key::Left,
        Key::Esc,
        Key::Backspace,
        Key::Home,
        Key::End,
        Key::PageDown,
        Key::PageUp,
    ] {
        rig.press(code);
    }
    rig.ctrl('d');
    rig.ctrl('u');
    rig.ctrl('s');
    rig.ctrl('e');
    let seen = rig.settle();
    assert_eq!(rig.focus(), Some(Id::TurnSidebar), "{seen:?}");
    assert!(sidebar_msgs(&seen).is_empty(), "{seen:?}");
    // The whole presentation: scroll, selection and fold states.
    assert_eq!(presentation(&rig.app), before);
    assert_eq!(sidebar_entry(&rig.app), Some(2));
    for id in [Id::Prompt, Id::HelpOverlay, Id::DetailsOverlay, Id::Toast] {
        assert!(!rig.app.application().mounted(&id), "{id:?}");
    }
    assert_eq!(rig.app.toast_text(), None);
    assert!(!rig.app.should_quit());
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert_eq!(hint_line(&rig.app), hints);
}

#[test]
fn quit_still_works_while_the_sidebar_has_the_focus() {
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.press(Key::Char('q'));
    rig.pump_until(App::should_quit);
}

#[test]
fn the_help_key_the_theme_key_and_ctrl_o_still_work_while_the_sidebar_has_the_focus() {
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.press(Key::Char('?'));
    rig.pump_until(|app| app.application().mounted(&Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));

    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.press(Key::Char('t'));
    rig.pump_until(|app| app.application().mounted(&Id::ThemeSelector));
    assert_eq!(rig.focus(), Some(Id::ThemeSelector));

    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    assert_eq!(rig.focus(), Some(Id::SessionList));
}

// -- overlays

#[test]
fn the_help_overlay_returns_the_focus_to_the_sidebar() {
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.open_help();
    for code in [Key::Esc, Key::Char('?')] {
        rig.press(code);
        rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
        assert_eq!(rig.focus(), Some(Id::TurnSidebar), "{code:?}");
        assert!(sidebar_flag(&rig.app));
        assert_eq!(hint_line(&rig.app), SIDEBAR_HINTS);
        rig.open_help();
    }
}

#[test]
fn the_theme_selector_returns_the_focus_to_the_sidebar() {
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.open_themes();
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    assert_eq!(rig.focus(), Some(Id::TurnSidebar));
    assert!(sidebar_flag(&rig.app));
}

#[test]
fn the_session_selector_returns_the_focus_to_the_sidebar() {
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.press(Key::Esc);
    rig.pump_until(|app| app.screen() == Screen::Browser);
    assert_eq!(rig.focus(), Some(Id::TurnSidebar));
    assert!(sidebar_flag(&rig.app));
    assert_eq!(hint_line(&rig.app), SIDEBAR_HINTS);
}

/// Moves the selection of the transcript on top by `j`, behind the app's
/// back (the overlay in front takes the keys), and lets the app notice.
fn move_the_transcripts_selection(rig: &mut Rig) {
    let top = rig.app.top_transcript();
    let component = rig.app.application_mut().get_component_mut(&top).unwrap();
    component.perform(Cmd::Move(Direction::Down));
    rig.app.update(Msg::TranscriptChanged);
}

#[test]
fn the_sidebar_does_not_follow_the_transcript_while_an_overlay_it_opened_is_open() {
    let mut rig = long_rig(10, 120, 20);
    rig.focus_sidebar();
    rig.select_down(2);
    let before = presentation(&rig.app).selected;
    rig.open_help();
    move_the_transcripts_selection(&mut rig);
    assert_ne!(presentation(&rig.app).selected, before, "it moved");
    assert_eq!(sidebar_entry(&rig.app), Some(2), "the sidebar stayed");
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::TurnSidebar));
    assert_eq!(sidebar_entry(&rig.app), Some(2));
    // Back in the transcript, the row is the transcript's turn again.
    rig.leave_sidebar_by(Key::Tab, KeyModifiers::NONE);
    let turn = section(&rig.app).unwrap().turn;
    assert_eq!(
        sidebar_entry(&rig.app),
        sidebar_rows(&rig.app).entry_index(&turn)
    );
    assert_ne!(sidebar_entry(&rig.app), Some(2));
}

#[test]
fn the_sidebar_follows_the_transcript_while_an_overlay_opened_from_the_transcript_is_open() {
    let mut rig = long_rig(10, 120, 20);
    rig.open_help();
    move_the_transcripts_selection(&mut rig);
    assert_eq!(sidebar_entry(&rig.app), Some(1), "the control: it follows");
}

#[test]
fn an_overlay_opened_from_the_transcript_returns_to_the_transcript() {
    let mut rig = long_rig(4, 120, 20);
    rig.open_help();
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(hint_line(&rig.app), BROWSER_HINTS);
}

// -- hiding

#[test]
fn below_90_columns_the_focus_keys_do_nothing() {
    for (code, modifiers) in [
        (Key::Char('1'), KeyModifiers::NONE),
        (Key::Tab, KeyModifiers::NONE),
        (Key::BackTab, KeyModifiers::SHIFT),
    ] {
        let mut rig = rig_with(theme("default"), 89, 30);
        rig.open_turns(long_turns(10));
        assert!(rig.app.routing().area(Id::TurnSidebar).is_none());
        let before = presentation(&rig.app);
        rig.key(code, modifiers);
        let seen = rig.settle();
        assert_eq!(rig.focus(), Some(Id::Transcript), "{code:?}");
        assert!(!sidebar_flag(&rig.app));
        assert_eq!(presentation(&rig.app), before);
        assert_eq!(hint_line(&rig.app), BROWSER_HINTS);
        assert!(
            seen.contains(&Msg::FocusTurns),
            "the key got there: {seen:?}"
        );
    }
}

#[test]
fn with_an_empty_session_the_focus_keys_do_nothing() {
    for (code, modifiers) in [
        (Key::Char('1'), KeyModifiers::NONE),
        (Key::Tab, KeyModifiers::NONE),
        (Key::BackTab, KeyModifiers::SHIFT),
    ] {
        let mut rig = rig();
        rig.harness.script(BRIO_SESSION, vec![ModelUpdate::Loaded]);
        rig.open(BRIO_SESSION);
        assert!(rig.app.session_is_empty());
        rig.key(code, modifiers);
        rig.settle();
        assert_eq!(rig.focus(), Some(Id::Transcript), "{code:?}");
        assert!(!sidebar_flag(&rig.app));
    }
}

#[test]
fn a_sidebar_that_becomes_hidden_hands_the_focus_to_the_transcript() {
    let mut rig = long_rig(10, 120, 20);
    rig.focus_sidebar();
    assert_eq!(hint_line(&rig.app), SIDEBAR_HINTS);
    rig.app.update(Msg::Resized(89, 20));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!sidebar_flag(&rig.app));
    assert_eq!(hint_line(&rig.app), BROWSER_HINTS);
    // It does not come back by itself.
    rig.app.update(Msg::Resized(120, 20));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    // The transcript's keys work after it.
    rig.press(Key::Char('j'));
    rig.settle();
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn a_screen_that_becomes_too_small_hands_the_focus_over_too() {
    let mut rig = long_rig(10, 120, 20);
    rig.focus_sidebar();
    rig.app.update(Msg::Resized(50, 15));
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn a_session_that_turns_out_empty_hands_the_focus_over() {
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.app.update(session_msg(&rig.app, ModelUpdate::Reset));
    rig.app.update(session_msg(&rig.app, ModelUpdate::Loaded));
    assert!(rig.app.session_is_empty());
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(hint_line(&rig.app), BROWSER_HINTS);
}

#[test]
fn hiding_while_an_overlay_opened_from_the_sidebar_is_open_returns_to_the_transcript() {
    // Help.
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.open_help();
    rig.app.update(Msg::Resized(80, 24));
    assert_eq!(rig.focus(), Some(Id::HelpOverlay), "the overlay keeps it");
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::Transcript), "not the hidden sidebar");
    assert_eq!(hint_line(&rig.app), BROWSER_HINTS);

    // The theme selector.
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.open_themes();
    rig.app.update(Msg::Resized(80, 24));
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    assert_eq!(rig.focus(), Some(Id::Transcript));

    // The session selector.
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.app.update(Msg::Resized(80, 24));
    rig.press(Key::Esc);
    rig.pump_until(|app| app.screen() == Screen::Browser);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn hiding_while_the_selector_has_a_search_prompt_open_returns_to_the_transcript() {
    let mut rig = long_rig(4, 120, 24);
    rig.focus_sidebar();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.press(Key::Char('/'));
    rig.pump_until(|app| app.application().mounted(&Id::Prompt));
    rig.app.update(Msg::Resized(80, 24));
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::Prompt));
    rig.press(Key::Esc);
    rig.pump_until(|app| app.screen() == Screen::Browser);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn the_selector_hidden_and_back_wide_does_not_give_the_sidebar_the_focus_back() {
    let mut rig = long_rig(4, 120, 20);
    rig.focus_sidebar();
    rig.open_help();
    rig.app.update(Msg::Resized(80, 24));
    rig.app.update(Msg::Resized(120, 40));
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

// -- the transcript's own focus changes

#[test]
fn a_click_on_the_transcript_takes_the_focus_and_the_hints_back() {
    let mut rig = long_rig(10, 120, 20);
    rig.focus_sidebar();
    rig.select_down(2);
    assert_eq!(hint_line(&rig.app), SIDEBAR_HINTS);
    rig.click(60, 10);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!sidebar_flag(&rig.app));
    assert_eq!(hint_line(&rig.app), BROWSER_HINTS);
    // The sidebar follows the transcript's selection again.
    assert_eq!(
        sidebar_entry(&rig.app),
        sidebar_rows(&rig.app).entry_index(&section(&rig.app).unwrap().turn)
    );
}

// -- the help line

#[test]
fn the_hints_are_the_sidebars_while_it_has_the_focus_and_change_back_when_it_loses_it() {
    let mut rig = long_rig(10, 120, 20);
    assert_eq!(hint_line(&rig.app), BROWSER_HINTS);
    rig.focus_sidebar();
    assert_eq!(hint_line(&rig.app), SIDEBAR_HINTS);
    let text = screen_text(&mut rig, 120, 20);
    let last = text.lines().last().unwrap();
    assert_eq!(last.trim(), SIDEBAR_HINTS, "drawn full width");
    for (code, modifiers) in [
        (Key::Tab, KeyModifiers::NONE),
        (Key::Right, KeyModifiers::NONE),
        (Key::Char('l'), KeyModifiers::NONE),
        (Key::BackTab, KeyModifiers::SHIFT),
    ] {
        rig.leave_sidebar_by(code, modifiers);
        assert_eq!(hint_line(&rig.app), BROWSER_HINTS, "{code:?}");
        rig.focus_sidebar();
        assert_eq!(hint_line(&rig.app), SIDEBAR_HINTS);
    }
    rig.press(Key::Enter);
    rig.pump_until(|app| app.application().focus() == Some(&Id::Transcript));
    assert_eq!(hint_line(&rig.app), BROWSER_HINTS);
}

#[test]
fn the_hints_come_from_the_active_keymap() {
    let mut raw = RawKeys::new();
    let turns = raw.entry("turns".to_owned()).or_default();
    turns.insert("open".to_owned(), vec!["o".to_owned()]);
    turns.insert("next_pane".to_owned(), vec!["ctrl-n".to_owned()]);
    turns.insert("down".to_owned(), vec!["s".to_owned()]);
    let browser = raw.entry("browser".to_owned()).or_default();
    browser.insert("focus_turns".to_owned(), vec!["f".to_owned()]);
    let (keymap, warnings) = Keymap::from_config(&raw);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut init = init(&[CLAUDE], theme("default"));
    init.keymap = keymap;
    let mut rig = build_rig_from(init, 120, 20, false);
    rig.open_turns(long_turns(6));
    assert_eq!(hint_line(&rig.app), BROWSER_HINTS);
    // The default key is not bound any more; the remapped one is.
    rig.press(Key::Char('1'));
    rig.settle();
    assert_eq!(rig.focus(), Some(Id::Transcript));
    rig.press(Key::Char('f'));
    rig.pump_until(|app| app.application().focus() == Some(&Id::TurnSidebar));
    assert_eq!(
        hint_line(&rig.app),
        "o to go to block | Ctrl-n to switch to transcript"
    );
    // The remapped keys work.
    rig.press(Key::Char('s'));
    rig.pump_until(|app| sidebar_entry(app) == Some(1));
    rig.press(Key::Char('j'));
    rig.settle();
    assert_eq!(sidebar_entry(&rig.app), Some(1), "j is no longer `down`");
    rig.key(Key::Char('n'), KeyModifiers::CONTROL);
    rig.pump_until(|app| app.application().focus() == Some(&Id::Transcript));
    assert_eq!(hint_line(&rig.app), BROWSER_HINTS);
    rig.press(Key::Char('f'));
    rig.pump_until(|app| app.application().focus() == Some(&Id::TurnSidebar));
    rig.press(Key::Char('o'));
    rig.pump_until(|app| app.application().focus() == Some(&Id::Transcript));
}

#[test]
fn the_title_shows_the_key_that_focuses_the_pane_and_just_turns_without_one() {
    let title = |keymap: Keymap| {
        let mut init = init(&[CLAUDE], theme("default"));
        init.keymap = keymap;
        let mut rig = build_rig_from(init, 120, 20, false);
        rig.open_turns(long_turns(3));
        let text = screen_text(&mut rig, 120, 20);
        let row = text.lines().nth(5).unwrap().to_owned();
        row.chars().take(30).collect::<String>()
    };
    assert_eq!(title(Keymap::default()), "╭─ [1] Turns  ───────────────╮");
    let remap = |keys: &[&str]| {
        let mut raw = RawKeys::new();
        raw.entry("browser".to_owned()).or_default().insert(
            "focus_turns".to_owned(),
            keys.iter().map(|k| (*k).to_owned()).collect(),
        );
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        keymap
    };
    assert_eq!(title(remap(&["f"])), "╭─ [f] Turns  ───────────────╮");
    assert_eq!(title(remap(&["ctrl-t"])), "╭─ [Ctrl-t] Turns  ──────────╮");
    assert_eq!(title(remap(&[])), "╭─ Turns  ───────────────────╮");
}

// -- the help overlay

#[test]
fn the_help_overlay_lists_the_sidebars_keys_in_a_group_before_application() {
    let mut rig = rig();
    rig.app.update(Msg::Resized(120, 70));
    rig.open_loaded();
    rig.open_help();
    let text = screen_text(&mut rig, 120, 70);
    let rows: Vec<&str> = text.lines().collect();
    let at = |needle: &str| {
        rows.iter()
            .position(|row| row.contains(needle))
            .unwrap_or_else(|| panic!("no row with {needle:?}\n{text}"))
    };
    assert!(at("Sessions") < at("Turns sidebar"));
    assert!(at("Turns sidebar") < at("Application"));
    for needle in [
        "Focus turns sidebar",
        "Previous entry",
        "Next entry",
        "Go to block",
        "Focus transcript",
    ] {
        assert!(at(needle) > at("Turns sidebar"), "{needle}");
        assert!(at(needle) < at("Application"), "{needle}");
    }
    let tab_rows = rows.iter().filter(|r| r.contains("Next pane")).count();
    assert_eq!(tab_rows, 1, "TAB is listed once");
}

// -- subagents

fn presentation_of(app: &App, id: Id) -> Presentation {
    let value = app
        .application()
        .query(&id, PRESENTATION_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

#[test]
fn in_a_subagent_the_sidebar_scrolls_the_transcript_on_top_and_the_hints_keep_the_depth() {
    use super::subagent::subagent_rig;
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    assert_eq!(rig.focus(), Some(Id::Subagent(1)));
    let parent = presentation_of(&rig.app, Id::Transcript);
    let depth_one = hint_line(&rig.app);
    assert!(depth_one.contains("Backspace to go back"), "{depth_one}");

    rig.focus_sidebar();
    assert_eq!(hint_line(&rig.app), SIDEBAR_HINTS);
    let start = presentation_of(&rig.app, Id::Subagent(1));
    rig.select_down(1);
    let moved = presentation_of(&rig.app, Id::Subagent(1));
    assert_ne!(moved.selected, start.selected, "the subagent's transcript");
    assert_eq!(
        presentation_of(&rig.app, Id::Transcript),
        parent,
        "the parent's was not touched"
    );

    rig.leave_sidebar_by(Key::Tab, KeyModifiers::NONE);
    assert_eq!(rig.focus(), Some(Id::Subagent(1)));
    assert_eq!(hint_line(&rig.app), depth_one);
    assert_eq!(presentation_of(&rig.app, Id::Subagent(1)), moved);
}

// -- the screens

macro_rules! focused_sidebar_screen {
    ($name:ident, $theme:expr, $width:expr, $height:expr, $downs:expr, $kind:expr) => {
        #[test]
        fn $name() {
            let mut rig = rig_with(theme($theme), $width, $height);
            rig.open_loaded();
            rig.focus_sidebar();
            rig.select_down($downs);
            let entry = sidebar_entry(&rig.app).unwrap();
            assert_eq!(sidebar_rows(&rig.app).entry(entry).unwrap().1.kind, $kind);
            assert_eq!(hint_line(&rig.app), SIDEBAR_HINTS);
            insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, $width, $height)));
        }
    };
}

use crate::components::EntryKind::{Assistant, Compaction, User};

focused_sidebar_screen!(
    the_focused_sidebar_on_a_user_120x40_default_styled,
    "default",
    120,
    40,
    2,
    User
);
focused_sidebar_screen!(
    the_focused_sidebar_on_an_assistant_120x40_default_styled,
    "default",
    120,
    40,
    1,
    Assistant
);
focused_sidebar_screen!(
    the_focused_sidebar_on_a_compaction_120x40_default_styled,
    "default",
    120,
    40,
    4,
    Compaction
);
focused_sidebar_screen!(
    the_focused_sidebar_on_a_user_90x24_default_styled,
    "default",
    90,
    24,
    2,
    User
);
focused_sidebar_screen!(
    the_focused_sidebar_on_an_assistant_90x24_default_styled,
    "default",
    90,
    24,
    1,
    Assistant
);
focused_sidebar_screen!(
    the_focused_sidebar_on_a_compaction_90x24_default_styled,
    "default",
    90,
    24,
    4,
    Compaction
);
focused_sidebar_screen!(
    the_focused_sidebar_on_a_user_120x40_terminal_styled,
    "terminal",
    120,
    40,
    2,
    User
);
focused_sidebar_screen!(
    the_focused_sidebar_on_an_assistant_120x40_terminal_styled,
    "terminal",
    120,
    40,
    1,
    Assistant
);
focused_sidebar_screen!(
    the_focused_sidebar_on_a_compaction_120x40_terminal_styled,
    "terminal",
    120,
    40,
    4,
    Compaction
);
focused_sidebar_screen!(
    the_focused_sidebar_on_a_user_90x24_terminal_styled,
    "terminal",
    90,
    24,
    2,
    User
);
focused_sidebar_screen!(
    the_focused_sidebar_on_an_assistant_90x24_terminal_styled,
    "terminal",
    90,
    24,
    1,
    Assistant
);
focused_sidebar_screen!(
    the_focused_sidebar_on_a_compaction_90x24_terminal_styled,
    "terminal",
    90,
    24,
    4,
    Compaction
);

#[test]
fn the_focused_pane_has_the_ring_border_and_the_selected_row_is_one_inverted_band() {
    let mut rig = rig();
    rig.open_loaded();
    rig.focus_sidebar();
    rig.select_down(1);
    let buffer = screen_buffer(&mut rig, 120, 40);
    let theme = rig.app.theme().clone();
    use crate::theme::Role;
    use ratatui::style::Modifier;
    // The border of the pane (below the header) is `ring`.
    assert_eq!(buffer[(0, 5)].fg, theme.role(Role::Ring));
    assert_eq!(buffer[(29, 37)].fg, theme.role(Role::Ring));
    // The marker is `primary` in column 3; the band runs from column 4 to
    // column 26 and is reversed.
    let selected_row = (5..38)
        .find(|&y| buffer[(3, y)].symbol() == "▒")
        .expect("a marker");
    assert_eq!(buffer[(3, selected_row)].fg, theme.role(Role::Primary));
    for x in 4..=26 {
        let cell = &buffer[(x, selected_row)];
        assert!(cell.modifier.contains(Modifier::REVERSED), "col {x}");
        assert_eq!(cell.fg, theme.role(Role::Foreground), "col {x}");
    }
    // Unfocused again: the border is `border`.
    rig.leave_sidebar_by(Key::Tab, KeyModifiers::NONE);
    let buffer = screen_buffer(&mut rig, 120, 40);
    assert_eq!(buffer[(0, 5)].fg, theme.role(Role::Border));
}
