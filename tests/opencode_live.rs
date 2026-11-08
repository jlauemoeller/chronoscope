//! Keeping an OpenCode session live (product §6.1): a separate read-write
//! connection plays OpenCode and writes to a temporary copy of the fixture
//! database, while the harness's worker watches (or polls) it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, UNIX_EPOCH};

use chronoscope::harness::opencode::OpenCodeHarness;
use chronoscope::harness::{Harness, WatchHandle};
use chronoscope::index::{IndexEvent, SessionIndex, Timing};
use chronoscope::model::{HarnessId, Item, ModelUpdate, SessionId, Turn};
use rusqlite::{Connection, params};
use tempfile::TempDir;

const FIXTURE: &str = include_str!("fixtures/opencode/fixture.sql");
const WAIT: Duration = Duration::from_secs(10);

struct Env {
    _dir: TempDir,
    path: PathBuf,
    /// OpenCode's own connection.
    writer: Connection,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opencode.db");
    let writer = Connection::open(&path).unwrap();
    writer.execute_batch(FIXTURE).unwrap();
    // Fast commits: the tests write many times.
    writer.execute_batch("PRAGMA synchronous = OFF").unwrap();
    Env {
        _dir: dir,
        path,
        writer,
    }
}

impl Env {
    fn harness(&self) -> OpenCodeHarness {
        OpenCodeHarness::with_db_path(self.path.clone(), BTreeMap::new())
    }

    fn polling(&self) -> OpenCodeHarness {
        self.harness().with_polling()
    }

    /// Inserts an assistant row with one text part.
    fn assistant(&self, session: &str, id: &str, seq: i64, text: &str, updated: i64) {
        self.writer
            .execute(
                "INSERT INTO session_message (id, session_id, type, seq, time_created, \
                 time_updated, data) VALUES (?1, ?2, 'assistant', ?3, ?4, ?4, ?5)",
                params![id, session, seq, updated, assistant_data(text, updated, 7)],
            )
            .unwrap();
    }

    fn user(&self, session: &str, id: &str, seq: i64, text: &str, updated: i64) {
        self.writer
            .execute(
                "INSERT INTO session_message (id, session_id, type, seq, time_created, \
                 time_updated, data) VALUES (?1, ?2, 'user', ?3, ?4, ?4, ?5)",
                params![
                    id,
                    session,
                    seq,
                    updated,
                    format!(r#"{{"time":{{"created":{updated}}},"text":"{text}"}}"#)
                ],
            )
            .unwrap();
    }

    /// Rewrites the data of a row and its `time_updated`.
    fn rewrite(&self, id: &str, data: &str, updated: i64) {
        let n = self
            .writer
            .execute(
                "UPDATE session_message SET data = ?2, time_updated = ?3 WHERE id = ?1",
                params![id, data, updated],
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    fn set_idle(&self, session: &str, idle: Option<i64>) {
        self.writer
            .execute(
                "UPDATE session_v2 SET time_idle = ?2 WHERE id = ?1",
                params![session, idle],
            )
            .unwrap();
    }
}

fn assistant_data(text: &str, at: i64, output_tokens: u64) -> String {
    format!(
        r#"{{"time":{{"created":{at},"completed":{at}}},"model":{{"id":"acme/coder-2","variant":"default"}},"content":[{{"type":"text","text":"{text}"}}],"cost":0.001,"tokens":{{"input":10,"output":{output_tokens}}}}}"#
    )
}

/// A running worker and what it sent.
struct Live {
    rx: Receiver<ModelUpdate>,
    handle: Option<WatchHandle>,
    seen: Vec<ModelUpdate>,
}

impl Live {
    /// Starts the worker and waits for the initial load's `Activity`.
    fn start(harness: &OpenCodeHarness, session: &str) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = harness.load(&SessionId::from(session), tx).unwrap();
        let mut live = Self {
            rx,
            handle: Some(handle),
            seen: Vec::new(),
        };
        live.wait_for("the initial load", |seen| {
            seen.iter()
                .skip_while(|u| **u != ModelUpdate::Loaded)
                .any(|u| matches!(u, ModelUpdate::Activity { .. }))
        });
        live
    }

    fn wait_for(&mut self, what: &str, done: impl Fn(&[ModelUpdate]) -> bool) {
        let deadline = Instant::now() + WAIT;
        while !done(&self.seen) {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(update) => self.seen.push(update),
                Err(RecvTimeoutError::Timeout) => {
                    panic!("timed out waiting for {what}; saw {:#?}", self.seen)
                }
                Err(RecvTimeoutError::Disconnected) => panic!("disconnected waiting for {what}"),
            }
        }
    }

    /// The turns after the initial load plus everything since.
    fn turns(&self) -> Vec<Arc<Turn>> {
        let mut turns = Vec::new();
        for update in &self.seen {
            update.apply(&mut turns);
        }
        turns
    }

    /// Forgets the updates seen so far (the turns stay as `base`).
    fn clear(&mut self) -> Vec<Arc<Turn>> {
        let turns = self.turns();
        self.seen = vec![ModelUpdate::Appended(turns.clone())];
        turns
    }

    /// Updates received since the last `clear`, after its snapshot.
    fn since(&self) -> &[ModelUpdate] {
        &self.seen[1..]
    }

    fn drop_handle(&mut self) {
        self.handle = None;
    }
}

fn ids(turn: &Turn) -> Vec<String> {
    match turn {
        Turn::Assistant(a) => a.items.iter().map(|i| i.id().to_string()).collect(),
        Turn::User(u) => u.items.iter().map(|i| i.id().to_string()).collect(),
        Turn::Compaction(_) => Vec::new(),
    }
}

fn updated_ids(updates: &[ModelUpdate]) -> Vec<String> {
    updates
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Updated(turns) => Some(turns),
            _ => None,
        })
        .flatten()
        .map(|t| t.id().to_string())
        .collect()
}

fn texts(turn: &Turn) -> Vec<String> {
    let Turn::Assistant(a) = turn else {
        return Vec::new();
    };
    a.items
        .iter()
        .filter_map(|i| match i {
            Item::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn last_activity(seen: &[ModelUpdate]) -> Option<(std::time::SystemTime, Option<bool>)> {
    seen.iter().rev().find_map(|u| match u {
        ModelUpdate::Activity {
            last_modified,
            idle,
        } => Some((*last_modified, *idle)),
        _ => None,
    })
}

// -- new rows

fn a_new_assistant_row_updates_the_current_turn(harness: impl Fn(&Env) -> OpenCodeHarness) {
    let env = env();
    let mut live = Live::start(&harness(&env), "ses_main");
    let before = live.clear();
    assert_eq!(before.len(), 4);

    env.assistant(
        "ses_main",
        "msg_main_11",
        11,
        "Live reply.",
        1_793_179_900_000,
    );
    live.wait_for("the update", |seen| seen.len() > 1);
    live.wait_for("the activity", |seen| last_activity(&seen[1..]).is_some());

    let updates = live.since();
    assert_eq!(updated_ids(updates), ["msg_main_08"], "{updates:#?}");
    assert!(
        updates
            .iter()
            .all(|u| !matches!(u, ModelUpdate::Appended(_))),
        "{updates:#?}"
    );
    let turns = live.turns();
    assert_eq!(turns.len(), 4);
    let Turn::Assistant(turn) = &*turns[3] else {
        panic!("an assistant turn")
    };
    assert_eq!(
        texts(&turns[3]).last().map(String::as_str),
        Some("Live reply.")
    );
    // Three rows now (08, 09, 11).
    assert_eq!(turn.stats.as_ref().unwrap().message_count, 3);
    // The earlier turns are the same turns.
    assert_eq!(&turns[..3], &before[..3]);
}

#[test]
fn a_new_assistant_row_updates_the_current_turn_when_watching() {
    a_new_assistant_row_updates_the_current_turn(Env::harness);
}

#[test]
fn a_new_assistant_row_updates_the_current_turn_when_polling() {
    a_new_assistant_row_updates_the_current_turn(Env::polling);
}

#[test]
fn a_new_user_row_appends_a_user_turn_and_then_an_assistant_turn_extends_it() {
    let env = env();
    let mut live = Live::start(&env.harness(), "ses_main");
    live.clear();

    env.user(
        "ses_main",
        "msg_main_11",
        11,
        "One more thing.",
        1_793_179_900_000,
    );
    live.wait_for("the new turn", |seen| {
        seen.iter()
            .any(|u| matches!(u, ModelUpdate::Appended(t) if t.len() == 1))
    });
    let turns = live.turns();
    assert_eq!(turns.len(), 5);
    let Turn::User(user) = &*turns[4] else {
        panic!("a user turn")
    };
    assert_eq!(user.id.as_str(), "msg_main_11");
    assert_eq!(user.text, "One more thing.");

    env.assistant("ses_main", "msg_main_12", 12, "Sure.", 1_793_179_901_000);
    live.wait_for("the assistant turn", |seen| {
        seen.iter().any(|u| matches!(u, ModelUpdate::Appended(t) if t.len() == 1 && t[0].id().as_str() == "msg_main_12"))
    });
    assert_eq!(live.turns().len(), 6);
}

#[test]
fn rows_written_while_the_session_loads_are_not_missed() {
    // The first pass of the watcher reads what was written after the
    // initial load: start, write at once, and the row arrives either way.
    let env = env();
    let (tx, rx) = mpsc::channel();
    let harness = env.harness();
    let _handle = harness.load(&SessionId::from("ses_main"), tx).unwrap();
    env.assistant("ses_main", "msg_main_11", 11, "Early.", 1_793_179_900_000);
    let deadline = Instant::now() + WAIT;
    let mut turns = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let update = rx.recv_timeout(left).expect("the row arrives");
        update.apply(&mut turns);
        if turns
            .last()
            .is_some_and(|t| texts(t).iter().any(|x| x == "Early."))
        {
            break;
        }
    }
}

// -- rows updated in place

#[test]
fn a_row_updated_in_place_updates_its_turn_with_stable_ids() {
    let env = env();
    let mut live = Live::start(&env.harness(), "ses_main");
    let before = live.clear();
    let Turn::Assistant(old) = &*before[3] else {
        panic!("an assistant turn")
    };
    let old_stats = old.stats.clone().unwrap();

    // The pending shell call of msg_main_09 finishes, and its text grows.
    let data = r#"{"time":{"created":1793178331000,"streamed":1793178340000,"completed":1793178340000},"agent":"build","model":{"id":"acme/coder-2","providerID":"acme","variant":"default"},"content":[{"type":"tool","id":"call-shell-3","name":"shell","executed":false,"state":{"status":"completed","input":{"command":"sleep 60"},"content":[{"type":"text","text":"slept\n"}]},"time":{"created":1793178332000,"completed":1793178339000}}],"finish":"stop","cost":0.003,"tokens":{"input":300,"output":30,"reasoning":0,"cache":{"read":0,"write":0}}}"#;
    env.rewrite("msg_main_09", data, 1_793_179_900_000);
    live.wait_for("the update", |seen| seen.len() > 1);

    let updates = live.since();
    assert_eq!(updated_ids(updates), ["msg_main_08"], "{updates:#?}");
    let turns = live.turns();
    assert_eq!(turns.len(), 4);
    // Same turn id, same item ids in the same order.
    assert_eq!(turns[3].id(), before[3].id());
    assert_eq!(ids(&turns[3]), ids(&before[3]));
    let Turn::Assistant(new) = &*turns[3] else {
        panic!("an assistant turn")
    };
    let call = new
        .items
        .iter()
        .find_map(|i| match i {
            Item::ToolCall(c) if c.id.as_str() == "call-shell-3" => Some(c),
            _ => None,
        })
        .unwrap();
    assert_eq!(call.status, chronoscope::model::ToolStatus::Ok);
    // The row is counted once: same messages, tokens and cost as before.
    assert_eq!(new.stats, Some(old_stats));
    assert_eq!(&turns[..3], &before[..3]);
}

#[test]
fn a_row_of_an_earlier_turn_updates_that_turn_only() {
    let env = env();
    let mut live = Live::start(&env.harness(), "ses_main");
    let before = live.clear();

    let data = assistant_data("Rewritten.", 1_793_178_010_000, 7);
    env.rewrite("msg_main_05", &data, 1_793_179_900_000);
    live.wait_for("the update", |seen| seen.len() > 1);
    let updates = live.since();
    assert_eq!(updated_ids(updates), ["msg_main_04"], "{updates:#?}");
    let turns = live.turns();
    assert!(texts(&turns[1]).contains(&"Rewritten.".to_owned()));
    assert_eq!(turns[2..], before[2..]);
}

fn an_update_within_the_same_millisecond_is_seen_too(harness: impl Fn(&Env) -> OpenCodeHarness) {
    let env = env();
    let mut live = Live::start(&harness(&env), "ses_long");
    let before = live.clear();
    // `msg_long_01` has the greatest time_updated; only its data changes.
    env.rewrite(
        "msg_long_01",
        r#"{"time":{"created":1792929600000},"text":"Edited prompt"}"#,
        1_792_929_600_000,
    );
    live.wait_for("the update", |seen| seen.len() > 1);
    assert_eq!(updated_ids(live.since()), ["msg_long_01"]);
    let Turn::User(user) = &*live.turns()[0] else {
        panic!("a user turn")
    };
    assert_eq!(user.text, "Edited prompt");
    assert_eq!(before.len(), 1);
}

#[test]
fn an_update_within_the_same_millisecond_is_seen_when_watching() {
    an_update_within_the_same_millisecond_is_seen_too(Env::harness);
}

#[test]
fn an_update_within_the_same_millisecond_is_seen_when_polling() {
    an_update_within_the_same_millisecond_is_seen_too(Env::polling);
}

// -- debounce

#[test]
fn ten_quick_writes_are_coalesced() {
    let env = env();
    let mut live = Live::start(&env.harness(), "ses_main");
    live.clear();
    // Writes 30 ms apart: closer than the debounce, over in under the cap.
    for n in 1..=10_i64 {
        let data = assistant_data(&format!("stream {n}"), 1_793_178_331_000, 7);
        env.rewrite("msg_main_09", &data, 1_793_179_900_000 + n);
        std::thread::sleep(Duration::from_millis(30));
    }
    live.wait_for("the last state", |seen| {
        seen.iter().any(|u| match u {
            ModelUpdate::Updated(turns) => turns
                .iter()
                .any(|t| texts(t).contains(&"stream 10".to_owned())),
            _ => false,
        })
    });
    let batches = live
        .since()
        .iter()
        .filter(|u| matches!(u, ModelUpdate::Updated(_)))
        .count();
    assert!((1..=3).contains(&batches), "{batches} batches");
}

// -- activity

fn activity_follows_time_idle(harness: impl Fn(&Env) -> OpenCodeHarness) {
    let env = env();
    let mut live = Live::start(&harness(&env), "ses_main");
    // The fixture's session is idle.
    assert_eq!(last_activity(&live.seen).unwrap().1, Some(true));
    live.clear();

    env.set_idle("ses_main", None);
    live.wait_for("not idle", |seen| {
        last_activity(&seen[1..]).is_some_and(|(_, idle)| idle == Some(false))
    });
    env.set_idle("ses_main", Some(1_793_180_000_000));
    live.wait_for("idle", |seen| {
        last_activity(&seen[1..]).is_some_and(|(_, idle)| idle == Some(true))
    });
    // Only the session row changed: no turn was touched.
    assert!(
        live.since()
            .iter()
            .all(|u| matches!(u, ModelUpdate::Activity { .. })),
        "{:#?}",
        live.since()
    );
}

#[test]
fn time_idle_null_is_active_and_a_timestamp_is_idle_when_watching() {
    activity_follows_time_idle(Env::harness);
}

#[test]
fn time_idle_null_is_active_and_a_timestamp_is_idle_when_polling() {
    activity_follows_time_idle(Env::polling);
}

#[test]
fn the_activity_carries_the_sessions_time_updated() {
    let env = env();
    let mut live = Live::start(&env.harness(), "ses_main");
    live.clear();
    env.writer
        .execute(
            "UPDATE session_v2 SET time_updated = 1793181000000 WHERE id = 'ses_main'",
            [],
        )
        .unwrap();
    live.wait_for("the activity", |seen| {
        last_activity(&seen[1..])
            .is_some_and(|(at, _)| at == UNIX_EPOCH + Duration::from_millis(1_793_181_000_000))
    });
}

// -- polling

#[test]
fn polling_detects_a_new_row_and_an_updated_one() {
    let env = env();
    let mut live = Live::start(&env.polling(), "ses_main");
    live.clear();
    env.assistant("ses_main", "msg_main_11", 11, "Polled.", 1_793_179_900_000);
    live.wait_for("the new row", |seen| {
        seen.iter().any(|u| matches!(u, ModelUpdate::Updated(t) if t.iter().any(|t| texts(t).contains(&"Polled.".to_owned()))))
    });
    env.rewrite(
        "msg_main_11",
        &assistant_data("Polled, grown.", 1_793_179_900_000, 7),
        1_793_179_901_000,
    );
    live.wait_for("the updated row", |seen| {
        seen.iter().any(|u| matches!(u, ModelUpdate::Updated(t) if t.iter().any(|t| texts(t).contains(&"Polled, grown.".to_owned()))))
    });
}

// -- stopping

fn assert_disconnects(live: &mut Live) {
    live.drop_handle();
    let started = Instant::now();
    loop {
        match live.rx.recv_timeout(Duration::from_secs(5)) {
            Err(RecvTimeoutError::Disconnected) => break,
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => panic!("the worker did not stop"),
        }
    }
    assert!(started.elapsed() < Duration::from_secs(2), "promptly");
}

#[test]
fn dropping_the_handle_stops_watching_and_disconnects() {
    let env = env();
    let mut live = Live::start(&env.harness(), "ses_main");
    assert_disconnects(&mut live);
    // Nothing is read any more.
    env.assistant(
        "ses_main",
        "msg_main_11",
        11,
        "Nobody listens.",
        1_793_179_900_000,
    );
    assert_eq!(
        live.rx.recv_timeout(Duration::from_millis(500)),
        Err(RecvTimeoutError::Disconnected)
    );
}

#[test]
fn dropping_the_handle_stops_a_polling_worker_too() {
    let env = env();
    let mut live = Live::start(&env.polling(), "ses_main");
    assert_disconnects(&mut live);
}

#[test]
fn a_missing_session_still_ends_in_loaded_and_stops() {
    let env = env();
    let (tx, rx) = mpsc::channel();
    let harness = env.harness();
    let _handle = harness.load(&SessionId::from("ses_nope"), tx).unwrap();
    assert_eq!(rx.recv_timeout(WAIT), Ok(ModelUpdate::Loaded));
    assert_eq!(rx.recv_timeout(WAIT), Err(RecvTimeoutError::Disconnected));
}

// -- read-only

fn snapshot_of(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    files.sort();
    files
}

#[test]
fn following_a_session_never_modifies_the_database_or_its_directory() {
    for polling in [false, true] {
        let env = env();
        let harness = if polling {
            env.polling()
        } else {
            env.harness()
        };
        let mut live = Live::start(&harness, "ses_main");
        env.assistant("ses_main", "msg_main_11", 11, "Seen.", 1_793_179_900_000);
        env.rewrite(
            "msg_main_09",
            &assistant_data("grown", 1_793_178_331_000, 7),
            1_793_179_901_000,
        );
        env.set_idle("ses_main", None);
        live.wait_for("everything", |seen| {
            last_activity(seen).is_some_and(|(_, idle)| idle == Some(false))
                && seen.iter().any(|u| matches!(u, ModelUpdate::Updated(t) if t.iter().any(|t| texts(t).contains(&"Seen.".to_owned()))))
        });
        // The writer is done; whatever the worker does from now on must leave
        // the files alone.
        let before = snapshot_of(env.path.parent().unwrap());
        std::thread::sleep(Duration::from_millis(1500));
        assert_disconnects(&mut live);
        assert_eq!(snapshot_of(env.path.parent().unwrap()), before);
    }
}

// -- the index

/// A listed session: id, title and last activity.
type Listed = (String, String, chrono::DateTime<chrono::Utc>);

/// Receives snapshots until `pick` accepts the sessions one lists.
fn wait_for_sessions(rx: &Receiver<IndexEvent>, pick: &mut dyn FnMut(&[Listed]) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        let event = rx.recv_timeout(left).expect("a matching snapshot");
        if let IndexEvent::Snapshot {
            harness, sessions, ..
        } = event
        {
            assert_eq!(harness, HarnessId::OpenCode);
            let listed: Vec<Listed> = sessions
                .values()
                .flatten()
                .map(|s| (s.id.to_string(), s.title.clone(), s.last_activity))
                .collect();
            if pick(&listed) {
                return;
            }
        }
    }
}

#[test]
fn the_index_delivers_a_new_snapshot_with_a_session_inserted_into_session_v2() {
    let env = env();
    let cache = tempfile::tempdir().unwrap();
    let harness: Arc<dyn Harness> = Arc::new(env.harness());
    let mut index = SessionIndex::start_with_timing(
        vec![harness],
        cache.path().to_path_buf(),
        Timing {
            coalesce: Duration::from_millis(100),
            poll: Duration::from_millis(300),
            save_interval: Duration::from_secs(5),
        },
    );
    let rx = index.take_events().unwrap();
    let wait = |pick: &mut dyn FnMut(&[Listed]) -> bool| wait_for_sessions(&rx, pick);
    wait(&mut |s| s.iter().any(|(id, ..)| id == "ses_main"));

    env.writer
        .execute(
            "INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, \
             time_created, time_updated, time_idle) VALUES ('ses_new', 'prj_alpha', NULL, 'ses_new', \
             '/home/user/alpha', 'A brand new session', '1.0.0', 1793190000000, 1793190000000, NULL)",
            [],
        )
        .unwrap();
    wait(&mut |s| {
        s.iter()
            .any(|(id, title, _)| id == "ses_new" && title == "A brand new session")
    });

    // Titles and last activity refresh as well.
    env.writer
        .execute(
            "UPDATE session_v2 SET title = 'Renamed', time_updated = 1793191000000 WHERE id = 'ses_new'",
            [],
        )
        .unwrap();
    wait(&mut |s| {
        s.iter().any(|(id, title, at)| {
            id == "ses_new" && title == "Renamed" && at.timestamp_millis() == 1_793_191_000_000
        })
    });
}

// -- write-ahead log

impl Env {
    /// Switches the database to write-ahead logging, as OpenCode runs it.
    fn wal(self) -> Self {
        let mode: String = self
            .writer
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        // A write creates the log.
        self.writer
            .execute("UPDATE project SET time_updated = time_updated", [])
            .unwrap();
        assert!(self.path.with_file_name("opencode.db-wal").exists());
        self
    }
}

#[test]
fn a_write_ahead_log_database_is_followed_through_its_wal() {
    let env = env().wal();
    let mut live = Live::start(&env.harness(), "ses_main");
    live.clear();
    env.assistant(
        "ses_main",
        "msg_main_11",
        11,
        "Through the WAL.",
        1_793_179_900_000,
    );
    live.wait_for("the new row", |seen| {
        seen.iter().any(|u| matches!(u, ModelUpdate::Updated(t) if t.iter().any(|t| texts(t).contains(&"Through the WAL.".to_owned()))))
    });
    env.rewrite(
        "msg_main_11",
        &assistant_data("Through the WAL, grown.", 1_793_179_900_000, 7),
        1_793_179_901_000,
    );
    live.wait_for("the updated row", |seen| {
        seen.iter().any(|u| matches!(u, ModelUpdate::Updated(t) if t.iter().any(|t| texts(t).contains(&"Through the WAL, grown.".to_owned()))))
    });
    assert_eq!(live.turns().len(), 4);
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "the index polls a database on macOS")]
fn the_index_follows_a_write_ahead_log_database_through_file_events() {
    let env = env().wal();
    let cache = tempfile::tempdir().unwrap();
    let harness: Arc<dyn Harness> = Arc::new(env.harness());
    let mut index = SessionIndex::start_with_timing(
        vec![harness],
        cache.path().to_path_buf(),
        // No polling to speak of: only events can deliver the change.
        Timing {
            coalesce: Duration::from_millis(100),
            poll: Duration::from_secs(3600),
            save_interval: Duration::from_secs(5),
        },
    );
    let rx = index.take_events().unwrap();
    let has = |event: &IndexEvent, wanted: &str| match event {
        IndexEvent::Snapshot { sessions, .. } => {
            sessions.values().flatten().any(|s| s.id.as_str() == wanted)
        }
        IndexEvent::Stats { .. } => false,
    };
    let end = Instant::now() + WAIT;
    while !has(&rx.recv_timeout(WAIT).unwrap(), "ses_main") {}
    env.writer
        .execute(
            "INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, \
             time_created, time_updated, time_idle) VALUES ('ses_new', 'prj_alpha', NULL, 'ses_new', \
             '/home/user/alpha', 'New', '1.0.0', 1793190000000, 1793190000000, NULL)",
            [],
        )
        .unwrap();
    loop {
        let left = end.saturating_duration_since(Instant::now());
        let event = rx
            .recv_timeout(left)
            .expect("a snapshot with the new session");
        if has(&event, "ses_new") {
            break;
        }
    }
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "the index polls a database on macOS")]
fn the_index_keeps_following_after_the_wal_is_deleted_and_created_again() {
    let Env { _dir, path, writer } = env().wal();
    let wal = path.with_file_name("opencode.db-wal");
    let cache = tempfile::tempdir().unwrap();
    let harness: Arc<dyn Harness> =
        Arc::new(OpenCodeHarness::with_db_path(path.clone(), BTreeMap::new()));
    let mut index = SessionIndex::start_with_timing(
        vec![harness],
        cache.path().to_path_buf(),
        // No polling to speak of: only events can deliver the change.
        Timing {
            coalesce: Duration::from_millis(100),
            poll: Duration::from_secs(3600),
            save_interval: Duration::from_secs(5),
        },
    );
    let rx = index.take_events().unwrap();
    wait_for_sessions(&rx, &mut |s| s.iter().any(|(id, ..)| id == "ses_main"));
    // The index's own connections are short-lived: wait until its stats
    // workers (the five top-level sessions) are done with them.
    let mut stats = std::collections::HashSet::new();
    let end = Instant::now() + WAIT;
    while stats.len() < 5 {
        let left = end.saturating_duration_since(Instant::now());
        if let IndexEvent::Stats { session, .. } = rx.recv_timeout(left).expect("the stats") {
            stats.insert(session);
        }
    }

    // OpenCode exits: the last connection closes and SQLite deletes the log.
    drop(writer);
    let end = Instant::now() + WAIT;
    while wal.exists() {
        assert!(Instant::now() < end, "the log was not deleted");
        std::thread::sleep(Duration::from_millis(20));
    }

    // Let the rebuild that the deletion itself causes finish, so that only a
    // watch on the new log can deliver what is written next.
    std::thread::sleep(Duration::from_millis(600));
    while rx.try_recv().is_ok() {}

    // OpenCode starts again and writes into a new log.
    let writer = Connection::open(&path).unwrap();
    writer.execute_batch("PRAGMA synchronous = OFF").unwrap();
    writer
        .execute(
            "INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, \
             time_created, time_updated, time_idle) VALUES ('ses_new', 'prj_alpha', NULL, 'ses_new', \
             '/home/user/alpha', 'After the restart', '1.0.0', 1793190000000, 1793190000000, NULL)",
            [],
        )
        .unwrap();
    assert!(wal.exists(), "the write went into a new log");
    wait_for_sessions(&rx, &mut |s| {
        s.iter()
            .any(|(id, title, _)| id == "ses_new" && title == "After the restart")
    });
}
