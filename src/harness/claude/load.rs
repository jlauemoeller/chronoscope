//! The session loader: parses a transcript on a worker thread and streams the
//! result as [`ModelUpdate`]s (SPEC §1.1, §9.2).
//!
//! The worker reads the file line by line and feeds an [`Assembler`]. The
//! assembler's updates are coalesced into batches: the first one goes out as
//! soon as there is something to show, later ones every couple of MiB. Each
//! batch is followed by a [`ModelUpdate::Progress`]; the stream ends with
//! `Progress(1.0)`, [`ModelUpdate::ParseWarnings`] and [`ModelUpdate::Loaded`].

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use super::assemble::Assembler;
use super::subagent::resolve;
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

/// Everything the worker thread needs.
pub(super) struct Job {
    pub data_dir: PathBuf,
    pub session: SessionId,
    pub tx: Sender<ModelUpdate>,
    pub stop: Arc<AtomicBool>,
    pub tuning: Tuning,
    pub hook: Option<LoadHook>,
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

/// Reads and streams the session. `Ok` when it finished or was stopped; an
/// error when the file could not be found or read, after the turns parsed so
/// far have been sent.
fn stream(job: &Job) -> io::Result<()> {
    let path = resolve(&job.data_dir, &job.session).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no transcript for session {}", job.session),
        )
    })?;
    let file = File::open(&path)?;
    let total = file.metadata()?.len();
    let mut streamer = Streamer::new(job, path, total);
    match streamer.read_all(file) {
        Ok(true) => {
            streamer.finish();
            Ok(())
        }
        Ok(false) => Ok(()),
        Err(err) => {
            // Best effort: what was parsed is still worth showing.
            if streamer.has_unsent() {
                streamer.flush();
            }
            Err(err)
        }
    }
}

struct Streamer<'a> {
    job: &'a Job,
    asm: Assembler,
    /// Size of the file when it was opened.
    total: u64,
    /// Bytes read so far.
    offset: u64,
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
}

impl<'a> Streamer<'a> {
    fn new(job: &'a Job, path: PathBuf, total: u64) -> Self {
        Self {
            job,
            asm: Assembler::new(ByteSource::File(Arc::new(path))),
            total,
            offset: 0,
            since_flush: 0,
            sent_len: 0,
            first_sent: false,
            dirty: HashSet::new(),
            done: false,
        }
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
            let start = self.offset;
            self.offset += read as u64;
            self.since_flush += read as u64;
            // Like `Assembler::push_chunk`: the range excludes the newline.
            let content = line.strip_suffix(b"\n").unwrap_or(&line);
            let range = start..start + content.len() as u64;
            let updates = self.asm.push_line(content, range);
            self.note(updates);
            if self.due() && !self.flush() {
                return Ok(false);
            }
        }
    }

    /// The end of the file: sends the last batch, then `Progress(1.0)`, the
    /// warning count and `Loaded`.
    fn finish(&mut self) {
        let updates = self.asm.flush_held();
        self.note(updates);
        self.done = true;
        if !self.flush() {
            return;
        }
        let _ = self
            .job
            .tx
            .send(ModelUpdate::ParseWarnings(self.asm.unparsed_count()))
            .and_then(|()| self.job.tx.send(ModelUpdate::Loaded));
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
        tx.send(ModelUpdate::Progress(self.progress())).is_ok()
    }
}
