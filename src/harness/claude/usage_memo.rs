//! The token use of each subagent transcript, kept in memory (SPEC cycle 3
//! §3.5).
//!
//! A running session with hundreds of subagent files is recomputed on every
//! coalesced rebuild. The memo keys each file's usage by (path, size, mtime),
//! as discovery does for its scans, so a recomputation reads only the files
//! that changed.
//!
//! The memo has a lock of its own: [`Harness::stats`](crate::harness::Harness)
//! runs on up to three pool threads while a refresh holds the discovery lock,
//! and no read happens while this lock is held.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use super::subagent::SubagentFile;
use crate::model::TokenUsage;

struct Entry {
    /// The transcript of the session the file belongs to.
    session: PathBuf,
    size: u64,
    mtime: SystemTime,
    tokens: TokenUsage,
}

/// Subagent file to its usage, valid while its size and mtime stay.
#[derive(Default)]
pub(super) struct UsageMemo(Mutex<HashMap<PathBuf, Entry>>);

impl UsageMemo {
    fn lock(&self) -> MutexGuard<'_, HashMap<PathBuf, Entry>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The memoised usage of each of `files`, which are all the subagent
    /// transcripts of the session whose transcript is `session`, in order:
    /// `None` for a file that is new or has changed. Drops the entries of
    /// files of that session that are not in `files` any more.
    pub(super) fn lookup(&self, session: &Path, files: &[SubagentFile]) -> Vec<Option<TokenUsage>> {
        let mut entries = self.lock();
        let listed: HashSet<&Path> = files.iter().map(|f| f.path.as_path()).collect();
        entries.retain(|path, entry| entry.session != session || listed.contains(path.as_path()));
        files
            .iter()
            .map(|file| {
                entries
                    .get(&file.path)
                    .filter(|e| e.size == file.size && e.mtime == file.mtime)
                    .map(|e| e.tokens)
            })
            .collect()
    }

    /// Remembers `tokens` as the usage of `file`, as it was when `file` was
    /// stat'ed (before it was read: a file that grew since is read again).
    pub(super) fn insert(&self, session: &Path, file: &SubagentFile, tokens: TokenUsage) {
        self.lock().insert(
            file.path.clone(),
            Entry {
                session: session.to_path_buf(),
                size: file.size,
                mtime: file.mtime,
                tokens,
            },
        );
    }

    /// Drops what is remembered of `path` (a read of it failed).
    pub(super) fn forget(&self, path: &Path) {
        self.lock().remove(path);
    }

    /// Drops the entries of sessions that are not in `sessions`.
    pub(super) fn retain_sessions(&self, sessions: &HashSet<&Path>) {
        self.lock()
            .retain(|_, entry| sessions.contains(entry.session.as_path()));
    }

    pub(super) fn len(&self) -> usize {
        self.lock().len()
    }
}
