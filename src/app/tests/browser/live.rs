//! The browser's live behaviour (product §6.2): following, `↓ N new`, in-place
//! updates and the `● live` marker, through the app as the running
//! application drives it, and the screens that show them.

use std::time::SystemTime;

use chrono::Duration as Span;
use tuirealm::event::Key;
use tuirealm::state::{State, StateValue};

use super::*;
use crate::app::redraw::RedrawRequest;
use crate::components::transcript::fixtures::{arc, reply};
use crate::components::{FOLLOWING_ATTR, LIVE_ATTR, NEW_SECTIONS_ATTR};

fn replies(ids: &[&str]) -> Vec<Arc<Turn>> {
    ids.iter()
        .map(|id| arc(reply(id, &format!("A reply for {id}."))))
        .collect()
}

fn activity(ago: Span) -> ModelUpdate {
    ModelUpdate::Activity {
        last_modified: SystemTime::from(now() - ago),
        idle: None,
    }
}

fn query(app: &App, id: Id, attr: tuirealm::props::Attribute) -> AttrValue {
    app.application()
        .query(&id, attr)
        .unwrap()
        .unwrap()
        .into_attr()
}

fn following(app: &App) -> bool {
    query(app, Id::Transcript, FOLLOWING_ATTR) == AttrValue::Flag(true)
}

fn new_in_transcript(app: &App) -> usize {
    match query(app, Id::Transcript, NEW_SECTIONS_ATTR) {
        AttrValue::Length(n) => n,
        other => panic!("{other:?}"),
    }
}

fn header_live(app: &App) -> bool {
    query(app, Id::BrowserHeader, LIVE_ATTR) == AttrValue::Flag(true)
}

fn offset(app: &App) -> usize {
    match app.application().state(&Id::Transcript).unwrap() {
        State::Single(StateValue::Usize(offset)) => offset,
        other => panic!("{other:?}"),
    }
}

impl Rig {
    /// Opens the BRIO session: the fixture turns, loaded, active 10 s ago.
    fn open_live(&mut self) {
        self.harness.script(
            BRIO_SESSION,
            vec![
                ModelUpdate::Appended(turns()),
                ModelUpdate::Progress(1.0),
                ModelUpdate::ParseWarnings(0),
                ModelUpdate::Loaded,
                activity(Span::seconds(10)),
            ],
        );
        self.open(BRIO_SESSION);
        self.settle();
    }

    /// Sends `update` as the worker of the first load and waits for it.
    fn live_update(&mut self, update: ModelUpdate) {
        self.harness.send(0, update);
        self.settle();
    }

    fn set_now(&self, now: chrono::DateTime<chrono::Utc>) {
        *self.clock.lock().unwrap() = now;
    }
}

// -- follow mode

#[test]
fn at_the_bottom_new_turns_scroll_into_view_through_the_app() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_live();
    rig.press(Key::Char('G'));
    rig.settle();
    assert!(following(&rig.app));
    let before = offset(&rig.app);
    rig.live_update(ModelUpdate::Appended(replies(&["t9"])));
    assert!(offset(&rig.app) > before, "the view moved to the new turn");
    assert!(following(&rig.app));
    assert!(text(&mut rig.app, 80, 24).contains("A reply for t9."));
    assert_eq!(rig.app.new_sections(), 0);
    assert!(status(&rig.app).is_empty());
}

#[test]
fn scrolled_up_new_turns_are_counted_in_the_help_line_and_g_resets_it() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_live();
    let top = offset(&rig.app);
    assert!(
        !following(&rig.app),
        "the fixture is taller than the screen"
    );
    rig.live_update(ModelUpdate::Appended(replies(&["t9", "t10", "t11"])));
    assert_eq!(offset(&rig.app), top);
    assert_eq!(new_in_transcript(&rig.app), 3);
    assert_eq!(rig.app.new_sections(), 3);
    assert_eq!(status(&rig.app), ["↓ 3 new"]);
    rig.live_update(ModelUpdate::Appended(replies(&["t12"])));
    assert_eq!(status(&rig.app), ["↓ 4 new"]);
    // `G` resumes following; the key's report reaches the app as a Msg.
    rig.press(Key::Char('G'));
    rig.settle();
    assert!(following(&rig.app));
    assert_eq!(rig.app.new_sections(), 0);
    assert!(status(&rig.app).is_empty());
    assert!(text(&mut rig.app, 80, 24).contains("A reply for t12."));
}

#[test]
fn a_resize_that_shows_the_end_again_removes_a_stale_indicator() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_live();
    rig.live_update(ModelUpdate::Appended(replies(&["t9"])));
    assert_eq!(status(&rig.app), ["↓ 1 new"]);
    // Tall enough for the whole transcript: the end is in view again.
    rig.app.update(Msg::Resized(80, 120));
    assert!(following(&rig.app));
    assert!(status(&rig.app).is_empty());
    assert_eq!(rig.app.new_sections(), 0);
}

#[test]
fn the_end_key_resumes_following_too() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_live();
    rig.live_update(ModelUpdate::Appended(replies(&["t9"])));
    assert_eq!(status(&rig.app), ["↓ 1 new"]);
    rig.press(Key::End);
    rig.settle();
    assert!(status(&rig.app).is_empty());
    assert!(following(&rig.app));
}

#[test]
fn the_new_sections_message_shows_and_removes_the_indicator() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.update(Msg::Transcript(TranscriptMsg::NewSections {
        count: 3,
        selection: None,
    }));
    assert_eq!(status(&rig.app), ["↓ 3 new"]);
    assert_eq!(rig.app.new_sections(), 3);
    rig.app.update(Msg::Transcript(TranscriptMsg::NewSections {
        count: 0,
        selection: None,
    }));
    assert!(status(&rig.app).is_empty());
    assert_eq!(rig.app.new_sections(), 0);
}

#[test]
fn a_search_key_that_reaches_the_end_removes_the_indicator_with_its_report() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.update(Msg::Transcript(TranscriptMsg::NewSections {
        count: 3,
        selection: None,
    }));
    assert_eq!(status(&rig.app), ["↓ 3 new"]);
    // `n` wrapped to the last match, at the end: following resumes.
    rig.app
        .update(Msg::Transcript(TranscriptMsg::SearchPosition {
            position: Some(crate::transcript_search::SearchPosition {
                current: 5,
                total: 5,
            }),
            view: crate::app::ViewChange {
                changed: true,
                new_sections: Some(0),
                selection: None,
            },
        }));
    assert_eq!(status(&rig.app), ["5/5"]);
    assert_eq!(rig.app.new_sections(), 0);
}

#[test]
fn the_indicator_and_the_load_progress_show_together() {
    let mut rig = rig();
    rig.app.update(Msg::Browser(BrowserMsg::OpenSession(
        CLAUDE,
        id(BRIO_SESSION),
    )));
    rig.app
        .update(session_msg(&rig.app, ModelUpdate::Progress(0.5)));
    rig.app.update(Msg::Transcript(TranscriptMsg::NewSections {
        count: 2,
        selection: None,
    }));
    assert_eq!(status(&rig.app), ["Loading… 50%", "↓ 2 new"]);
}

#[test]
fn an_update_marks_the_screen_dirty() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.take_redraw();
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(replies(&["t9"])),
    ));
    assert_eq!(rig.app.take_redraw(), RedrawRequest::Dirty);
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Updated(replies(&["t9"])),
    ));
    assert_eq!(rig.app.take_redraw(), RedrawRequest::Dirty);
}

#[test]
fn a_new_session_starts_without_an_indicator_or_the_marker() {
    let mut rig = rig();
    rig.open_live();
    rig.live_update(ModelUpdate::Appended(replies(&["t9"])));
    assert_eq!(rig.app.new_sections(), 1);
    assert!(rig.app.session_is_live());
    rig.app.update(Msg::Browser(BrowserMsg::OpenSelector));
    rig.app.update(Msg::Browser(BrowserMsg::OpenSession(
        CLAUDE,
        id(NEWER_SESSION),
    )));
    assert_eq!(rig.app.new_sections(), 0);
    assert!(!rig.app.session_is_live());
    assert!(status(&rig.app).is_empty());
    assert!(!header_live(&rig.app));
}

// -- the initial load

#[test]
fn while_the_load_streams_the_view_stays_at_the_top() {
    let mut rig = rig_with(theme("default"), 80, 24);
    let all = turns();
    rig.harness.script(
        BRIO_SESSION,
        vec![
            ModelUpdate::Appended(all[..3].to_vec()),
            ModelUpdate::Progress(0.4),
            ModelUpdate::Appended(all[3..].to_vec()),
            ModelUpdate::Progress(0.9),
        ],
    );
    rig.app.update(Msg::Browser(BrowserMsg::OpenSession(
        CLAUDE,
        id(BRIO_SESSION),
    )));
    rig.pump_until(|app| turn_count(app) == all.len());
    assert_eq!(offset(&rig.app), 0);
    assert!(!following(&rig.app));
    assert_eq!(rig.app.new_sections(), 0, "the load is not new content");
    assert_eq!(status(&rig.app), ["Loading… 90%"]);
}

#[test]
fn after_loaded_a_tall_session_does_not_follow() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_live();
    assert_eq!(offset(&rig.app), 0);
    assert!(!following(&rig.app));
}

#[test]
fn after_loaded_a_session_that_fits_follows() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.harness.script(
        BRIO_SESSION,
        vec![
            ModelUpdate::Appended(replies(&["t1", "t2"])),
            ModelUpdate::Loaded,
        ],
    );
    rig.open(BRIO_SESSION);
    assert!(following(&rig.app));
    // So a long turn that arrives scrolls into view.
    let long = arc(reply(
        "t3",
        &(1..=60)
            .map(|n| format!("- item {n}\n"))
            .collect::<String>(),
    ));
    rig.live_update(ModelUpdate::Appended(vec![long]));
    assert!(offset(&rig.app) > 0);
    assert!(text(&mut rig.app, 80, 24).contains("item 60"));
}

// -- in-place updates

#[test]
fn an_updated_turn_keeps_the_selection_and_the_scroll_offset_through_the_app() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_live();
    rig.press(Key::Char('j'));
    rig.press(Key::Char('j'));
    rig.settle();
    let before = text(&mut rig.app, 80, 24);
    let top = offset(&rig.app);
    let mut changed = (*turns()[1]).clone();
    if let Turn::Assistant(a) = &mut changed {
        a.model = "claude-opus-5.5".to_owned();
    }
    rig.live_update(ModelUpdate::Updated(vec![Arc::new(changed)]));
    assert_eq!(offset(&rig.app), top);
    assert_eq!(text(&mut rig.app, 80, 24), before);
    assert_eq!(status(&rig.app), Vec::<String>::new());
}

// -- the live marker

#[test]
fn the_marker_shows_for_activity_within_60_seconds() {
    let mut rig = rig();
    rig.open_live();
    assert!(rig.app.session_is_live());
    assert!(header_live(&rig.app));
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    assert!(rows[2].ends_with("● live │"), "{}", rows[2]);
}

#[test]
fn the_marker_goes_out_on_the_tick_after_60_seconds() {
    let mut rig = rig();
    rig.open_live();
    // Activity was 10 s before `now`.
    rig.set_now(now() + Span::seconds(50));
    rig.app.update(Msg::Tick);
    assert!(rig.app.session_is_live(), "exactly 60 s old is still live");
    rig.set_now(now() + Span::seconds(51));
    rig.app.update(Msg::Tick);
    assert!(!rig.app.session_is_live());
    assert!(!header_live(&rig.app));
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    assert!(!rows[2].contains("live"), "{}", rows[2]);
}

#[test]
fn the_marker_is_re_evaluated_only_on_tick_changes_and_new_activity() {
    let mut rig = rig();
    rig.open_live();
    rig.set_now(now() + Span::seconds(300));
    rig.app.update(Msg::Tick);
    assert!(!rig.app.session_is_live());
    // New activity lights it again.
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Activity {
            last_modified: SystemTime::from(now() + Span::seconds(299)),
            idle: None,
        },
    ));
    assert!(header_live(&rig.app));
    // A tick that changes nothing asks for no redraw.
    rig.app.take_redraw();
    rig.app.update(Msg::Tick);
    assert_eq!(rig.app.take_redraw(), RedrawRequest::None);
}

#[test]
fn not_idle_keeps_the_marker_however_old_the_activity_is() {
    let mut rig = rig();
    rig.harness.script(
        BRIO_SESSION,
        vec![
            ModelUpdate::Appended(turns()),
            ModelUpdate::Loaded,
            ModelUpdate::Activity {
                last_modified: SystemTime::from(now() - Span::hours(5)),
                idle: Some(false),
            },
        ],
    );
    rig.open(BRIO_SESSION);
    rig.settle();
    assert!(rig.app.session_is_live());
    rig.set_now(now() + Span::hours(3));
    rig.app.update(Msg::Tick);
    assert!(rig.app.session_is_live());
    // Once the harness says it is idle, the age decides.
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Activity {
            last_modified: SystemTime::from(now() - Span::hours(5)),
            idle: Some(true),
        },
    ));
    assert!(!rig.app.session_is_live());
}

#[test]
fn a_session_without_activity_is_not_live() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.update(Msg::Tick);
    assert!(!rig.app.session_is_live());
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    assert!(!rows[2].contains("live"));
}

// -- screens

fn parsed_with_warnings_and_live() -> Rig {
    let mut rig = rig();
    rig.harness.script(
        BRIO_SESSION,
        vec![
            ModelUpdate::Appended(turns()),
            ModelUpdate::ParseWarnings(2),
            ModelUpdate::Loaded,
            activity(Span::seconds(5)),
        ],
    );
    rig.open(BRIO_SESSION);
    rig.settle();
    rig
}

#[test]
fn live_session_at_the_bottom_120x40_text() {
    let mut rig = rig();
    rig.open_live();
    rig.press(Key::Char('G'));
    rig.settle();
    assert!(following(&rig.app));
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn live_session_at_the_bottom_120x40_styled() {
    let mut rig = rig();
    rig.open_live();
    rig.press(Key::Char('G'));
    rig.settle();
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

#[test]
fn live_session_at_the_bottom_80x24_text() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_live();
    rig.press(Key::Char('G'));
    rig.settle();
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn scrolled_up_after_three_new_sections_80x24_text() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_live();
    rig.press(Key::Char('G'));
    rig.key(Key::Char('u'), KeyModifiers::CONTROL);
    rig.settle();
    assert!(!following(&rig.app));
    let before = screen_text(&mut rig, 80, 24);
    rig.live_update(ModelUpdate::Appended(replies(&["t9", "t10", "t11"])));
    let after = screen_text(&mut rig, 80, 24);
    let rows = |text: &str| text.lines().map(str::to_owned).collect::<Vec<_>>();
    let (before, after) = (rows(&before), rows(&after));
    assert_eq!(before[..23], after[..23], "the view is unchanged");
    assert!(!before[23].contains("new"));
    assert!(after[23].trim_end().ends_with("↓ 3 new"), "{}", after[23]);
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn after_g_the_new_content_is_visible_and_the_indicator_is_gone_80x24_text() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_live();
    rig.press(Key::Char('G'));
    rig.key(Key::Char('u'), KeyModifiers::CONTROL);
    rig.settle();
    rig.live_update(ModelUpdate::Appended(replies(&["t9", "t10", "t11"])));
    assert_eq!(status(&rig.app), ["↓ 3 new"]);
    rig.press(Key::Char('G'));
    rig.settle();
    let text = screen_text(&mut rig, 80, 24);
    assert!(text.contains("A reply for t11."), "{text}");
    assert!(!text.contains("new"), "{text}");
    insta::assert_snapshot!(text);
}

#[test]
fn header_with_the_parse_warning_and_the_live_marker_120x40_text() {
    let mut rig = parsed_with_warnings_and_live();
    let text = screen_text(&mut rig, 120, 40);
    let row = text.lines().nth(2).unwrap();
    assert!(
        row.ends_with("2 entries could not be parsed  ● live │"),
        "{row}"
    );
    insta::assert_snapshot!(text);
}

#[test]
fn the_indicator_is_drawn_in_help_text_80x24_styled() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_live();
    rig.live_update(ModelUpdate::Appended(replies(&["t9", "t10", "t11"])));
    let buffer = screen_buffer(&mut rig, 80, 24);
    let help_text = theme("default").style(crate::theme::Token::HelpText).fg;
    // `↓ 3 new` ends one column before the edge.
    for x in 72..79 {
        assert_eq!(buffer[(x, 23)].fg, help_text.unwrap(), "col {x}");
    }
    assert_eq!(buffer[(72, 23)].symbol(), "↓");
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}
