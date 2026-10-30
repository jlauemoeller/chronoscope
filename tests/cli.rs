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
