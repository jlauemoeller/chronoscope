//! The session loader: reads a session's `session_message` rows in pages on a
//! worker thread and streams the turns as [`ModelUpdate`]s (SPEC §9.2).
//!
//! Each page is `SELECT ... WHERE session_id = ? AND seq > ? ORDER BY seq
//! LIMIT <page size>` (500 rows by default), so memory and the time to the
//! first screen do not depend on the size of the session. Every page is
//! followed by a [`ModelUpdate::Progress`] (the fraction of the session's
//! rows read). The stream ends with `Progress(1.0)`,
//! [`ModelUpdate::ParseWarnings`], [`ModelUpdate::Loaded`] and an
//! [`ModelUpdate::Activity`] with the session's `time_updated` and idleness.
//! Watching the session for new rows is not done here (task 029): the worker
//! ends after the initial load.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::anyhow;
use rusqlite::types::ValueRef;

use super::assemble::{Assembler, Row};
use super::db::Db;
use super::discovery::connect;
use crate::harness::Result;
use crate::model::{ModelUpdate, SessionId};

/// How many rows one query reads.
pub(super) const PAGE_SIZE: usize = 500;

/// Everything the worker thread needs.
pub(super) struct Job {
    pub db_path: PathBuf,
    pub session: SessionId,
    pub tx: Sender<ModelUpdate>,
    pub stop: Arc<AtomicBool>,
    pub page_size: usize,
    pub warned: Arc<AtomicBool>,
}

/// The worker's body. Never panics on bad input: a session that cannot be
/// found or read is logged and ends in [`ModelUpdate::Loaded`].
pub(super) fn run(job: Job) {
    if let Err(err) = stream(&job) {
        log::error!("could not load session {}: {err:#}", job.session);
        // The receiver may be gone already; nothing more to do then.
        let _ = job.tx.send(ModelUpdate::Loaded);
    }
}

/// What the loader needs to know about the session row.
struct SessionInfo {
    directory: String,
    updated: i64,
    idle: Option<i64>,
}

/// Reads and streams the session. `Ok` also when the worker was stopped or
/// the receiver is gone; an error when the session cannot be found or read.
fn stream(job: &Job) -> Result<()> {
    let db = connect(&job.db_path, &job.warned)?;
    let id = job.session.as_str();
    let info = db
        .select_one(
            "SELECT directory, time_updated, time_idle FROM session_v2 WHERE id = ?1",
            [id],
            |row| {
                Ok(SessionInfo {
                    directory: row.get(0)?,
                    updated: row.get(1)?,
                    idle: row.get(2)?,
                })
            },
        )?
        .ok_or_else(|| anyhow!("no session {id} in {}", job.db_path.display()))?;
    let total = db
        .select_one(
            "SELECT count(*) FROM session_message WHERE session_id = ?1",
            [id],
            |row| row.get::<_, i64>(0),
        )?
        .unwrap_or_default()
        .max(0) as f64;

    let mut asm = Assembler::new();
    if !info.directory.is_empty() {
        asm = asm.with_cwd(&info.directory);
    }
    let mut after = i64::MIN;
    let mut done = 0_f64;
    let mut progress = 0_f32;
    loop {
        if job.stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let rows = fetch_page(&db, id, after, job.page_size)?;
        for row in &rows {
            asm.push_row(row);
        }
        let Some(last) = rows.last() else { break };
        after = last.seq;
        done += rows.len() as f64;
        let last_page = rows.len() < job.page_size || done >= total;
        if last_page {
            // Injected entries still waiting go into the last turn, in the
            // last batch.
            asm.finish();
        }
        progress = if total > 0.0 {
            (done / total).min(1.0) as f32
        } else {
            1.0
        };
        if !send_all(&job.tx, asm.take_updates())
            || job.tx.send(ModelUpdate::Progress(progress)).is_err()
        {
            return Ok(());
        }
        if last_page {
            break;
        }
    }
    asm.finish();
    if !send_all(&job.tx, asm.take_updates()) {
        return Ok(());
    }
    let tail = [
        (progress < 1.0).then_some(ModelUpdate::Progress(1.0)),
        Some(ModelUpdate::ParseWarnings(asm.unparsed_count())),
        Some(ModelUpdate::Loaded),
        Some(ModelUpdate::Activity {
            last_modified: UNIX_EPOCH
                + Duration::from_millis(u64::try_from(info.updated).unwrap_or(0)),
            idle: Some(info.idle.is_some()),
        }),
    ];
    for update in tail.into_iter().flatten() {
        if job.tx.send(update).is_err() {
            break;
        }
    }
    Ok(())
}

fn send_all(tx: &Sender<ModelUpdate>, updates: Vec<ModelUpdate>) -> bool {
    updates.into_iter().all(|update| tx.send(update).is_ok())
}

/// The next at most `limit` rows of `session` after `seq` `after`, in `seq`
/// order.
pub(super) fn fetch_page(db: &Db, session: &str, after: i64, limit: usize) -> Result<Vec<Row>> {
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    db.select(
        "SELECT id, type, seq, time_created, data FROM session_message \
         WHERE session_id = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3",
        rusqlite::params![session, after, limit],
        |row| {
            Ok(Row {
                id: row.get(0)?,
                kind: row.get(1)?,
                seq: row.get(2)?,
                created: row.get(3)?,
                data: match row.get_ref(4)? {
                    ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
                        String::from_utf8_lossy(bytes).into_owned()
                    }
                    ValueRef::Null => String::new(),
                    other => match other {
                        ValueRef::Integer(n) => n.to_string(),
                        ValueRef::Real(f) => f.to_string(),
                        _ => String::new(),
                    },
                },
            })
        },
    )
}
