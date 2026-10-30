//! Harness adapters: translate native session formats into the shared model.

pub mod claude;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

use crate::config::Config;
use crate::model::{
    HarnessId, ModelUpdate, Project, ProjectId, SessionId, SessionStats, SessionSummary, ToolCallId,
};

/// Result type of harness operations.
pub use anyhow::Result;

/// A coding-agent harness whose sessions can be browsed (SPEC §1.1).
///
/// Harnesses are shared with the index worker threads, hence `Send + Sync`.
pub trait Harness: Send + Sync {
    /// Which harness this is.
    fn id(&self) -> HarnessId;

    /// Whether the harness's data store was found.
    fn available(&self) -> bool;

    /// Re-reads the harness's store so that [`projects`](Self::projects) and
    /// [`sessions`](Self::sessions) reflect what is on disk now. Only changed
    /// data is re-read. May take a while on a big store: call it from a
    /// worker thread.
    fn refresh(&self) -> Result<()>;

    /// The paths whose changes (watched recursively) can alter the project
    /// and session lists: the data directory, or the database's directory.
    /// The index worker watches them to know when to [`refresh`](Self::refresh).
    fn watch_paths(&self) -> Vec<PathBuf>;

    /// The projects, sorted by most recent activity, newest first.
    fn projects(&self) -> Result<Vec<Project>>;

    /// The sessions of `project`, sorted by start time, newest first.
    fn sessions(&self, project: &ProjectId) -> Result<Vec<SessionSummary>>;

    /// Computes the list stats of `session` (SPEC §4.1) with a full parse.
    /// Reads the whole transcript, so it is for worker threads.
    fn stats(&self, session: &SessionSummary) -> Result<SessionStats>;

    /// A string identifying the state of `session`'s data that
    /// [`stats`](Self::stats) depends on: it changes whenever the data does
    /// (Claude: path, size and mtime; OpenCode: db, session id and
    /// `time_updated`). It keys the stats cache (SPEC §9.2), so equal keys
    /// must mean equal stats. Cheap: it may stat a file but never parses.
    ///
    /// The key starts with [`HarnessId::cache_prefix`] and a colon
    /// (`claude:`): the cache is shared by all harnesses, and each prunes
    /// only the entries with its own prefix.
    fn stats_key(&self, session: &SessionSummary) -> String;

    /// Starts loading `session` on a worker thread and returns at once.
    ///
    /// The worker streams the parse over `tx` as [`ModelUpdate`]s: batches of
    /// turns (the first one early), a [`Progress`](ModelUpdate::Progress)
    /// after each, then [`ParseWarnings`](ModelUpdate::ParseWarnings) and
    /// [`Loaded`](ModelUpdate::Loaded). Dropping the returned handle stops
    /// the worker; so does the receiver hanging up.
    fn load(&self, session: &SessionId, tx: Sender<ModelUpdate>) -> Result<WatchHandle>;

    /// The transcript of the subagent started by tool call `call` of
    /// `parent`, as a session [`load`](Self::load) understands.
    fn subagent(&self, parent: &SessionId, call: &ToolCallId) -> Option<SessionId>;

    /// The command that resumes `session` in its harness, or `None` when the
    /// configured template is unusable.
    fn resume_command(&self, session: &SessionSummary) -> Option<Command>;
}

/// Keeps a session's worker alive. Dropping it tells the worker to stop; it
/// does not wait for it.
#[derive(Debug)]
#[must_use = "dropping the handle stops the worker"]
pub struct WatchHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WatchHandle {
    /// Wraps a worker thread and the flag it polls to know when to stop.
    pub fn new(stop: Arc<AtomicBool>, thread: JoinHandle<()>) -> Self {
        Self {
            stop,
            thread: Some(thread),
        }
    }

    /// Tells the worker to stop, without waiting for it.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Whether the worker thread has ended.
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The harnesses [`build_harnesses`] constructs for `config`, in tab order:
/// the ones with a tab in the selector.
pub fn registered_harnesses(config: &Config) -> Vec<HarnessId> {
    let mut harnesses = Vec::new();
    if config.harness.claude.enabled {
        harnesses.push(HarnessId::Claude);
    }
    harnesses
}

/// Constructs the enabled harnesses from `config` (SPEC §6.6), in tab order.
///
/// A harness with `enabled = false` is not constructed, so its tab is hidden
/// (its number stays reserved). One whose data store is missing is kept: its
/// [`available`](Harness::available) is false and the selector shows it
/// muted. Only Claude exists so far.
///
/// The home directory (for default data directories) is detected from the
/// environment; see [`build_harnesses_with_home`] to give it explicitly.
pub fn build_harnesses(config: &Config) -> Vec<Arc<dyn Harness>> {
    let home = crate::paths::Paths::detect().map_or_else(
        || {
            log::warn!(
                "the home directory is unknown: default data directories resolve against the current directory"
            );
            PathBuf::new()
        },
        |paths| paths.home_dir,
    );
    build_harnesses_with_home(config, &home)
}

/// [`build_harnesses`] with an explicit home directory.
pub fn build_harnesses_with_home(config: &Config, home: &Path) -> Vec<Arc<dyn Harness>> {
    let mut harnesses: Vec<Arc<dyn Harness>> = Vec::new();
    if config.harness.claude.enabled {
        harnesses.push(Arc::new(claude::ClaudeHarness::new(config, home)));
    }
    harnesses
}

#[cfg(test)]
mod registered_tests {
    use super::*;

    #[test]
    fn registered_harnesses_match_the_constructed_ones() {
        let mut config = Config::default();
        let home = Path::new("/nonexistent");
        for enabled in [true, false] {
            config.harness.claude.enabled = enabled;
            let built: Vec<HarnessId> = build_harnesses_with_home(&config, home)
                .iter()
                .map(|h| h.id())
                .collect();
            assert_eq!(registered_harnesses(&config), built);
        }
    }
}
