//! The session stats of the OpenCode adapter (SPEC cycle 3 §3): turns, agent
//! time, run time and the whole token use.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use super::testing::TempDb;
use super::*;
use crate::duration::format_duration;
use crate::harness::Harness;
use crate::model::{ModelUpdate, SessionId, SessionSummary, TokenUsage, Turn};

fn harness(db: &TempDb) -> OpenCodeHarness {
    OpenCodeHarness::with_db_path(db.path.clone(), Default::default())
}

fn summary(h: &OpenCodeHarness, id: &str) -> SessionSummary {
    for project in h.projects().unwrap() {
        for session in h.sessions(&project.id).unwrap() {
            if session.id.as_str() == id {
                return session;
            }
        }
    }
    panic!("no session {id}");
}

/// The turns the browser shows for the session, as `load` delivers them.
fn loaded_turns(h: &OpenCodeHarness, id: &str) -> Vec<Arc<Turn>> {
    let (tx, rx) = mpsc::channel();
    let _handle = h.load(&SessionId::from(id), tx).unwrap();
    let mut turns = Vec::new();
    while let Ok(update) = rx.recv_timeout(Duration::from_secs(30)) {
        update.apply(&mut turns);
        if matches!(update, ModelUpdate::Loaded) {
            break;
        }
    }
    turns
}

/// What the sidebar shows: the footer durations of the Assistant turns.
fn sidebar_durations(turns: &[Arc<Turn>]) -> Vec<Duration> {
    turns
        .iter()
        .filter_map(|turn| match &**turn {
            Turn::Assistant(t) => t.stats.as_ref().map(|s| s.duration),
            _ => None,
        })
        .collect()
}

fn usage(input: u64, output: u64, cache_read: u64, cache_write: u64, reasoning: u64) -> TokenUsage {
    TokenUsage {
        input,
        output,
        cache_read,
        cache_write,
        reasoning,
    }
}

/// A `session_v2` row.
fn add_session(db: &TempDb, id: &str, parent: Option<&str>, created: i64, updated: i64) {
    let parent = parent.map_or("NULL".to_owned(), |p| format!("'{p}'"));
    db.exec(&format!(
        "INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, \
         time_created, time_updated) VALUES ('{id}', 'prj_alpha', {parent}, '{id}', \
         '/home/user/alpha', '{id}', '1.0.0', {created}, {updated});"
    ));
}

/// A `session_message` row of any type.
fn add_row(db: &TempDb, id: &str, session: &str, kind: &str, seq: i64, created: i64, data: &str) {
    db.exec(&format!(
        "INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, \
         data) VALUES ('{id}', '{session}', '{kind}', {seq}, {created}, {created}, '{data}');"
    ));
}

fn user_row(db: &TempDb, id: &str, session: &str, seq: i64, created: i64) {
    let data = format!(r#"{{"time":{{"created":{created}}},"text":"Go","files":[],"agents":[]}}"#);
    add_row(db, id, session, "user", seq, created, &data);
}

/// An assistant row that took `millis`, with the given `tokens` object.
fn assistant_row(
    db: &TempDb,
    id: &str,
    session: &str,
    seq: i64,
    created: i64,
    millis: i64,
    tokens: &str,
) {
    let end = created + millis;
    let data = format!(
        r#"{{"time":{{"created":{created},"completed":{end}}},"model":{{"id":"m","providerID":"p"}},"content":[],"tokens":{tokens}}}"#
    );
    add_row(db, id, session, "assistant", seq, created, &data);
}

const T0: i64 = 1_793_000_000_000;

#[test]
fn stats_are_the_user_rows_the_footer_sum_the_time_span_and_the_tokens() {
    let db = TempDb::new();
    let h = harness(&db);
    let stats = h.stats(&summary(&h, "ses_main")).unwrap();
    assert_eq!(stats.turns, 2);
    // `time_updated` minus `time_created`, idle time included.
    assert_eq!(stats.run_time, Duration::from_secs(30 * 60));
    // The footers are 17 s and 39 s.
    assert_eq!(stats.agent_time, Duration::from_secs(56));
    // The main session's four rows and the two subagents' one row each.
    assert_eq!(stats.tokens, usage(3800 + 200, 380 + 40, 0, 0, 0));

    let blank = h.stats(&summary(&h, "ses_blank")).unwrap();
    assert_eq!(blank.turns, 0);
    assert_eq!(blank.agent_time, Duration::ZERO);
    assert_eq!(blank.run_time, Duration::ZERO);
    assert_eq!(blank.tokens, TokenUsage::default());
    assert_eq!(h.stats(&summary(&h, "ses_untitled")).unwrap().turns, 1);
}

#[test]
fn the_agent_time_is_the_sum_of_the_durations_the_sidebar_shows() {
    let db = TempDb::new();
    let h = harness(&db);
    for id in [
        "ses_main",
        "ses_untitled",
        "ses_bad",
        "ses_long",
        "ses_blank",
    ] {
        let sidebar: Duration = sidebar_durations(&loaded_turns(&h, id)).into_iter().sum();
        let stats = h.stats(&summary(&h, id)).unwrap();
        assert_eq!(stats.agent_time, sidebar, "{id}");
    }
    let sidebar = sidebar_durations(&loaded_turns(&h, "ses_main"));
    assert_eq!(
        sidebar,
        [Duration::from_secs(17), Duration::from_secs(39)],
        "the fixture's footers"
    );
}

#[test]
fn two_turns_of_idle_time_apart_add_their_durations_and_run_time_is_the_span() {
    let db = TempDb::new();
    add_session(&db, "ses_two", None, T0, T0 + 10 * 3_600_000);
    user_row(&db, "u1", "ses_two", 1, T0);
    assistant_row(&db, "a1", "ses_two", 2, T0 + 1_000, 30_000, "{}");
    user_row(&db, "u2", "ses_two", 3, T0 + 9 * 3_600_000);
    assistant_row(
        &db,
        "a2",
        "ses_two",
        4,
        T0 + 9 * 3_600_000 + 1_000,
        12_000,
        "{}",
    );
    let h = harness(&db);
    let stats = h.stats(&summary(&h, "ses_two")).unwrap();
    assert_eq!(stats.turns, 2);
    assert_eq!(stats.agent_time, Duration::from_secs(42));
    assert_eq!(stats.run_time, Duration::from_secs(10 * 3600));
}

#[test]
fn the_sum_is_taken_before_it_is_formatted() {
    let db = TempDb::new();
    add_session(&db, "ses_sum", None, T0, T0 + 3_600_000 * 3);
    // 1h 0m 59s twice: each shows as `1h 0m`, the sum is 2h 1m 58s.
    let each = 3_600_000 + 59_000;
    user_row(&db, "u1", "ses_sum", 1, T0);
    assistant_row(&db, "a1", "ses_sum", 2, T0, each, "{}");
    user_row(&db, "u2", "ses_sum", 3, T0 + 2 * each);
    assistant_row(&db, "a2", "ses_sum", 4, T0 + 2 * each, each, "{}");
    let h = harness(&db);
    let sidebar = sidebar_durations(&loaded_turns(&h, "ses_sum"));
    assert_eq!(sidebar.len(), 2);
    for duration in &sidebar {
        assert_eq!(format_duration(*duration), "1h 0m");
    }
    let stats = h.stats(&summary(&h, "ses_sum")).unwrap();
    assert_eq!(stats.agent_time, sidebar.iter().sum::<Duration>());
    assert_eq!(format_duration(stats.agent_time), "2h 1m");
}

#[test]
fn tokens_cover_the_child_and_the_grandchild_with_reasoning_in_output() {
    let db = TempDb::new();
    add_session(&db, "ses_grand", Some("ses_child1"), T0, T0 + 1_000);
    assistant_row(
        &db,
        "g1",
        "ses_grand",
        1,
        T0,
        1_000,
        r#"{"input":7,"output":10,"reasoning":4,"cache":{"read":300,"write":40}}"#,
    );
    // The columns of `session_v2` are not what the total is made of.
    db.exec("UPDATE session_v2 SET tokens_input = 987654, tokens_output = 876543;");
    let h = harness(&db);
    let stats = h.stats(&summary(&h, "ses_main")).unwrap();
    // The fixture's three sessions, then the grandchild: `output` is 10 + 4.
    assert_eq!(stats.tokens, usage(4000 + 7, 420 + 14, 300, 40, 4));
    // Total time and turns are those of the main session.
    assert_eq!(stats.turns, 2);
    assert_eq!(stats.run_time, Duration::from_secs(30 * 60));
    assert_eq!(stats.agent_time, Duration::from_secs(56));
}

#[test]
fn a_cycle_in_parent_id_does_not_hang_or_count_twice() {
    let db = TempDb::new();
    add_session(&db, "ses_grand", Some("ses_child1"), T0, T0 + 1_000);
    assistant_row(
        &db,
        "g1",
        "ses_grand",
        1,
        T0,
        1_000,
        r#"{"input":1,"output":1,"cache":{"read":0,"write":0}}"#,
    );
    let h = harness(&db);
    let main = summary(&h, "ses_main");
    // main <- child1 <- grand <- main
    db.exec("UPDATE session_v2 SET parent_id = 'ses_grand' WHERE id = 'ses_main';");
    let stats = h.stats(&main).unwrap();
    assert_eq!(stats.tokens.input, 4000 + 1);
    assert_eq!(stats.tokens.output, 420 + 1);
}

#[test]
fn a_row_that_does_not_parse_adds_no_tokens_and_a_bad_tokens_field_counts_as_zero() {
    let db = TempDb::new();
    let h = harness(&db);
    // `ses_bad` has an assistant row of `{not json` and one with 10 / 2.
    let stats = h.stats(&summary(&h, "ses_bad")).unwrap();
    assert_eq!(stats.tokens, usage(10, 2, 0, 0, 0));

    add_session(&db, "ses_odd", None, T0, T0 + 1_000);
    assistant_row(&db, "o1", "ses_odd", 1, T0, 1_000, r#""many""#);
    assistant_row(
        &db,
        "o2",
        "ses_odd",
        2,
        T0 + 1,
        1_000,
        r#"{"input":"x","output":5}"#,
    );
    add_row(&db, "o3", "ses_odd", "assistant", 3, T0 + 2, "[1,2]");
    let h = harness(&db);
    let stats = h.stats(&summary(&h, "ses_odd")).unwrap();
    assert_eq!(stats.tokens, usage(0, 5, 0, 0, 0));
}

#[test]
fn a_session_with_more_rows_than_a_page_is_summed_over_all_pages() {
    let db = TempDb::new();
    add_session(&db, "ses_pages", None, T0, T0 + 100_000);
    for i in 0..7_i64 {
        user_row(
            &db,
            &format!("pu{i}"),
            "ses_pages",
            2 * i + 1,
            T0 + 10_000 * i,
        );
        assistant_row(
            &db,
            &format!("pa{i}"),
            "ses_pages",
            2 * i + 2,
            T0 + 10_000 * i + 1_000,
            2_000,
            r#"{"input":1,"output":1}"#,
        );
    }
    let h = harness(&db).with_page_size(3);
    let stats = h.stats(&summary(&h, "ses_pages")).unwrap();
    assert_eq!(stats.turns, 7);
    assert_eq!(stats.agent_time, Duration::from_secs(14));
    assert_eq!(stats.tokens, usage(7, 7, 0, 0, 0));
}
