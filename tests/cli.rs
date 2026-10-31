use std::process::Command;

fn chronoscope(arg: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_chronoscope"))
        .arg(arg)
        .output()
        .expect("chronoscope binary runs")
}

#[test]
fn version_prints_crate_version_and_exits_zero() {
    let out = chronoscope("--version");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        stdout.trim(),
        format!("chronoscope {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_prints_usage_and_exits_zero() {
    let out = chronoscope("--help");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Usage: chronoscope"), "{stdout}");
    assert!(stdout.contains(env!("CARGO_PKG_DESCRIPTION")), "{stdout}");
}

#[test]
fn unknown_argument_exits_non_zero() {
    let out = chronoscope("--bogus");
    assert_ne!(out.status.code(), Some(0));
}

/// Runs the binary with `args` in a temporary home, with the given working
/// directory, so that no real data is read. The terminal is not a tty, so a
/// start that got as far as the TUI would fail differently (and print to
/// stdout).
fn in_empty_home(args: &[&str], cwd: Option<&std::path::Path>) -> std::process::Output {
    let home = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_chronoscope"));
    command
        .args(args)
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("config"))
        .env("XDG_CACHE_HOME", home.path().join("cache"))
        .env("XDG_STATE_HOME", home.path().join("state"))
        .env("XDG_DATA_HOME", home.path().join("data"))
        .env_remove("NO_COLOR");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.output().expect("chronoscope binary runs")
}

fn assert_failed_with(out: &std::process::Output, message: &str) {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(String::from_utf8_lossy(&out.stderr), message);
    // Nothing was drawn: the terminal was never set up.
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
}

#[test]
fn help_lists_the_positional_argument_and_every_option() {
    let out = chronoscope("--help");
    let stdout = String::from_utf8(out.stdout).unwrap();
    for needle in [
        "[SESSION_ID | PATH]",
        "--latest",
        "--harness",
        "--project",
        "--theme",
        "--config",
    ] {
        assert!(stdout.contains(needle), "{needle} in\n{stdout}");
    }
}

#[test]
fn latest_with_a_positional_argument_is_rejected_by_clap() {
    let out = in_empty_home(&["--latest", "85a8"], None);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("cannot be used with"), "{stderr}");
}

#[test]
fn an_unknown_harness_is_rejected_by_clap() {
    let out = in_empty_home(&["--harness", "emacs"], None);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn an_id_that_matches_nothing_exits_with_1_and_a_message() {
    let out = in_empty_home(&["abc"], None);
    assert_failed_with(&out, "chronoscope: no session matches \"abc\"\n");
}

#[test]
fn a_missing_jsonl_path_exits_with_1_and_a_message() {
    let out = in_empty_home(&["/no/such/dir/1234.jsonl"], None);
    assert_failed_with(&out, "chronoscope: no such file: /no/such/dir/1234.jsonl\n");
}

#[test]
fn latest_without_a_project_for_the_directory_exits_with_1() {
    let cwd = tempfile::tempdir().unwrap();
    let out = in_empty_home(&["--latest"], Some(cwd.path()));
    let expected = format!(
        "chronoscope: no project for {}\n",
        cwd.path().canonicalize().unwrap().display()
    );
    assert_failed_with(&out, &expected);
}

#[test]
fn an_error_is_reported_with_an_alternative_config_file_too() {
    let config = tempfile::NamedTempFile::new().unwrap();
    let path = config.path().to_str().unwrap();
    let out = in_empty_home(&["--config", path, "--theme", "gruvbox", "abc"], None);
    assert_failed_with(&out, "chronoscope: no session matches \"abc\"\n");
}
