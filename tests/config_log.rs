//! Config warnings are also written to the log. Own test binary: the logger
//! is process-global.

use std::fs;
use std::path::Path;

use chronoscope::config::Config;
use chronoscope::logging::init_logging;

#[test]
fn config_warnings_are_written_to_the_log_file() {
    let dir = tempfile::tempdir().unwrap();
    let log_file = dir.path().join("chronoscope.log");
    init_logging(&log_file).expect("logging initialises");

    let (_, warnings) = Config::from_toml_str("colour = 1\nmouse = 3\n", Path::new("/home/x"));
    assert_eq!(warnings.len(), 2);

    let text = fs::read_to_string(&log_file).unwrap();
    let warn_lines: Vec<&str> = text.lines().filter(|l| l.contains("WARN")).collect();
    assert_eq!(warn_lines.len(), 2, "{text}");
    assert!(
        warn_lines
            .iter()
            .any(|l| l.contains("unknown key \"colour\""))
    );
    assert!(
        warn_lines
            .iter()
            .any(|l| l.contains("invalid value for \"mouse\""))
    );
}
