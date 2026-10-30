//! Loading failures and unusable resume templates are written to the log.
//! Own test binary: the logger is process-global.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::Duration;

use chrono::DateTime;
use chronoscope::harness::Harness;
use chronoscope::harness::claude::ClaudeHarness;
use chronoscope::logging::init_logging;
use chronoscope::model::{HarnessId, ModelUpdate, ProjectId, SessionId, SessionSummary};
use tempfile::TempDir;

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

fn log_text() -> String {
    fs::read_to_string(log_file()).unwrap_or_default()
}

fn load_to_end(h: &ClaudeHarness, id: &str) -> Vec<ModelUpdate> {
    let (tx, rx) = mpsc::channel();
    let _handle = h.load(&SessionId::from(id), tx).unwrap();
    let mut out = Vec::new();
    while let Ok(update) = rx.recv_timeout(Duration::from_secs(30)) {
        out.push(update);
        if out.last() == Some(&ModelUpdate::Loaded) {
            break;
        }
    }
    out
}

fn error_lines(needle: &str) -> Vec<String> {
    log_text()
        .lines()
        .filter(|l| l.contains("ERROR") && l.contains(needle))
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_missing_file_is_logged_as_an_error_and_ends_in_loaded() {
    log_file();
    let dir = tempfile::tempdir().unwrap();
    let h = ClaudeHarness::with_data_dir(dir.path().to_path_buf(), Default::default());
    let updates = load_to_end(&h, "missing-session-77");
    assert_eq!(updates, [ModelUpdate::Loaded]);
    let lines = error_lines("missing-session-77");
    assert_eq!(lines.len(), 1, "{}", log_text());
    assert!(lines[0].contains("could not load session"), "{}", lines[0]);
}

#[test]
fn a_read_error_is_logged_as_an_error_and_ends_in_loaded() {
    log_file();
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("-home-user-p");
    fs::create_dir_all(project.join("unreadable-session-78.jsonl")).unwrap();
    let h = ClaudeHarness::with_data_dir(dir.path().to_path_buf(), Default::default());
    let updates = load_to_end(&h, "unreadable-session-78");
    assert_eq!(updates, [ModelUpdate::Loaded]);
    assert_eq!(
        error_lines("unreadable-session-78").len(),
        1,
        "{}",
        log_text()
    );
}

#[test]
fn an_unusable_resume_template_is_logged_as_a_warning() {
    log_file();
    let session = SessionSummary {
        id: "s".into(),
        harness: HarnessId::Claude,
        project: ProjectId::from("/p"),
        cwd: None,
        title: String::new(),
        first_prompt: None,
        started_at: DateTime::UNIX_EPOCH,
        last_activity: DateTime::UNIX_EPOCH,
        path: PathBuf::from("/x.jsonl"),
    };
    let warns = || {
        log_text()
            .lines()
            .filter(|l| l.contains("WARN") && l.contains("resume command"))
            .count()
    };
    let harness = |template: &str| {
        ClaudeHarness::with_data_dir(PathBuf::from("/none"), Default::default())
            .with_resume_command(template)
    };
    assert_eq!(warns(), 0);
    assert!(harness("").resume_command(&session).is_none());
    assert_eq!(warns(), 1, "{}", log_text());
    assert!(harness("a 'b").resume_command(&session).is_none());
    assert_eq!(warns(), 2, "{}", log_text());
}
