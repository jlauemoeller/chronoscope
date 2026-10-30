//! The search worker (SPEC §1.1): matching thousands of sessions against a
//! query takes longer than a frame, so the selector's search runs on a
//! thread. Requests go in over a channel; results come back over another, a
//! [`SearchPort`] reads it and turns the results into user events.
//!
//! The worker skips the requests that a newer one has overtaken, so typing
//! faster than it matches costs one search, not one per key.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use tuirealm::event::Event;
use tuirealm::listener::{Poll, PortResult};

use super::UserEvent;
use super::search::search_sessions;
use crate::index::SessionMap;
use crate::model::{Project, SessionSummary};

/// How often the search port is polled.
pub const SEARCH_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// The most results one poll takes from the worker.
pub const SEARCH_MAX_PER_POLL: usize = 4;

/// A search to run over one snapshot of a harness.
#[derive(Debug, Clone)]
pub struct SearchRequest {
    /// Numbers the request; the result carries it back.
    pub seq: u64,
    /// What to look for.
    pub query: String,
    /// The projects of the harness.
    pub projects: Arc<Vec<Project>>,
    /// Their sessions.
    pub sessions: Arc<SessionMap>,
}

/// Where a hit is: the project's index in the request's projects, and the
/// session's index in that project's sessions.
pub type HitPosition = (usize, usize);

/// What a search found, with the snapshot it searched so that the hits can
/// be read from it.
#[derive(Debug)]
pub struct SearchResult {
    /// The number of the request this answers.
    pub seq: u64,
    /// The query searched for.
    pub query: String,
    /// The projects that were searched.
    pub projects: Arc<Vec<Project>>,
    /// Their sessions.
    pub sessions: Arc<SessionMap>,
    /// The hits, best first.
    pub hits: Vec<HitPosition>,
}

impl SearchResult {
    /// Runs `request`.
    pub fn of(request: SearchRequest) -> Self {
        let hits = search_sessions(&request.query, &request.projects, &request.sessions)
            .iter()
            .map(|hit| hit.position)
            .collect();
        Self {
            seq: request.seq,
            query: request.query,
            projects: request.projects,
            sessions: request.sessions,
            hits,
        }
    }

    /// The hits, best first, as the project and the session, read from the
    /// searched snapshot.
    pub fn found(&self) -> impl Iterator<Item = (&Project, &SessionSummary)> {
        self.hits.iter().filter_map(|&(project, session)| {
            let project = self.projects.get(project)?;
            let session = self.sessions.get(&project.id)?.get(session)?;
            Some((project, session))
        })
    }
}

/// A [`SearchResult`] on its way to the application.
///
/// User events must be `Eq + Clone`: the payload is `Arc`-wrapped and two
/// updates are equal when they answer the same request.
#[derive(Debug, Clone)]
pub struct SearchUpdate(Arc<SearchResult>);

impl SearchUpdate {
    /// Wraps `result`.
    pub fn new(result: SearchResult) -> Self {
        Self(Arc::new(result))
    }

    /// The number of the request this answers.
    pub fn seq(&self) -> u64 {
        self.0.seq
    }

    /// What the search found.
    pub fn result(&self) -> &SearchResult {
        &self.0
    }
}

impl PartialEq for SearchUpdate {
    fn eq(&self, other: &Self) -> bool {
        self.seq() == other.seq()
    }
}

impl Eq for SearchUpdate {}

/// A user event standing for the kind "search result", to subscribe to by
/// discriminant (the value is never delivered).
pub fn search_event_kind() -> UserEvent {
    UserEvent::Search(SearchUpdate::new(SearchResult::of(SearchRequest {
        seq: 0,
        query: String::new(),
        projects: Arc::default(),
        sessions: Arc::default(),
    })))
}

/// The thread that runs searches.
#[derive(Debug)]
pub struct SearchWorker {
    requests: Option<Sender<SearchRequest>>,
    thread: Option<JoinHandle<()>>,
}

impl SearchWorker {
    /// Starts the thread. The receiver is for a [`SearchPort`].
    pub fn start() -> (Self, Receiver<SearchResult>) {
        let (request_tx, request_rx) = channel::<SearchRequest>();
        let (result_tx, result_rx) = channel();
        let thread = std::thread::Builder::new()
            .name("search".to_owned())
            .spawn(move || run(&request_rx, &result_tx))
            .map_err(|err| log::warn!("could not start the search worker: {err}"))
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
            log::warn!("the search worker panicked");
        }
    }
}

impl Drop for SearchWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// The worker's loop: take the newest request, search, answer.
fn run(requests: &Receiver<SearchRequest>, results: &Sender<SearchResult>) {
    while let Ok(mut request) = requests.recv() {
        // Requests that arrived meanwhile have overtaken this one.
        while let Ok(newer) = requests.try_recv() {
            request = newer;
        }
        if results.send(SearchResult::of(request)).is_err() {
            return;
        }
    }
}

/// Reads the worker's results without blocking and emits
/// [`UserEvent::Search`] events.
///
/// When the worker has stopped there is nothing more to report; the port
/// reports no error.
#[derive(Debug)]
pub struct SearchPort {
    results: Receiver<SearchResult>,
}

impl SearchPort {
    /// A port reading `results`.
    pub fn new(results: Receiver<SearchResult>) -> Self {
        Self { results }
    }
}

impl Poll<UserEvent> for SearchPort {
    fn poll(&mut self) -> PortResult<Option<Event<UserEvent>>> {
        match self.results.try_recv() {
            Ok(result) => Ok(Some(Event::User(UserEvent::Search(SearchUpdate::new(
                result,
            ))))),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::{at, project, session};
    use crate::model::HarnessId;

    fn data() -> (Arc<Vec<Project>>, Arc<SessionMap>) {
        let p = project(HarnessId::Claude, "BRIO", at(2026, 10, 1, 0, 0, 0));
        let t = at(2026, 10, 2, 0, 0, 0);
        let mut sessions = SessionMap::new();
        sessions.insert(p.id.clone(), vec![session(&p, "a", "Fix it", t, t)]);
        (Arc::new(vec![p]), Arc::new(sessions))
    }

    fn request(seq: u64, query: &str) -> SearchRequest {
        let (projects, sessions) = data();
        SearchRequest {
            seq,
            query: query.to_owned(),
            projects,
            sessions,
        }
    }

    #[test]
    fn a_request_is_answered_with_its_number_and_hits() {
        let (mut worker, results) = SearchWorker::start();
        assert!(worker.request(request(7, "brio")));
        let result = results.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(result.seq, 7);
        assert_eq!(result.query, "brio");
        let ids: Vec<&str> = result.found().map(|(_, s)| s.id.as_str()).collect();
        assert_eq!(ids, ["a"]);
        worker.shutdown();
    }

    #[test]
    fn requests_that_a_newer_one_overtook_are_skipped() {
        // Requests are queued before the worker can take them, by holding the
        // channel's reader busy: the worker answers the last one and, at
        // most, the one it had already started.
        let (mut worker, results) = SearchWorker::start();
        for seq in 1..=200 {
            worker.request(request(seq, "brio"));
        }
        worker.shutdown();
        let answered: Vec<u64> = results.try_iter().map(|r| r.seq).collect();
        assert_eq!(answered.last(), Some(&200), "{answered:?}");
        assert!(answered.len() < 200, "{} answers", answered.len());
        assert!(answered.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn a_stopped_worker_takes_no_requests() {
        let (mut worker, results) = SearchWorker::start();
        worker.shutdown();
        assert!(!worker.request(request(1, "brio")));
        assert!(results.try_recv().is_err());
    }

    #[test]
    fn the_port_turns_results_into_user_events() {
        let (tx, rx) = channel();
        let mut port = SearchPort::new(rx);
        assert!(matches!(port.poll(), Ok(None)));
        tx.send(SearchResult::of(request(3, "brio"))).unwrap();
        match port.poll() {
            Ok(Some(Event::User(UserEvent::Search(update)))) => {
                assert_eq!(update.seq(), 3);
                assert_eq!(update.result().hits.len(), 1);
            }
            other => panic!("{other:?}"),
        }
        drop(tx);
        assert!(
            matches!(port.poll(), Ok(None)),
            "disconnected is not an error"
        );
    }

    #[test]
    fn updates_are_equal_when_they_answer_the_same_request() {
        let a = SearchUpdate::new(SearchResult::of(request(1, "brio")));
        let b = SearchUpdate::new(SearchResult::of(request(1, "zzz")));
        let c = SearchUpdate::new(SearchResult::of(request(2, "brio")));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
