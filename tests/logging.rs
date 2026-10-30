//! `init_logging` installs a process-global logger, so this lives in its own
//! test binary (see also `logging_append.rs`).

use std::fs;

use chronoscope::logging::init_logging;

#[test]
fn init_logging_creates_parent_dirs_and_writes_lines() {
    let dir = tempfile::tempdir().unwrap();
    let log_file = dir
        .path()
        .join("state")
        .join("chronoscope")
        .join("chronoscope.log");
    assert!(!log_file.parent().unwrap().exists());

    init_logging(&log_file).expect("logging initialises");
    log::info!("first line");
    log::warn!("second line");
    log::debug!("debug is not written");

    let text = fs::read_to_string(&log_file).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].contains("INFO") && lines[0].ends_with("first line"));
    assert!(lines[1].contains("WARN") && lines[1].ends_with("second line"));

    // A second initialisation reports an error instead of panicking.
    assert!(init_logging(&log_file).is_err());
}
