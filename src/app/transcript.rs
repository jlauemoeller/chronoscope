//! The transcript concern (architecture.md#application-structure): what the
//! transcript on top reports of its view (cycle 5 spec, §9.1) and asks of
//! its search, and the dispatcher that hands it to the handlers in
//! [`browser`](super::browser) and [`transcript_search`](super::transcript_search).

use super::redraw::RedrawRequest;
use super::{App, ViewChange};
use crate::components::turn_block::SectionId;
use crate::transcript_search::{SearchCorpus, SearchPosition};

/// The transcript on top reported a change of its view or asked for a
/// search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptMsg {
    /// A key or the wheel changed what the transcript shows (scroll, folds)
    /// but not its selection: nothing but a redraw is needed.
    Changed,
    /// A key or the wheel moved the transcript's selection to this section,
    /// and changed nothing else the App shows: the turns sidebar follows it.
    SelectionChanged(SectionId),
    /// The transcript counts `count` sections that arrived while the user
    /// was away from the end (`↓ N new`); 0 once following resumes. A key or
    /// the wheel reports it this way, with `selection`, the section it
    /// selected, when it also moved the selection (`G`). After the updates
    /// and viewport changes the App pushes, it reads the count right after
    /// the push (cycle 5 spec, §0 item 5).
    NewSections {
        count: usize,
        selection: Option<SectionId>,
    },
    /// A left press on the transcript on top, on a section or not: it asks
    /// for the focus, and says what the press did to its view (the section
    /// it selected, among others).
    Pressed(ViewChange),
    /// `/` in the transcript: open the search prompt in the help line.
    OpenSearch,
    /// The transcript asks for this search to be run on a snapshot of its
    /// turns. `generation` numbers the search; answers to any other are
    /// dropped.
    StartSearch {
        query: String,
        corpus: SearchCorpus,
        generation: u64,
    },
    /// A key moved among the matches (`3/17`) or cleared the search
    /// (`None`), and did to the transcript's view what `view` says (the
    /// selection a jump moved, the count of new sections).
    SearchPosition {
        position: Option<SearchPosition>,
        view: ViewChange,
    },
}

impl App {
    /// Hands a message of the transcript to its handler.
    pub(super) fn transcript_msg(&mut self, msg: TranscriptMsg) {
        match msg {
            TranscriptMsg::Changed => self.request_redraw(RedrawRequest::Dirty),
            TranscriptMsg::SelectionChanged(section) => self.view_changed(ViewChange {
                changed: true,
                new_sections: None,
                selection: Some(section),
            }),
            TranscriptMsg::NewSections { count, selection } => self.view_changed(ViewChange {
                changed: true,
                new_sections: Some(count),
                selection,
            }),
            TranscriptMsg::Pressed(change) => self.transcript_pressed(change),
            TranscriptMsg::OpenSearch => self.open_transcript_search(),
            TranscriptMsg::StartSearch {
                query,
                corpus,
                generation,
            } => self.start_transcript_search(query, corpus, generation),
            TranscriptMsg::SearchPosition { position, view } => {
                self.search_position_changed(position, view);
            }
        }
    }
}
