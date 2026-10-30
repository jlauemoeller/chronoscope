//! Harness adapters: translate native session formats into the shared model.

pub mod claude;

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

use crate::model::{
    HarnessId, ModelUpdate, Project, ProjectId, SessionId, SessionSummary, ToolCallId,
};

/// Result type of harness operations.
pub use anyhow::Result;

/// A coding-agent harness whose sessions can be browsed (SPEC §1.1).
pub trait Harness {
    /// Which harness this is.
    fn id(&self) -> HarnessId;

    /// Whether the harness's data store was found.
    fn available(&self) -> bool;

    /// The projects, sorted by most recent activity, newest first.
    fn projects(&self) -> Result<Vec<Project>>;

    /// The sessions of `project`, sorted by start time, newest first.
    fn sessions(&self, project: &ProjectId) -> Result<Vec<SessionSummary>>;

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
