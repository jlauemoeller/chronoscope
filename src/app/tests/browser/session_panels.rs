//! The Session Time and Session Tokens panels over an open session
//! (`Ctrl-o`, product §3.1): they show the open session's values on the first
//! frame the selector is drawn, and again after it was closed and reopened.

use super::*;
use crate::app::fixtures::full_stats_event;
use crate::app::tests::session_panels_values::{expected, numbered, shown};

#[test]
fn the_panels_show_the_open_sessions_values_on_the_first_frame_after_ctrl_o() {
    let mut rig = rig();
    rig.app.update(fixtures::message(full_stats_event(
        CLAUDE,
        BRIO_SESSION,
        numbered(3),
    )));
    rig.open_loaded();
    assert!(!rig.app.application().mounted(&Id::SessionTime));
    // The key goes through the port and `GlobalEvents`; one message later
    // the selector is on screen with the open session selected.
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    assert_eq!(
        rig.app.selected_session().map(SessionId::as_str),
        Some(BRIO_SESSION)
    );
    assert_eq!(shown(&mut rig.app), expected(3));
}

#[test]
fn they_show_them_again_after_the_selector_was_closed_and_reopened() {
    let mut rig = rig();
    rig.app.update(fixtures::message(full_stats_event(
        CLAUDE,
        BRIO_SESSION,
        numbered(3),
    )));
    rig.open_loaded();
    for _ in 0..3 {
        rig.app.update(Msg::Browser(BrowserMsg::OpenSelector));
        assert_eq!(shown(&mut rig.app), expected(3));
        rig.app.update(Msg::Browser(BrowserMsg::CloseSelector));
        assert!(!rig.app.application().mounted(&Id::SessionTokens));
    }
}

#[test]
fn stats_that_arrive_behind_the_browser_are_shown_when_the_selector_opens() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.update(Msg::Browser(BrowserMsg::OpenSelector));
    assert_eq!(shown(&mut rig.app), Vec::new(), "no stats yet");
    rig.app.update(Msg::Browser(BrowserMsg::CloseSelector));
    rig.app.update(fixtures::message(full_stats_event(
        CLAUDE,
        BRIO_SESSION,
        numbered(5),
    )));
    rig.app.update(Msg::Browser(BrowserMsg::OpenSelector));
    assert_eq!(shown(&mut rig.app), expected(5));
    // Stats that arrive while the selector is open show at once.
    rig.app.update(fixtures::message(full_stats_event(
        CLAUDE,
        BRIO_SESSION,
        numbered(6),
    )));
    assert_eq!(shown(&mut rig.app), expected(6));
}

#[test]
fn another_session_s_values_do_not_stay_after_opening_it_and_the_selector_again() {
    let mut rig = rig();
    for (session, n) in [(BRIO_SESSION, 3), (NEWER_SESSION, 4)] {
        rig.app.update(fixtures::message(full_stats_event(
            CLAUDE,
            session,
            numbered(n),
        )));
    }
    rig.harness.script(NEWER_SESSION, vec![ModelUpdate::Loaded]);
    rig.open_loaded();
    rig.app.update(Msg::Browser(BrowserMsg::OpenSelector));
    assert_eq!(shown(&mut rig.app), expected(3));
    rig.app.update(Msg::Browser(BrowserMsg::OpenSession(
        CLAUDE,
        id(NEWER_SESSION),
    )));
    rig.pump_until(App::session_loaded);
    rig.app.update(Msg::Browser(BrowserMsg::OpenSelector));
    assert_eq!(
        rig.app.selected_session().map(SessionId::as_str),
        Some(NEWER_SESSION)
    );
    assert_eq!(shown(&mut rig.app), expected(4));
}
