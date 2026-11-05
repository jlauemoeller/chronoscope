//! Tests of what the Session Time and Session Tokens panels show in the app
//! (no terminal), SPEC §4.3: the values follow the selected session by every
//! route, update as stats arrive and clear when nothing is selected.
//!
//! The panels are read from the screen as drawn, because the rule is about
//! what is shown in the same frame.

use std::time::Duration;

use tuirealm::command::Direction;
use tuirealm::event::{Key, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use super::fixtures::{fixture_snapshot, full_stats_event, search_snapshot, worked_example_stats};
use super::redraw::RedrawRequest;
use super::selector_tests::{
    CLAUDE, OPENCODE, empty_app, empty_app_in, focus_sessions, press, send, styled, text, theme,
    two_harnesses,
};
use super::{App, Id, Msg, Pane, SelectorMsg};
use crate::components::{SESSION_TIME_ATTR, SESSION_TOKENS_ATTR, SessionTimes, payload_from_value};
use crate::model::{HarnessId, ProjectId, SessionId, SessionStats, TokenUsage};
use crate::test_support::{buffer_to_text, render};

// -- data

/// Stats that tell sessions apart: session number `n` (1 to 9) has a run time
/// of `n` hours, `n` minutes of agent time, `n` thousand input tokens, `2n`
/// million output, `n` hundred thinking, `5n` thousand cache reads and `n`
/// cache writes. See [`expected`].
pub(super) fn numbered(n: u64) -> SessionStats {
    SessionStats {
        turns: usize::try_from(n).unwrap(),
        agent_time: Duration::from_secs(n * 60),
        run_time: Duration::from_secs(n * 3600),
        tokens: TokenUsage {
            input: n * 1_000,
            output: n * 2_000_000,
            reasoning: n * 100,
            cache_read: n * 5_000,
            cache_write: n,
        },
    }
}

type Rows = Vec<(String, String)>;

fn pair(label: &str, value: impl Into<String>) -> (String, String) {
    (label.to_owned(), value.into())
}

/// The rows both panels show for [`numbered`]`(n)`, as written out by hand.
pub(super) fn expected(n: u64) -> Rows {
    vec![
        pair("Total", format!("{n}h 0m")),
        pair("Agent", format!("{n}m 0s")),
        pair("Input", format!("{n}.0k")),
        pair("Output", format!("{}.0M", 2 * n)),
        pair("Thinking", format!("{n}00")),
        pair("Cache read", format!("{}.0k", 5 * n)),
        pair("Cache write", format!("{n}")),
    ]
}

/// The rows both panels show for the worked example of the specification.
fn worked_example() -> Rows {
    vec![
        pair("Total", "81h 33m"),
        pair("Agent", "17h 56m"),
        pair("Input", "15.8k"),
        pair("Output", "6.3M"),
        pair("Thinking", "2.2M"),
        pair("Cache read", "862.3M"),
        pair("Cache write", "30.7M"),
    ]
}

fn stats_of(app: &mut App, harness: HarnessId, session: &str, n: u64) {
    send(app, full_stats_event(harness, session, numbered(n)));
}

/// The fixture app (120x40, Claude): `s1`, `s2`, `s4` and one session of
/// another project (`other1`) have stats 1, 2, 4 and 5; `s3` and `s5` have
/// none yet.
pub(super) fn with_stats() -> App {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    send(&mut app, fixture_snapshot(CLAUDE));
    for (session, n) in [("s1", 1), ("s2", 2), ("s4", 4), ("other1", 5)] {
        stats_of(&mut app, CLAUDE, session, n);
    }
    app
}

// -- reading the panels

/// What the two panels show as drawn now, row by row.
pub(super) fn shown(app: &mut App) -> Rows {
    let (width, height) = app.size();
    let buffer = render(width, height, |frame| app.view(frame));
    let text = buffer_to_text(&buffer);
    let lines: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
    let mut rows = Vec::new();
    for id in [Id::SessionTime, Id::SessionTokens] {
        let Some(area) = app.routing().area(id) else {
            continue;
        };
        for y in area.y + 1..area.bottom() - 1 {
            let line = &lines[usize::from(y)];
            let inside: String = line[usize::from(area.x) + 1..usize::from(area.right()) - 1]
                .iter()
                .collect();
            let inside = inside.trim();
            if inside.is_empty() {
                continue;
            }
            // The label and the value are apart by a run of blanks.
            let (label, value) = inside.split_once("  ").expect("a label and a value");
            rows.push(pair(label.trim(), value.trim()));
        }
    }
    rows
}

/// What the components hold, from their attributes.
fn held(app: &App) -> (Option<SessionTimes>, Option<TokenUsage>) {
    let times = app
        .application()
        .query(&Id::SessionTime, SESSION_TIME_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    let tokens = app
        .application()
        .query(&Id::SessionTokens, SESSION_TOKENS_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    (
        payload_from_value(&times).unwrap(),
        payload_from_value(&tokens).unwrap(),
    )
}

fn selected(app: &App) -> Option<&str> {
    app.selected_session().map(SessionId::as_str)
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Msg {
    Msg::Mouse(MouseEvent {
        kind,
        modifiers: KeyModifiers::NONE,
        column,
        row,
    })
}

/// A left press on list row `line` (0 is the first drawn) of `id`.
fn click_row(app: &mut App, id: Id, line: u16) {
    let area = app.routing().area(id).unwrap();
    let press = MouseEventKind::Down(MouseButton::Left);
    app.update(mouse(press, area.x + 6, area.y + 2 + line));
}

fn wheel_down(app: &mut App, id: Id) {
    let area = app.routing().area(id).unwrap();
    app.update(mouse(MouseEventKind::ScrollDown, area.x + 6, area.y + 3));
}

fn project(app: &mut App, name: &str) {
    let id = ProjectId::from(format!("/home/jacob/Development/projects/{name}"));
    app.update(Msg::Selector(SelectorMsg::ProjectSelected(id)));
}

// -- showing the selected session

#[test]
fn with_nothing_selected_both_panels_are_empty() {
    let mut app = with_stats();
    assert_eq!(selected(&app), None);
    assert_eq!(shown(&mut app), Vec::new());
    assert_eq!(held(&app), (None, None));
}

#[test]
fn selecting_a_session_shows_its_values_in_both_panels() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    assert_eq!(selected(&app), Some("s1"));
    assert_eq!(shown(&mut app), expected(1));
    press(&mut app, Key::Char('j'));
    assert_eq!(selected(&app), Some("s2"));
    assert_eq!(shown(&mut app), expected(2));
}

#[test]
fn the_worked_example_reads_as_in_the_wireframe() {
    let mut app = with_stats();
    send(
        &mut app,
        full_stats_event(CLAUDE, "s1", worked_example_stats()),
    );
    focus_sessions(&mut app);
    assert_eq!(shown(&mut app), worked_example());
    let screen = buffer_to_text(&render(120, 40, |frame| app.view(frame)));
    let total = screen.lines().find(|l| l.contains("Total")).unwrap();
    assert!(
        total.contains("Total") && total.contains("81h 33m │"),
        "{total}"
    );
}

#[test]
fn moving_to_another_project_clears_both_panels() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    assert_eq!(shown(&mut app), expected(1));
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    project(&mut app, "BRIO");
    assert_eq!(selected(&app), None);
    assert_eq!(shown(&mut app), Vec::new());
    assert_eq!(held(&app), (None, None));
}

#[test]
fn moving_to_another_project_by_a_key_clears_both_panels() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    press(&mut app, Key::BackTab);
    assert_eq!(shown(&mut app), expected(1));
    press(&mut app, Key::Char('j'));
    assert_eq!(shown(&mut app), Vec::new());
}

#[test]
fn selecting_a_session_with_no_stats_shows_no_values_from_the_one_before() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    press(&mut app, Key::Char('j'));
    assert_eq!(shown(&mut app), expected(2));
    press(&mut app, Key::Char('j'));
    assert_eq!(selected(&app), Some("s3"), "s3 has no stats yet");
    assert_eq!(shown(&mut app), Vec::new());
    assert_eq!(held(&app), (None, None));
    press(&mut app, Key::Char('j'));
    assert_eq!(shown(&mut app), expected(4));
}

#[test]
fn stats_arriving_for_the_selected_session_show_at_once() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    for _ in 0..4 {
        press(&mut app, Key::Char('j'));
    }
    assert_eq!(selected(&app), Some("s5"));
    assert_eq!(shown(&mut app), Vec::new());
    app.take_redraw();
    stats_of(&mut app, CLAUDE, "s5", 3);
    assert_eq!(app.take_redraw(), RedrawRequest::Dirty);
    assert_eq!(shown(&mut app), expected(3));
}

#[test]
fn stats_for_another_session_change_nothing() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    // One of the listed project, one of another project, one of a project
    // that is not listed, and one of an unknown session.
    for (session, n) in [("s3", 7), ("other2", 8), ("s5", 9), ("nobody", 6)] {
        stats_of(&mut app, CLAUDE, session, n);
        assert_eq!(shown(&mut app), expected(1), "{session}");
    }
    // Nor does a harness that is not shown, with the id of the session.
    stats_of(&mut app, OPENCODE, "s1", 9);
    assert_eq!(shown(&mut app), expected(1));
}

#[test]
fn a_running_sessions_values_follow_its_stats() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    assert_eq!(shown(&mut app), expected(1));
    stats_of(&mut app, CLAUDE, "s1", 6);
    assert_eq!(shown(&mut app), expected(6));
    stats_of(&mut app, CLAUDE, "s1", 7);
    assert_eq!(shown(&mut app), expected(7));
}

#[test]
fn until_a_newer_computation_finishes_the_last_known_values_stay() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    // The session changed on disk: the index sends the lists again, and the
    // stats of the session are computed again in the background.
    let mut changed = fixture_snapshot(CLAUDE);
    if let crate::index::IndexEvent::Snapshot { sessions, .. } = &mut changed {
        let mut map = (**sessions).clone();
        for list in map.values_mut() {
            for s in list.iter_mut().filter(|s| s.id.as_str() == "s1") {
                s.last_activity += chrono::Duration::minutes(5);
            }
        }
        *sessions = map.into();
    }
    send(&mut app, changed);
    app.update(Msg::Tick);
    assert_eq!(selected(&app), Some("s1"));
    assert_eq!(shown(&mut app), expected(1));
    stats_of(&mut app, CLAUDE, "s1", 2);
    assert_eq!(shown(&mut app), expected(2));
}

#[test]
fn a_session_that_disappears_while_selected_leaves_what_the_new_selection_has() {
    let mut app = with_stats();
    stats_of(&mut app, CLAUDE, "s3", 3);
    focus_sessions(&mut app);
    press(&mut app, Key::Char('j'));
    assert_eq!(selected(&app), Some("s2"));
    assert_eq!(shown(&mut app), expected(2));
    // A snapshot without `s2`: the selection stays at the same index, on s3.
    send(&mut app, snapshot_without(&["s2"]));
    assert_eq!(selected(&app), Some("s3"));
    assert_eq!(shown(&mut app), expected(3));
    // Without s3 as well, the selection falls on s4.
    send(&mut app, snapshot_without(&["s2", "s3"]));
    assert_eq!(selected(&app), Some("s4"));
    assert_eq!(shown(&mut app), expected(4));
    // And with no session left, the panels are empty.
    send(&mut app, snapshot_without(&["s1", "s2", "s3", "s4", "s5"]));
    assert_eq!(selected(&app), None);
    assert_eq!(shown(&mut app), Vec::new());
}

/// The fixture snapshot without the sessions `gone`.
fn snapshot_without(gone: &[&str]) -> crate::index::IndexEvent {
    let mut event = fixture_snapshot(CLAUDE);
    if let crate::index::IndexEvent::Snapshot { sessions, .. } = &mut event {
        let mut map = (**sessions).clone();
        for list in map.values_mut() {
            list.retain(|s| !gone.contains(&s.id.as_str()));
        }
        *sessions = map.into();
    }
    event
}

// -- the Agent value is the row's duration

/// The duration text of the row of the session titled `title`, as drawn.
fn row_duration(app: &mut App, title: &str) -> String {
    let (width, height) = app.size();
    let text = buffer_to_text(&render(width, height, |frame| app.view(frame)));
    let line = text
        .lines()
        .find(|line| line.contains(title))
        .unwrap_or_else(|| panic!("no row {title}\n{text}"));
    let (_, tail) = line.rsplit_once("· ").expect("the row has its stats");
    tail.split('│').next().unwrap().trim().to_owned()
}

#[test]
fn the_agent_values_text_equals_the_selected_rows_duration() {
    let mut app = with_stats();
    let mut example = worked_example_stats();
    example.agent_time = Duration::from_millis(64_560_999);
    send(&mut app, full_stats_event(CLAUDE, "s1", example));
    let mut short = numbered(2);
    short.agent_time = Duration::from_millis(45_900);
    send(&mut app, full_stats_event(CLAUDE, "s2", short));
    focus_sessions(&mut app);
    for (session, title, text) in [
        ("s1", "Fix the flaky login", "17h 56m"),
        ("s2", "Refactor the session", "45s"),
        ("s4", "Add dark mode", "4m 0s"),
    ] {
        let id = SessionId::from(session);
        app.update(Msg::Selector(SelectorMsg::SessionSelected(id)));
        let rows = shown(&mut app);
        let agent = rows.iter().find(|(label, _)| label == "Agent").unwrap();
        assert_eq!(agent.1, text, "{title}");
        assert_eq!(agent.1, row_duration(&mut app, title), "{title}");
    }
}

// -- every route

#[test]
fn a_click_on_a_row_updates_the_panels() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    click_row(&mut app, Id::SessionList, 1);
    assert_eq!(selected(&app), Some("s2"));
    assert_eq!(shown(&mut app), expected(2));
    click_row(&mut app, Id::SessionList, 2);
    assert_eq!(selected(&app), Some("s3"));
    assert_eq!(shown(&mut app), Vec::new());
    // The first click on a row with the Projects pane focused too.
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    click_row(&mut app, Id::SessionList, 3);
    assert_eq!(selected(&app), Some("s4"));
    assert_eq!(shown(&mut app), expected(4));
}

#[test]
fn the_wheel_updates_the_panels() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    wheel_down(&mut app, Id::SessionList);
    assert_eq!(selected(&app), Some("s2"));
    assert_eq!(shown(&mut app), expected(2));
    wheel_down(&mut app, Id::SessionList);
    assert_eq!(shown(&mut app), Vec::new());
    wheel_down(&mut app, Id::SessionList);
    assert_eq!(shown(&mut app), expected(4));
}

#[test]
fn the_wheel_over_projects_clears_the_panels_with_the_session() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    assert_eq!(shown(&mut app), expected(1));
    wheel_down(&mut app, Id::ProjectList);
    assert_eq!(selected(&app), None);
    assert_eq!(shown(&mut app), Vec::new());
}

#[test]
fn switching_harness_updates_the_panels_and_stats_are_per_harness() {
    let mut app = two_harnesses();
    // `m00` is the first session of the second harness. Stats of a session of
    // that id in the first harness must not show there.
    send(&mut app, full_stats_event(CLAUDE, "m00", numbered(8)));
    send(&mut app, full_stats_event(CLAUDE, "s1", numbered(1)));
    focus_sessions(&mut app);
    assert_eq!(shown(&mut app), expected(1));
    app.update(Msg::Selector(SelectorMsg::SwitchHarness(OPENCODE)));
    assert_eq!(app.selected_harness(), Some(OPENCODE));
    assert_eq!(selected(&app), Some("m00"));
    assert_eq!(
        shown(&mut app),
        Vec::new(),
        "the stats of the other harness"
    );
    send(&mut app, full_stats_event(OPENCODE, "m00", numbered(3)));
    assert_eq!(shown(&mut app), expected(3));
    app.update(Msg::Selector(SelectorMsg::SwitchHarness(CLAUDE)));
    assert_eq!(selected(&app), Some("s1"));
    assert_eq!(shown(&mut app), expected(1));
}

#[test]
fn switching_harness_with_projects_focused_empties_the_panels() {
    let mut app = two_harnesses();
    send(&mut app, full_stats_event(CLAUDE, "s1", numbered(1)));
    focus_sessions(&mut app);
    assert_eq!(shown(&mut app), expected(1));
    app.update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    press(&mut app, Key::Char('2'));
    assert_eq!(selected(&app), None);
    assert_eq!(shown(&mut app), Vec::new());
}

// -- search

/// The search fixture of Claude with stats for `s2` (2), `s3` (3), `b1` (4),
/// `b2` (5) and `c2` (6): all of them match `brio`.
fn searchable() -> App {
    let mut app = empty_app_in(&[CLAUDE], theme("default"), "/nowhere");
    send(&mut app, search_snapshot(CLAUDE));
    for (session, n) in [
        ("s1", 1),
        ("s2", 2),
        ("s3", 3),
        ("b1", 4),
        ("b2", 5),
        ("c2", 6),
    ] {
        stats_of(&mut app, CLAUDE, session, n);
    }
    app
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, Key::Char(c));
    }
}

fn searching(query: &str) -> App {
    let mut app = searchable();
    press(&mut app, Key::Char('/'));
    type_text(&mut app, query);
    app
}

/// The number [`searchable`] gave the session.
fn number_of(session: &str) -> u64 {
    match session {
        "s1" => 1,
        "s2" => 2,
        "s3" => 3,
        "b1" => 4,
        "b2" => 5,
        "c2" => 6,
        other => panic!("{other}"),
    }
}

#[test]
fn a_search_with_no_result_clears_both_panels() {
    let mut app = searchable();
    focus_sessions(&mut app);
    assert_eq!(shown(&mut app), expected(1));
    press(&mut app, Key::Char('/'));
    assert_eq!(
        shown(&mut app),
        expected(1),
        "an empty query lists the project"
    );
    type_text(&mut app, "brio");
    assert!(!app.search_results().is_empty());
    assert_ne!(shown(&mut app), Vec::new());
    type_text(&mut app, "zzzz");
    assert_eq!(app.search_results(), []);
    assert_eq!(shown(&mut app), Vec::new());
    assert_eq!(held(&app), (None, None));
}

#[test]
fn search_results_show_the_selected_results_values() {
    let mut app = searching("brio");
    let first = app.selected_result().expect("a result is selected");
    assert_eq!(shown(&mut app), expected(number_of(first.as_str())));
}

#[test]
fn moving_through_results_with_the_arrow_keys_updates_the_panels() {
    let mut app = searching("brio");
    let results = app.search_results().to_vec();
    assert!(results.len() >= 4, "{results:?}");
    let mut seen = Vec::new();
    for expected_id in &results[..results.len() - 1] {
        let id = app.selected_result().unwrap();
        assert_eq!(&id, expected_id);
        assert_eq!(shown(&mut app), expected(number_of(id.as_str())), "{id}");
        seen.push(number_of(id.as_str()));
        press(&mut app, Key::Down);
    }
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen.len(),
        results.len() - 1,
        "every result had its own values"
    );
    // And back up.
    press(&mut app, Key::Up);
    let id = app.selected_result().unwrap();
    assert_eq!(shown(&mut app), expected(number_of(id.as_str())));
}

#[test]
fn moving_through_results_one_update_per_move_shows_the_move_in_the_same_frame() {
    let mut app = searching("brio");
    for _ in 0..3 {
        let before = shown(&mut app);
        app.update(Msg::PromptNav(Direction::Down));
        let id = app.selected_result().unwrap();
        // Drawn once, right after the one message.
        let now = shown(&mut app);
        assert_eq!(now, expected(number_of(id.as_str())));
        assert_ne!(now, before);
    }
}

#[test]
fn a_click_on_a_search_result_updates_the_panels() {
    let mut app = searching("brio");
    let results = app.search_results().to_vec();
    for line in [2, 0, 3] {
        click_row(&mut app, Id::SessionList, line);
        let id = app.selected_result().unwrap();
        assert_eq!(id, results[usize::from(line)], "row {line}");
        assert_eq!(
            shown(&mut app),
            expected(number_of(id.as_str())),
            "row {line}"
        );
    }
}

#[test]
fn the_wheel_over_search_results_updates_the_panels() {
    let mut app = searching("brio");
    for _ in 0..3 {
        wheel_down(&mut app, Id::SessionList);
        let id = app.selected_result().unwrap();
        assert_eq!(shown(&mut app), expected(number_of(id.as_str())));
    }
}

#[test]
fn clearing_the_query_shows_the_project_s_selection_again() {
    let mut app = searchable();
    focus_sessions(&mut app);
    press(&mut app, Key::Char('/'));
    type_text(&mut app, "brio");
    press(&mut app, Key::Down);
    press(&mut app, Key::Down);
    for _ in 0..4 {
        press(&mut app, Key::Backspace);
    }
    assert_eq!(app.search_query(), Some(""));
    assert_eq!(selected(&app), Some("s1"));
    assert_eq!(shown(&mut app), expected(1));
    // Escape: back to what the search found.
    press(&mut app, Key::Esc);
    assert_eq!(shown(&mut app), expected(1));
}

#[test]
fn escape_on_a_search_that_started_with_nothing_selected_clears_the_panels() {
    let mut app = searchable();
    // Starting a search focuses Sessions and selects the newest session.
    press(&mut app, Key::Char('/'));
    assert_eq!(shown(&mut app), expected(1));
    type_text(&mut app, "brio");
    press(&mut app, Key::Esc);
    assert_eq!(selected(&app), None);
    assert_eq!(shown(&mut app), Vec::new());
}

#[test]
fn a_result_whose_stats_arrive_later_shows_them_at_once() {
    let mut app = searching("brio");
    let id = app.selected_result().unwrap();
    // Stats of a search result in another project than the listed one.
    send(&mut app, full_stats_event(CLAUDE, id.as_str(), numbered(9)));
    assert_eq!(shown(&mut app), expected(9));
}

// -- the filtered selector

fn filtered() -> App {
    let mut app = empty_app_in(&[CLAUDE], theme("default"), "/nowhere");
    send(&mut app, fixture_snapshot(CLAUDE));
    for (session, n) in [("s1", 1), ("s2", 2), ("s3", 3), ("s4", 4), ("other1", 5)] {
        stats_of(&mut app, CLAUDE, session, n);
    }
    app.update(Msg::Resized(120, 40));
    app
}

#[test]
fn the_filtered_selector_shows_the_first_matchs_values_and_follows_the_cursor() {
    let mut app = filtered();
    let matches: Vec<(HarnessId, SessionId)> = ["s2", "s3", "s4"]
        .into_iter()
        .map(|id| (CLAUDE, SessionId::from(id)))
        .collect();
    app.show_filtered("s", &matches);
    // The newest match first: s2.
    assert_eq!(shown(&mut app), expected(2));
    press(&mut app, Key::Char('j'));
    assert_eq!(shown(&mut app), expected(3));
    press(&mut app, Key::Char('j'));
    assert_eq!(shown(&mut app), expected(4));
    click_row(&mut app, Id::SessionList, 0);
    assert_eq!(shown(&mut app), expected(2));
    wheel_down(&mut app, Id::SessionList);
    assert_eq!(shown(&mut app), expected(3));
}

#[test]
fn leaving_the_filtered_selector_clears_the_panels() {
    let mut app = filtered();
    let matches = vec![(CLAUDE, SessionId::from("s2"))];
    app.show_filtered("s2", &matches);
    assert_eq!(shown(&mut app), expected(2));
    press(&mut app, Key::Esc);
    assert_eq!(app.filter_prefix(), None);
    assert_eq!(selected(&app), None);
    assert_eq!(shown(&mut app), Vec::new());
}

// -- nothing changing

#[test]
fn an_update_that_changes_nothing_about_the_panels_pushes_nothing() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    assert_eq!(shown(&mut app), expected(1));
    app.take_redraw();
    // A tick, the same selection again, a press that moves nothing, and stats
    // of a session of a project that is not listed.
    app.update(Msg::Tick);
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    app.update(Msg::Selector(SelectorMsg::SessionSelected(
        SessionId::from("s1"),
    )));
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    stats_of(&mut app, CLAUDE, "other2", 7);
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    app.push_session_panels();
    assert_eq!(app.take_redraw(), RedrawRequest::None);
    // The same stats again for the selected session: the list repaints its
    // row, but the panels were not pushed anything: nothing about them
    // changed, so the components still hold the same values.
    let before = held(&app);
    stats_of(&mut app, CLAUDE, "s1", 1);
    assert_eq!(held(&app), before);
}

#[test]
fn the_panels_push_when_something_does_change() {
    let mut app = with_stats();
    focus_sessions(&mut app);
    app.take_redraw();
    app.forget_pushed_panels();
    app.push_session_panels();
    assert_eq!(app.take_redraw(), RedrawRequest::Dirty);
    assert_eq!(shown(&mut app), expected(1));
}

// -- screens

/// The fixture app with the worked example for `s1`, `s1` selected.
fn example_selected() -> App {
    let mut app = with_stats();
    send(
        &mut app,
        full_stats_event(CLAUDE, "s1", worked_example_stats()),
    );
    focus_sessions(&mut app);
    app
}

/// The fixture app with `s3`, which has no stats yet, selected.
fn pending_selected() -> App {
    let mut app = with_stats();
    focus_sessions(&mut app);
    let id = SessionId::from("s3");
    app.update(Msg::Selector(SelectorMsg::SessionSelected(id)));
    app
}

#[test]
fn with_stats_160x50_text() {
    insta::assert_snapshot!(text(&mut example_selected(), 160, 50));
}

#[test]
fn with_stats_160x50_styled() {
    insta::assert_snapshot!(styled(&mut example_selected(), 160, 50));
}

#[test]
fn with_stats_60x33_text() {
    insta::assert_snapshot!(text(&mut example_selected(), 60, 33));
}

#[test]
fn with_stats_60x33_styled() {
    insta::assert_snapshot!(styled(&mut example_selected(), 60, 33));
}

#[test]
fn with_no_session_selected_160x50_text() {
    insta::assert_snapshot!(text(&mut with_stats(), 160, 50));
}

#[test]
fn with_no_session_selected_60x33_text() {
    insta::assert_snapshot!(text(&mut with_stats(), 60, 33));
}

#[test]
fn with_stats_pending_160x50_text() {
    insta::assert_snapshot!(text(&mut pending_selected(), 160, 50));
}

#[test]
fn with_stats_pending_60x33_text() {
    insta::assert_snapshot!(text(&mut pending_selected(), 60, 33));
}

#[test]
fn at_60x33_every_label_and_value_fits_whole() {
    let mut app = example_selected();
    app.update(Msg::Resized(60, 33));
    let screen = buffer_to_text(&render(60, 33, |frame| app.view(frame)));
    assert!(!screen.lines().any(|line| line.contains("Cache…")));
    assert_eq!(shown(&mut app), worked_example());
}

#[test]
fn a_theme_change_recolours_the_labels_and_values_of_both_panels() {
    use crate::theme::Role;

    let mut app = example_selected();
    let default = theme("default");
    let nord = theme("nord");
    assert_ne!(default.role(Role::Foreground), nord.role(Role::Foreground));
    assert_ne!(
        default.role(Role::MutedForeground),
        nord.role(Role::MutedForeground)
    );
    assert_ne!(
        nord.role(Role::Foreground),
        nord.role(Role::MutedForeground)
    );
    let check = |app: &mut App, theme: &crate::theme::Theme| {
        let (width, height) = app.size();
        let buffer = render(width, height, |frame| app.view(frame));
        for id in [Id::SessionTime, Id::SessionTokens] {
            let area = app.routing().area(id).unwrap();
            for y in area.y + 1..area.bottom() - 1 {
                let label = &buffer[(area.x + 2, y)];
                let value = &buffer[(area.right() - 3, y)];
                assert_eq!(label.fg, theme.role(Role::MutedForeground), "{id:?} {y}");
                assert_eq!(value.fg, theme.role(Role::Foreground), "{id:?} {y}");
            }
        }
    };
    check(&mut app, &default);
    app.apply_theme_id("nord");
    check(&mut app, &nord);
}
