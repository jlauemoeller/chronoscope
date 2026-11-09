//! The prompt concern (architecture.md#application-structure, product §5):
//! what the help-line [`Prompt`](crate::components::Prompt) is for, what it
//! reports, and the dispatcher that hands each report to the owner of the
//! prompt, by the purpose the prompt was mounted with: the selector's search
//! ([`search`](super::search)), the transcript's search
//! ([`transcript_search`](super::transcript_search)), the save path and the
//! overwrite question ([`save`](super::save)), and the resume question
//! ([`resume`](super::resume)).

use tuirealm::command::Direction;

use super::App;
use super::redraw::RedrawRequest;

/// What a prompt is for. The `App` mounts the prompt with it, and the
/// prompt sends it back with each message, so the message reaches the owner
/// of the prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptPurpose {
    /// The query of the session search in the selector.
    SelectorSearch,
    /// The query of the search of the transcript.
    TranscriptSearch,
    /// The path a section is saved or the session exported to.
    SavePath,
    /// Whether to overwrite the file at the path given.
    Overwrite,
    /// Whether to resume a session in its harness.
    Resume,
}

impl PromptPurpose {
    /// Whether a prompt for this purpose asks a `y/n` question, which it
    /// answers with [`PromptMsg::Confirm`], rather than taking a line of
    /// text.
    pub fn asks_yes_no(self) -> bool {
        matches!(self, Self::Overwrite | Self::Resume)
    }
}

/// The help-line prompt reported an edit or an answer. Every message but
/// [`Moved`](Self::Moved) carries the purpose the prompt was mounted with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptMsg {
    /// The text of the prompt changed to this.
    Changed(PromptPurpose, String),
    /// Only the prompt's cursor moved: nothing but a redraw is needed,
    /// whatever the prompt is for.
    Moved,
    /// `Enter` in a text prompt, with its text.
    Submit(PromptPurpose, String),
    /// `Esc` in a text prompt.
    Cancel(PromptPurpose),
    /// `↑` or `↓` in a text prompt, for its owner to use: over search results
    /// in the selector, the App moves the selected result it keeps and
    /// pushes it to the Sessions pane.
    Nav(PromptPurpose, Direction),
    /// The answer to a `y/n` prompt: yes (`y` or `Y`) or no (any other key
    /// but `Ctrl-c`).
    Confirm(PromptPurpose, bool),
}

impl App {
    /// Hands a message of the prompt to the owner its purpose names. What a
    /// prompt for that purpose never sends (an edit of a `y/n` prompt, an
    /// answer of a text prompt, `↑` / `↓` anywhere but in the selector's
    /// search) is ignored.
    pub(super) fn prompt_msg(&mut self, msg: PromptMsg) {
        use PromptPurpose::{Overwrite, Resume, SavePath, SelectorSearch, TranscriptSearch};
        match msg {
            PromptMsg::Changed(SelectorSearch, text) => self.search_changed(text),
            PromptMsg::Changed(TranscriptSearch | SavePath, _) | PromptMsg::Moved => {
                self.request_redraw(RedrawRequest::Dirty);
            }
            PromptMsg::Submit(SelectorSearch, _) => self.search_submit(),
            PromptMsg::Submit(TranscriptSearch, query) => self.transcript_prompt_submit(query),
            PromptMsg::Submit(SavePath, path) => self.path_submitted(&path),
            PromptMsg::Cancel(SelectorSearch) => self.search_cancel(),
            PromptMsg::Cancel(TranscriptSearch) => self.transcript_prompt_cancel(),
            PromptMsg::Cancel(SavePath) => self.write_prompt_cancel(),
            PromptMsg::Nav(SelectorSearch, direction) => self.search_nav(direction),
            PromptMsg::Confirm(Overwrite, yes) => self.overwrite_answered(yes),
            PromptMsg::Confirm(Resume, yes) => self.resume_answered(yes),
            PromptMsg::Changed(Overwrite | Resume, _)
            | PromptMsg::Submit(Overwrite | Resume, _)
            | PromptMsg::Cancel(Overwrite | Resume)
            | PromptMsg::Nav(TranscriptSearch | SavePath | Overwrite | Resume, _)
            | PromptMsg::Confirm(SelectorSearch | TranscriptSearch | SavePath, _) => {}
        }
    }
}
