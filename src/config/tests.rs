use std::fs;

use super::*;

const HOME: &str = "/home/tester";

fn home() -> &'static Path {
    Path::new(HOME)
}

fn parse(text: &str) -> (Config, Vec<ConfigWarning>) {
    Config::from_toml_str(text, home())
}

fn messages(warnings: &[ConfigWarning]) -> Vec<&str> {
    warnings.iter().map(|w| w.message.as_str()).collect()
}

/// The example file of SPEC §8.5, verbatim.
const SPEC_EXAMPLE: &str = r#"
theme = "tokyo-night"
transparent_background = false   # overrides the theme's setting when present
mouse = true
show_system = true
dim_unselected = true
timestamp_format = "%Y-%m-%d %H:%M:%S"   # strftime; displayed in local time
utc = false
truncate_output_lines = 200

[harness.claude]
enabled = true
data_dir = "~/.claude/projects"            # override auto-detection
resume_command = "claude --resume {id}"

[harness.opencode]
enabled = true
db_path = "~/.local/share/opencode/opencode.db"
resume_command = "opencode --session {id}"

[projects.aliases]
"/home/jacob/Development/projects/brio" = "BRIO"
"/home/jacob/Development/projects/camera-settings" = "CameraSettings"

[keys.browser]                 # action = key or [keys]; replaces the default for that action
fold_toggle = ["enter", "space"]
copy = "y"
copy_full = "Y"

[keys.selector]
search = "/"
"#;

#[test]
fn missing_file_yields_defaults_without_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let (config, warnings) = Config::load(&dir.path().join("nope.toml"), home());
    assert_eq!(config, Config::default());
    assert!(warnings.is_empty());
}

#[test]
fn defaults_match_the_documented_table() {
    let c = Config::default();
    assert_eq!(c.theme, "default");
    assert_eq!(c.transparent_background, None);
    assert!(c.mouse && c.show_system && c.dim_unselected);
    assert_eq!(c.timestamp_format, "%Y-%m-%d %H:%M:%S");
    assert!(!c.utc);
    assert_eq!(c.truncate_output_lines, 200);
    assert!(c.harness.claude.enabled);
    assert_eq!(c.harness.claude.data_dir, None);
    assert_eq!(c.harness.claude.resume_command, "claude --resume {id}");
    assert!(c.harness.opencode.enabled);
    assert_eq!(c.harness.opencode.db_path, None);
    assert_eq!(c.harness.opencode.resume_command, "opencode --session {id}");
    assert!(c.projects.aliases.is_empty());
    assert!(c.keys.is_empty());
}

#[test]
fn empty_file_yields_defaults_without_warnings() {
    let (config, warnings) = parse("");
    assert_eq!(config, Config::default());
    assert!(warnings.is_empty());
}

#[test]
fn spec_example_loads_from_a_file_with_every_value() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, SPEC_EXAMPLE).unwrap();
    let (c, warnings) = Config::load(&path, home());
    assert!(warnings.is_empty(), "{warnings:?}");

    assert_eq!(c.theme, "tokyo-night");
    assert_eq!(c.transparent_background, Some(false));
    assert!(c.mouse);
    assert!(c.show_system);
    assert!(c.dim_unselected);
    assert_eq!(c.timestamp_format, "%Y-%m-%d %H:%M:%S");
    assert!(!c.utc);
    assert_eq!(c.truncate_output_lines, 200);

    assert!(c.harness.claude.enabled);
    assert_eq!(
        c.harness.claude.data_dir,
        Some(home().join(".claude").join("projects"))
    );
    assert_eq!(c.harness.claude.resume_command, "claude --resume {id}");
    assert!(c.harness.opencode.enabled);
    assert_eq!(
        c.harness.opencode.db_path,
        Some(
            home()
                .join(".local")
                .join("share")
                .join("opencode")
                .join("opencode.db")
        )
    );
    assert_eq!(c.harness.opencode.resume_command, "opencode --session {id}");

    assert_eq!(c.projects.aliases.len(), 2);
    assert_eq!(
        c.projects.aliases[Path::new("/home/jacob/Development/projects/brio")],
        "BRIO"
    );
    assert_eq!(
        c.projects.aliases[Path::new("/home/jacob/Development/projects/camera-settings")],
        "CameraSettings"
    );

    assert_eq!(c.keys.len(), 2);
    assert_eq!(c.keys["browser"]["fold_toggle"], ["enter", "space"]);
    assert_eq!(c.keys["browser"]["copy"], ["y"]);
    assert_eq!(c.keys["browser"]["copy_full"], ["Y"]);
    assert_eq!(c.keys["selector"]["search"], ["/"]);
}

#[test]
fn non_default_values_are_taken() {
    let (c, warnings) = parse(
        r#"
        mouse = false
        show_system = false
        dim_unselected = false
        utc = true
        transparent_background = true
        truncate_output_lines = 5
        timestamp_format = "%H:%M"
        [harness.claude]
        enabled = false
        resume_command = "x {id}"
        [harness.opencode]
        enabled = false
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(!c.mouse && !c.show_system && !c.dim_unselected);
    assert!(c.utc);
    assert_eq!(c.transparent_background, Some(true));
    assert_eq!(c.truncate_output_lines, 5);
    assert_eq!(c.timestamp_format, "%H:%M");
    assert!(!c.harness.claude.enabled);
    assert_eq!(c.harness.claude.resume_command, "x {id}");
    assert!(!c.harness.opencode.enabled);
}

#[test]
fn unknown_keys_warn_once_each_with_the_full_dotted_key() {
    let (c, warnings) = parse(
        r#"
        colour = "red"
        mouse = false
        [harness.claude]
        colour = 1
        [harness.codex]
        enabled = true
        [projects]
        other = 1
        [extra]
        a = 1
        "#,
    );
    assert!(!c.mouse, "known keys still apply");
    let mut msgs = messages(&warnings);
    msgs.sort_unstable();
    assert_eq!(
        msgs,
        [
            "unknown key \"colour\"",
            "unknown key \"extra\"",
            "unknown key \"harness.claude.colour\"",
            "unknown key \"harness.codex\"",
            "unknown key \"projects.other\"",
        ]
    );
    assert_eq!(warnings.len(), 5);
}

#[test]
fn unknown_keys_under_keys_and_aliases_do_not_warn() {
    let (c, warnings) = parse(
        r#"
        [keys.some_future_context]
        some_future_action = "x"
        [keys.browser]
        anything = ["a", "b"]
        [projects.aliases]
        "/any/path" = "Name"
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(c.keys["some_future_context"]["some_future_action"], ["x"]);
    assert_eq!(c.keys["browser"]["anything"], ["a", "b"]);
    assert_eq!(c.projects.aliases.len(), 1);
}

#[test]
fn wrong_type_falls_back_to_default_with_a_warning() {
    let (c, warnings) = parse("mouse = \"yes\"\nshow_system = false\n");
    assert!(c.mouse);
    assert!(!c.show_system);
    assert_eq!(
        messages(&warnings),
        ["invalid value for \"mouse\": expected a boolean; using default true"]
    );
    assert_eq!(warnings[0].key, "mouse");
}

#[test]
fn wrong_types_elsewhere_warn_and_use_defaults() {
    let (c, warnings) = parse(
        r#"
        theme = 3
        truncate_output_lines = "many"
        harness = 4
        [projects]
        aliases = "x"
        [keys]
        browser = 1
        [harness_ignored]
        "#,
    );
    assert_eq!(c, Config::default());
    let msgs = messages(&warnings);
    assert!(msgs.iter().any(|m| m.contains("\"theme\"")));
    assert!(msgs.iter().any(|m| m.contains("\"truncate_output_lines\"")));
    assert!(msgs.iter().any(|m| m.contains("\"harness\"")));
    assert!(msgs.iter().any(|m| m.contains("\"projects.aliases\"")));
    assert!(msgs.iter().any(|m| m.contains("\"keys.browser\"")));
}

#[test]
fn nested_wrong_type_names_the_full_key() {
    let (c, warnings) = parse("[harness.claude]\nenabled = \"no\"\n");
    assert!(c.harness.claude.enabled);
    assert_eq!(
        messages(&warnings),
        ["invalid value for \"harness.claude.enabled\": expected a boolean; using default true"]
    );
}

#[test]
fn zero_truncate_output_lines_falls_back_to_default() {
    let (c, warnings) = parse("truncate_output_lines = 0\n");
    assert_eq!(c.truncate_output_lines, 200);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].key, "truncate_output_lines");
    assert!(warnings[0].message.contains("\"truncate_output_lines\""));
}

#[test]
fn negative_truncate_output_lines_falls_back_to_default() {
    let (c, warnings) = parse("truncate_output_lines = -3\n");
    assert_eq!(c.truncate_output_lines, 200);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn invalid_timestamp_format_falls_back_to_default() {
    let (c, warnings) = parse("timestamp_format = \"%Q\"\n");
    assert_eq!(c.timestamp_format, "%Y-%m-%d %H:%M:%S");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].key, "timestamp_format");
    assert!(warnings[0].message.contains("\"timestamp_format\""));
}

#[test]
fn malformed_toml_gives_defaults_and_one_warning() {
    let (c, warnings) = parse("mouse = = false\n[oops\n");
    assert_eq!(c, Config::default());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message.contains("malformed"));
}

#[test]
fn malformed_file_gives_defaults_and_one_warning() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "theme = \n").unwrap();
    let (c, warnings) = Config::load(&path, home());
    assert_eq!(c, Config::default());
    assert_eq!(warnings.len(), 1);
}

#[test]
fn unreadable_path_gives_defaults_and_one_warning() {
    let dir = tempfile::tempdir().unwrap();
    // A directory cannot be read as a file.
    let (c, warnings) = Config::load(dir.path(), home());
    assert_eq!(c, Config::default());
    assert_eq!(warnings.len(), 1);
}

#[test]
fn tilde_expands_in_data_dir_db_path_and_alias_keys() {
    let (c, warnings) = parse(
        r#"
        [harness.claude]
        data_dir = "~/foo"
        [harness.opencode]
        db_path = "~/bar/db.sqlite"
        [projects.aliases]
        "~/proj" = "Proj"
        "/abs/proj" = "Abs"
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(c.harness.claude.data_dir, Some(home().join("foo")));
    assert_eq!(
        c.harness.opencode.db_path,
        Some(home().join("bar").join("db.sqlite"))
    );
    assert_eq!(c.projects.aliases[&home().join("proj")], "Proj");
    assert_eq!(c.projects.aliases[Path::new("/abs/proj")], "Abs");
}

#[test]
fn non_string_alias_and_key_binding_are_skipped_with_a_warning() {
    let (c, warnings) = parse(
        r#"
        [projects.aliases]
        "/a" = 1
        [keys.browser]
        copy = 1
        quit = ["q", 2]
        "#,
    );
    assert!(c.projects.aliases.is_empty());
    assert!(c.keys["browser"].is_empty());
    assert_eq!(warnings.len(), 3);
}
