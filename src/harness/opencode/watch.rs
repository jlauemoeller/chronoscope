//! Keeping a loaded session live (product §6.1).
//!
//! After the initial load the worker watches the database's directory with
//! `notify` (not the files themselves: the `-wal` file is created and removed
//! as OpenCode opens and closes the database) and reacts to events about
//! `opencode.db` and `opencode.db-wal` only. Events are debounced: after the
//! first one the worker waits until [`DEBOUNCE`] has passed without another,
//! then reads once. A stream of writes that never pauses (a long streaming
//! message) would postpone that forever, so a read is made at the latest
//! [`MAX_WAIT`] after the first event. When `notify` cannot be started, the
//! worker polls the session's extremes every [`POLL_EVERY`] instead. It always
//! polls on macOS, where writes to the open database produce no events.
//!
//! The database is only ever read (see [`super::db`]).

use std::ffi::OsString;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::harness::WRITES_TO_OPEN_FILES_RAISE_EVENTS;
use crate::harness::watch_event::notice;

use super::db::wal_path;
use super::follow::Follower;
use super::load::{Job, WatchMode};

/// How long the database must be quiet after an event before it is read.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// The longest a read is postponed by a stream of events.
const MAX_WAIT: Duration = Duration::from_secs(1);

/// How often the polling fallback looks at the session.
const POLL_EVERY: Duration = Duration::from_secs(1);

/// How often the worker looks at its stop flag while it waits.
const TICK: Duration = Duration::from_millis(50);

/// Runs the watch loop until the worker is stopped (or the receiver is
/// gone). `follower` is the state after the initial load.
pub(super) fn run(job: &Job, mut follower: Follower<'_>) {
    if !WRITES_TO_OPEN_FILES_RAISE_EVENTS {
        log::info!(
            "file events miss database writes on this platform: polling {} every {POLL_EVERY:?}",
            job.db_path.display()
        );
    } else if job.mode == WatchMode::Notify {
        match start_notify(&job.db_path) {
            Ok((_watcher, events)) => {
                // Whatever was written since the initial read would not
                // produce an event.
                if follower.refresh() {
                    watch_events(&mut follower, &events);
                }
                return;
            }
            Err(err) => log::warn!(
                "cannot watch {}: {err}; polling every {POLL_EVERY:?} instead",
                job.db_path.display()
            ),
        }
    } else {
        log::info!(
            "watching is off: polling {} every {POLL_EVERY:?}",
            job.db_path.display()
        );
    }
    poll(&mut follower);
}

/// Starts watching the database's directory. The watcher must be kept alive.
fn start_notify(db: &Path) -> notify::Result<(RecommendedWatcher, Receiver<()>)> {
    let dir = db
        .parent()
        .ok_or_else(|| notify::Error::generic("the database has no directory"))?;
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let names: Vec<OsString> = [db.to_path_buf(), wal_path(db)]
        .iter()
        .filter_map(|path| path.file_name().map(ToOwned::to_owned))
        .collect();
    let (tx, events) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        forward(&names, &tx, event);
    })?;
    watcher.watch(dir, RecursiveMode::NonRecursive)?;
    Ok((watcher, events))
}

/// Passes what concerns the database to the worker; the callback runs on
/// `notify`'s thread and holds no reference to the update channel.
fn forward(names: &[OsString], tx: &Sender<()>, event: notify::Result<notify::Event>) {
    let notice = notice(event);
    let concerns_us = notice.wake
        || notice.paths.iter().any(|path| {
            path.file_name()
                .is_some_and(|name| names.iter().any(|n| n == name))
        });
    if concerns_us {
        // The worker may be gone already.
        let _ = tx.send(());
    }
}

/// Waits for events, debounces them and reads. Returns when the worker is
/// stopped.
fn watch_events(follower: &mut Follower<'_>, events: &Receiver<()>) {
    loop {
        if follower.stopped() {
            return;
        }
        match events.recv_timeout(TICK) {
            Ok(()) => {
                let first = Instant::now();
                let mut last = first;
                loop {
                    let due = (last + DEBOUNCE).min(first + MAX_WAIT);
                    let left = due.saturating_duration_since(Instant::now());
                    if left.is_zero() || follower.stopped() {
                        break;
                    }
                    match events.recv_timeout(left.min(TICK)) {
                        Ok(()) => last = Instant::now(),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
                if follower.stopped() || !follower.refresh() {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                log::warn!("the file watcher stopped; polling instead");
                poll(follower);
                return;
            }
        }
    }
}

/// Looks at the session every [`POLL_EVERY`] and reads when it changed.
fn poll(follower: &mut Follower<'_>) {
    let mut last = follower.signature();
    // Whatever was written since the initial read.
    if !follower.refresh() {
        return;
    }
    loop {
        let deadline = Instant::now() + POLL_EVERY;
        while Instant::now() < deadline {
            if follower.stopped() {
                return;
            }
            std::thread::sleep(TICK.min(deadline.saturating_duration_since(Instant::now())));
        }
        let now = follower.signature();
        if now.is_some() && now != last {
            last = now;
            if !follower.refresh() {
                return;
            }
        }
    }
}
