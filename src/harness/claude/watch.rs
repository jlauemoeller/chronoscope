//! Keeping a loaded session live (product §6.1).
//!
//! After the initial parse the worker watches the session file's directory
//! (and the session's `subagents/` directory) with `notify`. Events are
//! coalesced: after the first one the worker waits [`COALESCE`] for more, then
//! looks at the file once. Appended bytes go through the same assembler as the
//! initial parse; a file that shrank or was replaced is parsed again from
//! scratch, after a [`ModelUpdate::Reset`]. When `notify` cannot be started
//! the worker polls the file's size and modification time instead.
//!
//! Like the spec says, only the size and the file's identity tell a reload
//! from an append: a file truncated and regrown past the old offset within
//! one coalescing window (or poll interval) is read as an append.
//!
//! Files are only ever opened for reading.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use super::load::{Identity, Job, Streamer, WatchMode, load_file};
use super::subagent::subagents_dir;
use crate::harness::watch_event::notice;
use crate::model::ModelUpdate;

/// After the first event, how long to wait for more before reading.
const COALESCE: Duration = Duration::from_millis(100);

/// How often the polling fallback looks at the file.
const POLL_EVERY: Duration = Duration::from_secs(1);

/// How often the worker looks at its stop flag while it waits.
const TICK: Duration = Duration::from_millis(50);

/// How often the worker checks whether the `subagents/` directory appeared.
const SUBAGENTS_CHECK: Duration = Duration::from_secs(1);

/// What a watch event was about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wake {
    /// The session file.
    Session,
    /// Something in the session's `subagents/` directory.
    Subagents,
}

/// Where the paths of interest are, to tell events apart.
#[derive(Clone)]
struct Targets {
    file_name: Option<std::ffi::OsString>,
    subagents: Option<PathBuf>,
}

impl Targets {
    /// What `path` (from an event) is. Only names are compared, not whole
    /// paths: event paths may be spelled differently from the watched ones
    /// (macOS reports `/private/var` for `/var`).
    fn classify(&self, path: &Path) -> Option<Wake> {
        if let Some(sub) = &self.subagents {
            let name = path.file_name();
            let parent = path.parent();
            let session_dir = sub.parent().and_then(Path::file_name);
            let in_sub = parent.and_then(Path::file_name) == sub.file_name()
                && parent.and_then(Path::parent).and_then(Path::file_name) == session_dir;
            // The session's own directory, or `subagents` itself, appearing.
            let dir = name == session_dir
                || (name == sub.file_name() && parent.and_then(Path::file_name) == session_dir);
            if in_sub || dir {
                return Some(Wake::Subagents);
            }
        }
        (path.file_name() == self.file_name.as_deref()).then_some(Wake::Session)
    }
}

/// Runs the watch loop until the worker is stopped (or the receiver is
/// gone). `streamer` is the state after the initial load.
pub(super) fn run(job: &Job, path: PathBuf, streamer: Streamer<'_>) {
    let mut watch = Watch::new(job, path, streamer);
    watch.run();
}

struct Watch<'a> {
    job: &'a Job,
    path: PathBuf,
    streamer: Streamer<'a>,
    /// The `subagents/` directory of a main session.
    subagents: Option<PathBuf>,
    /// A reload failed after its `Reset`: the receiver holds nothing that
    /// matches the streamer, so the next change reloads again.
    needs_reload: bool,
}

/// The `notify` side: the watcher and what it reports.
struct Notifier {
    watcher: RecommendedWatcher,
    events: Receiver<Wake>,
    subagents_watched: bool,
}

impl<'a> Watch<'a> {
    fn new(job: &'a Job, path: PathBuf, streamer: Streamer<'a>) -> Self {
        let subagents =
            subagents_dir(&path, &job.session).filter(|dir| Some(dir.as_path()) != path.parent());
        Self {
            job,
            path,
            streamer,
            subagents,
            needs_reload: false,
        }
    }

    fn stopped(&self) -> bool {
        self.job.stop.load(Ordering::Relaxed)
    }

    fn run(&mut self) {
        if self.job.mode == WatchMode::Notify {
            match self.start_notify() {
                Ok(mut notifier) => {
                    // Whatever was appended since the end of the initial read
                    // would not produce an event.
                    if self.on_change(false) {
                        self.watch_events(&mut notifier);
                    }
                    return;
                }
                Err(err) => log::warn!(
                    "cannot watch {}: {err}; polling every {:?} instead",
                    self.path.display(),
                    POLL_EVERY
                ),
            }
        } else {
            log::info!(
                "watching is off: polling {} every {:?}",
                self.path.display(),
                POLL_EVERY
            );
        }
        self.poll();
    }

    // -- notify

    fn start_notify(&self) -> notify::Result<Notifier> {
        let dir = self
            .path
            .parent()
            .ok_or_else(|| notify::Error::generic("the session file has no directory"))?;
        let targets = Targets {
            file_name: self.path.file_name().map(ToOwned::to_owned),
            subagents: self.subagents.clone(),
        };
        let (tx, events) = mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                forward(&targets, &tx, event);
            })?;
        watcher.watch(dir, RecursiveMode::NonRecursive)?;
        let mut notifier = Notifier {
            watcher,
            events,
            subagents_watched: false,
        };
        self.watch_subagents(&mut notifier);
        Ok(notifier)
    }

    /// Starts watching the `subagents/` directory once it exists.
    fn watch_subagents(&self, notifier: &mut Notifier) {
        let Some(dir) = &self.subagents else { return };
        if notifier.subagents_watched || !dir.is_dir() {
            return;
        }
        match notifier.watcher.watch(dir, RecursiveMode::NonRecursive) {
            Ok(()) => {
                log::debug!("watching {}", dir.display());
                notifier.subagents_watched = true;
            }
            Err(err) => log::debug!("cannot watch {}: {err}", dir.display()),
        }
    }

    /// Waits for events, coalesces them and processes them. Returns when the
    /// worker is stopped.
    fn watch_events(&mut self, notifier: &mut Notifier) {
        let mut last_check = Instant::now();
        loop {
            if self.stopped() {
                return;
            }
            match notifier.events.recv_timeout(TICK) {
                Ok(first) => {
                    let mut wakes = vec![first];
                    let deadline = Instant::now() + COALESCE;
                    loop {
                        let left = deadline.saturating_duration_since(Instant::now());
                        if left.is_zero() || self.stopped() {
                            break;
                        }
                        match notifier.events.recv_timeout(left.min(TICK)) {
                            Ok(wake) => wakes.push(wake),
                            Err(RecvTimeoutError::Timeout) => {}
                            Err(RecvTimeoutError::Disconnected) => break,
                        }
                    }
                    if self.stopped() {
                        return;
                    }
                    if !self.handle(&wakes, notifier) {
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if !notifier.subagents_watched && last_check.elapsed() >= SUBAGENTS_CHECK {
                        last_check = Instant::now();
                        self.watch_subagents(notifier);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    log::warn!("the file watcher stopped; polling instead");
                    self.poll();
                    return;
                }
            }
        }
    }

    /// Processes a coalesced burst of events. `false` when the worker should
    /// stop.
    fn handle(&mut self, wakes: &[Wake], notifier: &mut Notifier) -> bool {
        let subagent_events = wakes.iter().filter(|w| **w == Wake::Subagents).count();
        if subagent_events > 0 {
            log::debug!("{subagent_events} change(s) in the subagents directory");
        }
        self.watch_subagents(notifier);
        if wakes.contains(&Wake::Session) {
            self.on_change(true)
        } else {
            true
        }
    }

    // -- polling

    fn poll(&mut self) {
        let mut last = self.signature();
        // Whatever was appended since the end of the initial read.
        if !self.on_change(false) {
            return;
        }
        loop {
            let deadline = Instant::now() + POLL_EVERY;
            while Instant::now() < deadline {
                if self.stopped() {
                    return;
                }
                std::thread::sleep(TICK.min(deadline.saturating_duration_since(Instant::now())));
            }
            let now = self.signature();
            if now != last || self.needs_reload {
                last = now;
                if !self.on_change(true) {
                    return;
                }
            }
        }
    }

    /// What polling compares: size, modification time and identity.
    fn signature(&self) -> Option<(u64, Option<SystemTime>, Identity)> {
        let meta = fs::metadata(&self.path).ok()?;
        Some((meta.len(), meta.modified().ok(), Identity::of(&meta)))
    }

    // -- the change itself

    /// Looks at the file after a change: reloads it when it shrank or was
    /// replaced, else reads what was appended. `notified`: a change was
    /// reported (so `Activity` goes out even when no byte was added).
    /// `false` when the worker should stop.
    fn on_change(&mut self, notified: bool) -> bool {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(err) => {
                // Mid-replacement, or gone: the next event tells.
                log::debug!("cannot open {}: {err}", self.path.display());
                return true;
            }
        };
        let meta = match file.metadata() {
            Ok(meta) => meta,
            Err(err) => {
                log::debug!("cannot stat {}: {err}", self.path.display());
                return true;
            }
        };
        let pos = self.streamer.read_pos();
        if self.needs_reload || Identity::of(&meta) != self.streamer.identity() || meta.len() < pos
        {
            return self.reload(file);
        }
        let grew = meta.len() > pos;
        if grew {
            match self.streamer.catch_up(file) {
                Ok(true) => {}
                Ok(false) => return false,
                Err(err) => {
                    log::warn!("cannot read {}: {err}", self.path.display());
                    return true;
                }
            }
        }
        if grew || notified {
            return self.streamer.send_activity();
        }
        true
    }

    /// Parses the file again from the start, after a `Reset`.
    fn reload(&mut self, file: File) -> bool {
        log::info!(
            "{} shrank or was replaced: loading it again",
            self.path.display()
        );
        if self.job.tx.send(ModelUpdate::Reset).is_err() {
            return false;
        }
        match load_file(self.job, &self.path, file) {
            Ok(Some(streamer)) => {
                self.streamer = streamer;
                self.needs_reload = false;
                true
            }
            Ok(None) => false,
            Err(err) => {
                // Keep watching: the next change tries again.
                log::error!("could not load {} again: {err}", self.path.display());
                self.needs_reload = true;
                self.job.tx.send(ModelUpdate::Loaded).is_ok()
            }
        }
    }
}

/// Passes what concerns the session to the worker; the callback runs on
/// `notify`'s thread and holds no reference to the update channel.
fn forward(targets: &Targets, tx: &Sender<Wake>, event: notify::Result<notify::Event>) {
    let notice = notice(event);
    if notice.wake {
        let _ = tx.send(Wake::Session);
    }
    for path in &notice.paths {
        if let Some(wake) = targets.classify(path) {
            // The worker may be gone already.
            let _ = tx.send(wake);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets() -> Targets {
        Targets {
            file_name: Some("s1.jsonl".into()),
            subagents: Some(PathBuf::from("/d/proj/s1/subagents")),
        }
    }

    #[test]
    fn the_session_file_is_told_by_its_name() {
        let t = targets();
        assert_eq!(
            t.classify(Path::new("/d/proj/s1.jsonl")),
            Some(Wake::Session)
        );
        // Spelled differently, as on macOS.
        assert_eq!(
            t.classify(Path::new("/private/d/proj/s1.jsonl")),
            Some(Wake::Session)
        );
        assert_eq!(t.classify(Path::new("/d/proj/s2.jsonl")), None);
    }

    #[test]
    fn subagent_files_and_their_directories_are_told_apart() {
        let t = targets();
        for path in [
            "/d/proj/s1/subagents/agent-a.jsonl",
            "/d/proj/s1/subagents/agent-a.meta.json",
            "/d/proj/s1",
            "/d/proj/s1/subagents",
        ] {
            assert_eq!(t.classify(Path::new(path)), Some(Wake::Subagents), "{path}");
        }
        // Another session's subagents.
        assert_eq!(
            t.classify(Path::new("/d/proj/s2/subagents/agent-a.jsonl")),
            None
        );
    }

    #[test]
    fn without_a_subagents_directory_only_the_file_counts() {
        let t = Targets {
            subagents: None,
            ..targets()
        };
        assert_eq!(t.classify(Path::new("/d/proj/s1/subagents/a.jsonl")), None);
    }
}
