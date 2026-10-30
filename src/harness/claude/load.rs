//! The session loader: parses a transcript on a worker thread and streams the
//! result as [`ModelUpdate`]s (SPEC §1.1, §7.1, §9.2).
//!
//! The worker reads the file line by line and feeds an [`Assembler`]. The
//! assembler's updates are coalesced into batches: the first one goes out as
//! soon as there is something to show, later ones every couple of MiB. Each
//! batch is followed by a [`ModelUpdate::Progress`]; the initial stream ends
//! with `Progress(1.0)`, [`ModelUpdate::ParseWarnings`], [`ModelUpdate::Loaded`]
//! and a first [`ModelUpdate::Activity`]. The worker then keeps the session
//! live (see [`super::watch`]) until it is stopped.
//!
//! A final line without its newline may still be being written: it is kept in
//! a buffer and parsed once the newline arrives (unless it is already a
//! complete JSON value, which a file that simply lacks a final newline is).
//! The price: a session that died mid-write leaves a cut-off last line that
//! is never parsed, so it is not counted among the unparsable entries either.

use std::collections::HashSet;
use std::fs::{self, File, Metadata};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use super::assemble::Assembler;
use super::subagent::resolve;
use super::watch;
use crate::model::{ByteSource, ModelUpdate, SessionId, Turn, TurnId};

/// A callback the worker runs before it sends each batch. A test hook.
pub(super) type LoadHook = Arc<dyn Fn() + Send + Sync>;

/// When batches are sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Tuning {
    /// The first batch goes out once this many turns have been assembled...
    pub first_turns: usize,
    /// ...or this many bytes of input have been read.
    pub first_bytes: u64,
    /// Later batches go out after this many bytes of input.
    pub batch_bytes: u64,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            first_turns: 50,
            first_bytes: 256 * 1024,
            batch_bytes: 2 * 1024 * 1024,
        }
    }
}

/// How the worker learns that the file changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WatchMode {
    /// A `notify` watch; polling when it cannot be started.
    Notify,
    /// Polling only (a test seam for "watching is unavailable").
    Poll,
}

/// Everything the worker thread needs.
pub(super) struct Job {
    pub data_dir: PathBuf,
    pub session: SessionId,
    pub tx: Sender<ModelUpdate>,
    pub stop: Arc<AtomicBool>,
    pub tuning: Tuning,
    pub hook: Option<LoadHook>,
    pub mode: WatchMode,
}

/// The worker's body. Never panics on bad input: a session that cannot be
/// found or read is logged and ends in [`ModelUpdate::Loaded`].
pub(super) fn run(job: Job) {
    match stream(&job) {
        Ok(()) => {}
        Err(err) => {
            log::error!("could not load session {}: {err}", job.session);
            // The receiver may be gone already; nothing more to do then.
            let _ = job.tx.send(ModelUpdate::Loaded);
        }
    }
}

/// Reads and streams the session, then keeps it live. `Ok` when the worker
/// was stopped; an error when the file could not be found or read, after the
/// turns parsed so far have been sent.
fn stream(job: &Job) -> io::Result<()> {
    let path = resolve(&job.data_dir, &job.session).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no transcript for session {}", job.session),
        )
    })?;
    let file = File::open(&path)?;
    match load_file(job, &path, file)? {
        Some(streamer) => {
            watch::run(job, path, streamer);
            Ok(())
        }
        None => Ok(()),
    }
}

/// Parses `file` from the start and streams it, ending with `Loaded` and an
/// `Activity`. `None` when the worker was stopped (or the receiver is gone)
/// before the end.
pub(super) fn load_file<'a>(
    job: &'a Job,
    path: &Path,
    file: File,
) -> io::Result<Option<Streamer<'a>>> {
    let meta = file.metadata()?;
    let mut streamer = Streamer::new(job, path.to_path_buf(), &meta);
    match streamer.read_all(file) {
        Ok(true) => Ok(streamer.finish().then_some(streamer)),
        Ok(false) => Ok(None),
        Err(err) => {
            // Best effort: what was parsed is still worth showing.
            if streamer.has_unsent() {
                streamer.flush();
            }
            Err(err)
        }
    }
}

/// What tells two files at one path apart: the inode (with the device) on
/// Unix, the creation time elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Identity {
    #[cfg(unix)]
    id: (u64, u64),
    #[cfg(not(unix))]
    created: Option<std::time::SystemTime>,
}

impl Identity {
    pub(super) fn of(meta: &Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Self {
                id: (meta.dev(), meta.ino()),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                created: meta.created().ok(),
            }
        }
    }
}

pub(super) struct Streamer<'a> {
    job: &'a Job,
    path: PathBuf,
    /// The file the stream was read from.
    identity: Identity,
    asm: Assembler,
    /// Size of the file when it was opened.
    total: u64,
    /// Bytes consumed so far: the end of the last complete line fed to the
    /// assembler.
    offset: u64,
    /// Bytes read past `offset` that do not end in a newline yet.
    partial: Vec<u8>,
    /// Bytes read since the last batch.
    since_flush: u64,
    /// How many of the assembler's turns have been sent.
    sent_len: usize,
    /// Whether an `Appended` batch has gone out.
    first_sent: bool,
    /// Turns updated since the last batch.
    dirty: HashSet<TurnId>,
    /// The end of the file was reached: progress is complete.
    done: bool,
    /// The last warning count sent.
    warnings_sent: Option<usize>,
}

impl<'a> Streamer<'a> {
    fn new(job: &'a Job, path: PathBuf, meta: &Metadata) -> Self {
        Self {
            job,
            asm: Assembler::new(ByteSource::File(Arc::new(path.clone()))),
            path,
            identity: Identity::of(meta),
            total: meta.len(),
            offset: 0,
            partial: Vec::new(),
            since_flush: 0,
            sent_len: 0,
            first_sent: false,
            dirty: HashSet::new(),
            done: false,
            warnings_sent: None,
        }
    }

    /// Where the next byte to read is: past the buffered partial line.
    pub(super) fn read_pos(&self) -> u64 {
        self.offset + self.partial.len() as u64
    }

    /// The identity of the file this stream was read from.
    pub(super) fn identity(&self) -> Identity {
        self.identity
    }

    /// Feeds the whole file to the assembler, sending batches on the way.
    /// `Ok(false)` when stopped before the end.
    fn read_all(&mut self, file: File) -> io::Result<bool> {
        let mut reader = BufReader::with_capacity(256 * 1024, file);
        let mut line = Vec::new();
        loop {
            if self.job.stop.load(Ordering::Relaxed) {
                return Ok(false);
            }
            line.clear();
            let read = reader.read_until(b'\n', &mut line)?;
            if read == 0 {
                return Ok(true);
            }
            if !line.ends_with(b"\n") {
                // The end of the file, in the middle of a line or at a final
                // line that just has no newline.
                if serde_json::from_slice::<serde::de::IgnoredAny>(&line).is_ok() {
                    self.feed(&line);
                } else {
                    self.partial = line;
                }
                return Ok(true);
            }
            self.feed(&line);
            if self.due() && !self.flush() {
                return Ok(false);
            }
        }
    }

    /// Feeds one line, with or without its newline, to the assembler.
    fn feed(&mut self, line: &[u8]) {
        let start = self.offset;
        self.offset += line.len() as u64;
        self.since_flush += line.len() as u64;
        // Like `Assembler::push_chunk`: the range excludes the newline.
        let content = line.strip_suffix(b"\n").unwrap_or(line);
        let range = start..start + content.len() as u64;
        let updates = self.asm.push_line(content, range);
        self.note(updates);
    }

    /// The end of the file: sends the last batch, then `Progress(1.0)`, the
    /// warning count, `Loaded` and the file's `Activity`. `false` when the
    /// worker should stop.
    fn finish(&mut self) -> bool {
        let updates = self.asm.flush_held();
        self.note(updates);
        self.done = true;
        if !self.flush() {
            return false;
        }
        self.send_warnings()
            && self.job.tx.send(ModelUpdate::Loaded).is_ok()
            && self.send_activity()
    }

    /// Reads what was appended since the last read, from `file` (opened just
    /// now), and sends it as one batch of updates. `Ok(false)` when the
    /// worker should stop.
    pub(super) fn catch_up(&mut self, mut file: File) -> io::Result<bool> {
        file.seek(SeekFrom::Start(self.read_pos()))?;
        let wanted = file.metadata()?.len().saturating_sub(self.read_pos());
        let mut chunk = vec![0; wanted.clamp(4096, 256 * 1024) as usize];
        loop {
            if self.job.stop.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let read = file.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            self.push_bytes(&chunk[..read]);
        }
        let updates = self.asm.flush_held();
        self.note(updates);
        if self.job.stop.load(Ordering::Relaxed) {
            return Ok(false);
        }
        Ok((!self.has_unsent() || self.send_batch()) && self.send_warnings())
    }

    /// Feeds bytes read from the end of the file: complete lines go to the
    /// assembler, the rest waits in the buffer for its newline.
    fn push_bytes(&mut self, data: &[u8]) {
        let mut buf = std::mem::take(&mut self.partial);
        // Bytes already buffered contain no newline: search only the new ones.
        let mut search_from = buf.len();
        buf.extend_from_slice(data);
        let mut consumed = 0;
        while let Some(rel) = buf[search_from..].iter().position(|&b| b == b'\n') {
            let end = search_from + rel;
            self.feed(&buf[consumed..=end]);
            consumed = end + 1;
            search_from = consumed;
        }
        buf.drain(..consumed);
        self.partial = buf;
    }

    /// Sends the warning count when it differs from the last one sent.
    fn send_warnings(&mut self) -> bool {
        let count = self.asm.unparsed_count();
        if self.warnings_sent == Some(count) {
            return true;
        }
        self.warnings_sent = Some(count);
        self.job.tx.send(ModelUpdate::ParseWarnings(count)).is_ok()
    }

    /// Sends the file's modification time. `true` also when it cannot be
    /// read (nothing to send then).
    pub(super) fn send_activity(&self) -> bool {
        let modified = fs::metadata(&self.path).and_then(|m| m.modified());
        match modified {
            Ok(last_modified) => self
                .job
                .tx
                .send(ModelUpdate::Activity {
                    last_modified,
                    idle: None,
                })
                .is_ok(),
            Err(err) => {
                log::debug!("no modification time for {}: {err}", self.path.display());
                true
            }
        }
    }

    /// Remembers which already-sent turns the assembler changed.
    fn note(&mut self, updates: Vec<ModelUpdate>) {
        for update in updates {
            if let ModelUpdate::Updated(turns) = update {
                self.dirty.extend(turns.iter().map(|t| t.id().clone()));
            }
        }
    }

    fn has_unsent(&self) -> bool {
        self.asm.turns().len() > self.sent_len || !self.dirty.is_empty()
    }

    fn due(&self) -> bool {
        let new_turns = self.asm.turns().len() - self.sent_len;
        if self.first_sent {
            self.since_flush >= self.job.tuning.batch_bytes
        } else {
            new_turns > 0
                && (new_turns >= self.job.tuning.first_turns
                    || self.since_flush >= self.job.tuning.first_bytes)
        }
    }

    fn progress(&self) -> f32 {
        if self.done || self.total == 0 {
            1.0
        } else {
            (self.offset as f64 / self.total as f64).min(1.0) as f32
        }
    }

    /// Sends the batch and its `Progress`. A turn that was both appended and
    /// updated since the last batch is sent once, as appended. `false` when
    /// the worker should stop: it was told to, or the receiver is gone.
    fn flush(&mut self) -> bool {
        if let Some(hook) = &self.job.hook {
            hook();
        }
        if self.job.stop.load(Ordering::Relaxed) {
            return false;
        }
        self.send_batch()
            && self
                .job
                .tx
                .send(ModelUpdate::Progress(self.progress()))
                .is_ok()
    }

    /// Sends the turns appended and updated since the last batch.
    fn send_batch(&mut self) -> bool {
        let turns = self.asm.turns();
        let updated: Vec<Arc<Turn>> = turns[..self.sent_len]
            .iter()
            .filter(|t| self.dirty.contains(t.id()))
            .cloned()
            .collect();
        let appended = turns[self.sent_len..].to_vec();
        self.dirty.clear();
        self.sent_len = turns.len();
        self.since_flush = 0;
        let tx = &self.job.tx;
        if !updated.is_empty() && tx.send(ModelUpdate::Updated(updated)).is_err() {
            return false;
        }
        if !appended.is_empty() {
            self.first_sent = true;
            if tx.send(ModelUpdate::Appended(appended)).is_err() {
                return false;
            }
        }
        true
    }
}
