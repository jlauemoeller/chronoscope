//! The worker concern (architecture.md#application-structure,
//! architecture.md#data-flow): the answers of the session index and of the
//! two searches, which `GlobalEvents` makes from the Ports' events, and the
//! dispatcher that hands them to the handlers in [`selector`](super::selector),
//! [`search`](super::search) and [`transcript_search`](super::transcript_search).
//! The updates of a session's load belong to the browser
//! ([`BrowserMsg::Session`](super::BrowserMsg::Session)).

use super::App;
use super::index_port::IndexUpdate;
use super::search_worker::SearchUpdate;
use crate::transcript_search::SearchMatches;

/// A worker thread answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerMsg {
    /// The session index worker reported a snapshot or stats.
    Index(IndexUpdate),
    /// The search worker answered a search of the sessions.
    SearchResults(SearchUpdate),
    /// The worker answered a search of the transcript.
    TranscriptSearchResults(SearchMatches),
}

impl App {
    /// Hands a worker's answer to its handler.
    pub(super) fn worker_msg(&mut self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::Index(update) => self.index_update(&update),
            WorkerMsg::SearchResults(update) => self.search_result_arrived(&update),
            WorkerMsg::TranscriptSearchResults(matches) => {
                self.transcript_matches_arrived(&matches);
            }
        }
    }
}
