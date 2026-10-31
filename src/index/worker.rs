//! The index worker: one coordinator thread plus a small pool for stats.
//!
//! The coordinator owns all index state. It reacts to messages on one control
//! channel (file events from the watchers, finished stats from the pool,
//! stop) and to timers (coalesced rebuilds, polling, debounced cache saves).

use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::AssertUnwindSafe;
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
    /// The `seq` of the job.
    seq: u64,
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
    /// When the job was made, on the coordinator's counter.
    seq: u64,
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

    /// Replaces the waiting jobs by what `jobs` makes of them. Jobs a pool
    /// thread has taken are not waiting any more.
    fn replace_with(&self, jobs: impl FnOnce(VecDeque<StatsJob>) -> VecDeque<StatsJob>) {
        let mut state = self.lock();
        let old = std::mem::take(&mut state.jobs);
        state.jobs = jobs(old);
        self.ready.notify_all();
    }

    fn push(&self, job: StatsJob) {
        self.lock().jobs.push_back(job);
        self.ready.notify_one();
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
    /// The ids of the sessions in the last snapshot.
    live_sessions: HashSet<SessionId>,
    warned_watch: bool,
    /// A failed build was logged and none has succeeded since.
    warned_build: bool,
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
    /// The job queued or running for a session, as its stats key. A session
    /// has at most one, so a growing transcript is not parsed several times
    /// at once.
    pending: HashMap<(usize, SessionId), String>,
    /// Jobs for a newer key of a session whose older job is still running;
    /// queued when that one finishes.
    deferred: HashMap<(usize, SessionId), StatsJob>,
    /// Keys whose computation failed; not retried while the key is current.
    failed: HashSet<(usize, String)>,
    /// The key and `seq` of the stats last sent per session. A session whose
    /// current key is cached is recorded here at every build with a fresh
    /// `seq`, whether or not its stats were sent again (they are not when the
    /// key is the one already sent).
    sent: HashMap<(usize, SessionId), (String, u64)>,
    /// Counts stats jobs and the builds that record a session's cached
    /// stats, in the order they happened. A result for an outdated key is
    /// only sent when its `seq` is newer than the last one recorded for its
    /// session. A session whose current key is cached records a fresh `seq`
    /// at every build, even when its stats are not sent again, so a job for
    /// an older key never overwrites them. (A result for a current key is
    /// always sent.)
    seq: u64,
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
                live_sessions: HashSet::new(),
                warned_watch: false,
                warned_build: false,
            })
            .collect(),
        cache: StatsCache::load(&job.cache_dir),
        last_save: Instant::now(),
        queue: Arc::clone(&queue),
        pending: HashMap::new(),
        deferred: HashMap::new(),
        failed: HashSet::new(),
        sent: HashMap::new(),
        seq: 0,
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
        // A harness that panics on a transcript must not take the thread (and
        // the session's slot) with it.
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            harnesses[job.harness].stats(&job.summary)
        }))
        .unwrap_or_else(|_| Err(anyhow::anyhow!("stats computation panicked")));
        let done = Done {
            harness: job.harness,
            session: job.summary.id,
            key: job.key,
            seq: job.seq,
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
        let files = state.harness.watch_files();
        match watch(&paths, &files, i, self.ctl_tx.clone()) {
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
                // Logged once per streak: a store that stays unreadable is
                // rebuilt every few seconds.
                if self.harnesses[i].warned_build {
                    log::debug!("could not build the {} index: {err}", harness.id());
                } else {
                    self.harnesses[i].warned_build = true;
                    log::error!("could not build the {} index: {err}", harness.id());
                }
                if self.harnesses[i].last.is_some() {
                    return true;
                }
                // Tell the UI there is nothing (yet): empty lists.
                (Vec::new(), SessionMap::new(), false)
            }
        };
        let available = harness.available();
        let state = &mut self.harnesses[i];
        state.warned_build &= !ok;
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
                let slot = (i, summary.id.clone());
                // Sent only when it differs from what the UI has. Either
                // way the session takes a fresh seq, so a job for an older
                // key (say, one running while the file reverted to this
                // key) cannot overwrite these stats later.
                let shown = self.sent.get(&slot).is_some_and(|(k, _)| *k == key);
                self.seq += 1;
                self.sent.insert(slot, (key, self.seq));
                if !shown {
                    let event = IndexEvent::Stats {
                        harness: harness.id(),
                        session: summary.id.clone(),
                        stats,
                    };
                    if self.events.send(event).is_err() {
                        return false;
                    }
                }
            } else if !self.failed.contains(&(i, key.clone())) {
                self.seq += 1;
                wanted.push(StatsJob {
                    harness: i,
                    summary: summary.clone(),
                    key,
                    seq: self.seq,
                });
            }
        }
        // Forget sessions that are gone.
        let ids: HashSet<&SessionId> = sessions.values().flatten().map(|s| &s.id).collect();
        self.sent.retain(|(h, id), _| *h != i || ids.contains(id));
        self.failed.retain(|(h, key)| *h != i || live.contains(key));
        self.harnesses[i].live_keys = Some(live);
        self.harnesses[i].live_sessions = ids.into_iter().cloned().collect();

        // The newest list replaces what harness `i` still has waiting; the
        // other harnesses' waiting jobs and all running ones stay.
        let pending = &mut self.pending;
        let deferred = &mut self.deferred;
        deferred.retain(|(h, _), _| *h != i);
        self.queue.replace_with(|old| {
            let mut jobs = VecDeque::new();
            for job in old {
                if job.harness == i {
                    pending.remove(&(i, job.summary.id.clone()));
                } else {
                    jobs.push_back(job);
                }
            }
            // What is pending for `i` now is running.
            for job in wanted {
                let slot = (i, job.summary.id.clone());
                match pending.get(&slot) {
                    None => {
                        pending.insert(slot, job.key.clone());
                        jobs.push_back(job);
                    }
                    Some(running) if *running == job.key => {}
                    Some(_) => {
                        deferred.insert(slot, job);
                    }
                }
            }
            jobs
        });
        self.prune();
        true
    }

    /// Takes a finished job into the cache and, with `send`, reports it.
    ///
    /// - A result for a current key (one in the harness's latest snapshot)
    ///   is cached and sent.
    /// - A result for an outdated key (the session changed while the job
    ///   ran) is still sent, so that a session written to continuously does
    ///   not go without stats until the writing stops, but only when its
    ///   `seq` is newer than that of the stats last sent for the session
    ///   and the session is still listed. It is not cached: the next prune
    ///   would remove it.
    /// - Anything else is dropped: the session is gone, or something newer
    ///   was sent already (so an older result never overwrites a newer one).
    ///
    /// `false` when the receiver is gone.
    fn record(&mut self, done: Done, send: bool) -> bool {
        let slot = (done.harness, done.session.clone());
        if self.pending.get(&slot) == Some(&done.key) {
            self.pending.remove(&slot);
        }
        if send && let Some(job) = self.deferred.remove(&slot) {
            let wanted = self.cache.get(&job.key).is_none() && self.is_live(job.harness, &job.key);
            if wanted && !self.pending.contains_key(&slot) {
                self.pending.insert(slot.clone(), job.key.clone());
                self.queue.push(job);
            }
        }
        let current = self.is_live(done.harness, &done.key);
        let stats = match done.result {
            Ok(stats) => stats,
            Err(err) => {
                log::warn!("could not compute the stats of {}: {err}", done.session);
                if current {
                    self.failed.insert((done.harness, done.key));
                }
                return true;
            }
        };
        if current {
            self.cache.insert(done.key.clone(), stats);
        }
        if !send {
            return true;
        }
        if !current {
            let listed = self.harnesses[done.harness]
                .live_sessions
                .contains(&done.session);
            let last = self.sent.get(&slot).map_or(0, |(_, seq)| *seq);
            if !listed || done.seq <= last {
                return true;
            }
        }
        let event = IndexEvent::Stats {
            harness: self.harnesses[done.harness].harness.id(),
            session: done.session.clone(),
            stats,
        };
        self.sent.insert(slot, (done.key, done.seq));
        self.events.send(event).is_ok()
    }

    /// Whether `key` belongs to a session of harness `i`'s last snapshot.
    fn is_live(&self, i: usize, key: &str) -> bool {
        self.harnesses[i]
            .live_keys
            .as_ref()
            .is_some_and(|keys| keys.contains(key))
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
        // Only the keys of the harnesses that ran: the cache may hold
        // others' (a harness disabled for now).
        let owned: Vec<String> = self
            .harnesses
            .iter()
            .map(|h| key_prefix(h.harness.id()))
            .collect();
        self.cache.retain_live(&live, &owned);
    }
}

/// The prefix every `stats_key` of harness `id` starts with.
fn key_prefix(id: crate::model::HarnessId) -> String {
    format!("{}:", id.cache_prefix())
}

/// Reads a harness's projects and all their sessions.
pub(crate) fn collect(harness: &dyn Harness) -> Result<(Vec<Project>, SessionMap)> {
    let projects = harness.projects()?;
    let mut sessions = SessionMap::new();
    for project in &projects {
        sessions.insert(project.id.clone(), harness.sessions(&project.id)?);
    }
    Ok((projects, sessions))
}

/// Watches `paths` recursively, reporting changes as `Msg::Fs(index)`, except
/// the `files` among them: those are watched through their directory,
/// non-recursively, and only events about their names count (a file may be
/// created and removed, which would end a watch on the file itself). Reads
/// (`Access` events) are ignored: the worker's own scans must not cause
/// rebuilds.
fn watch(
    paths: &[std::path::PathBuf],
    files: &[std::path::PathBuf],
    index: usize,
    tx: Sender<Msg>,
) -> notify::Result<RecommendedWatcher> {
    if paths.is_empty() {
        return Err(notify::Error::generic("the harness names nothing to watch"));
    }
    let dirs: Vec<std::path::PathBuf> = paths
        .iter()
        .filter(|p| !files.contains(p))
        .cloned()
        .collect();
    let names: Vec<std::ffi::OsString> = files
        .iter()
        .filter_map(|file| file.file_name().map(ToOwned::to_owned))
        .collect();
    let recursive = dirs.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        match &event {
            Ok(e) if matches!(e.kind, EventKind::Access(_)) => return,
            Ok(e) if !concerns(e, &recursive, &names) => return,
            _ => {}
        }
        // The worker may be gone already.
        let _ = tx.send(Msg::Fs(index));
    })?;
    for path in &dirs {
        watcher.watch(path, RecursiveMode::Recursive)?;
    }
    let mut parents: Vec<&std::path::Path> = Vec::new();
    for file in files {
        let parent = match file.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => std::path::Path::new("."),
        };
        if !parents.contains(&parent) {
            parents.push(parent);
            watcher.watch(parent, RecursiveMode::NonRecursive)?;
        }
    }
    Ok(watcher)
}

/// Whether `event` is about something under a recursively watched directory
/// or a watched file's name. Events that name no path (or ask for a rescan)
/// count.
fn concerns(
    event: &notify::Event,
    dirs: &[std::path::PathBuf],
    names: &[std::ffi::OsString],
) -> bool {
    if names.is_empty() || event.need_rescan() || event.paths.is_empty() {
        return true;
    }
    event.paths.iter().any(|path| {
        dirs.iter().any(|dir| path.starts_with(dir))
            || path
                .file_name()
                .is_some_and(|name| names.iter().any(|n| n == name))
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::*;

    fn event(paths: &[&str]) -> notify::Event {
        let mut event = notify::Event::new(EventKind::Any);
        event.paths = paths.iter().map(PathBuf::from).collect();
        event
    }

    #[test]
    fn without_watched_files_every_event_counts() {
        assert!(concerns(&event(&["/d/anything"]), &[], &[]));
    }

    #[test]
    fn with_watched_files_only_their_names_and_recursive_directories_count() {
        let names = [OsString::from("o.db"), OsString::from("o.db-wal")];
        let dirs = [PathBuf::from("/data")];
        assert!(concerns(&event(&["/d/o.db-wal"]), &[], &names));
        assert!(concerns(&event(&["/d/o.db"]), &[], &names));
        assert!(!concerns(&event(&["/d/o.db-shm"]), &[], &names));
        assert!(!concerns(&event(&["/d/o.db-journal"]), &[], &names));
        assert!(!concerns(&event(&["/d/snapshot/x"]), &dirs, &names));
        assert!(concerns(&event(&["/data/sub/x"]), &dirs, &names));
        // Events that name nothing, or ask for a rescan, count.
        assert!(concerns(&event(&[]), &[], &names));
        let rescan = event(&["/d/o.db-shm"]).set_flag(notify::event::Flag::Rescan);
        assert!(concerns(&rescan, &[], &names));
    }
}
