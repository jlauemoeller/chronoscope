//! The background session index (SPEC §1.1, §7.3, §9.2).
//!
//! [`SessionIndex::start`] spawns worker threads that build the project and
//! session lists of every harness, rebuild them when the harness's store
//! changes, compute per-session stats and cache them on disk. Everything
//! comes back over one channel of [`IndexEvent`]s, which the UI turns into
//! user events through a sync Port.
//!
//! - A [`Snapshot`](IndexEvent::Snapshot) is sent once per harness after each
//!   build, then [`Stats`](IndexEvent::Stats) for the sessions whose stats are
//!   already cached; the others follow as the pool computes them.
//! - Changes are detected with `notify` (events are coalesced for 500 ms). If
//!   watching is not possible, the harness is rebuilt every 2 s instead, and a
//!   snapshot is only sent when it differs from the last one.
//! - Dropping the [`IndexHandle`] stops the worker, the stats pool and the
//!   watchers.

mod cache;
mod worker;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

pub use cache::CACHE_FILE;
pub(crate) use worker::collect;

use crate::harness::Harness;
use crate::model::{HarnessId, Project, ProjectId, SessionId, SessionStats, SessionSummary};

/// The sessions of every project of a harness.
pub type SessionMap = HashMap<ProjectId, Vec<SessionSummary>>;

/// What the index worker reports.
#[derive(Debug, Clone, PartialEq)]
pub enum IndexEvent {
    /// The result of one build of a harness's index.
    Snapshot {
        harness: HarnessId,
        /// Whether the harness's data store exists. When false the lists are
        /// empty and the selector shows the harness muted.
        available: bool,
        /// The projects, newest activity first.
        projects: Arc<Vec<Project>>,
        /// The sessions of each project, newest first.
        sessions: Arc<SessionMap>,
    },
    /// The stats of a session, from the cache or freshly computed.
    ///
    /// For a session that changes while its stats are computed, this may
    /// describe an earlier version of it; a newer `Stats` follows.
    Stats {
        harness: HarnessId,
        session: SessionId,
        stats: SessionStats,
    },
}

/// The intervals the worker works by. The defaults are the product values;
/// tests use shorter ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// How long file events are collected before a harness is rebuilt.
    pub coalesce: Duration,
    /// How often a harness that cannot be watched is rebuilt.
    pub poll: Duration,
    /// The shortest time between two saves of the stats cache.
    pub save_interval: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            coalesce: Duration::from_millis(500),
            poll: Duration::from_secs(2),
            save_interval: Duration::from_secs(5),
        }
    }
}

/// Entry point of the index worker.
pub struct SessionIndex;

impl SessionIndex {
    /// Starts the worker over `harnesses`, caching stats in `cache_dir`
    /// (`$XDG_CACHE_HOME/chronoscope`). Returns at once; the first snapshots
    /// arrive on the handle's channel.
    pub fn start(harnesses: Vec<Arc<dyn Harness>>, cache_dir: PathBuf) -> IndexHandle {
        Self::start_with_timing(harnesses, cache_dir, Timing::default())
    }

    /// [`start`](Self::start) with explicit [`Timing`].
    pub fn start_with_timing(
        harnesses: Vec<Arc<dyn Harness>>,
        cache_dir: PathBuf,
        timing: Timing,
    ) -> IndexHandle {
        let (events_tx, events_rx) = mpsc::channel();
        let (ctl_tx, ctl_rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let job = worker::Job {
            harnesses,
            cache_dir,
            timing,
            events: events_tx,
            ctl_tx: ctl_tx.clone(),
            ctl_rx,
            stop: Arc::clone(&stop),
        };
        let thread = std::thread::Builder::new()
            .name("index".to_owned())
            .spawn(move || worker::run(job));
        let thread = match thread {
            Ok(thread) => Some(thread),
            Err(err) => {
                // The events sender went down with the closure: the
                // receiver reports a disconnect.
                log::error!("could not start the index worker: {err}");
                None
            }
        };
        IndexHandle {
            events: Some(events_rx),
            ctl: ctl_tx,
            stop,
            thread,
        }
    }
}

/// Owns the index worker. Dropping it tells the worker to stop (it does not
/// wait). The final save of the stats cache happens on the worker thread
/// after that, so the app must call [`shutdown`](Self::shutdown) when it
/// exits: a process that ends right after a drop may lose the save.
#[must_use = "dropping the handle stops the index"]
pub struct IndexHandle {
    events: Option<Receiver<IndexEvent>>,
    ctl: Sender<worker::Msg>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl IndexHandle {
    /// The channel the worker reports on; `None` after
    /// [`take_events`](Self::take_events).
    pub fn events(&self) -> Option<&Receiver<IndexEvent>> {
        self.events.as_ref()
    }

    /// Moves the channel out, for a Port to own. The receiver disconnects
    /// when the worker has stopped.
    pub fn take_events(&mut self) -> Option<Receiver<IndexEvent>> {
        self.events.take()
    }

    /// Tells the worker to stop, without waiting for it.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        // The worker may be gone already.
        let _ = self.ctl.send(worker::Msg::Stop);
    }

    /// Whether the worker thread has ended.
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Stops the worker and waits until it has saved the stats cache and
    /// ended. Waits for stats computations already running (one parse per
    /// pool thread at most).
    pub fn shutdown(mut self) {
        self.stop();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::error!("the index worker panicked");
        }
    }
}

impl Drop for IndexHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

impl std::fmt::Debug for IndexHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexHandle")
            .field("finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}
