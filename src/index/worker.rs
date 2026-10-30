//! The index worker: one coordinator thread plus a small pool for stats.
//!
//! The coordinator owns all index state. It reacts to messages on one control
//! channel (file events from the watchers, finished stats from the pool,
//! stop) and to timers (coalesced rebuilds, polling, debounced cache saves).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use super::cache::StatsCache;
use super::{IndexEvent, SessionMap, Timing};
use crate::harness::{Harness, Result};
use crate::model::{Project, SessionId, SessionStats, SessionSummary};

/// The most stats computations running at once.
const MAX_STATS_THREADS: usize = 3;

/// The control channel's messages.
pub(super) enum Msg {
    /// Something changed under the watched paths of harness `.0`.
    Fs(usize),
    /// The pool finished a job.
    Stats(Done),
    /// The handle was dropped.
    Stop,
}

pub(super) struct Done {
    harness: usize,
    session: SessionId,
    key: String,
    result: Result<SessionStats>,
}

/// Everything the coordinator needs.
pub(super) struct Job {
    pub harnesses: Vec<Arc<dyn Harness>>,
    pub cache_dir: std::path::PathBuf,
    pub timing: Timing,
    pub events: Sender<IndexEvent>,
    pub ctl_tx: Sender<Msg>,
    pub ctl_rx: Receiver<Msg>,
    pub stop: Arc<AtomicBool>,
}

/// A stats computation waiting for a pool thread.
struct StatsJob {
    harness: usize,
    summary: SessionSummary,
    key: String,
}

#[derive(Default)]
struct QueueState {
    jobs: VecDeque<StatsJob>,
    stop: bool,
}

/// The pool's job queue.
#[derive(Default)]
struct Queue {
    state: Mutex<QueueState>,
    ready: Condvar,
}

impl Queue {
    fn lock(&self) -> std::sync::MutexGuard<'_, QueueState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes the next job, waiting for one; `None` once stopped.
    fn pop(&self) -> Option<StatsJob> {
        let mut state = self.lock();
        loop {
            if state.stop {
                return None;
            }
            if let Some(job) = state.jobs.pop_front() {
                return Some(job);
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Replaces the waiting jobs by `jobs`, built from what was waiting.
    fn replace_with(&self, jobs: impl FnOnce(VecDeque<StatsJob>) -> VecDeque<StatsJob>) {
        let mut state = self.lock();
        let old = std::mem::take(&mut state.jobs);
        state.jobs = jobs(old);
        self.ready.notify_all();
    }

    fn stop(&self) {
        self.lock().stop = true;
        self.ready.notify_all();
    }
}

/// Per-harness coordinator state.
struct HarnessState {
    harness: Arc<dyn Harness>,
    /// Alive while the harness's paths are watched.
    watcher: Option<RecommendedWatcher>,
    /// When the coalesced rebuild is due.
    rebuild_at: Option<Instant>,
    /// When to rebuild next while the harness cannot be watched.
    poll_at: Option<Instant>,
    /// What the last snapshot held, to skip identical ones when polling.
    last: Option<Last>,
    /// The `stats_key`s of the sessions in the last snapshot.
    live_keys: Option<HashSet<String>>,
    warned_watch: bool,
}

struct Last {
    available: bool,
    projects: Arc<Vec<Project>>,
    sessions: Arc<SessionMap>,
}

struct Coordinator {
    events: Sender<IndexEvent>,
    ctl_tx: Sender<Msg>,
    timing: Timing,
    stop: Arc<AtomicBool>,
    harnesses: Vec<HarnessState>,
    cache: StatsCache,
    last_save: Instant,
    queue: Arc<Queue>,
    /// Jobs queued or running, by (harness, key).
    pending: HashSet<(usize, String)>,
    /// Keys whose computation failed; not retried while the key is current.
    failed: HashSet<(usize, String)>,
    /// The key of the stats last sent per session.
    sent: HashMap<(usize, SessionId), String>,
}

/// The worker thread's body.
pub(super) fn run(job: Job) {
    let queue = Arc::new(Queue::default());
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .div_ceil(2)
        .clamp(1, MAX_STATS_THREADS);
    let harnesses: Arc<Vec<Arc<dyn Harness>>> = Arc::new(job.harnesses.clone());
    let pool: Vec<JoinHandle<()>> = (0..threads)
        .filter_map(|n| {
            let queue = Arc::clone(&queue);
            let harnesses = Arc::clone(&harnesses);
            let tx = job.ctl_tx.clone();
            std::thread::Builder::new()
                .name(format!("index-stats-{n}"))
                .spawn(move || pool_thread(&queue, &harnesses, &tx))
                .map_err(|err| log::error!("could not start a stats thread: {err}"))
                .ok()
        })
        .collect();

    let mut coordinator = Coordinator {
        events: job.events,
        ctl_tx: job.ctl_tx,
        timing: job.timing,
        stop: job.stop,
        harnesses: job
            .harnesses
            .into_iter()
            .map(|harness| HarnessState {
                harness,
                watcher: None,
                rebuild_at: None,
                poll_at: None,
                last: None,
                live_keys: None,
                warned_watch: false,
            })
            .collect(),
        cache: StatsCache::load(&job.cache_dir),
        last_save: Instant::now(),
        queue: Arc::clone(&queue),
        pending: HashSet::new(),
        failed: HashSet::new(),
        sent: HashMap::new(),
    };
    coordinator.main_loop(&job.ctl_rx);

    // Shutdown: stop the pool, collect what it finished, save, then drop
    // everything (the watchers and the events sender with it).
    queue.stop();
    for thread in pool {
        if thread.join().is_err() {
            log::error!("a stats thread panicked");
        }
    }
    while let Ok(msg) = job.ctl_rx.try_recv() {
        if let Msg::Stats(done) = msg {
            coordinator.record(done, false);
        }
    }
    coordinator.prune();
    if coordinator.cache.is_dirty() {
        coordinator.cache.save();
    }
}

fn pool_thread(queue: &Queue, harnesses: &[Arc<dyn Harness>], tx: &Sender<Msg>) {
    while let Some(job) = queue.pop() {
        let result = harnesses[job.harness].stats(&job.summary);
        let done = Done {
            harness: job.harness,
            session: job.summary.id,
            key: job.key,
            result,
        };
        if tx.send(Msg::Stats(done)).is_err() {
            return;
        }
    }
}

impl Coordinator {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn main_loop(&mut self, ctl_rx: &Receiver<Msg>) {
        // Watch before the first build, so no change in between is missed.
        for i in 0..self.harnesses.len() {
            self.start_watching(i);
        }
        for i in 0..self.harnesses.len() {
            if self.stopped() || !self.build(i, false) {
                return;
            }
        }
        loop {
            if self.stopped() {
                return;
            }
            let wake = self.next_wake();
            let msg = match wake {
                Some(at) => ctl_rx.recv_timeout(at.saturating_duration_since(Instant::now())),
                None => ctl_rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match msg {
                Ok(Msg::Fs(i)) => {
                    let coalesce = self.timing.coalesce;
                    self.harnesses[i]
                        .rebuild_at
                        .get_or_insert_with(|| Instant::now() + coalesce);
                }
                Ok(Msg::Stats(done)) => {
                    if !self.record(done, true) {
                        return;
                    }
                }
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {}
            }
            if !self.run_timers() {
                return;
            }
        }
    }

    /// The earliest time something is due.
    fn next_wake(&self) -> Option<Instant> {
        let timers = self
            .harnesses
            .iter()
            .flat_map(|h| [h.rebuild_at, h.poll_at])
            .flatten();
        let save = self
            .cache
            .is_dirty()
            .then_some(self.last_save + self.timing.save_interval);
        timers.chain(save).min()
    }

    /// Runs what is due. `false` when the worker should end.
    fn run_timers(&mut self) -> bool {
        let now = Instant::now();
        for i in 0..self.harnesses.len() {
            if self.stopped() {
                return false;
            }
            if self.harnesses[i].rebuild_at.is_some_and(|at| at <= now) {
                self.harnesses[i].rebuild_at = None;
                if !self.build(i, false) {
                    return false;
                }
            } else if self.harnesses[i].poll_at.is_some_and(|at| at <= now) {
                // Maybe the store exists by now and can be watched.
                self.start_watching(i);
                if !self.build(i, true) {
                    return false;
                }
            }
        }
        if self.cache.is_dirty() && now >= self.last_save + self.timing.save_interval {
            self.cache.save();
            self.last_save = Instant::now();
        }
        true
    }

    /// Starts watching harness `i`'s paths; on failure, schedules polling.
    fn start_watching(&mut self, i: usize) {
        let state = &mut self.harnesses[i];
        if state.watcher.is_some() {
            return;
        }
        let paths = state.harness.watch_paths();
        match watch(&paths, i, self.ctl_tx.clone()) {
            Ok(watcher) => {
                state.watcher = Some(watcher);
                state.poll_at = None;
            }
            Err(err) => {
                if !state.warned_watch {
                    state.warned_watch = true;
                    log::warn!(
                        "cannot watch {paths:?} ({err}); rebuilding every {:?}",
                        self.timing.poll
                    );
                }
                state.poll_at = Some(Instant::now() + self.timing.poll);
            }
        }
    }

    /// Builds harness `i`'s index and sends the snapshot, then its stats.
    /// With `dedupe`, an unchanged snapshot is not sent. `false` when the
    /// receiver is gone.
    fn build(&mut self, i: usize, dedupe: bool) -> bool {
        let harness = Arc::clone(&self.harnesses[i].harness);
        let built = harness.refresh().and_then(|()| collect(&*harness));
        let (projects, sessions, ok) = match built {
            Ok((projects, sessions)) => (projects, sessions, true),
            Err(err) => {
                log::error!("could not build the {} index: {err}", harness.id());
                if self.harnesses[i].last.is_some() {
                    return true;
                }
                // Tell the UI there is nothing (yet): empty lists.
                (Vec::new(), SessionMap::new(), false)
            }
        };
        let available = harness.available();
        let state = &mut self.harnesses[i];
        if dedupe
            && state.last.as_ref().is_some_and(|l| {
                l.available == available && *l.projects == projects && *l.sessions == sessions
            })
        {
            return true;
        }
        let projects = Arc::new(projects);
        let sessions = Arc::new(sessions);
        state.last = Some(Last {
            available,
            projects: Arc::clone(&projects),
            sessions: Arc::clone(&sessions),
        });
        let snapshot = IndexEvent::Snapshot {
            harness: harness.id(),
            available,
            projects,
            sessions: Arc::clone(&sessions),
        };
        if self.events.send(snapshot).is_err() {
            return false;
        }
        // After a failed build the cache is left alone: the sessions are
        // not known to be gone.
        !ok || self.schedule_stats(i, &sessions)
    }

    /// Sends the cached stats of `sessions` and queues the others, newest
    /// first. `false` when the receiver is gone.
    fn schedule_stats(&mut self, i: usize, sessions: &SessionMap) -> bool {
        let harness = Arc::clone(&self.harnesses[i].harness);
        let mut all: Vec<&SessionSummary> = sessions.values().flatten().collect();
        all.sort_by(|a, b| {
            b.started_at
                .cmp(&a.started_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut live = HashSet::new();
        let mut wanted = Vec::new();
        for summary in all {
            let key = harness.stats_key(summary);
            live.insert(key.clone());
            if let Some(stats) = self.cache.get(&key) {
                let sent = (i, summary.id.clone());
                if self.sent.get(&sent) != Some(&key) {
                    let event = IndexEvent::Stats {
                        harness: harness.id(),
                        session: summary.id.clone(),
                        stats,
                    };
                    if self.events.send(event).is_err() {
                        return false;
                    }
                    self.sent.insert(sent, key);
                }
            } else if !self.failed.contains(&(i, key.clone())) {
                wanted.push(StatsJob {
                    harness: i,
                    summary: summary.clone(),
                    key,
                });
            }
        }
        // Forget sessions that are gone.
        self.sent
            .retain(|(h, id), _| *h != i || sessions.values().flatten().any(|s| &s.id == id));
        self.failed.retain(|(h, key)| *h != i || live.contains(key));
        self.harnesses[i].live_keys = Some(live);

        // The newest list replaces what is still waiting; running jobs stay.
        let pending = &mut self.pending;
        self.queue.replace_with(|old| {
            for job in old {
                pending.remove(&(job.harness, job.key));
            }
            wanted
                .into_iter()
                .filter(|job| pending.insert((job.harness, job.key.clone())))
                .collect()
        });
        self.prune();
        true
    }

    /// Takes a finished job into the cache and, with `send`, reports it.
    /// `false` when the receiver is gone.
    fn record(&mut self, done: Done, send: bool) -> bool {
        self.pending.remove(&(done.harness, done.key.clone()));
        let stats = match done.result {
            Ok(stats) => stats,
            Err(err) => {
                log::warn!("could not compute the stats of {}: {err}", done.session);
                self.failed.insert((done.harness, done.key));
                return true;
            }
        };
        self.cache.insert(done.key.clone(), stats);
        if !send {
            return true;
        }
        let event = IndexEvent::Stats {
            harness: self.harnesses[done.harness].harness.id(),
            session: done.session.clone(),
            stats,
        };
        self.sent.insert((done.harness, done.session), done.key);
        self.events.send(event).is_ok()
    }

    /// Drops cache entries of sessions that are gone or changed, once every
    /// harness has reported its sessions.
    fn prune(&mut self) {
        if self.harnesses.iter().any(|h| h.live_keys.is_none()) {
            return;
        }
        let live: HashSet<&str> = self
            .harnesses
            .iter()
            .flat_map(|h| h.live_keys.iter().flatten().map(String::as_str))
            .collect();
        self.cache.retain_live(&live);
    }
}

/// Reads a harness's projects and all their sessions.
fn collect(harness: &dyn Harness) -> Result<(Vec<Project>, SessionMap)> {
    let projects = harness.projects()?;
    let mut sessions = SessionMap::new();
    for project in &projects {
        sessions.insert(project.id.clone(), harness.sessions(&project.id)?);
    }
    Ok((projects, sessions))
}

/// Watches `paths` recursively, reporting changes as `Msg::Fs(index)`. Reads
/// (`Access` events) are ignored: the worker's own scans must not cause
/// rebuilds.
fn watch(
    paths: &[std::path::PathBuf],
    index: usize,
    tx: Sender<Msg>,
) -> notify::Result<RecommendedWatcher> {
    if paths.is_empty() {
        return Err(notify::Error::generic("the harness names nothing to watch"));
    }
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if matches!(&event, Ok(e) if matches!(e.kind, EventKind::Access(_))) {
            return;
        }
        // The worker may be gone already.
        let _ = tx.send(Msg::Fs(index));
    })?;
    for path in paths {
        watcher.watch(path, RecursiveMode::Recursive)?;
    }
    Ok(watcher)
}
