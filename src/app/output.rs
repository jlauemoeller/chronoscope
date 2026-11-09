//! The output concern (architecture.md#application-structure): the user's
//! requests to copy, save, export or resume, and the dispatcher that hands
//! them to the handlers in [`copy`](super::copy), [`save`](super::save) and
//! [`resume`](super::resume).

use super::App;
use crate::model::{HarnessId, SessionId};

/// The user asked to copy, save, export or resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputMsg {
    /// The transcript asks for this text to be put on the clipboard: the
    /// selected section as displayed (`y`) or in full (`Y`).
    CopyText(String),
    /// The transcript asks for the selected section's text to be saved to a
    /// file (`w`, or `W` for its full content). `kind` is the word for the
    /// section in the default file name (`shell`, `user`, ...).
    SaveSection { text: String, kind: &'static str },
    /// The transcript asks for the session to be exported to a markdown file
    /// (`e` as currently folded, `E` in full, as `full` says). `document` is
    /// the markdown.
    Export { full: bool, document: String },
    /// `R`: the user asks to resume a session in its harness. The target is
    /// the session of that harness the selector has selected; `None` (the
    /// browser) is the open session, whatever transcript is on top.
    RequestResume(Option<(HarnessId, SessionId)>),
}

impl App {
    /// Hands an output request to its handler.
    pub(super) fn output_msg(&mut self, msg: OutputMsg) {
        match msg {
            OutputMsg::CopyText(text) => self.copy_text(&text),
            OutputMsg::SaveSection { text, kind } => self.save_section(text, kind),
            OutputMsg::Export { full: _, document } => self.export_session(document),
            OutputMsg::RequestResume(target) => self.request_resume(target),
        }
    }
}
