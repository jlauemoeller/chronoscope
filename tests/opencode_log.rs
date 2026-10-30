//! The schema-version warning of the OpenCode adapter is written to the log,
//! once per harness. Own test binary: the logger is process-global.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::Duration;

use chronoscope::harness::Harness;
use chronoscope::harness::opencode::OpenCodeHarness;
use chronoscope::logging::init_logging;
use chronoscope::model::{ModelUpdate, SessionId};
use rusqlite::Connection;
use tempfile::TempDir;

const FIXTURE: &str = include_str!("fixtures/opencode/fixture.sql");
const NEWER: &str = include_str!("fixtures/opencode/newer_migration.sql");

fn log_file() -> &'static Path {
    static LOG: OnceLock<(TempDir, PathBuf)> = OnceLock::new();
    let (_, file) = LOG.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("chronoscope.log");
        init_logging(&file).expect("logging initialises");
        (dir, file)
    });
    file
}

/// The warning lines about the database at `db`.
fn warnings_about(db: &Path) -> Vec<String> {
    let needle = db.display().to_string();
    fs::read_to_string(log_file())
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("WARN") && l.contains(&needle))
        .map(str::to_owned)
        .collect()
}

fn database(extra: &[&str]) -> (TempDir, PathBuf) {
    log_file();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opencode.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(FIXTURE).unwrap();
    for sql in extra {
        conn.execute_batch(sql).unwrap();
    }
    (dir, path)
}

fn load_to_end(h: &OpenCodeHarness, id: &str) -> Vec<ModelUpdate> {
    let (tx, rx) = mpsc::channel();
    let _handle = h.load(&SessionId::from(id), tx).unwrap();
    let mut out = Vec::new();
    while let Ok(update) = rx.recv_timeout(Duration::from_secs(30)) {
        let done = matches!(update, ModelUpdate::Activity { .. });
        out.push(update);
        if done {
            break;
        }
    }
    out
}

#[test]
fn a_newer_migration_logs_one_warning_and_parsing_still_succeeds() {
    let (_dir, path) = database(&[NEWER]);
    let h = OpenCodeHarness::with_db_path(path.clone(), Default::default());
    assert!(warnings_about(&path).is_empty());

    h.refresh().unwrap();
    let projects = h.projects().unwrap();
    assert_eq!(projects.len(), 2, "the lists still parse");
    // More connections (a load, a stats run, a subagent lookup) do not warn
    // again.
    let updates = load_to_end(&h, "ses_main");
    assert!(updates.contains(&ModelUpdate::Loaded));
    assert!(matches!(&updates[0], ModelUpdate::Appended(turns) if !turns.is_empty()));
    let session = h.sessions(&projects[0].id).unwrap().remove(0);
    h.stats(&session).unwrap();
    h.subagent(&SessionId::from("ses_main"), &"call-sub-1".into());
    h.refresh().unwrap();

    let lines = warnings_about(&path);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("20270101000000_future_change"),
        "{}",
        lines[0]
    );
    assert!(
        lines[0].contains("20261029165540_schedule_checks"),
        "{}",
        lines[0]
    );
}

#[test]
fn the_known_migration_logs_no_warning() {
    let (_dir, path) = database(&[]);
    let h = OpenCodeHarness::with_db_path(path.clone(), Default::default());
    h.refresh().unwrap();
    load_to_end(&h, "ses_main");
    assert!(warnings_about(&path).is_empty());
}

#[test]
fn a_missing_session_is_logged_as_an_error_and_ends_in_loaded() {
    let (_dir, path) = database(&[]);
    let h = OpenCodeHarness::with_db_path(path.clone(), Default::default());
    let (tx, rx) = mpsc::channel();
    let _handle = h.load(&SessionId::from("ses_nowhere"), tx).unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(30)),
        Ok(ModelUpdate::Loaded)
    );
    let log = fs::read_to_string(log_file()).unwrap();
    let errors = log
        .lines()
        .filter(|l| l.contains("ERROR") && l.contains("ses_nowhere"))
        .count();
    assert_eq!(errors, 1, "{log}");
}
