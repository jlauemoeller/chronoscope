//! Copying to the clipboard (SPEC §6.4, §9.4): `y` and `Y` through the app as
//! the running application drives it, what the `App` does with the text, and
//! the toast screens.
//!
//! The app always gets a fake clipboard and a captured terminal output, so
//! nothing here touches the system clipboard or the real terminal.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use tuirealm::event::{Key, KeyModifiers};

use super::*;
use crate::clipboard::{
    Clipboard, ClipboardEnv, CopyOutcome, SystemClipboard, TextSink, osc52 as osc52_of,
};

/// What the fake clipboard was asked, shared with the test.
#[derive(Default)]
struct Copies(Rc<RefCell<Vec<String>>>);

impl Copies {
    fn all(&self) -> Vec<String> {
        self.0.borrow().clone()
    }
}

/// A clipboard that records the text and answers with a fixed outcome.
struct FakeClipboard {
    copies: Rc<RefCell<Vec<String>>>,
    system: Result<(), String>,
    osc52: Option<String>,
}

impl Clipboard for FakeClipboard {
    fn copy(&mut self, text: &str) -> CopyOutcome {
        self.copies.borrow_mut().push(text.to_owned());
        CopyOutcome {
            system: self.system.clone(),
            osc52: self.osc52.clone(),
        }
    }
}

/// A terminal output that keeps what is written, or fails.
#[derive(Clone, Default)]
struct Terminal {
    written: Rc<RefCell<Vec<u8>>>,
    broken: bool,
}

impl Terminal {
    fn written(&self) -> Vec<u8> {
        self.written.borrow().clone()
    }
}

impl Write for Terminal {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.broken {
            return Err(std::io::Error::other("broken pipe"));
        }
        self.written.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Rig {
    /// Installs a fake clipboard (working, no OSC52) and a captured terminal.
    fn fake_clipboard(&mut self) -> (Copies, Terminal) {
        self.clipboard_that(Ok(()), None)
    }

    fn clipboard_that(
        &mut self,
        system: Result<(), String>,
        osc52: Option<String>,
    ) -> (Copies, Terminal) {
        let copies = Copies::default();
        let terminal = Terminal::default();
        self.app.set_clipboard(
            Box::new(FakeClipboard {
                copies: Rc::clone(&copies.0),
                system,
                osc52,
            }),
            Some(Box::new(terminal.clone())),
        );
        (copies, terminal)
    }

    fn copy(&mut self, text: &str) {
        self.app.update(Msg::CopyText(text.to_owned()));
    }

    fn toast(&self) -> Option<String> {
        self.app.toast_text().map(str::to_owned)
    }
}

fn lines(n: usize) -> String {
    (1..=n)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_text_goes_to_the_clipboard_and_a_toast_counts_the_lines() {
    let mut rig = rig();
    let (copies, _) = rig.fake_clipboard();
    rig.copy(&lines(42));
    assert_eq!(copies.all(), [lines(42)]);
    assert_eq!(rig.toast().as_deref(), Some("Copied 42 lines"));
}

#[test]
fn one_line_is_singular() {
    let mut rig = rig();
    let (copies, _) = rig.fake_clipboard();
    rig.copy("just one");
    assert_eq!(copies.all(), ["just one"]);
    assert_eq!(rig.toast().as_deref(), Some("Copied 1 line"));
}

#[test]
fn two_lines_are_plural_and_a_trailing_newline_is_not_a_line() {
    let mut rig = rig();
    rig.fake_clipboard();
    rig.copy("a\nb");
    assert_eq!(rig.toast().as_deref(), Some("Copied 2 lines"));
    rig.copy("a\nb\n");
    assert_eq!(rig.toast().as_deref(), Some("Copied 2 lines"));
}

#[test]
fn a_second_copy_replaces_the_toast() {
    let mut rig = rig();
    rig.fake_clipboard();
    rig.copy("a\nb");
    rig.copy("a");
    assert_eq!(rig.toast().as_deref(), Some("Copied 1 line"));
}

#[test]
fn a_clipboard_failure_without_osc52_shows_copy_failed() {
    let mut rig = rig();
    let (copies, terminal) = rig.clipboard_that(Err("no clipboard available".to_owned()), None);
    rig.copy("text");
    assert_eq!(copies.all(), ["text"]);
    assert_eq!(
        rig.toast().as_deref(),
        Some("Copy failed: no clipboard available")
    );
    assert!(terminal.written().is_empty());
}

#[test]
fn the_app_starts_with_a_clipboard_that_fails() {
    let mut rig = rig();
    rig.copy("text");
    assert_eq!(
        rig.toast().as_deref(),
        Some("Copy failed: no clipboard available")
    );
}

#[test]
fn osc52_is_written_to_the_terminal_and_counts_as_a_copy() {
    let mut rig = rig();
    let sequence = osc52_of("hi", false);
    let (_, terminal) = rig.clipboard_that(Ok(()), Some(sequence.clone()));
    rig.copy("hi");
    assert_eq!(terminal.written(), sequence.as_bytes());
    assert_eq!(rig.toast().as_deref(), Some("Copied 1 line"));
}

#[test]
fn osc52_saves_a_copy_the_system_clipboard_refused() {
    let mut rig = rig();
    let sequence = osc52_of("hi", false);
    let (_, terminal) = rig.clipboard_that(Err("no display".to_owned()), Some(sequence.clone()));
    rig.copy("hi");
    assert_eq!(terminal.written(), sequence.as_bytes());
    assert_eq!(rig.toast().as_deref(), Some("Copied 1 line"));
}

#[test]
fn osc52_without_a_terminal_to_write_to_is_not_a_copy() {
    let mut rig = rig();
    rig.app.set_clipboard(
        Box::new(FakeClipboard {
            copies: Rc::default(),
            system: Err("no display".to_owned()),
            osc52: Some(osc52_of("hi", false)),
        }),
        None,
    );
    rig.copy("hi");
    assert_eq!(rig.toast().as_deref(), Some("Copy failed: no display"));
}

#[test]
fn a_terminal_that_cannot_be_written_fails_the_copy_only_when_the_system_did() {
    let mut rig = rig();
    let broken = Terminal {
        broken: true,
        ..Terminal::default()
    };
    let set = |rig: &mut Rig, system: Result<(), String>| {
        rig.app.set_clipboard(
            Box::new(FakeClipboard {
                copies: Rc::default(),
                system,
                osc52: Some(osc52_of("hi", false)),
            }),
            Some(Box::new(broken.clone())),
        );
    };
    set(&mut rig, Err("no display".to_owned()));
    rig.copy("hi");
    assert_eq!(
        rig.toast().as_deref(),
        Some("Copy failed: cannot write to the terminal: broken pipe")
    );
    set(&mut rig, Ok(()));
    rig.copy("hi");
    assert_eq!(rig.toast().as_deref(), Some("Copied 1 line"));
}

/// A text sink for the real clipboard that records what it is given.
struct Sink(Rc<RefCell<Vec<String>>>);

impl TextSink for Sink {
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        self.0.borrow_mut().push(text.to_owned());
        Ok(())
    }
}

#[test]
fn the_system_clipboard_under_tmux_writes_the_wrapped_sequence_to_the_terminal() {
    let mut rig = rig();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let terminal = Terminal::default();
    let env = ClipboardEnv {
        tmux: true,
        display: true,
        linux: true,
    };
    rig.app.set_clipboard(
        Box::new(SystemClipboard::with(Sink(Rc::clone(&seen)), env)),
        Some(Box::new(terminal.clone())),
    );
    rig.copy("hello");
    assert_eq!(*seen.borrow(), ["hello"]);
    assert_eq!(
        terminal.written(),
        b"\x1bPtmux;\x1b\x1b]52;c;aGVsbG8=\x07\x1b\\"
    );
}

#[test]
fn the_system_clipboard_with_a_display_and_no_tmux_writes_nothing_to_the_terminal() {
    let mut rig = rig();
    let terminal = Terminal::default();
    let env = ClipboardEnv {
        tmux: false,
        display: true,
        linux: true,
    };
    rig.app.set_clipboard(
        Box::new(SystemClipboard::with(Sink(Rc::default()), env)),
        Some(Box::new(terminal.clone())),
    );
    rig.copy("hello");
    assert!(terminal.written().is_empty());
    assert_eq!(rig.toast().as_deref(), Some("Copied 1 line"));
}

// -- through the keys

#[test]
fn y_copies_the_selected_section_as_displayed() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_loaded();
    let (copies, _) = rig.fake_clipboard();
    rig.press(Key::Char('y'));
    rig.pump_until(|app| app.toast_text().is_some());
    let copied = copies.all();
    assert_eq!(copied.len(), 1);
    // The first section of the fixture session is its folded prompt: the
    // two rows the screen shows (see the 80x24 snapshot), as one line.
    assert_eq!(
        copied[0],
        "I have a Logitech BRIO web camera attached to this machine via USB. It works fine - both camera and microphone - but I don't have a way to…"
    );
    assert_eq!(rig.toast().as_deref(), Some("Copied 1 line"));
}

#[test]
fn capital_y_copies_the_full_section() {
    let mut rig = rig();
    rig.open_loaded();
    let (copies, _) = rig.fake_clipboard();
    // Down to the first tool call of the second turn.
    for _ in 0..3 {
        rig.press(Key::Char('j'));
    }
    rig.key(Key::Char('Y'), KeyModifiers::SHIFT);
    rig.pump_until(|app| app.toast_text().is_some());
    let copied = copies.all();
    assert_eq!(copied.len(), 1, "{copied:?}");
    assert!(copied[0].contains("Result:"), "{}", copied[0]);
    assert_eq!(
        rig.toast().as_deref(),
        Some(format!("Copied {} lines", copied[0].lines().count()).as_str())
    );
}

#[test]
fn ctrl_c_is_not_copy_it_still_quits() {
    let mut rig = rig();
    rig.open_loaded();
    let (copies, _) = rig.fake_clipboard();
    rig.key(Key::Char('c'), KeyModifiers::CONTROL);
    rig.pump_until(App::should_quit);
    assert!(copies.all().is_empty());
}

// -- screens

fn copied_42_lines(width: u16, height: u16) -> Rig {
    let mut rig = rig_with(theme("default"), width, height);
    rig.open_loaded();
    rig.fake_clipboard();
    rig.copy(&lines(42));
    rig
}

#[test]
fn copied_toast_80x24_text() {
    let mut rig = copied_42_lines(80, 24);
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn copied_toast_120x40_text() {
    let mut rig = copied_42_lines(120, 40);
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn copied_toast_120x40_styled() {
    let mut rig = copied_42_lines(120, 40);
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

#[test]
fn copy_failed_toast_80x24_text() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_loaded();
    rig.clipboard_that(Err("no clipboard available".to_owned()), None);
    rig.copy(&lines(42));
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}
