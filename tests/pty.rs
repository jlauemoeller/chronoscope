//! Runs the real binary in a pseudo-terminal: it starts, draws, quits with
//! each quit key and leaves the terminal restored.

#![cfg(unix)]

use std::fs;
use std::process::Command;
use std::time::Duration;

use expectrl::process::unix::{Signal, WaitStatus};
use expectrl::session::OsSession;
use expectrl::{Eof, Expect};

const ENTER_ALTERNATE_SCREEN: &str = "\x1b[?1049h";
const LEAVE_ALTERNATE_SCREEN: &str = "\x1b[?1049l";
const SHOW_CURSOR: &str = "\x1b[?25h";
const ENABLE_MOUSE_CAPTURE: &str = "\x1b[?1000h";
const DISABLE_MOUSE_CAPTURE: &str = "\x1b[?1000l";
/// The top-left corner of the selector frame, drawn once the app is running.
const FRAME_CORNER: &str = "╭";

/// A running instance with its own empty home directory.
struct Instance {
    session: OsSession,
    /// Everything the app wrote before its first frame was complete.
    startup: String,
    _home: tempfile::TempDir,
}

fn start(config: Option<&str>) -> Instance {
    let home = tempfile::tempdir().expect("temp dir");
    let root = home.path();
    if let Some(config) = config {
        let dir = root.join("config").join("chronoscope");
        fs::create_dir_all(&dir).expect("config dir");
        fs::write(dir.join("config.toml"), config).expect("config file");
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_chronoscope"));
    command
        .env_clear()
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor");
    let mut session = OsSession::spawn(command).expect("spawn chronoscope in a pty");
    session
        .get_process_mut()
        .set_window_size(80, 24)
        .expect("set the pty size");
    session.set_expect_timeout(Some(Duration::from_secs(20)));
    Instance {
        session,
        startup: String::new(),
        _home: home,
    }
}

impl Instance {
    /// Waits until the app has entered the alternate screen and drawn its
    /// frame, so it is reading keys.
    fn wait_until_running(&mut self) {
        let entered = self
            .session
            .expect(ENTER_ALTERNATE_SCREEN)
            .expect("the app enters the alternate screen");
        self.startup
            .push_str(&String::from_utf8_lossy(entered.as_bytes()));
        let drawn = self
            .session
            .expect(FRAME_CORNER)
            .expect("the app draws the selector frame");
        self.startup
            .push_str(&String::from_utf8_lossy(drawn.as_bytes()));
    }

    /// Reads everything up to the end of the output and returns it with the
    /// exit status.
    fn finish(mut self) -> (String, WaitStatus) {
        let captures = self.session.expect(Eof).expect("the app exits");
        let output = String::from_utf8_lossy(captures.as_bytes()).into_owned();
        let status = self.session.get_process().wait().expect("exit status");
        (output, status)
    }
}

fn assert_clean_exit(output: &str, status: WaitStatus) {
    assert!(
        matches!(status, WaitStatus::Exited(_, 0)),
        "exit status {status:?}, output {output:?}"
    );
    assert!(
        output.contains(LEAVE_ALTERNATE_SCREEN),
        "alternate screen not left: {output:?}"
    );
    assert!(output.contains(SHOW_CURSOR), "cursor not shown: {output:?}");
}

fn quits_on(keys: &[u8]) {
    let mut app = start(None);
    app.wait_until_running();
    app.session.send(keys).expect("send the key");
    let (output, status) = app.finish();
    assert_clean_exit(&output, status);
}

#[test]
fn q_quits_and_restores_the_terminal() {
    quits_on(b"q");
}

#[test]
fn ctrl_q_quits_and_restores_the_terminal() {
    quits_on(&[0x11]);
}

#[test]
fn ctrl_w_quits_and_restores_the_terminal() {
    quits_on(&[0x17]);
}

#[test]
fn ctrl_c_quits_and_restores_the_terminal() {
    quits_on(&[0x03]);
}

#[test]
fn other_keys_do_not_quit() {
    let mut app = start(None);
    app.wait_until_running();
    app.session.send("ax").expect("send keys");
    std::thread::sleep(Duration::from_millis(300));
    assert!(app.session.get_process().is_alive().expect("alive check"));
    app.session.send("q").expect("send q");
    let (output, status) = app.finish();
    assert_clean_exit(&output, status);
}

#[test]
fn a_remapped_quit_key_quits_and_q_does_not() {
    let mut app = start(Some("[keys.global]\nquit = \"x\"\n"));
    app.wait_until_running();
    app.session.send("q").expect("send q");
    std::thread::sleep(Duration::from_millis(300));
    assert!(app.session.get_process().is_alive().expect("alive check"));
    app.session.send("x").expect("send x");
    let (output, status) = app.finish();
    assert_clean_exit(&output, status);
}

fn exits_cleanly_on_signal(signal: Signal) {
    let mut app = start(None);
    app.wait_until_running();
    app.session
        .get_process_mut()
        .signal(signal)
        .expect("send the signal");
    let (output, status) = app.finish();
    assert_clean_exit(&output, status);
}

#[test]
fn sigterm_leaves_through_the_normal_path() {
    exits_cleanly_on_signal(Signal::SIGTERM);
}

#[test]
fn sighup_leaves_through_the_normal_path() {
    exits_cleanly_on_signal(Signal::SIGHUP);
}

#[test]
fn sigint_leaves_through_the_normal_path() {
    exits_cleanly_on_signal(Signal::SIGINT);
}

#[test]
fn mouse_capture_is_on_by_default_and_released_on_exit() {
    let mut app = start(None);
    app.wait_until_running();
    assert!(
        app.startup.contains(ENABLE_MOUSE_CAPTURE),
        "{:?}",
        app.startup
    );
    app.session.send("q").expect("send q");
    let (output, status) = app.finish();
    assert_clean_exit(&output, status);
    assert!(output.contains(DISABLE_MOUSE_CAPTURE), "{output:?}");
}

#[test]
fn mouse_capture_is_not_enabled_when_the_config_turns_it_off() {
    let mut app = start(Some("mouse = false\n"));
    app.wait_until_running();
    assert!(
        !app.startup.contains(ENABLE_MOUSE_CAPTURE),
        "{:?}",
        app.startup
    );
    app.session.send("q").expect("send q");
    let (output, status) = app.finish();
    assert_clean_exit(&output, status);
    assert!(!output.contains(ENABLE_MOUSE_CAPTURE), "{output:?}");
}
