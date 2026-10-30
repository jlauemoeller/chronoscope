//! Own test binary, so no other test can have installed the global logger:
//! the error here must come from the filesystem.

use std::fs;

use chronoscope::logging::init_logging;

#[test]
fn init_logging_failure_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    // The parent "directory" is a file, so it cannot be created.
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "x").unwrap();
    init_logging(&blocker.join("sub").join("x.log")).unwrap_err();
}
