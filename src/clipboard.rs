//! The clipboard (SPEC §6.4, §9.4), owned by the `App`.
//!
//! Text goes to the system clipboard through `arboard`. Where that cannot be
//! relied on (no display server reachable, or inside tmux, whose clipboard is
//! not the system's) the text is also sent to the terminal as an OSC52
//! escape sequence, which most terminals turn into a clipboard write, over
//! SSH too. Under tmux the sequence is wrapped in tmux's DCS passthrough.
//!
//! The `App` only knows the [`Clipboard`] trait, so tests give it a fake.
//! [`SystemClipboard`] is the real one; it decides what to do from the
//! environment it is given ([`ClipboardEnv`]) and the result of `arboard`.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// What a copy did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyOutcome {
    /// Whether the system clipboard took the text; the reason when it did
    /// not.
    pub system: Result<(), String>,
    /// The OSC52 sequence the `App` must write to the terminal, if one is
    /// due.
    pub osc52: Option<String>,
}

/// Puts text on the clipboard.
pub trait Clipboard {
    /// Copies `text`.
    fn copy(&mut self, text: &str) -> CopyOutcome;
}

/// A clipboard that is not there: every copy fails. The `App` starts with
/// this one, so that nothing in a test touches the real clipboard or
/// terminal.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoClipboard;

impl Clipboard for NoClipboard {
    fn copy(&mut self, _text: &str) -> CopyOutcome {
        CopyOutcome {
            system: Err("no clipboard available".to_owned()),
            osc52: None,
        }
    }
}

/// What decides whether OSC52 is emitted too. Injected, so that tests do not
/// depend on the environment they run in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardEnv {
    /// `$TMUX` is set.
    pub tmux: bool,
    /// `$DISPLAY` or `$WAYLAND_DISPLAY` is set.
    pub display: bool,
    /// Running on Linux, where a missing display means no display server.
    pub linux: bool,
}

impl ClipboardEnv {
    /// The environment of this process.
    pub fn from_env() -> Self {
        let set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
        Self {
            tmux: set("TMUX"),
            display: set("DISPLAY") || set("WAYLAND_DISPLAY"),
            linux: cfg!(target_os = "linux"),
        }
    }

    /// Whether OSC52 is emitted besides the system clipboard: when `arboard`
    /// failed, under tmux, or on Linux without a display server.
    pub fn wants_osc52(&self, system_ok: bool) -> bool {
        !system_ok || self.tmux || (self.linux && !self.display)
    }
}

/// The OSC52 sequence that sets the clipboard to `text`:
/// `ESC ] 52 ; c ; <base64> BEL`. Under `tmux` it is wrapped in the DCS
/// passthrough, `ESC P tmux ; <sequence with every ESC doubled> ESC \`.
pub fn osc52(text: &str, tmux: bool) -> String {
    let sequence = format!("\x1b]52;c;{}\x07", STANDARD.encode(text));
    if tmux {
        format!("\x1bPtmux;{}\x1b\\", sequence.replace('\x1b', "\x1b\x1b"))
    } else {
        sequence
    }
}

/// Something that sets the system clipboard's text.
pub trait TextSink {
    /// Sets the text; the error is a reason fit for a toast.
    fn set_text(&mut self, text: &str) -> Result<(), String>;
}

/// The system clipboard through `arboard`.
///
/// One `arboard::Clipboard` is kept for the life of the app: on X11 the
/// content disappears when it is dropped. It is made on the first copy, and
/// if that fails, the failure is remembered and not retried.
#[derive(Default)]
pub struct Arboard {
    state: ArboardState,
}

#[derive(Default)]
enum ArboardState {
    #[default]
    Unopened,
    Open(arboard::Clipboard),
    Failed(String),
}

impl TextSink for Arboard {
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        if matches!(self.state, ArboardState::Unopened) {
            self.state = match arboard::Clipboard::new() {
                Ok(clipboard) => ArboardState::Open(clipboard),
                Err(err) => {
                    log::warn!("no system clipboard: {err}");
                    ArboardState::Failed(err.to_string())
                }
            };
        }
        match &mut self.state {
            ArboardState::Open(clipboard) => clipboard.set_text(text).map_err(|err| {
                log::warn!("could not set the clipboard: {err}");
                err.to_string()
            }),
            ArboardState::Failed(reason) => Err(reason.clone()),
            ArboardState::Unopened => Err("no clipboard available".to_owned()),
        }
    }
}

/// The real clipboard: a [`TextSink`] and the environment, which decides
/// about OSC52.
pub struct SystemClipboard<S: TextSink = Arboard> {
    sink: S,
    env: ClipboardEnv,
}

impl SystemClipboard {
    /// `arboard` with the environment of this process.
    pub fn new() -> Self {
        Self::with(Arboard::default(), ClipboardEnv::from_env())
    }
}

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: TextSink> SystemClipboard<S> {
    /// A clipboard that sets text through `sink` and reads `env`.
    pub fn with(sink: S, env: ClipboardEnv) -> Self {
        Self { sink, env }
    }
}

impl<S: TextSink> Clipboard for SystemClipboard<S> {
    fn copy(&mut self, text: &str) -> CopyOutcome {
        let system = self.sink.set_text(text);
        let osc52 = self
            .env
            .wants_osc52(system.is_ok())
            .then(|| osc52(text, self.env.tmux));
        CopyOutcome { system, osc52 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINUX_DISPLAY: ClipboardEnv = ClipboardEnv {
        tmux: false,
        display: true,
        linux: true,
    };

    /// A sink that records the text and works or fails as told.
    struct FakeSink {
        fail: Option<&'static str>,
        set: Vec<String>,
    }

    impl FakeSink {
        fn working() -> Self {
            Self {
                fail: None,
                set: Vec::new(),
            }
        }

        fn failing(reason: &'static str) -> Self {
            Self {
                fail: Some(reason),
                set: Vec::new(),
            }
        }
    }

    impl TextSink for FakeSink {
        fn set_text(&mut self, text: &str) -> Result<(), String> {
            self.set.push(text.to_owned());
            self.fail.map_or(Ok(()), |reason| Err(reason.to_owned()))
        }
    }

    fn copy(sink: FakeSink, env: ClipboardEnv, text: &str) -> (CopyOutcome, Vec<String>) {
        let mut clipboard = SystemClipboard::with(sink, env);
        let outcome = clipboard.copy(text);
        (outcome, clipboard.sink.set)
    }

    #[test]
    fn osc52_is_esc_52_c_base64_bel() {
        assert_eq!(osc52("hello", false), "\x1b]52;c;aGVsbG8=\x07");
        assert_eq!(osc52("", false), "\x1b]52;c;\x07");
    }

    #[test]
    fn osc52_base64_has_padding_and_encodes_utf8() {
        assert_eq!(osc52("a", false), "\x1b]52;c;YQ==\x07");
        assert_eq!(osc52("ab", false), "\x1b]52;c;YWI=\x07");
        assert_eq!(osc52("abc", false), "\x1b]52;c;YWJj\x07");
        assert_eq!(osc52("é…", false), "\x1b]52;c;w6nigKY=\x07");
    }

    #[test]
    fn under_tmux_osc52_is_wrapped_with_every_esc_doubled() {
        assert_eq!(
            osc52("hello", true),
            "\x1bPtmux;\x1b\x1b]52;c;aGVsbG8=\x07\x1b\\"
        );
    }

    #[test]
    fn osc52_is_emitted_when_tmux_is_set() {
        let env = ClipboardEnv {
            tmux: true,
            ..LINUX_DISPLAY
        };
        let (outcome, set) = copy(FakeSink::working(), env, "hi");
        assert_eq!(set, ["hi"]);
        assert_eq!(outcome.system, Ok(()));
        assert_eq!(outcome.osc52, Some(osc52("hi", true)));
    }

    #[test]
    fn osc52_is_emitted_on_linux_without_a_display_variable() {
        let env = ClipboardEnv {
            tmux: false,
            display: false,
            linux: true,
        };
        let (outcome, _) = copy(FakeSink::working(), env, "hi");
        assert_eq!(outcome.osc52, Some(osc52("hi", false)));
    }

    #[test]
    fn osc52_is_not_emitted_with_a_display_and_no_tmux_while_arboard_works() {
        let (outcome, set) = copy(FakeSink::working(), LINUX_DISPLAY, "hi");
        assert_eq!(set, ["hi"]);
        assert_eq!(
            outcome,
            CopyOutcome {
                system: Ok(()),
                osc52: None
            }
        );
    }

    #[test]
    fn osc52_is_emitted_when_arboard_failed() {
        let (outcome, _) = copy(FakeSink::failing("boom"), LINUX_DISPLAY, "hi");
        assert_eq!(outcome.system, Err("boom".to_owned()));
        assert_eq!(outcome.osc52, Some(osc52("hi", false)));
    }

    #[test]
    fn a_missing_display_is_only_a_reason_on_linux() {
        let env = ClipboardEnv {
            tmux: false,
            display: false,
            linux: false,
        };
        let (outcome, _) = copy(FakeSink::working(), env, "hi");
        assert_eq!(outcome.osc52, None);
    }

    #[test]
    fn the_environment_is_read_from_the_process() {
        let env = ClipboardEnv::from_env();
        assert_eq!(env.linux, cfg!(target_os = "linux"));
        assert_eq!(
            env.tmux,
            std::env::var_os("TMUX").is_some_and(|v| !v.is_empty())
        );
    }

    #[test]
    fn no_clipboard_fails_without_osc52() {
        let outcome = NoClipboard.copy("hi");
        assert_eq!(outcome.system, Err("no clipboard available".to_owned()));
        assert_eq!(outcome.osc52, None);
    }
}
