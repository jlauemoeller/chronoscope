//! Searching the transcript in the browser (product §5.2, architecture.md#data-flow): the worker
//! thread that matches, the port that brings its answers back, and the
//! `App`'s part of the flow.
//!
//! 1. `/` in the transcript asks ([`Msg::OpenSearch`]) for the [`Prompt`] in
//!    the help line.
//! 2. `Enter` in the prompt with a query: the App hands the query to the
//!    transcript (an attribute) and, right after, queries the request it
//!    builds from it (cycle 5 spec, §0 item 5):
//!    the query, a snapshot of its turns and a generation, unique over the
//!    run of the application ([`Msg::StartTranscriptSearch`]).
//! 3. The worker thread searches and a [`TranscriptSearchPort`] delivers
//!    [`UserEvent::TranscriptSearch`]; `GlobalEvents` makes it a
//!    [`Msg::TranscriptSearchResults`].
//! 4. The App pushes the matches to the transcript, which highlights and
//!    jumps, and shows the position (`3/17`) in the help line. Answers to a
//!    request other than the one waited for are dropped, so are those that
//!    arrive after the search was cleared, the session reloaded or another
//!    session opened.

use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use tuirealm::event::Event;
use tuirealm::listener::{Poll, PortResult};
use tuirealm::props::AttrValue;

use super::layout::Screen;
use super::redraw::RedrawRequest;
use super::{App, Id, Msg, UserEvent, ViewChange};
use crate::components::{
    Prompt, PromptMode, SEARCH_POSITION_ATTR, SEARCH_QUERY_ATTR, SEARCH_REQUEST_ATTR,
    SEARCH_RESULTS_ATTR, SearchStart, payload_from_value, search_results_value, search_start_value,
};
use crate::theme::Role;
use crate::transcript_search::{SearchCorpus, SearchMatches, SearchPosition, SearchRequest};

/// The label of the search prompt.
const SEARCH_LABEL: &str = "/";
/// The columns the prompt is inset from the left, like the browser's help
/// line text.
const PROMPT_INSET: u16 = 1;

/// How often the transcript search port is polled.
pub const TRANSCRIPT_SEARCH_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// The most answers one poll takes from the worker.
pub const TRANSCRIPT_SEARCH_MAX_PER_POLL: usize = 4;

// -- the worker

/// The thread that searches transcripts. Requests go in over a channel and
/// answers come back over another, which a [`TranscriptSearchPort`] reads.
/// Requests that a newer one has overtaken are skipped.
#[derive(Debug)]
pub struct TranscriptSearchWorker {
    requests: Option<Sender<SearchRequest>>,
    thread: Option<JoinHandle<()>>,
}

impl TranscriptSearchWorker {
    /// Starts the thread. The receiver is for a [`TranscriptSearchPort`].
    pub fn start() -> (Self, Receiver<SearchMatches>) {
        let (request_tx, request_rx) = channel::<SearchRequest>();
        let (result_tx, result_rx) = channel();
        let thread = std::thread::Builder::new()
            .name("transcript-search".to_owned())
            .spawn(move || run(&request_rx, &result_tx))
            .map_err(|err| log::warn!("could not start the transcript search worker: {err}"))
            .ok();
        let worker = Self {
            requests: thread.is_some().then_some(request_tx),
            thread,
        };
        (worker, result_rx)
    }

    /// Asks for a search. Returns whether the worker could take it.
    pub fn request(&self, request: SearchRequest) -> bool {
        self.requests
            .as_ref()
            .is_some_and(|tx| tx.send(request).is_ok())
    }

    /// Stops the thread after the search it is running, if any.
    pub fn shutdown(&mut self) {
        self.requests = None;
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::warn!("the transcript search worker panicked");
        }
    }
}

impl Drop for TranscriptSearchWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(requests: &Receiver<SearchRequest>, results: &Sender<SearchMatches>) {
    while let Ok(mut request) = requests.recv() {
        while let Ok(newer) = requests.try_recv() {
            request = newer;
        }
        if results.send(request.run()).is_err() {
            return;
        }
    }
}

/// Reads the worker's answers without blocking and emits
/// [`UserEvent::TranscriptSearch`] events.
#[derive(Debug)]
pub struct TranscriptSearchPort {
    results: Receiver<SearchMatches>,
}

impl TranscriptSearchPort {
    /// A port reading `results`.
    pub fn new(results: Receiver<SearchMatches>) -> Self {
        Self { results }
    }
}

impl Poll<UserEvent> for TranscriptSearchPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        match self.results.try_recv() {
            Ok(matches) => Ok(Some(Event::User(UserEvent::TranscriptSearch(matches)))),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(None),
        }
    }
}

/// A user event standing for the kind "transcript search answer", to
/// subscribe to by discriminant (the value is never delivered).
pub fn transcript_search_event_kind() -> UserEvent {
    UserEvent::TranscriptSearch(SearchMatches::new(0, Vec::new()))
}

// -- the App's part

/// What the browser knows about the search of its transcript.
#[derive(Debug, Default)]
pub(super) struct BrowserSearch {
    /// Whether the prompt on the help line is the transcript's search.
    pub(super) prompt_open: bool,
    /// The request the worker has not answered yet.
    pending: Option<u64>,
    /// What was searched for, for the toast.
    query: String,
    /// The position the help line shows.
    pub(super) position: Option<SearchPosition>,
}

impl BrowserSearch {
    /// Stops waiting for the answer to a search that is under way.
    pub(super) fn forget_pending(&mut self) {
        self.pending = None;
    }
}

impl App {
    /// Hands the app the thread that searches transcripts; without one the
    /// search runs on the calling thread. The app owns it from now on;
    /// [`shutdown`](Self::shutdown) stops it.
    pub fn set_transcript_search_worker(&mut self, worker: TranscriptSearchWorker) {
        self.transcript_worker = Some(worker);
    }

    /// Where the user is among the matches (the `3/17` of the help line).
    pub fn transcript_search_position(&self) -> Option<SearchPosition> {
        self.browser.search.position
    }

    /// Whether the prompt on the help line is the transcript's search.
    pub fn transcript_prompt_open(&self) -> bool {
        self.browser.search.prompt_open
    }

    /// `/` in the transcript: opens the prompt in the help line.
    pub(super) fn open_transcript_search(&mut self) {
        if self.screen != Screen::Browser
            || self.browser.search.prompt_open
            || !self.application.mounted(&self.top_transcript())
        {
            return;
        }
        let prompt = Prompt::new(SEARCH_LABEL, "", PromptMode::Text, self.theme.clone())
            .with_fill(Role::Background)
            .inset(PROMPT_INSET);
        match self.open_overlay(Id::Prompt, Box::new(prompt)) {
            Ok(()) => self.browser.search.prompt_open = true,
            Err(err) => log::warn!("could not open the search prompt: {err}"),
        }
    }

    /// The prompt's text changed: it draws itself, the screen needs a redraw.
    pub(super) fn transcript_prompt_changed(&mut self) {
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// `Esc` in the prompt: closes it. The search that is on stays.
    pub(super) fn transcript_prompt_cancel(&mut self) {
        self.close_transcript_prompt();
    }

    fn close_transcript_prompt(&mut self) {
        self.browser.search.prompt_open = false;
        self.close_overlay(Id::Prompt);
    }

    /// `Enter` in the prompt: closes it and, for a query, starts the search.
    pub(super) fn transcript_prompt_submit(&mut self, query: String) {
        self.close_transcript_prompt();
        if query.is_empty() {
            return;
        }
        self.transcript_seq += 1;
        let start = SearchStart {
            query,
            generation: self.transcript_seq,
        };
        self.set_attr(
            self.top_transcript(),
            SEARCH_QUERY_ATTR,
            search_start_value(start),
        );
        if let Some(request) = self.transcript_search_request() {
            self.emit(Msg::StartTranscriptSearch {
                query: request.query,
                corpus: request.corpus,
                generation: request.generation,
            });
        }
    }

    /// The search the transcript on top asks to be run, right after the App
    /// pushed it the query (`SEARCH_QUERY_ATTR`): the query, a snapshot of
    /// its turns and the generation. The transcript builds it from the query
    /// it was just given, so it is queried (cycle 5 spec, §0 item 5).
    fn transcript_search_request(&self) -> Option<SearchRequest> {
        self.application
            .query(&self.top_transcript(), SEARCH_REQUEST_ATTR)
            .ok()
            .flatten()
            .and_then(|value| payload_from_value::<SearchRequest>(&value.into_attr()))
    }

    /// Runs a search: on the worker when there is one, else right here.
    pub(super) fn start_transcript_search(
        &mut self,
        query: String,
        corpus: SearchCorpus,
        generation: u64,
    ) {
        self.browser.search.pending = Some(generation);
        self.browser.search.query.clone_from(&query);
        let request = SearchRequest {
            generation,
            query,
            corpus,
        };
        let sent = self
            .transcript_worker
            .as_ref()
            .is_some_and(|worker| worker.request(request.clone()));
        if !sent {
            self.transcript_matches_arrived(&request.run());
        }
    }

    /// The worker answered a search. An answer to another request than the
    /// one waited for is dropped.
    pub(super) fn transcript_matches_arrived(&mut self, matches: &SearchMatches) {
        if self.browser.search.pending != Some(matches.generation()) {
            log::debug!(
                "dropping the matches of search {}; the current one is {:?}",
                matches.generation(),
                self.browser.search.pending
            );
            return;
        }
        self.browser.search.pending = None;
        self.set_attr(
            self.top_transcript(),
            SEARCH_RESULTS_ATTR,
            search_results_value(matches.clone()),
        );
        self.sync_search_position();
        self.sync_view_after_push();
        if matches.matches().is_empty() {
            let text = format!("No matches for \"{}\"", self.browser.search.query);
            if let Err(err) = self.show_toast(text) {
                log::warn!("could not show a toast: {err}");
            }
        }
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// A key moved among the matches or cleared the search, and changed the
    /// view as `view` says (the selection a jump moved, the count of new
    /// sections).
    pub(super) fn search_position_changed(
        &mut self,
        position: Option<SearchPosition>,
        view: ViewChange,
    ) {
        if position.is_none() {
            // Cleared: an answer still on its way is for a search that is gone.
            self.browser.search.pending = None;
        }
        self.browser.search.position = position;
        self.refresh_status();
        self.view_changed(view);
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Takes the position from the transcript on top, right after a push
    /// that can have changed it: search results, a reload (`Reset`), or the
    /// swap of the transcript on top. The transcript computes it from the
    /// matches and turns it was pushed, so it is queried (cycle 5 spec, §0
    /// item 5); a key reports it in [`Msg::SearchPosition`].
    pub(super) fn sync_search_position(&mut self) {
        let value = self
            .application
            .query(&self.top_transcript(), SEARCH_POSITION_ATTR)
            .ok()
            .flatten()
            .map(tuirealm::props::QueryResult::into_attr);
        let position = match value {
            Some(value @ AttrValue::Payload(_)) => {
                payload_from_value::<Option<SearchPosition>>(&value).flatten()
            }
            _ => None,
        };
        self.browser.search.position = position;
        self.refresh_status();
    }

    /// The loaded session started over: no search outlives it.
    pub(super) fn search_reloaded(&mut self) {
        self.browser.search.pending = None;
        self.sync_search_position();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::transcript::fixtures::search_session;
    use crate::transcript_search::SearchCorpus;

    fn request(generation: u64, query: &str) -> SearchRequest {
        SearchRequest {
            generation,
            query: query.to_owned(),
            corpus: SearchCorpus::new(search_session(), true),
        }
    }

    #[test]
    fn a_request_is_answered_with_its_generation_and_matches() {
        let (mut worker, results) = TranscriptSearchWorker::start();
        assert!(worker.request(request(7, "usb")));
        let answer = results.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(answer.generation(), 7);
        assert_eq!(answer.matches().len(), 5);
        worker.shutdown();
    }

    #[test]
    fn the_worker_searches_on_a_thread_of_its_own() {
        // The search has run when the worker answers; it was not run by the
        // caller of `request`, which returns before.
        let (worker, results) = TranscriptSearchWorker::start();
        let me = std::thread::current().id();
        assert!(worker.request(request(1, "usb")));
        assert!(results.recv_timeout(Duration::from_secs(5)).is_ok());
        let named = worker
            .thread
            .as_ref()
            .map(|t| (t.thread().name().map(str::to_owned), t.thread().id()));
        let (name, id) = named.unwrap();
        assert_eq!(name.as_deref(), Some("transcript-search"));
        assert_ne!(id, me);
    }

    #[test]
    fn requests_that_a_newer_one_overtook_are_skipped() {
        let (request_tx, request_rx) = channel();
        let (result_tx, result_rx) = channel();
        for generation in 1..=50 {
            request_tx.send(request(generation, "usb")).unwrap();
        }
        drop(request_tx);
        run(&request_rx, &result_tx);
        let answered: Vec<u64> = result_rx.try_iter().map(|r| r.generation()).collect();
        assert_eq!(answered, [50]);
    }

    #[test]
    fn a_stopped_worker_takes_no_requests() {
        let (mut worker, _results) = TranscriptSearchWorker::start();
        worker.shutdown();
        assert!(!worker.request(request(1, "usb")));
    }

    #[test]
    fn the_port_turns_answers_into_user_events() {
        let (tx, rx) = channel();
        let mut port = TranscriptSearchPort::new(rx);
        assert!(matches!(port.poll(), Ok(None)));
        tx.send(request(3, "usb").run()).unwrap();
        match port.poll() {
            Ok(Some(Event::User(UserEvent::TranscriptSearch(answer)))) => {
                assert_eq!(answer.generation(), 3);
                assert_eq!(answer.matches().len(), 5);
            }
            other => panic!("{other:?}"),
        }
        drop(tx);
        assert!(matches!(port.poll(), Ok(None)), "disconnected is no error");
    }

    #[test]
    fn global_events_turns_the_event_into_a_message() {
        use tuirealm::component::AppComponent;

        let mut component = crate::components::GlobalEvents::new(crate::keymap::Keymap::default());
        let answer = request(4, "usb").run();
        let event = Event::User(UserEvent::TranscriptSearch(answer.clone()));
        assert_eq!(
            component.on(&event),
            Some(Msg::TranscriptSearchResults(answer))
        );
    }
}
