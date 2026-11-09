//! Copying to the clipboard (product §5.4, §8.4): what the `App` does with
//! [`OutputMsg::CopyText`](super::OutputMsg::CopyText), a request of the
//! output concern, which `output.rs` dispatches.

use std::io::Write;

use super::App;
use crate::clipboard::Clipboard;

impl App {
    /// Replaces the clipboard (the app starts with one that always fails, so
    /// that nothing touches the system by accident). `osc52_out` is where
    /// OSC52 sequences are written: the terminal's output.
    pub fn set_clipboard(
        &mut self,
        clipboard: Box<dyn Clipboard>,
        osc52_out: Option<Box<dyn Write>>,
    ) {
        self.clipboard = clipboard;
        self.osc52_out = osc52_out;
    }

    /// Puts `text` on the clipboard and says so in a toast: `Copied 42
    /// lines`, or `Copy failed: <reason>` when neither the system clipboard
    /// nor OSC52 took it.
    ///
    /// The OSC52 sequence is written (and flushed) here, in `update`, rather
    /// than in the draw step: the terminal has finished the last frame when
    /// `update` runs, so it lands between frames all the same. It is not
    /// size-limited; see `doc/clipboard.md`.
    pub(super) fn copy_text(&mut self, text: &str) {
        let outcome = self.clipboard.copy(text);
        if let Err(reason) = &outcome.system {
            // The toast is cut to its width; the log has the whole reason.
            log::warn!("the system clipboard failed: {reason}");
        }
        let osc52 = match (outcome.osc52, self.osc52_out.as_mut()) {
            (Some(sequence), Some(out)) => Some(
                out.write_all(sequence.as_bytes())
                    .and_then(|()| out.flush())
                    .map_err(|err| format!("cannot write to the terminal: {err}")),
            ),
            _ => None,
        };
        let message = match (outcome.system, osc52) {
            (Ok(()), _) | (Err(_), Some(Ok(()))) => copied_text(text),
            (Err(reason), None) | (Err(_), Some(Err(reason))) => {
                log::warn!("copy failed: {reason}");
                format!("Copy failed: {reason}")
            }
        };
        self.toast_quiet(message);
    }
}

/// `Copied 42 lines`, or `Copied 1 line` (`Copied 0 lines` for no text).
fn copied_text(text: &str) -> String {
    let lines = text.lines().count();
    let noun = if lines == 1 { "line" } else { "lines" };
    format!("Copied {lines} {noun}")
}
