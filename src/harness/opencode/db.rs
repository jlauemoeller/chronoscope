//! Read-only access to OpenCode's SQLite database (SPEC §3.2, §7.1, §9.5).
//!
//! [`Db`] is the only way the adapter touches the database. It opens the file
//! with `SQLITE_OPEN_READ_ONLY` and runs every statement through
//! [`Db::select`], which prepares it and refuses anything that is not a plain
//! read (`SELECT`), so a bug cannot make Chronoscope write to a harness
//! store. No `PRAGMA` is ever issued.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use rusqlite::{Connection, OpenFlags, Params, Row};

use crate::harness::Result;

/// The most recent `migration` id this adapter was written against
/// (`<yyyymmddhhmmss>_<name>`). A database whose newest migration is later
/// than this was written by a newer OpenCode.
pub const NEWEST_KNOWN_MIGRATION: &str = "20261029165540_schedule_checks";

/// How the database was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    /// Read-only, with the `-wal`/`-shm` files OpenCode maintains.
    ReadOnly,
    /// With `immutable=1`: read-only open failed (the `-wal`/`-shm` files
    /// are missing and the directory is not writable), so the file is read
    /// as it is, without locking or the write-ahead log.
    Immutable,
}

/// A read-only connection to the database.
#[derive(Debug)]
pub struct Db {
    conn: Connection,
    mode: OpenMode,
}

impl Db {
    /// Opens `path` read-only and checks that it can be read. When that
    /// fails, retries with the `immutable=1` URI parameter (and logs it).
    pub fn open(path: &Path) -> Result<Self> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let first =
            Connection::open_with_flags(path, flags).and_then(|conn| probe(&conn).map(|()| conn));
        match first {
            Ok(conn) => Ok(Self {
                conn,
                mode: OpenMode::ReadOnly,
            }),
            Err(err) => {
                let uri = immutable_uri(path);
                let flags = flags | OpenFlags::SQLITE_OPEN_URI;
                let conn = Connection::open_with_flags(&uri, flags)
                    .and_then(|conn| probe(&conn).map(|()| conn))
                    .with_context(|| {
                        format!(
                            "opening {} read-only (first attempt: {err})",
                            path.display()
                        )
                    })?;
                log::info!(
                    "could not open {} read-only ({err}); reading it as immutable",
                    path.display()
                );
                Ok(Self {
                    conn,
                    mode: OpenMode::Immutable,
                })
            }
        }
    }

    /// How the database was opened.
    pub fn mode(&self) -> OpenMode {
        self.mode
    }

    /// Runs a `SELECT` and maps its rows.
    ///
    /// Anything else is refused before it is executed: the statement must
    /// start with `SELECT` and SQLite itself must report it as read-only.
    pub fn select<T, P: Params>(
        &self,
        sql: &str,
        params: P,
        mut map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>> {
        if !starts_with_select(sql) {
            bail!("refusing to run a statement that is not a SELECT: {sql}");
        }
        let mut stmt = self
            .conn
            .prepare(sql)
            .with_context(|| format!("preparing {sql}"))?;
        if !stmt.readonly() {
            bail!("refusing to run a statement that writes: {sql}");
        }
        let rows = stmt
            .query_map(params, |row| map(row))
            .with_context(|| format!("running {sql}"))?;
        rows.collect::<rusqlite::Result<Vec<T>>>()
            .with_context(|| format!("reading the rows of {sql}"))
    }

    /// Like [`select`](Self::select), for a statement that gives at most one
    /// row.
    pub fn select_one<T, P: Params>(
        &self,
        sql: &str,
        params: P,
        map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Option<T>> {
        Ok(self.select(sql, params, map)?.into_iter().next())
    }

    /// The id of the newest `migration` row (ids start with a timestamp, so
    /// the greatest id is the newest), or `None` without any.
    pub fn newest_migration(&self) -> Result<Option<String>> {
        self.select_one(
            "SELECT id FROM migration ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
    }
}

/// A cheap read that proves the file is a database that can be read now (an
/// open succeeds lazily, and a read-only open of a write-ahead-log database
/// fails on the first read when its `-shm` cannot be created).
fn probe(conn: &Connection) -> rusqlite::Result<()> {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
}

fn starts_with_select(sql: &str) -> bool {
    let sql = sql.trim_start();
    sql.get(..6)
        .is_some_and(|head| head.eq_ignore_ascii_case("select"))
}

/// `file:<path>?immutable=1`, with the characters that have a meaning in a
/// URI percent-encoded (and a `/` before a Windows drive letter).
pub(super) fn immutable_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from("file:");
    if !text.starts_with('/') {
        uri.push('/');
    }
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                uri.push(char::from(byte));
            }
            other => uri.push_str(&format!("%{other:02X}")),
        }
    }
    uri.push_str("?immutable=1");
    uri
}

/// The `-wal` file next to `db`.
pub fn wal_path(db: &Path) -> PathBuf {
    let mut name = db.as_os_str().to_owned();
    name.push("-wal");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uri_encodes_what_a_uri_would_misread() {
        assert_eq!(
            immutable_uri(Path::new("/home/u/a b/c#d?e%f.db")),
            "file:/home/u/a%20b/c%23d%3Fe%25f.db?immutable=1"
        );
        assert_eq!(
            immutable_uri(Path::new("C:\\Users\\u\\x.db")),
            "file:/C:/Users/u/x.db?immutable=1"
        );
    }

    #[test]
    fn the_wal_path_appends_to_the_file_name() {
        assert_eq!(
            wal_path(Path::new("/d/opencode.db")),
            Path::new("/d/opencode.db-wal")
        );
    }

    #[test]
    fn only_selects_pass_the_prefix_check() {
        assert!(starts_with_select("  SELECT 1"));
        assert!(starts_with_select("select 1"));
        assert!(!starts_with_select("INSERT INTO t VALUES (1)"));
        assert!(!starts_with_select("PRAGMA journal_mode = WAL"));
        assert!(!starts_with_select("sel"));
        assert!(!starts_with_select(""));
    }
}
