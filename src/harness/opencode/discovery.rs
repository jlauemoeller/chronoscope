//! OpenCode projects, session summaries and the [`Harness`] implementation
//! (SPEC §3.2, §4.1, §9.2).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use anyhow::Context;
use chrono::DateTime;

use super::assemble::millis;
use super::db::{Db, NEWEST_KNOWN_MIGRATION, wal_path};
use super::load;
use super::location::default_db_path;
use super::subagent;
use crate::config::Config;
use crate::harness::claude::{SubagentInfo, UNTITLED, truncate_title};
use crate::harness::{Harness, Result, WatchHandle, resume};
use crate::model::{
    HarnessId, ModelUpdate, Project, ProjectId, SessionId, SessionStats, SessionSummary, ToolCallId,
};

/// The default resume command template: `{id}` stands for the session id.
///
/// `opencode --help` of OpenCode (checked on Linux) lists
/// `--session, -s string  Session ID to continue, or to create if it does not
/// exist`. SPEC §11 still lists the flag as unverified on other platforms.
pub const DEFAULT_RESUME_COMMAND: &str = "opencode --session {id}";

/// The built project and session lists.
#[derive(Default)]
struct Index {
    projects: Vec<Project>,
    sessions: HashMap<ProjectId, Vec<SessionSummary>>,
}

/// The OpenCode adapter.
pub struct OpenCodeHarness {
    db_path: PathBuf,
    aliases: BTreeMap<PathBuf, String>,
    resume_command: String,
    page_size: usize,
    state: Mutex<Option<Arc<Index>>>,
    /// Whether the newer-schema warning has been logged (once per harness,
    /// however many connections it opens).
    warned: Arc<AtomicBool>,
}

impl OpenCodeHarness {
    /// Builds the harness from the config: `harness.opencode.db_path` when
    /// set, else the first existing of the per-platform candidates (see
    /// [`default_db_path`]); `projects.aliases` for display names and
    /// `harness.opencode.resume_command`.
    pub fn new(config: &Config, home: &Path) -> Self {
        let db_path = config.harness.opencode.db_path.clone().unwrap_or_else(|| {
            default_db_path(
                home,
                std::env::var_os("XDG_DATA_HOME"),
                std::env::var_os("LOCALAPPDATA"),
            )
        });
        Self::with_db_path(db_path, config.projects.aliases.clone())
            .with_resume_command(config.harness.opencode.resume_command.clone())
    }

    /// Builds the harness for an explicit database.
    pub fn with_db_path(db_path: PathBuf, aliases: BTreeMap<PathBuf, String>) -> Self {
        Self {
            db_path,
            aliases,
            resume_command: DEFAULT_RESUME_COMMAND.to_owned(),
            page_size: load::PAGE_SIZE,
            state: Mutex::new(None),
            warned: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Sets the resume command template (`harness.opencode.resume_command`).
    #[must_use]
    pub fn with_resume_command(mut self, template: impl Into<String>) -> Self {
        self.resume_command = template.into();
        self
    }

    /// Sets how many rows one query of [`load`](Harness::load) reads
    /// (default 500). Meant for tests.
    #[doc(hidden)]
    #[must_use]
    pub fn with_page_size(mut self, rows: usize) -> Self {
        self.page_size = rows.max(1);
        self
    }

    /// The database file.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// What the parent's tool call says about a subagent session: the
    /// agent and the description it was given, for the breadcrumb. Falls
    /// back to the child's own agent and title. `None` for a session that is
    /// not a child session.
    pub fn subagent_info(&self, session: &SessionId) -> Option<SubagentInfo> {
        let db = self.connect().ok()?;
        subagent::info(&db, session)
            .map_err(|err| log::warn!("could not read the subagent {session}: {err:#}"))
            .ok()
            .flatten()
    }

    /// Opens the database and checks its schema version.
    fn connect(&self) -> Result<Db> {
        connect(&self.db_path, &self.warned)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Arc<Index>>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn index(&self) -> Result<Arc<Index>> {
        let mut state = self.lock();
        match &*state {
            Some(index) => Ok(Arc::clone(index)),
            None => {
                let index = self.build()?;
                *state = Some(Arc::clone(&index));
                Ok(index)
            }
        }
    }

    /// Reads the project and session lists. A database that does not exist
    /// gives empty lists.
    fn build(&self) -> Result<Arc<Index>> {
        if !self.db_path.is_file() {
            return Ok(Arc::new(Index::default()));
        }
        let db = self.connect()?;
        Ok(Arc::new(build_index(&db, &self.db_path, &self.aliases)?))
    }
}

/// Opens `path` and, once per `warned`, logs a warning when the database's
/// newest migration is newer than the one this adapter knows.
pub(super) fn connect(path: &Path, warned: &AtomicBool) -> Result<Db> {
    let db = Db::open(path)?;
    if !warned.load(Ordering::Relaxed) {
        match db.newest_migration() {
            Ok(Some(newest)) if migration_newer(&newest, NEWEST_KNOWN_MIGRATION) => {
                if !warned.swap(true, Ordering::Relaxed) {
                    log::warn!(
                        "{}: the newest migration {newest} is newer than the newest known \
                         {NEWEST_KNOWN_MIGRATION}; parsing on a best-effort basis",
                        path.display()
                    );
                }
            }
            Ok(_) => {}
            Err(err) => log::debug!("could not read the migration table: {err:#}"),
        }
    }
    Ok(db)
}

/// Whether migration `name` is newer than `known`: their timestamp prefixes
/// (the leading digits) compared as numbers, which for equal-length
/// timestamps is the lexicographic order.
pub(super) fn migration_newer(name: &str, known: &str) -> bool {
    let stamp = |s: &str| -> String {
        let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
        digits.trim_start_matches('0').to_owned()
    };
    let (name, known) = (stamp(name), stamp(known));
    (name.len(), name.as_str()) > (known.len(), known.as_str())
}

impl Harness for OpenCodeHarness {
    fn id(&self) -> HarnessId {
        HarnessId::OpenCode
    }

    /// The database file exists and opens.
    fn available(&self) -> bool {
        self.db_path.is_file() && Db::open(&self.db_path).is_ok()
    }

    /// Rereads the project and session lists from the database.
    fn refresh(&self) -> Result<()> {
        let mut state = self.lock();
        *state = Some(self.build()?);
        Ok(())
    }

    /// The database and its write-ahead log.
    fn watch_paths(&self) -> Vec<PathBuf> {
        vec![self.db_path.clone(), wal_path(&self.db_path)]
    }

    /// Builds the index first when there is none. A missing database gives no
    /// projects.
    fn projects(&self) -> Result<Vec<Project>> {
        Ok(self.index()?.projects.clone())
    }

    /// Builds the index first when there is none. An unknown project gives no
    /// sessions.
    fn sessions(&self, project: &ProjectId) -> Result<Vec<SessionSummary>> {
        Ok(self
            .index()?
            .sessions
            .get(project)
            .cloned()
            .unwrap_or_default())
    }

    /// `turns` is the number of `user` rows, `duration` is `time_updated`
    /// minus `time_created` (the summary's last activity minus its start).
    fn stats(&self, session: &SessionSummary) -> Result<SessionStats> {
        let db = self.connect()?;
        let count = db
            .select_one(
                "SELECT count(*) FROM session_message WHERE session_id = ?1 AND type = 'user'",
                [session.id.as_str()],
                |row| row.get::<_, i64>(0),
            )?
            .unwrap_or_default();
        let duration = (session.last_activity - session.started_at)
            .to_std()
            .unwrap_or(Duration::ZERO);
        Ok(SessionStats {
            turns: usize::try_from(count).unwrap_or_default(),
            duration,
        })
    }

    /// `opencode:<db path>:<session id>:<time_updated in ms>`.
    fn stats_key(&self, session: &SessionSummary) -> String {
        format!(
            "{}:{}:{}:{}",
            HarnessId::OpenCode.cache_prefix(),
            self.db_path.display(),
            session.id,
            session.last_activity.timestamp_millis()
        )
    }

    /// Returns at once: a worker thread reads the session's rows in pages
    /// and streams the turns. A session that cannot be found or read ends in
    /// [`ModelUpdate::Loaded`] with the error logged, not in an `Err`.
    fn load(&self, session: &SessionId, tx: Sender<ModelUpdate>) -> Result<WatchHandle> {
        let stop = Arc::new(AtomicBool::new(false));
        let job = load::Job {
            db_path: self.db_path.clone(),
            session: session.clone(),
            tx,
            stop: Arc::clone(&stop),
            page_size: self.page_size,
            warned: Arc::clone(&self.warned),
        };
        let thread = std::thread::Builder::new()
            .name("opencode-load".to_owned())
            .spawn(move || load::run(job))
            .context("starting the session loader")?;
        Ok(WatchHandle::new(stop, thread))
    }

    /// The child session that the tool call `call` of `parent` started: the
    /// `state.metadata.sessionID` of the call when that session is a child of
    /// `parent`, else the child whose title is the call's description.
    fn subagent(&self, parent: &SessionId, call: &ToolCallId) -> Option<SessionId> {
        let db = self.connect().ok()?;
        match subagent::find(&db, parent, call.as_str()) {
            Ok(found) => found,
            Err(err) => {
                log::warn!("could not look up the subagent of {call} in {parent}: {err:#}");
                None
            }
        }
    }

    fn resume_command(&self, session: &SessionSummary) -> Option<Command> {
        resume::build(&self.resume_command, session)
    }
}

struct ProjectRow {
    id: String,
    worktree: String,
    name: Option<String>,
}

struct SessionRow {
    id: String,
    project: String,
    directory: String,
    title: Option<String>,
    created: i64,
    updated: i64,
    idle: Option<i64>,
    first_prompt: Option<String>,
}

/// Reads the projects with at least one top-level session, and those
/// sessions.
fn build_index(db: &Db, db_path: &Path, aliases: &BTreeMap<PathBuf, String>) -> Result<Index> {
    let project_rows = db.select("SELECT id, worktree, name FROM project", [], |row| {
        Ok(ProjectRow {
            id: row.get(0)?,
            worktree: row.get(1)?,
            name: row.get(2)?,
        })
    })?;
    // The text of the first `user` row that has one.
    let first_text = "CASE WHEN json_valid(m.data) THEN json_extract(m.data, '$.text') END";
    let session_rows = db.select(
        &format!(
            "SELECT s.id, s.project_id, s.directory, s.title, s.time_created, s.time_updated, \
                    s.time_idle, \
                    (SELECT {first_text} FROM session_message m \
                      WHERE m.session_id = s.id AND m.type = 'user' \
                        AND trim(COALESCE({first_text}, '')) <> '' \
                      ORDER BY m.seq LIMIT 1) \
             FROM session_v2 s WHERE s.parent_id IS NULL"
        ),
        [],
        |row| {
            Ok(SessionRow {
                id: row.get(0)?,
                project: row.get(1)?,
                directory: row.get(2)?,
                title: row.get(3)?,
                created: row.get(4)?,
                updated: row.get(5)?,
                idle: row.get(6)?,
                first_prompt: row.get(7)?,
            })
        },
    )?;

    let mut groups: HashMap<ProjectId, Vec<SessionSummary>> = HashMap::new();
    let mut orphans: HashMap<String, String> = HashMap::new();
    for row in session_rows {
        if !project_rows.iter().any(|p| p.id == row.project) {
            orphans
                .entry(row.project.clone())
                .or_insert_with(|| row.directory.clone());
        }
        let session = summarize(row, db_path);
        groups
            .entry(session.project.clone())
            .or_default()
            .push(session);
    }

    let known = project_rows.into_iter().map(|p| (p.id, p.worktree, p.name));
    // A session whose project row is missing still gets a project, named
    // after the session's directory.
    let unknown = orphans
        .into_iter()
        .map(|(id, directory)| (id, directory, None));
    let mut projects = Vec::new();
    let mut sessions = HashMap::new();
    for (id, worktree, name) in known.chain(unknown) {
        let id = ProjectId::from(id);
        let Some(mut list) = groups.remove(&id) else {
            continue;
        };
        list.sort_by(|a, b| {
            b.started_at
                .cmp(&a.started_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        let cwd = normalized(&worktree);
        let last_activity = list
            .iter()
            .map(|s| s.last_activity)
            .max()
            .unwrap_or(DateTime::UNIX_EPOCH);
        projects.push(Project {
            name: display_name(&worktree, name.as_deref(), cwd.as_deref(), aliases),
            id: id.clone(),
            harness: HarnessId::OpenCode,
            cwd,
            last_activity,
            session_count: list.len(),
        });
        sessions.insert(id, list);
    }
    projects.sort_by(|a, b| {
        b.last_activity
            .cmp(&a.last_activity)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(Index { projects, sessions })
}

/// `path` with its components normalised, as alias matching compares
/// components; `None` for an empty path.
fn normalized(path: &str) -> Option<PathBuf> {
    (!path.is_empty()).then(|| Path::new(path).components().collect())
}

fn summarize(row: SessionRow, db_path: &Path) -> SessionSummary {
    let first_prompt = row.first_prompt.filter(|p| !p.trim().is_empty());
    SessionSummary {
        id: SessionId::from(row.id),
        harness: HarnessId::OpenCode,
        project: ProjectId::from(row.project),
        cwd: normalized(&row.directory),
        title: session_title(row.title.as_deref(), first_prompt.as_deref()),
        first_prompt,
        started_at: millis(row.created),
        last_activity: millis(row.updated),
        path: db_path.to_path_buf(),
        idle: Some(row.idle.is_some()),
    }
}

/// `session_v2.title`, else the first prompt's first line truncated, else
/// [`UNTITLED`].
fn session_title(title: Option<&str>, first_prompt: Option<&str>) -> String {
    if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
        return title.to_owned();
    }
    first_prompt
        .and_then(|p| p.lines().map(str::trim).find(|l| !l.is_empty()))
        .map_or_else(|| UNTITLED.to_owned(), truncate_title)
}

/// The alias configured for the project's worktree, else the project's
/// `name`, else the worktree's basename, else the worktree itself.
fn display_name(
    worktree: &str,
    name: Option<&str>,
    cwd: Option<&Path>,
    aliases: &BTreeMap<PathBuf, String>,
) -> String {
    if let Some(alias) = cwd.and_then(|cwd| aliases.get(cwd)) {
        return alias.clone();
    }
    if let Some(name) = name.map(str::trim).filter(|n| !n.is_empty()) {
        return name.to_owned();
    }
    cwd.and_then(Path::file_name)
        .map_or_else(|| worktree.to_owned(), |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_compared_by_their_timestamp_prefix() {
        let known = "20261029165540_schedule_checks";
        assert!(!migration_newer(known, known));
        assert!(!migration_newer("20261029073502_session_time_busy", known));
        assert!(migration_newer("20261029165541_anything", known));
        assert!(migration_newer("20270101000000_z", known));
        // The name after the timestamp does not matter.
        assert!(!migration_newer("20261029165540_aaa", known));
        // No timestamp at all is never newer.
        assert!(!migration_newer("garbage", known));
    }

    #[test]
    fn the_title_falls_back_to_the_first_prompt_then_untitled() {
        assert_eq!(session_title(Some(" Fix it "), Some("p")), "Fix it");
        assert_eq!(
            session_title(Some("  "), Some("\n  first line \nsecond")),
            "first line"
        );
        assert_eq!(session_title(None, None), UNTITLED);
        assert_eq!(session_title(None, Some(" \n ")), UNTITLED);
        let long = "x".repeat(200);
        let title = session_title(None, Some(&long));
        assert_eq!(title.chars().count(), 80);
        assert!(title.ends_with('…'));
    }
}
