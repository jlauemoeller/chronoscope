//! Following a loaded session: reads what changed in the database since the
//! last read and turns it into [`ModelUpdate`]s (SPEC §7.1).
//!
//! A change is one pass of [`Follower::refresh`]:
//!
//! 1. rows seen before whose `time_updated` moved (OpenCode updates a
//!    streaming message in place) are fed to the assembler again, which
//!    replaces their share of the turn: `Updated` with stable ids;
//! 2. rows with a greater `seq` than any seen are new: `Appended` for a new
//!    turn, `Updated` for the Assistant turn they extend;
//! 3. the unparsable-row count and the session's activity (`time_updated`,
//!    `time_idle`) are sent when they changed.
//!
//! Known limits: rows that OpenCode deletes are not noticed (no `Reset`);
//! a part that appears in a row updated in place is added at the end of its
//! turn; `synthetic` and `system` rows updated in place are not re-read; a
//! row whose `data` could not be parsed when it was first read is not read
//! again to be shown (it stays counted in the warnings); injected entries
//! that arrive in one pass and precede a `user` row that arrives in the next
//! stay in the earlier turn (a reload would put them before the prompt).
//!
//! Every statement goes through [`Db::select`], which only runs `SELECT`s on
//! a read-only connection.

use std::collections::HashMap;

use super::assemble::{Assembler, Row};
use super::db::{Db, OpenMode};
use super::discovery::connect;
use super::load::{Job, ROW_COLUMNS, activity_update, fetch_page, read_row, send_all};
use crate::harness::Result;
use crate::model::ModelUpdate;

/// Which rows have been read, and in what state.
#[derive(Default)]
pub(super) struct Cursor {
    /// The greatest `seq` read (`None` before any row).
    last_seq: Option<i64>,
    /// The greatest `time_updated` read.
    max_updated: Option<i64>,
    /// Each row read: its `time_updated` and the byte length of its `data`.
    /// A row is changed when either differs, so most updates within one
    /// millisecond are told apart too (not a rewrite to text of the same
    /// length). Only these two are compared, so that a pass does not read
    /// the `data` of rows that did not change.
    seen: HashMap<String, (i64, usize)>,
}

impl Cursor {
    /// Records that `row` was read (and fed to the assembler).
    pub(super) fn note(&mut self, row: &Row) {
        self.last_seq = Some(self.last_seq.map_or(row.seq, |seq| seq.max(row.seq)));
        self.max_updated = Some(
            self.max_updated
                .map_or(row.updated, |updated| updated.max(row.updated)),
        );
        self.seen
            .insert(row.id.clone(), (row.updated, row.data.len()));
    }

    /// Whether a row with this `time_updated` and `data` length differs from
    /// what was read before. `None` for a row that was never read.
    fn changed(&self, id: &str, updated: i64, len: usize) -> Option<bool> {
        self.seen.get(id).map(|&seen| seen != (updated, len))
    }
}

/// What polling compares: the extremes of the session's rows and the session
/// row's times. A rewrite of a row's `data` that leaves its `time_updated`
/// (to the millisecond) and the extremes alone is not seen by polling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Signature {
    max_seq: Option<i64>,
    max_updated: Option<i64>,
    rows: i64,
    session_updated: Option<i64>,
    session_idle: Option<i64>,
}

/// The live state of one loaded session.
pub(super) struct Follower<'a> {
    job: &'a Job,
    /// `None` after a failure: reopened by the next pass.
    db: Option<Db>,
    asm: Assembler,
    cursor: Cursor,
    /// The `time_updated` and `time_idle` last sent as an `Activity`.
    activity: (i64, Option<i64>),
    /// The unparsable-row count last sent.
    warnings: usize,
}

impl<'a> Follower<'a> {
    /// The state after the initial load: `asm` and `cursor` have seen every
    /// row, and `activity` was sent.
    pub(super) fn new(
        job: &'a Job,
        db: Db,
        asm: Assembler,
        cursor: Cursor,
        activity: (i64, Option<i64>),
    ) -> Self {
        let warnings = asm.unparsed_count();
        Self {
            job,
            db: Some(db),
            asm,
            cursor,
            activity,
            warnings,
        }
    }

    /// Whether the worker was told to stop.
    pub(super) fn stopped(&self) -> bool {
        self.job.stop.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The database, opened again after a failure (or when it is read as
    /// immutable, which never sees a change).
    fn connection(&mut self) -> Option<Db> {
        if let Some(db) = self.db.take() {
            return Some(db);
        }
        match connect(&self.job.db_path, &self.job.warned) {
            Ok(db) => Some(db),
            Err(err) => {
                log::debug!("cannot open {}: {err:#}", self.job.db_path.display());
                None
            }
        }
    }

    /// The signature of the session now; `None` when it cannot be read.
    pub(super) fn signature(&mut self) -> Option<Signature> {
        let db = self.connection()?;
        let id = self.job.session.as_str();
        let signature = db
            .select_one(
                "SELECT (SELECT max(seq) FROM session_message WHERE session_id = ?1), \
                        (SELECT max(time_updated) FROM session_message WHERE session_id = ?1), \
                        (SELECT count(*) FROM session_message WHERE session_id = ?1), \
                        (SELECT time_updated FROM session_v2 WHERE id = ?1), \
                        (SELECT time_idle FROM session_v2 WHERE id = ?1)",
                [id],
                |row| {
                    Ok(Signature {
                        max_seq: row.get(0)?,
                        max_updated: row.get(1)?,
                        rows: row.get(2)?,
                        session_updated: row.get(3)?,
                        session_idle: row.get(4)?,
                    })
                },
            )
            .unwrap_or_else(|err| {
                log::debug!("cannot poll session {id}: {err:#}");
                None
            });
        self.keep(db);
        signature
    }

    /// Keeps the connection for the next pass, unless it cannot see changes.
    fn keep(&mut self, db: Db) {
        if db.mode() == OpenMode::ReadOnly {
            self.db = Some(db);
        }
    }

    /// Reads what changed and sends it. `false` when the worker should stop:
    /// it was told to, or the receiver is gone. A database that cannot be
    /// read is logged and tried again by the next pass.
    pub(super) fn refresh(&mut self) -> bool {
        if self.stopped() {
            return false;
        }
        let Some(db) = self.connection() else {
            return true;
        };
        match self.step(&db) {
            Ok(keep_going) => {
                self.keep(db);
                keep_going
            }
            Err(err) => {
                // The connection is dropped: the next pass opens it again.
                log::warn!("could not read session {}: {err:#}", self.job.session);
                true
            }
        }
    }

    fn step(&mut self, db: &Db) -> Result<bool> {
        let id = self.job.session.as_str();
        let mut changed = false;

        // 1. Rows updated in place. Their `time_updated` is not less than the
        // greatest one read before this pass. A first query lists them with
        // the length of their `data`; only those that differ from what was
        // read are read in full.
        if let (Some(since), Some(last)) = (self.cursor.max_updated, self.cursor.last_seq) {
            let listed = db.select(
                "SELECT id, time_updated, length(CAST(data AS BLOB)) FROM session_message \
                 WHERE session_id = ?1 AND seq <= ?2 AND time_updated >= ?3 ORDER BY seq",
                rusqlite::params![id, last, since],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        usize::try_from(row.get::<_, i64>(2)?).unwrap_or_default(),
                    ))
                },
            )?;
            for (row_id, updated, len) in listed {
                match self.cursor.changed(&row_id, updated, len) {
                    Some(true) => {
                        let row = db.select_one(
                            &format!(
                                "SELECT {ROW_COLUMNS} FROM session_message \
                                 WHERE session_id = ?1 AND id = ?2"
                            ),
                            rusqlite::params![id, row_id],
                            read_row,
                        )?;
                        if let Some(row) = row {
                            self.asm.replace_row(&row);
                            self.cursor.note(&row);
                            changed = true;
                        }
                    }
                    Some(false) => {}
                    None => log::debug!(
                        "session_message {row_id} appeared behind the last one read; skipped"
                    ),
                }
            }
        }

        // 2. New rows, in pages.
        loop {
            if self.stopped() {
                return Ok(false);
            }
            let after = self.cursor.last_seq.unwrap_or(i64::MIN);
            let rows = fetch_page(db, id, after, self.job.page_size)?;
            for row in &rows {
                self.asm.push_row(row);
                self.cursor.note(row);
            }
            changed |= !rows.is_empty();
            if rows.len() < self.job.page_size {
                break;
            }
        }

        // Injected entries still waiting go into the current turn, as at the
        // end of the initial load.
        self.asm.finish();
        let updates = self.asm.take_updates();
        changed |= !updates.is_empty();
        if !send_all(&self.job.tx, updates) {
            return Ok(false);
        }
        if self.asm.unparsed_count() != self.warnings {
            self.warnings = self.asm.unparsed_count();
            if self
                .job
                .tx
                .send(ModelUpdate::ParseWarnings(self.warnings))
                .is_err()
            {
                return Ok(false);
            }
        }
        self.send_activity(db, changed)
    }

    /// Sends the session's activity: after a change of rows, or when the
    /// session row itself changed (`time_idle` is set and cleared there).
    fn send_activity(&mut self, db: &Db, rows_changed: bool) -> Result<bool> {
        let session = db.select_one(
            "SELECT time_updated, time_idle FROM session_v2 WHERE id = ?1",
            [self.job.session.as_str()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
        )?;
        let Some(now) = session else {
            return Ok(true);
        };
        if !rows_changed && now == self.activity {
            return Ok(true);
        }
        self.activity = now;
        Ok(self.job.tx.send(activity_update(now.0, now.1)).is_ok())
    }
}
