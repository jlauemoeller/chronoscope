//! Searching the transcript (product §5.2) through the app: the prompt, the
//! worker and its port, the results, the help line and the screens.

use tuirealm::state::{State, StateValue};

use super::*;
use crate::components::TURN_COUNT_ATTR;
use crate::components::transcript::fixtures::search_session;
use crate::transcript_search::{SearchMatches, SearchPosition};

fn search_turns() -> Vec<Arc<Turn>> {
    search_session()
}

fn prompt_text(app: &App) -> String {
    match app.application().state(&Id::Prompt).unwrap() {
        State::Single(StateValue::String(text)) => text,
        other => panic!("{other:?}"),
    }
}

fn position(app: &App) -> Option<String> {
    app.transcript_search_position().map(|p| p.to_string())
}

fn position_item(app: &App) -> Option<String> {
    status(app).into_iter().find(|item| item.contains('/'))
}

impl Rig {
    /// Opens the BRIO session with the search fixture turns, loaded.
    fn open_searchable(&mut self) {
        self.harness.script(
            BRIO_SESSION,
            vec![
                ModelUpdate::Appended(search_turns()),
                ModelUpdate::Progress(1.0),
                ModelUpdate::ParseWarnings(0),
                ModelUpdate::Loaded,
            ],
        );
        self.open(BRIO_SESSION);
    }

    fn char_key(&self, c: char) {
        self.press(Key::Char(c));
    }

    pub(super) fn type_text(&self, text: &str) {
        text.chars().for_each(|c| self.char_key(c));
    }

    /// Presses `/` and waits for the prompt.
    pub(super) fn open_prompt(&mut self) {
        self.char_key('/');
        self.pump_until(|app| app.transcript_prompt_open());
    }

    /// Opens the prompt, types `query` and presses `Enter`, and waits until
    /// the search has an answer.
    pub(super) fn search(&mut self, query: &str) {
        self.open_prompt();
        self.type_text(query);
        let typed = query.to_owned();
        self.pump_until(|app| prompt_text(app) == typed);
        self.press(Key::Enter);
        self.pump_until(|app| app.transcript_search_position().is_some());
    }

    /// Handles the messages that `update` queued for itself.
    fn drain(&mut self) {
        for msg in self.app.take_queued() {
            self.app.update(msg);
        }
    }

    /// Presses `key` and lets the app handle it.
    fn press_and_settle(&mut self, code: Key) {
        self.press(code);
        self.settle();
    }
}

fn searching_rig() -> Rig {
    let mut rig = rig();
    rig.open_searchable();
    rig
}

fn worker_rig(width: u16, height: u16) -> Rig {
    let mut rig = build_rig(theme("default"), width, height, true);
    rig.open_searchable();
    rig
}

// -- the prompt

#[test]
fn slash_opens_the_prompt_in_place_of_the_help_line_and_gives_it_the_focus() {
    let mut rig = searching_rig();
    assert_eq!(rig.focus(), Some(Id::Transcript));
    rig.char_key('/');
    rig.pump_until(|app| app.transcript_prompt_open());
    assert_eq!(rig.focus(), Some(Id::Prompt));
    assert!(rig.app.application().mounted(&Id::Prompt));
    let text = screen_text(&mut rig, 120, 40);
    let last = text.lines().nth(39).unwrap();
    assert!(last.starts_with(" /"), "{last:?}");
    assert!(
        !last.contains("to search"),
        "the hints are replaced: {last:?}"
    );
    assert!(
        text.lines().nth(38).unwrap().trim().is_empty(),
        "the row above stays blank"
    );
}

#[test]
fn printable_keys_go_into_the_prompt_and_trigger_no_action() {
    let mut rig = searching_rig();
    // A search is on, so that `n` would move to the second match if it got
    // through.
    rig.search("usb");
    assert_eq!(position(&rig.app), Some("1/5".to_owned()));
    let before = rows_of(&screen_buffer(&mut rig, 120, 40));
    rig.open_prompt();
    rig.type_text("nqt");
    rig.pump_until(|app| prompt_text(app) == "nqt");
    rig.settle();
    assert_eq!(prompt_text(&rig.app), "nqt");
    assert!(!rig.app.should_quit(), "q did not quit");
    assert_eq!(rig.focus(), Some(Id::Prompt));
    assert_eq!(position(&rig.app), Some("1/5".to_owned()), "n did not step");
    // `t` would have opened the theme selector (a global key).
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert!(!rig.app.application().mounted(&Id::SelectorFrame));
    // Not a fold, not a selection: the transcript is as it was.
    let after = rows_of(&screen_buffer(&mut rig, 120, 40));
    assert_eq!(before[..38], after[..38]);
}

#[test]
fn digits_j_and_s_are_typed_too() {
    let mut rig = searching_rig();
    rig.open_prompt();
    rig.type_text("j1s4G");
    rig.pump_until(|app| prompt_text(app) == "j1s4G");
    assert!(!rig.app.should_quit());
}

#[test]
fn ctrl_c_still_quits_from_the_prompt() {
    let mut rig = searching_rig();
    rig.open_prompt();
    rig.key(Key::Char('c'), KeyModifiers::CONTROL);
    rig.pump_until(App::should_quit);
}

#[test]
fn typing_in_the_prompt_asks_for_a_redraw() {
    let mut rig = searching_rig();
    rig.open_prompt();
    rig.app.take_redraw();
    rig.app.update(Msg::PromptChanged("u".to_owned()));
    assert_ne!(
        rig.app.take_redraw(),
        crate::app::redraw::RedrawRequest::None
    );
}

#[test]
fn escape_in_the_prompt_closes_it_without_searching() {
    let mut rig = searching_rig();
    rig.open_prompt();
    rig.type_text("usb");
    rig.pump_until(|app| prompt_text(app) == "usb");
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.transcript_prompt_open());
    assert!(!rig.app.application().mounted(&Id::Prompt));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    rig.settle();
    assert_eq!(position(&rig.app), None);
    assert!(status(&rig.app).is_empty());
    assert!(rig.app.toast_text().is_none());
}

#[test]
fn enter_on_an_empty_query_closes_the_prompt_without_searching() {
    let mut rig = searching_rig();
    rig.open_prompt();
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.transcript_prompt_open());
    assert_eq!(rig.focus(), Some(Id::Transcript));
    rig.settle();
    assert_eq!(position(&rig.app), None);
    assert!(
        rig.app.toast_text().is_none(),
        "no \"no matches\" for nothing"
    );
}

#[test]
fn the_selectors_prompt_messages_do_not_reach_the_browser_prompt() {
    // `PromptSubmit` while the browser prompt is open must not open a session.
    let mut rig = searching_rig();
    rig.open_prompt();
    rig.app.update(Msg::PromptSubmit("anything".to_owned()));
    let queued = rig.app.take_queued();
    assert!(
        queued
            .iter()
            .all(|m| matches!(m, Msg::StartTranscriptSearch { .. })),
        "{queued:?}"
    );
    assert_eq!(rig.app.screen(), Screen::Browser);
}

#[test]
fn the_prompt_covers_the_whole_row_with_the_background() {
    let mut rig = searching_rig();
    rig.open_prompt();
    let theme = theme("default");
    let buffer = screen_buffer(&mut rig, 120, 40);
    for y in 0..40 {
        for x in 0..120 {
            if (x, y) == (2, 39) {
                // The cursor, after the label.
                assert_eq!(buffer[(x, y)].bg, theme.role(Role::Cursor));
                continue;
            }
            assert_eq!(
                buffer[(x, y)].bg,
                theme.role(Role::Background),
                "cell {x},{y} {:?}",
                buffer[(x, y)].symbol()
            );
        }
    }
}

// -- searching

#[test]
fn enter_jumps_to_the_first_match_and_the_help_line_shows_the_position() {
    let mut rig = searching_rig();
    rig.search("usb");
    assert_eq!(position(&rig.app), Some("1/5".to_owned()));
    assert_eq!(status(&rig.app), ["1/5"]);
    assert_eq!(rig.focus(), Some(Id::Transcript), "the prompt is closed");
    assert!(!rig.app.application().mounted(&Id::Prompt));
    let text = screen_text(&mut rig, 120, 40);
    assert!(text.lines().nth(39).unwrap().ends_with("1/5 "), "{text}");
    // The prompt gave way to the hints.
    assert!(text.lines().nth(39).unwrap().contains("to search"));
}

#[test]
fn n_and_capital_n_move_among_the_matches_and_wrap() {
    let mut rig = searching_rig();
    rig.search("usb");
    let mut seen = vec![position_item(&rig.app).unwrap()];
    for _ in 0..5 {
        rig.press_and_settle(Key::Char('n'));
        seen.push(position_item(&rig.app).unwrap());
    }
    assert_eq!(seen, ["1/5", "2/5", "3/5", "4/5", "5/5", "1/5"]);
    rig.press_and_settle(Key::Char('N'));
    assert_eq!(position_item(&rig.app).as_deref(), Some("5/5"));
    rig.key(Key::Char('N'), KeyModifiers::SHIFT);
    rig.settle();
    assert_eq!(position_item(&rig.app).as_deref(), Some("4/5"));
}

#[test]
fn no_matches_shows_zero_of_zero_and_a_toast() {
    let mut rig = searching_rig();
    rig.search("zebra");
    assert_eq!(status(&rig.app), ["0/0"]);
    assert_eq!(rig.app.toast_text(), Some("No matches for \"zebra\""));
    let text = screen_text(&mut rig, 120, 40);
    assert!(text.contains("No matches for \"zebra\""), "{text}");
    rig.press_and_settle(Key::Char('n'));
    assert_eq!(status(&rig.app), ["0/0"]);
}

#[test]
fn the_position_goes_after_the_new_content_indicator() {
    use crate::components::transcript::fixtures::{arc, reply};
    let mut rig = rig();
    // Long enough that the end is off screen once the view is at the top.
    let mut turns = search_turns();
    turns.extend((0..12).map(|n| arc(reply(&format!("r{n}"), "Another reply."))));
    rig.harness.script(
        BRIO_SESSION,
        vec![ModelUpdate::Appended(turns), ModelUpdate::Loaded],
    );
    rig.open(BRIO_SESSION);
    rig.search("usb");
    rig.press_and_settle(Key::Char('g'));
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![arc(reply("late", "A late reply."))]),
    ));
    assert_eq!(status(&rig.app), ["↓ 1 new", "1/5"]);
    let text = screen_text(&mut rig, 120, 40);
    assert!(
        text.lines().nth(39).unwrap().ends_with("↓ 1 new  1/5 "),
        "{text}"
    );
}

#[test]
fn escape_clears_the_search_the_highlights_and_the_position() {
    let mut rig = searching_rig();
    rig.search("usb");
    rig.press_and_settle(Key::Char('n'));
    assert_eq!(position(&rig.app), Some("2/5".to_owned()));
    let before = screen_text(&mut rig, 120, 40);
    assert!(before.contains("comes from the usb driver"));
    rig.press_and_settle(Key::Esc);
    assert_eq!(position(&rig.app), None);
    assert!(status(&rig.app).is_empty());
    let after = screen_text(&mut rig, 120, 40);
    assert!(!after.contains("comes from the usb driver"), "refolded");
    let buffer = screen_buffer(&mut rig, 120, 40);
    let highlight = theme("default").role(Role::Primary);
    for y in 0..40 {
        for x in 0..120 {
            assert_ne!(buffer[(x, y)].bg, highlight, "{x},{y}");
        }
    }
    // `n` does nothing now.
    rig.press_and_settle(Key::Char('n'));
    assert_eq!(position(&rig.app), None);
}

#[test]
fn a_new_search_replaces_the_previous_one() {
    let mut rig = searching_rig();
    rig.search("usb");
    rig.press_and_settle(Key::Char('n'));
    rig.open_prompt();
    rig.type_text("camera");
    rig.pump_until(|app| prompt_text(app) == "camera");
    rig.press(Key::Enter);
    rig.pump_until(|app| position(app) == Some("1/1".to_owned()));
    assert_eq!(status(&rig.app), ["1/1"]);
    let text = screen_text(&mut rig, 120, 40);
    assert!(
        !text.contains("comes from the usb driver"),
        "the call refolded"
    );
}

#[test]
fn smart_case_applies_to_the_query_typed() {
    let mut rig = searching_rig();
    rig.search("USB");
    assert_eq!(position(&rig.app), Some("1/3".to_owned()));
}

#[test]
fn hidden_system_content_is_not_searched() {
    let mut rig = rig();
    let injected = crate::components::turn_block::fixtures::injected(
        "i1",
        &[("file", "notes about the usbhub")],
    );
    let turn = crate::components::transcript::fixtures::with_id(
        crate::components::turn_block::fixtures::user("a prompt", vec![injected]),
        "u1",
    );
    rig.harness.script(
        BRIO_SESSION,
        vec![
            ModelUpdate::Appended(vec![Arc::new(turn)]),
            ModelUpdate::Loaded,
        ],
    );
    rig.open(BRIO_SESSION);
    rig.search("usbhub");
    assert_eq!(position(&rig.app), Some("1/1".to_owned()), "shown: found");
    rig.press_and_settle(Key::Esc);
    rig.press_and_settle(Key::Char('s'));
    rig.search("usbhub");
    assert_eq!(
        position(&rig.app),
        Some("0/0".to_owned()),
        "hidden: not found"
    );
}

// -- the worker, the port and the generations

#[test]
fn the_search_runs_on_the_worker_and_its_answer_comes_back_through_the_port() {
    let mut rig = worker_rig(120, 40);
    rig.open_prompt();
    rig.type_text("usb");
    rig.pump_until(|app| prompt_text(app) == "usb");
    rig.press(Key::Enter);
    let seen = rig.pump_until(|app| app.transcript_search_position().is_some());
    let start = seen
        .iter()
        .position(|m| matches!(m, Msg::StartTranscriptSearch { query, .. } if query == "usb"));
    let answer = seen
        .iter()
        .position(|m| matches!(m, Msg::TranscriptSearchResults(r) if r.matches().len() == 5));
    // The answer is a message that a tick produced from a user event, after
    // the request was made.
    assert!(start.is_some(), "{seen:?}");
    assert!(
        answer.is_some_and(|answer| answer > start.unwrap()),
        "{seen:?}"
    );
    assert_eq!(status(&rig.app), ["1/5"]);
}

#[test]
fn the_answer_is_not_there_before_the_worker_has_delivered_it() {
    let mut rig = worker_rig(120, 40);
    rig.app.update(Msg::OpenSearch);
    rig.app.update(Msg::PromptSubmit("usb".to_owned()));
    rig.drain();
    // The request was handed to the worker; nothing was searched here.
    assert_eq!(position(&rig.app), None);
    rig.pump_until(|app| app.transcript_search_position().is_some());
    assert_eq!(position(&rig.app), Some("1/5".to_owned()));
}

#[test]
fn an_answer_with_an_old_generation_is_ignored() {
    let mut rig = worker_rig(120, 40);
    rig.app.update(Msg::OpenSearch);
    rig.app.update(Msg::PromptSubmit("usb".to_owned()));
    rig.drain();
    // Answers to searches that are not the one asked for.
    for generation in [0, 1000] {
        rig.app
            .update(Msg::TranscriptSearchResults(SearchMatches::new(
                generation,
                Vec::new(),
            )));
    }
    assert_eq!(position(&rig.app), None, "neither was the answer");
    rig.pump_until(|app| app.transcript_search_position().is_some());
    assert_eq!(position(&rig.app), Some("1/5".to_owned()));
    // Nor does one that arrives after the answer replace it.
    rig.app
        .update(Msg::TranscriptSearchResults(SearchMatches::new(
            1,
            Vec::new(),
        )));
    assert_eq!(position(&rig.app), Some("1/5".to_owned()));
}

#[test]
fn only_the_newest_search_is_shown() {
    let mut rig = worker_rig(120, 40);
    rig.app.update(Msg::OpenSearch);
    rig.app.update(Msg::PromptSubmit("usb".to_owned()));
    rig.drain();
    rig.app.update(Msg::OpenSearch);
    rig.app.update(Msg::PromptSubmit("zebra".to_owned()));
    rig.drain();
    rig.pump_until(|app| app.transcript_search_position().is_some());
    rig.settle();
    assert_eq!(position(&rig.app), Some("0/0".to_owned()));
    assert_eq!(rig.app.toast_text(), Some("No matches for \"zebra\""));
}

#[test]
fn clearing_the_search_drops_an_answer_still_on_its_way() {
    let mut rig = worker_rig(120, 40);
    rig.app.update(Msg::OpenSearch);
    rig.app.update(Msg::PromptSubmit("usb".to_owned()));
    rig.drain();
    rig.app.update(Msg::SearchPosition(None));
    rig.settle();
    assert_eq!(position(&rig.app), None);
    assert!(status(&rig.app).is_empty());
}

#[test]
fn an_answer_to_a_search_of_another_session_is_ignored() {
    let mut rig = worker_rig(120, 40);
    rig.search("usb");
    let first = position(&rig.app);
    assert_eq!(first, Some("1/5".to_owned()));
    // Another session; the first search's answer is late.
    rig.harness.script(
        NEWER_SESSION,
        vec![ModelUpdate::Appended(search_turns()), ModelUpdate::Loaded],
    );
    rig.app.update(Msg::OpenSelector);
    rig.app.update(Msg::OpenSession(CLAUDE, id(NEWER_SESSION)));
    rig.pump_until(App::session_loaded);
    assert_eq!(position(&rig.app), None, "a fresh browser");
    let late = SearchMatches::new(
        1,
        vec![crate::transcript_search::Match {
            section: crate::components::turn_block::SectionId {
                turn: crate::model::TurnId::from("u1"),
                key: crate::components::turn_block::SectionKey::Prompt,
            },
            line: 0,
            range: 0..3,
        }],
    );
    rig.app.update(Msg::TranscriptSearchResults(late.clone()));
    assert_eq!(position(&rig.app), None);
    // A search of this session has a generation of its own.
    rig.search("usb");
    assert_eq!(position(&rig.app), Some("1/5".to_owned()));
    rig.app.update(Msg::TranscriptSearchResults(late));
    assert_eq!(position(&rig.app), Some("1/5".to_owned()));
}

#[test]
fn a_reload_of_the_session_clears_the_search() {
    let mut rig = searching_rig();
    rig.search("usb");
    assert_eq!(status(&rig.app), ["1/5"]);
    rig.app.update(session_msg(&rig.app, ModelUpdate::Reset));
    assert_eq!(position(&rig.app), None);
    assert!(status(&rig.app).is_empty());
    assert_eq!(
        rig.app
            .application()
            .query(&Id::Transcript, TURN_COUNT_ATTR)
            .unwrap()
            .unwrap()
            .into_attr(),
        AttrValue::Length(0)
    );
}

#[test]
fn the_position_is_zero_when_there_is_no_match_and_displays_as_such() {
    assert_eq!(
        SearchPosition {
            current: 0,
            total: 0
        }
        .to_string(),
        "0/0"
    );
}

// -- the screens

/// The fixture session at `width` x `height`, with `/usb` typed.
fn typed(width: u16, height: u16) -> Rig {
    let mut rig = build_rig(theme("default"), width, height, false);
    rig.open_searchable();
    rig.open_prompt();
    rig.type_text("usb");
    rig.pump_until(|app| prompt_text(app) == "usb");
    rig
}

#[test]
fn prompt_with_usb_typed_80x24_text() {
    let mut rig = typed(80, 24);
    let text = screen_text(&mut rig, 80, 24);
    assert!(text.lines().nth(23).unwrap().starts_with(" /usb"), "{text}");
    insta::assert_snapshot!(text);
}

#[test]
fn prompt_with_usb_typed_80x24_styled() {
    let mut rig = typed(80, 24);
    let buffer = screen_buffer(&mut rig, 80, 24);
    // The cursor cell follows the text, in the `cursor` colour.
    let cursor = theme("default").role(Role::Cursor);
    assert_eq!(buffer[(5, 23)].bg, cursor);
    assert_eq!(buffer[(1, 23)].symbol(), "/");
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}

fn searched(width: u16, height: u16) -> Rig {
    let mut rig = typed(width, height);
    rig.press(Key::Enter);
    rig.pump_until(|app| app.transcript_search_position().is_some());
    rig
}

#[test]
fn after_enter_the_first_match_is_selected_and_highlighted_120x40_text() {
    let mut rig = searched(120, 40);
    let text = screen_text(&mut rig, 120, 40);
    assert!(text.lines().nth(39).unwrap().ends_with("1/5 "), "{text}");
    insta::assert_snapshot!(text);
}

#[test]
fn after_enter_the_first_match_is_selected_and_highlighted_120x40_styled() {
    let mut rig = searched(120, 40);
    let buffer = screen_buffer(&mut rig, 120, 40);
    let theme = theme("default");
    let highlight = theme.style(crate::theme::Token::SearchMatch);
    let marked: Vec<String> = (0..40)
        .flat_map(|y| {
            let cells: Vec<&str> = (0..120)
                .filter(|&x| buffer[(x, y)].bg == highlight.bg.unwrap())
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            (!cells.is_empty()).then(|| cells.concat())
        })
        .collect();
    // One row holds two: `USB` and `usb`.
    assert_eq!(marked, ["USB", "USB", "USBusb"]);
    // Hints and the position are in the help line's text colour.
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}

#[test]
fn after_n_into_the_folded_tool_call_it_is_unfolded_and_highlighted_120x40_text() {
    let mut rig = searched(120, 40);
    rig.press_and_settle(Key::Char('n'));
    // The match in the result is drawn with the `search_match` style.
    let buffer = screen_buffer(&mut rig, 120, 40);
    let highlight = theme("default").role(Role::Primary);
    let row = rows_of(&buffer)
        .into_iter()
        .position(|row| row.contains("comes from the usb driver"))
        .unwrap();
    let marked: String = (0..120)
        .filter(|&x| buffer[(x, row as u16)].bg == highlight)
        .map(|x| buffer[(x, row as u16)].symbol())
        .collect();
    assert_eq!(marked, "usb");
    let text = buffer_to_text(&buffer);
    assert!(text.lines().nth(39).unwrap().ends_with("2/5 "), "{text}");
    insta::assert_snapshot!(text);
}

#[test]
fn after_n_into_the_folded_tool_call_it_is_unfolded_and_highlighted_120x40_styled() {
    let mut rig = searched(120, 40);
    rig.press_and_settle(Key::Char('n'));
    let buffer = screen_buffer(&mut rig, 120, 40);
    let highlight = theme("default").style(crate::theme::Token::SearchMatch);
    let row = rows_of(&buffer)
        .into_iter()
        .position(|row| row.contains("comes from the usb driver"))
        .unwrap() as u16;
    let marked: Vec<u16> = (0..120)
        .filter(|&x| buffer[(x, row)].bg == highlight.bg.unwrap())
        .collect();
    assert_eq!(marked.len(), 3);
    assert!(
        marked
            .iter()
            .all(|&x| buffer[(x, row)].fg == highlight.fg.unwrap())
    );
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}

#[test]
fn after_moving_off_the_tool_call_it_is_folded_again_and_the_position_stays_120x40_text() {
    let mut rig = searched(120, 40);
    rig.press_and_settle(Key::Char('n'));
    rig.press_and_settle(Key::Char('j'));
    let text = screen_text(&mut rig, 120, 40);
    assert!(!text.contains("comes from the usb driver"), "{text}");
    assert!(text.lines().nth(39).unwrap().ends_with("2/5 "), "{text}");
    insta::assert_snapshot!(text);
}

#[test]
fn after_escape_there_are_no_highlights_and_no_position_80x24_styled() {
    let mut rig = searched(80, 24);
    rig.press_and_settle(Key::Esc);
    let buffer = screen_buffer(&mut rig, 80, 24);
    let highlight = theme("default").role(Role::Primary);
    assert!(
        (0..24).all(|y| (0..80).all(|x| buffer[(x, y)].bg != highlight)),
        "no highlight is left"
    );
    let text = buffer_to_text(&buffer);
    let help = text.lines().nth(23).unwrap();
    assert!(help.trim_end().ends_with("y to copy current"), "{help:?}");
    assert!(!text.contains("1/5"));
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}
