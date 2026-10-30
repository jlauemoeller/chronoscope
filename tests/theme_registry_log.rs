//! A user theme that fails to parse is logged. Own test binary: the logger is
//! process-global.

use std::fs;

use chronoscope::logging::init_logging;
use chronoscope::theme::ThemeRegistry;

#[test]
fn invalid_user_theme_error_is_written_to_the_log_file() {
    let dir = tempfile::tempdir().unwrap();
    let log_file = dir.path().join("chronoscope.log");
    init_logging(&log_file).expect("logging initialises");

    let themes = dir.path().join("themes");
    fs::create_dir(&themes).unwrap();
    fs::write(themes.join("broken.toml"), "name = \"Broken\"\n").unwrap();
    fs::write(themes.join("fine.toml"), fine_theme()).unwrap();

    let registry = ThemeRegistry::load(&themes);
    assert!(!registry.get("broken").unwrap().is_selectable());
    assert!(registry.get("fine").unwrap().is_selectable());

    let text = fs::read_to_string(&log_file).unwrap();
    let warn_lines: Vec<&str> = text.lines().filter(|l| l.contains("WARN")).collect();
    assert_eq!(warn_lines.len(), 1, "{text}");
    assert!(warn_lines[0].contains("broken"), "{text}");
    assert!(warn_lines[0].contains("missing required key"), "{text}");
}

fn fine_theme() -> String {
    let mut out = String::from("name = \"Fine\"\n");
    for role in chronoscope::theme::Role::ALL {
        out.push_str(&format!("{} = \"#102030\"\n", role.key()));
    }
    out
}
