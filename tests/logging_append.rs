//! Own test binary: `init_logging` installs a process-global logger.

use std::fs;

use chronoscope::logging::init_logging;

#[test]
fn init_logging_appends_to_an_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let log_file = dir.path().join("chronoscope.log");
    fs::write(&log_file, "earlier line\n").unwrap();

    init_logging(&log_file).expect("logging initialises");
    log::info!("new line");

    let text = fs::read_to_string(&log_file).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert_eq!(lines[0], "earlier line");
    assert!(lines[1].ends_with("new line"));
}

#[test]
fn init_logging_failure_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    // The parent "directory" is a file, so it cannot be created.
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "x").unwrap();
    assert!(init_logging(&blocker.join("sub").join("x.log")).is_err());
}
