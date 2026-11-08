//! Test support: a temporary database built from the fixture script.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;

/// `tests/fixtures/opencode/fixture.sql`.
pub(crate) const FIXTURE_SQL: &str = include_str!("../../../tests/fixtures/opencode/fixture.sql");

/// `tests/fixtures/opencode/newer_migration.sql`.
pub(crate) const NEWER_MIGRATION_SQL: &str =
    include_str!("../../../tests/fixtures/opencode/newer_migration.sql");

/// A database in a temporary directory, built from the fixture script.
pub(crate) struct TempDb {
    _dir: TempDir,
    pub path: PathBuf,
}

impl TempDb {
    /// The fixture database (rollback-journal mode).
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(FIXTURE_SQL).unwrap();
        drop(conn);
        Self { _dir: dir, path }
    }

    /// The fixture database plus a migration newer than the adapter knows.
    pub fn with_newer_migration() -> Self {
        let db = Self::new();
        db.exec(NEWER_MIGRATION_SQL);
        db
    }

    /// Runs `sql` on the database, as OpenCode itself would write to it.
    pub fn exec(&self, sql: &str) {
        let conn = Connection::open(&self.path).unwrap();
        conn.execute_batch(sql).unwrap();
    }

    pub fn dir(&self) -> &Path {
        self.path.parent().unwrap()
    }

    /// The names of the files in the database's directory.
    pub fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.dir())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}
