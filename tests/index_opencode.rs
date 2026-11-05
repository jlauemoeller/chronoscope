//! The background index over an OpenCode database (SPEC cycle 3 §3.3, §3.4):
//! a write to only a child or grandchild session changes the stats key and
//! updates the stats, though no session summary changes.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use chronoscope::harness::Harness;
use chronoscope::harness::opencode::OpenCodeHarness;
use chronoscope::index::{IndexEvent, SessionIndex, SessionMap};
use chronoscope::model::{SessionId, SessionStats};
use rusqlite::Connection;
use tempfile::TempDir;

const FIXTURE: &str = include_str!("fixtures/opencode/fixture.sql");
const WAIT: Duration = Duration::from_secs(10);

/// The fixture's top-level sessions.
const TOP_LEVEL: usize = 5;

/// A tokens-only `assistant` row for `session`.
fn assistant_sql(id: &str, session: &str, seq: i64, input: u64) -> String {
    format!(
        "INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, \
         data) VALUES ('{id}', '{session}', 'assistant', {seq}, 1793178500000, 1793178500000, \
         '{{\"time\":{{\"created\":1793178500000}},\"content\":[],\"finish\":\"stop\",\
         \"tokens\":{{\"input\":{input},\"output\":0,\"reasoning\":0,\
         \"cache\":{{\"read\":0,\"write\":0}}}}}}');"
    )
}

fn grandchild_sql() -> &'static str {
    "INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, agent, \
     model, time_created, time_updated, time_idle) VALUES ('ses_grand', 'prj_alpha', \
     'ses_child1', 'ses_grand', '/home/user/alpha', 'Nested', '1.0.0', NULL, NULL, \
     1793178400000, 1793178400000, NULL);"
}

struct Rig {
    _dir: TempDir,
    /// OpenCode's own connection.
    writer: Connection,
    rx: std::sync::mpsc::Receiver<IndexEvent>,
    _handle: chronoscope::index::IndexHandle,
    _cache: TempDir,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opencode.db");
    let writer = Connection::open(&path).unwrap();
    writer.execute_batch(FIXTURE).unwrap();
    writer.execute_batch(grandchild_sql()).unwrap();
    writer.execute_batch("PRAGMA synchronous = OFF").unwrap();
    let cache = tempfile::tempdir().unwrap();
    let harness: Arc<dyn Harness> = Arc::new(OpenCodeHarness::with_db_path(path, BTreeMap::new()));
    let mut handle = SessionIndex::start(vec![harness], cache.path().to_owned());
    let rx = handle.take_events().unwrap();
    Rig {
        _dir: dir,
        writer,
        rx,
        _handle: handle,
        _cache: cache,
    }
}

impl Rig {
    /// Receives events until the stats of `session` arrive; the sessions of
    /// every snapshot on the way are returned with them.
    fn next_stats(&self, session: &str) -> (SessionStats, Vec<Arc<SessionMap>>) {
        let mut snapshots = Vec::new();
        loop {
            match self.rx.recv_timeout(WAIT).expect("an event") {
                IndexEvent::Stats {
                    session: id, stats, ..
                } if id.as_str() == session => return (stats, snapshots),
                IndexEvent::Snapshot { sessions, .. } => snapshots.push(sessions),
                IndexEvent::Stats { .. } => {}
            }
        }
    }
}

#[test]
fn a_write_to_only_a_child_or_grandchild_updates_the_stats_of_the_unchanged_session() {
    let rig = rig();
    let (first, snapshots) = rig.next_stats("ses_main");
    let listed = snapshots.last().expect("a snapshot first").clone();
    assert_eq!(listed.values().flatten().count(), TOP_LEVEL);
    // The session's tokens include those of its children.
    let main_input = first.tokens.input;

    // A child writes: a new row, and the session is bumped past its parent.
    rig.writer
        .execute_batch(&format!(
            "{} UPDATE session_v2 SET time_updated = 1793179900000 WHERE id = 'ses_child1';",
            assistant_sql("msg_child1_99", "ses_child1", 3, 1000)
        ))
        .unwrap();
    let (second, snapshots) = rig.next_stats("ses_main");
    assert_eq!(second.tokens.input, main_input + 1000);
    assert!(!snapshots.is_empty(), "the rebuild sent its snapshot");
    for sessions in &snapshots {
        assert_eq!(**sessions, *listed, "no session summary changed");
    }

    // A grandchild writes.
    rig.writer
        .execute_batch(&format!(
            "{} UPDATE session_v2 SET time_updated = 1793180000000 WHERE id = 'ses_grand';",
            assistant_sql("msg_grand_01", "ses_grand", 1, 20_000)
        ))
        .unwrap();
    let (third, snapshots) = rig.next_stats("ses_main");
    assert_eq!(third.tokens.input, main_input + 1000 + 20_000);
    for sessions in &snapshots {
        assert_eq!(**sessions, *listed, "no session summary changed");
    }
    assert!(
        listed
            .values()
            .flatten()
            .any(|s| s.id == SessionId::from("ses_main"))
    );
}
