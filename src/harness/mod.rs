//! Harness adapters: translate native session formats into the shared model.

pub mod claude;
mod describe;
pub mod opencode;
pub(crate) mod resume;
pub mod shared;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

use crate::config::Config;
use crate::model::{
    HarnessId, ModelUpdate, Project, ProjectId, SectionSource, SessionId, SessionStats,
    SessionSummary, SourceGroup, ToolCallId,
};

/// Result type of harness operations.
pub use anyhow::Result;

/// Whether writing to a file that stays open produces a file event. On macOS
/// the kernel reports a change to a file's content only when the file is
/// closed, and SQLite keeps its files open, so a database has to be polled
/// there.
pub(crate) const WRITES_TO_OPEN_FILES_RAISE_EVENTS: bool = !cfg!(target_os = "macos");

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

    /// Those of [`watch_paths`](Self::watch_paths) that are individual files
    /// (the default: none). The index worker watches such a file through its
    /// directory, non-recursively, and reacts to events about the file's name
    /// only, because the file may be created, replaced or removed (SQLite's
    /// `-wal` file is), which would end a watch on the file itself.
    fn watch_files(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    /// The projects, sorted by most recent activity, newest first.
    fn projects(&self) -> Result<Vec<Project>>;

    /// The sessions of `project`, sorted by start time, newest first.
    fn sessions(&self, project: &ProjectId) -> Result<Vec<SessionSummary>>;

    /// Computes the list stats of `session` (SPEC §4.1) with a full parse.
    /// Reads the whole transcript, so it is for worker threads.
    fn stats(&self, session: &SessionSummary) -> Result<SessionStats>;

    /// A string identifying the state of `session`'s data that
    /// [`stats`](Self::stats) depends on: it changes whenever the data does
    /// (Claude: path, size and mtime, and the count, total size and newest
    /// mtime of the subagent transcripts; OpenCode: db, session id and the
    /// newest `time_updated` of the session and its descendants). It keys the
    /// stats cache (SPEC §9.2), so equal keys must mean equal stats. Cheap: it
    /// may stat files but never reads them.
    ///
    /// The key starts with [`HarnessId::cache_prefix`] and a colon
    /// (`claude:`): the cache is shared by all harnesses, and each prunes
    /// only the entries with its own prefix.
    fn stats_key(&self, session: &SessionSummary) -> String;

    /// Starts loading `session` on a worker thread and returns at once.
    ///
    /// The worker streams the parse over `tx` as [`ModelUpdate`]s: batches of
    /// turns (the first one early), a [`Progress`](ModelUpdate::Progress)
    /// after each, then, for a session without turns that ran local commands,
    /// [`LocalCommands`](ModelUpdate::LocalCommands) (sent again when the list
    /// changes), then [`ParseWarnings`](ModelUpdate::ParseWarnings) and
    /// [`Loaded`](ModelUpdate::Loaded) and an
    /// [`Activity`](ModelUpdate::Activity). The worker then keeps the session
    /// live: appended data arrives as `Appended` and `Updated` batches, each
    /// followed by an `Activity`, and a session that was truncated or
    /// replaced is sent again after a `Reset`. Dropping the returned handle
    /// stops the worker; so does the receiver hanging up.
    fn load(&self, session: &SessionId, tx: Sender<ModelUpdate>) -> Result<WatchHandle>;

    /// The transcript of the subagent started by tool call `call` of
    /// `parent`, as a session [`load`](Self::load) understands.
    fn subagent(&self, parent: &SessionId, call: &ToolCallId) -> Option<SessionId>;

    /// The command that resumes `session` in its harness, or `None` when the
    /// configured template is unusable.
    fn resume_command(&self, session: &SessionSummary) -> Option<Command>;

    /// Reads what the harness records about a section from its source, for
    /// the details overlay (SPEC cycle 2 §5.2): the Message, Tool call,
    /// Prompt and Environment groups of §5.3. `source` holds the section's
    /// references, which the turns keep instead of the field values.
    ///
    /// Runs on the UI thread when the overlay opens, so it reads one or two
    /// lines. A group whose source cannot be read comes back `unavailable`.
    ///
    /// The default provides nothing, and the overlay then shows only what
    /// the model knows, with no `unavailable` row. The Codex and Pi adapters
    /// inherit it.
    fn section_details(&self, _source: &SectionSource) -> Vec<SourceGroup> {
        Vec::new()
    }

    /// Reads the transcript file at `path` (given on the command line, so it
    /// need not be in the harness's data store) and returns its summary. The
    /// harness can [`load`](Self::load) the session from then on. The
    /// default: the harness has no transcript files.
    fn open_file(&self, path: &Path) -> Result<SessionSummary> {
        anyhow::bail!(
            "{} sessions are not files ({})",
            self.id().display_name(),
            path.display()
        )
    }
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

/// The harnesses that exist, in tab order, with whether `config` enables
/// each. The one place that knows which harnesses are enabled.
fn enabled_harnesses(config: &Config) -> [(HarnessId, bool); 2] {
    [
        (HarnessId::Claude, config.harness.claude.enabled),
        (HarnessId::OpenCode, config.harness.opencode.enabled),
    ]
}

/// The harnesses [`build_harnesses`] constructs for `config`, in tab order:
/// the ones with a tab in the selector.
pub fn registered_harnesses(config: &Config) -> Vec<HarnessId> {
    enabled_harnesses(config)
        .into_iter()
        .filter_map(|(id, enabled)| enabled.then_some(id))
        .collect()
}

/// Constructs the enabled harnesses from `config` (SPEC §6.6), in tab order.
///
/// A harness with `enabled = false` is not constructed, so its tab is hidden
/// (its number stays reserved). One whose data store is missing is kept: its
/// [`available`](Harness::available) is false and the selector shows it
/// muted. Claude and OpenCode exist so far.
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
    registered_harnesses(config)
        .into_iter()
        .filter_map(|id| match id {
            HarnessId::Claude => {
                Some(Arc::new(claude::ClaudeHarness::new(config, home)) as Arc<dyn Harness>)
            }
            HarnessId::OpenCode => {
                Some(Arc::new(opencode::OpenCodeHarness::new(config, home)) as Arc<dyn Harness>)
            }
            // No adapter yet.
            HarnessId::Codex | HarnessId::Pi => None,
        })
        .collect()
}

#[cfg(test)]
mod default_details_tests {
    use std::process::Command;

    use super::*;
    use crate::model::{ByteSource, LineRefs, SectionRefs, SourceKind, SourceRef};

    /// A harness that implements only what the trait requires, as the Codex
    /// and Pi adapters would.
    struct Stub;

    impl Harness for Stub {
        fn id(&self) -> HarnessId {
            HarnessId::Codex
        }
        fn available(&self) -> bool {
            true
        }
        fn refresh(&self) -> Result<()> {
            Ok(())
        }
        fn watch_paths(&self) -> Vec<PathBuf> {
            Vec::new()
        }
        fn projects(&self) -> Result<Vec<Project>> {
            Ok(Vec::new())
        }
        fn sessions(&self, _: &ProjectId) -> Result<Vec<SessionSummary>> {
            Ok(Vec::new())
        }
        fn stats(&self, _: &SessionSummary) -> Result<SessionStats> {
            anyhow::bail!("not used")
        }
        fn stats_key(&self, _: &SessionSummary) -> String {
            String::new()
        }
        fn load(&self, _: &SessionId, _: Sender<ModelUpdate>) -> Result<WatchHandle> {
            anyhow::bail!("not used")
        }
        fn subagent(&self, _: &SessionId, _: &ToolCallId) -> Option<SessionId> {
            None
        }
        fn resume_command(&self, _: &SessionSummary) -> Option<Command> {
            None
        }
    }

    #[test]
    fn a_harness_provides_no_section_details_by_default() {
        let source = SectionSource {
            kind: SourceKind::Message,
            refs: SectionRefs::Lines(LineRefs {
                line: SourceRef {
                    bytes: ByteSource::Memory(Arc::from(&b"{}"[..])),
                    range: 0..2,
                },
                result: None,
            }),
        };
        assert!(Stub.section_details(&source).is_empty());
    }
}

#[cfg(test)]
mod registered_tests {
    use super::*;

    #[test]
    fn registered_harnesses_match_the_constructed_ones() {
        let mut config = Config::default();
        let home = Path::new("/nonexistent");
        for (claude, opencode) in [(true, true), (false, true), (true, false), (false, false)] {
            config.harness.claude.enabled = claude;
            config.harness.opencode.enabled = opencode;
            let built: Vec<HarnessId> = build_harnesses_with_home(&config, home)
                .iter()
                .map(|h| h.id())
                .collect();
            assert_eq!(registered_harnesses(&config), built);
        }
    }
}
