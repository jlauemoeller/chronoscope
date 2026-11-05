//! Claude session discovery: projects and session summaries (SPEC §3.1, §4.1,
//! §9.2).
//!
//! [`ClaudeHarness`] scans every transcript under the data directory once,
//! keeps each file's [`FileScan`] in memory keyed by (path, size, mtime), and
//! builds the project and session lists from the scans. A rebuild rescans
//! only new or changed files and forgets the ones that are gone.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use anyhow::Context;
use chrono::{DateTime, Utc};

use super::assemble::Assembler;
use super::load::{self, LoadHook, Tuning, WatchMode};
use super::resume::{self, DEFAULT_RESUME_COMMAND};
use super::scan::{FileScan, scan_reader};
use super::subagent::{self, Opened, SubagentFile, SubagentInfo};
use super::usage_memo::UsageMemo;
use crate::config::Config;
use crate::harness::shared::{UNTITLED, truncate_title};
use crate::harness::{Harness, Result, WatchHandle};
use crate::model::{
    HarnessId, ModelUpdate, Project, ProjectId, SectionSource, SessionId, SessionStats,
    SessionSummary, SourceGroup, TokenUsage, ToolCallId, Turn,
};

/// The default data directory: `<home>/.claude/projects` on every platform.
pub fn default_data_dir(home: &Path) -> PathBuf {
    home.join(".claude").join("projects")
}

/// What the last index build did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildStats {
    /// Transcripts found.
    pub files: usize,
    /// Transcripts read.
    pub scanned: usize,
    /// Transcripts whose cached scan was still valid.
    pub reused: usize,
    /// Cached scans dropped because their file is gone.
    pub dropped: usize,
    /// Threads the scanning ran on (0 when nothing needed scanning).
    pub workers: usize,
}

/// A transcript found on disk, with the stat the cache is keyed by.
struct FileMeta {
    path: PathBuf,
    /// The encoded cwd: the name of the project directory.
    dir_name: String,
    /// The file stem, the session id.
    id: String,
    size: u64,
    mtime: SystemTime,
}

struct CacheEntry {
    size: u64,
    mtime: SystemTime,
    scan: Arc<FileScan>,
}

/// The built project and session lists.
#[derive(Default)]
struct Index {
    projects: Vec<Project>,
    sessions: HashMap<ProjectId, Vec<SessionSummary>>,
}

#[derive(Default)]
struct State {
    cache: HashMap<PathBuf, CacheEntry>,
    index: Option<Arc<Index>>,
    last_build: BuildStats,
}

type ScanHook = Box<dyn Fn(&Path) + Send + Sync>;

/// Nanoseconds since the epoch; 0 before it.
fn nanos(time: SystemTime) -> u128 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

/// The Claude Code adapter's discovery half.
pub struct ClaudeHarness {
    data_dir: PathBuf,
    /// The transcripts opened by their path (see [`Harness::open_file`]).
    opened: Opened,
    aliases: BTreeMap<PathBuf, String>,
    max_workers: Option<usize>,
    scans: AtomicUsize,
    stat_parses: AtomicUsize,
    sub_reads: AtomicUsize,
    state: Mutex<State>,
    /// The usage of each subagent transcript, by (path, size, mtime). Not
    /// under the `state` lock: `stats` must run during a `refresh`.
    usage: UsageMemo,
    on_scan: Option<ScanHook>,
    on_sub_read: Option<ScanHook>,
    resume_command: String,
    tuning: Tuning,
    on_batch: Option<LoadHook>,
    watch_mode: WatchMode,
}

impl ClaudeHarness {
    /// Builds the harness from the config: `harness.claude.data_dir` when set,
    /// else `<home>/.claude/projects`; `projects.aliases` for display names.
    pub fn new(config: &Config, home: &Path) -> Self {
        let data_dir = config
            .harness
            .claude
            .data_dir
            .clone()
            .unwrap_or_else(|| default_data_dir(home));
        Self::with_data_dir(data_dir, config.projects.aliases.clone())
            .with_resume_command(config.harness.claude.resume_command.clone())
    }

    /// Builds the harness for an explicit data directory.
    pub fn with_data_dir(data_dir: PathBuf, aliases: BTreeMap<PathBuf, String>) -> Self {
        Self {
            data_dir,
            opened: Opened::default(),
            aliases,
            max_workers: None,
            scans: AtomicUsize::new(0),
            stat_parses: AtomicUsize::new(0),
            sub_reads: AtomicUsize::new(0),
            state: Mutex::new(State::default()),
            usage: UsageMemo::default(),
            on_scan: None,
            on_sub_read: None,
            resume_command: DEFAULT_RESUME_COMMAND.to_owned(),
            tuning: Tuning::default(),
            on_batch: None,
            watch_mode: WatchMode::Notify,
        }
    }

    /// Sets the resume command template (`harness.claude.resume_command`):
    /// `{id}` stands for the session id.
    #[must_use]
    pub fn with_resume_command(mut self, template: impl Into<String>) -> Self {
        self.resume_command = template.into();
        self
    }

    /// Sets when [`load`](Harness::load) sends batches (defaults: the first
    /// after 50 turns or 256 KiB of input, then every 2 MiB). Meant for tests.
    #[doc(hidden)]
    #[must_use]
    pub fn with_batch_limits(mut self, first_turns: usize, first_bytes: u64, bytes: u64) -> Self {
        self.tuning = Tuning {
            first_turns,
            first_bytes,
            batch_bytes: bytes,
        };
        self
    }

    /// Makes [`load`](Harness::load) poll the file's size and modification
    /// time instead of watching it with `notify`, as it does when watching
    /// cannot be started. Meant for tests.
    #[doc(hidden)]
    #[must_use]
    pub fn with_polling(mut self) -> Self {
        self.watch_mode = WatchMode::Poll;
        self
    }

    /// Calls `hook` on the loader's worker thread before each batch is sent.
    /// A test hook.
    #[doc(hidden)]
    #[must_use]
    pub fn with_load_hook(mut self, hook: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_batch = Some(Arc::new(hook));
        self
    }

    /// What the subagent's meta file says (`agentType`, `description`), for
    /// the breadcrumb. `None` for a session that is not a subagent, or whose
    /// meta file is missing or unreadable.
    pub fn subagent_info(&self, session: &SessionId) -> Option<SubagentInfo> {
        subagent::info(&self.data_dir, &self.opened, session)
    }

    /// Caps the scan worker threads (default: `available_parallelism()`).
    #[must_use]
    pub fn with_max_workers(mut self, workers: usize) -> Self {
        self.max_workers = Some(workers.max(1));
        self
    }

    /// Calls `hook` with the path of every file just before it is read, on the
    /// scanning thread. A test hook.
    #[doc(hidden)]
    #[must_use]
    pub fn with_scan_hook(mut self, hook: impl Fn(&Path) + Send + Sync + 'static) -> Self {
        self.on_scan = Some(Box::new(hook));
        self
    }

    /// Calls `hook` with the path of every subagent transcript just before
    /// [`stats`](Harness::stats) reads it. A test hook.
    #[doc(hidden)]
    #[must_use]
    pub fn with_subagent_read_hook(mut self, hook: impl Fn(&Path) + Send + Sync + 'static) -> Self {
        self.on_sub_read = Some(Box::new(hook));
        self
    }

    /// The directory the transcripts are read from.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// How many files have been read since construction. Every read counts,
    /// including one that failed.
    pub fn scan_count(&self) -> usize {
        self.scans.load(Ordering::Relaxed)
    }

    /// How many sessions [`stats`](Harness::stats) has parsed since
    /// construction. Every parse counts, including one that failed.
    pub fn stats_parse_count(&self) -> usize {
        self.stat_parses.load(Ordering::Relaxed)
    }

    /// How many subagent transcripts [`stats`](Harness::stats) has read since
    /// construction. Every read counts, including one that failed; a
    /// transcript whose usage is remembered is not read.
    #[doc(hidden)]
    pub fn subagent_read_count(&self) -> usize {
        self.sub_reads.load(Ordering::Relaxed)
    }

    /// How many subagent transcripts have their usage remembered.
    #[doc(hidden)]
    pub fn subagent_memo_len(&self) -> usize {
        self.usage.len()
    }

    /// What the last build did.
    pub fn last_build(&self) -> BuildStats {
        self.lock().last_build
    }

    /// Rebuilds the index: stats every transcript, rescans the new or changed
    /// ones (in parallel) and drops what is gone.
    ///
    /// [`projects`](Harness::projects) and [`sessions`](Harness::sessions)
    /// only build when there is no index yet, so call this to pick up changes
    /// on disk. The state lock is held for the whole build, so other callers
    /// wait for it; do not call this on the UI thread. The first `projects()` or
    /// `sessions()` call also builds the index under the same lock, so the
    /// same caveat applies to it.
    pub fn refresh(&self) -> Result<()> {
        let mut state = self.lock();
        self.build(&mut state)?;
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The current index, building it first when there is none.
    fn index(&self) -> Result<Arc<Index>> {
        let mut state = self.lock();
        match &state.index {
            Some(index) => Ok(Arc::clone(index)),
            None => self.build(&mut state),
        }
    }

    fn build(&self, state: &mut State) -> Result<Arc<Index>> {
        let files = list_transcripts(&self.data_dir)?;
        let mut stats = BuildStats {
            files: files.len(),
            ..BuildStats::default()
        };

        let before = state.cache.len();
        let present: std::collections::HashSet<&Path> =
            files.iter().map(|f| f.path.as_path()).collect();
        state
            .cache
            .retain(|path, _| present.contains(path.as_path()));
        stats.dropped = before - state.cache.len();
        // The usage of the subagents of a session that is gone goes with it.
        self.usage.retain_sessions(&present);

        let stale: Vec<&FileMeta> = files
            .iter()
            .filter(|f| {
                !state
                    .cache
                    .get(&f.path)
                    .is_some_and(|c| c.size == f.size && c.mtime == f.mtime)
            })
            .collect();
        stats.reused = files.len() - stale.len();
        stats.scanned = stale.len();

        let (results, workers) = self.scan_all(&stale)?;
        stats.workers = workers;
        for (file, result) in stale.iter().zip(results) {
            match result {
                Ok(scan) => {
                    state.cache.insert(
                        file.path.clone(),
                        CacheEntry {
                            size: file.size,
                            mtime: file.mtime,
                            scan: Arc::new(scan),
                        },
                    );
                }
                Err(err) => {
                    log::warn!("could not read {}: {err}", file.path.display());
                    state.cache.remove(&file.path);
                }
            }
        }

        let scanned: Vec<(&FileMeta, &FileScan)> = files
            .iter()
            .filter_map(|f| Some((f, &*state.cache.get(&f.path)?.scan)))
            .collect();
        let index = Arc::new(build_index(&scanned, &self.aliases));
        state.index = Some(Arc::clone(&index));
        state.last_build = stats;
        Ok(index)
    }

    /// Scans `files` on up to `max_workers` threads, which take files from a
    /// shared counter so that one big file does not hold up the rest. Returns
    /// the results in `files` order and the number of threads used.
    fn scan_all(&self, files: &[&FileMeta]) -> Result<(Vec<io::Result<FileScan>>, usize)> {
        if files.is_empty() {
            return Ok((Vec::new(), 0));
        }
        let workers = self
            .max_workers
            .unwrap_or_else(|| {
                std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
            })
            .clamp(1, files.len());
        let next = AtomicUsize::new(0);
        let mut results: Vec<Option<io::Result<FileScan>>> = files.iter().map(|_| None).collect();
        std::thread::scope(|scope| -> Result<()> {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    scope.spawn(|| {
                        let mut done = Vec::new();
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            let Some(file) = files.get(i) else {
                                return done;
                            };
                            done.push((i, self.scan_file(&file.path)));
                        }
                    })
                })
                .collect();
            for handle in handles {
                let done = handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("a scan worker panicked"))?;
                for (i, result) in done {
                    results[i] = Some(result);
                }
            }
            Ok(())
        })?;
        let results = results
            .into_iter()
            .map(|r| r.unwrap_or_else(|| Err(io::Error::other("file was not scanned"))))
            .collect();
        Ok((results, workers))
    }

    /// The token use of the subagent transcript `file`.
    fn read_subagent(&self, file: &SubagentFile) -> io::Result<TokenUsage> {
        self.sub_reads.fetch_add(1, Ordering::Relaxed);
        if let Some(hook) = &self.on_sub_read {
            hook(&file.path);
        }
        Assembler::from_file(&file.path).map(|asm| asm.session_tokens())
    }

    fn scan_file(&self, path: &Path) -> io::Result<FileScan> {
        self.scans.fetch_add(1, Ordering::Relaxed);
        if let Some(hook) = &self.on_scan {
            hook(path);
        }
        scan_reader(BufReader::with_capacity(64 * 1024, File::open(path)?))
    }
}

impl Harness for ClaudeHarness {
    fn id(&self) -> HarnessId {
        HarnessId::Claude
    }

    fn available(&self) -> bool {
        self.data_dir.is_dir()
    }

    /// Rescans new and changed transcripts; see [`ClaudeHarness::refresh`].
    fn refresh(&self) -> Result<()> {
        ClaudeHarness::refresh(self)
    }

    /// The data directory.
    fn watch_paths(&self) -> Vec<PathBuf> {
        vec![self.data_dir.clone()]
    }

    /// Parses the whole transcript with the [`Assembler`] (SPEC cycle 3 §3):
    /// - `turns` is the number of User turns;
    /// - `agent_time` is the sum of the Assistant turns' footer durations,
    ///   summed before it is formatted; a turn without a footer adds nothing;
    /// - `run_time` is the last conversation entry's timestamp minus the
    ///   first's (zero when there are none), local commands included. Only
    ///   entries the assembler handles count: timestamped noise and metadata
    ///   entries are ignored (see [`Assembler::first_timestamp`]);
    /// - `tokens` is the whole use of the main transcript and of every
    ///   subagent transcript below `subagents/`; a subagent file that cannot
    ///   be read is logged and skipped (its tokens stay uncounted until its
    ///   size or mtime changes: the result is cached under a key that only
    ///   those change), and read again next time. The usage
    ///   of each subagent file is remembered by (path, size, mtime), so a
    ///   recomputation reads only the files that changed; the entries of
    ///   files that are gone are dropped.
    ///
    /// Does not take the index lock, so it can run while a
    /// [`refresh`](Harness::refresh) does.
    fn stats(&self, session: &SessionSummary) -> Result<SessionStats> {
        self.stat_parses.fetch_add(1, Ordering::Relaxed);
        let asm = Assembler::from_file(&session.path)
            .with_context(|| format!("reading {}", session.path.display()))?;
        let turns = asm
            .turns()
            .iter()
            .filter(|t| matches!(***t, Turn::User(_)))
            .count();
        let agent_time = asm
            .turns()
            .iter()
            .filter_map(|t| match &**t {
                Turn::Assistant(turn) => turn.stats.as_ref().map(|s| s.duration),
                _ => None,
            })
            // Saturating: crafted `durationMs` values must not panic.
            .fold(Duration::ZERO, Duration::saturating_add);
        let run_time = match (asm.first_timestamp(), asm.last_timestamp()) {
            (Some(first), Some(last)) => (last - first).to_std().unwrap_or_default(),
            _ => Duration::ZERO,
        };
        let mut tokens = asm.session_tokens();
        // Listed (and stat'ed) before any file is read, so a file that grows
        // during the read is read again next time.
        let files = subagent::list_transcripts(&session.path);
        let known = self.usage.lookup(&session.path, &files);
        for (file, known) in files.iter().zip(known) {
            if let Some(usage) = known {
                tokens.accumulate(&usage);
                continue;
            }
            match self.read_subagent(file) {
                Ok(usage) => {
                    self.usage.insert(&session.path, file, usage);
                    tokens.accumulate(&usage);
                }
                Err(err) => {
                    self.usage.forget(&file.path);
                    log::warn!(
                        "could not read the subagent transcript {}: {err}",
                        file.path.display()
                    );
                }
            }
        }
        Ok(SessionStats {
            turns,
            agent_time,
            run_time,
            tokens,
        })
    }

    /// Reads the fields of the section's lines again; see
    /// [`details::section_details`](super::details::section_details).
    fn section_details(&self, source: &SectionSource) -> Vec<SourceGroup> {
        super::details::section_details(source)
    }

    /// `claude:<path>:<size>:<mtime in ns since the epoch>:<subagent
    /// transcripts>:<their total size>:<their newest mtime in ns>`, so that a
    /// write to a subagent file changes the key (`:0:0:0` without subagents;
    /// `journal.jsonl` and the meta files are not transcripts). A main file
    /// that cannot be stat'ed gives size and mtime `missing`, and no subagent
    /// fields. Only stats files: the subagent files are listed, never read.
    fn stats_key(&self, session: &SessionSummary) -> String {
        let path = session.path.display();
        let prefix = HarnessId::Claude.cache_prefix();
        match fs::metadata(&session.path) {
            Ok(meta) => {
                let mtime = meta.modified().map_or(0, nanos);
                let files = subagent::list_transcripts_quietly(&session.path);
                let total = files
                    .iter()
                    .fold(0_u64, |sum, f| sum.saturating_add(f.size));
                let newest = files.iter().map(|f| nanos(f.mtime)).max().unwrap_or(0);
                format!(
                    "{prefix}:{path}:{}:{mtime}:{}:{total}:{newest}",
                    meta.len(),
                    files.len()
                )
            }
            Err(_) => format!("{prefix}:{path}:missing"),
        }
    }

    /// Builds the index first when there is none. A missing data directory
    /// gives no projects.
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

    /// Returns at once: the worker thread finds the transcript, opens it and
    /// parses it, then keeps watching it (SPEC §7.1) until the handle is
    /// dropped. A session that cannot be found or read ends in
    /// [`ModelUpdate::Loaded`] with the error logged, not in an `Err`.
    fn load(&self, session: &SessionId, tx: Sender<ModelUpdate>) -> Result<WatchHandle> {
        let stop = Arc::new(AtomicBool::new(false));
        let job = load::Job {
            data_dir: self.data_dir.clone(),
            opened: self.opened.clone(),
            session: session.clone(),
            tx,
            stop: Arc::clone(&stop),
            tuning: self.tuning,
            hook: self.on_batch.clone(),
            mode: self.watch_mode,
        };
        let thread = std::thread::Builder::new()
            .name("claude-load".to_owned())
            .spawn(move || load::run(job))
            .context("starting the session loader")?;
        Ok(WatchHandle::new(stop, thread))
    }

    /// Looks in the parent's `subagents` directory for the meta file naming
    /// `call`, then in the parent transcript for the call's `toolUseResult`.
    /// Reads files, so it is not for the UI thread.
    fn subagent(&self, parent: &SessionId, call: &ToolCallId) -> Option<SessionId> {
        subagent::find(&self.data_dir, &self.opened, parent, call.as_str())
    }

    fn resume_command(&self, session: &SessionSummary) -> Option<Command> {
        resume::build(&self.resume_command, session)
    }

    /// Reads the transcript at `path`, wherever it is, and remembers it, so
    /// that [`load`](Harness::load) and [`subagent`](Harness::subagent) find
    /// it (and its `<stem>/subagents/` directory) next to the file.
    fn open_file(&self, path: &Path) -> Result<SessionSummary> {
        let path =
            std::path::absolute(path).with_context(|| format!("resolving {}", path.display()))?;
        let meta = fs::metadata(&path).with_context(|| format!("reading {}", path.display()))?;
        anyhow::ensure!(meta.is_file(), "{} is not a file", path.display());
        let id = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .with_context(|| format!("{} has no file name", path.display()))?;
        let dir_name = path
            .parent()
            .and_then(Path::file_name)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let scan = self.scan_file(&path)?;
        let file = FileMeta {
            path: path.clone(),
            dir_name,
            id: id.clone(),
            size: meta.len(),
            mtime: meta.modified()?,
        };
        let summary = summarize(&file, &scan);
        self.opened.insert(id, path);
        Ok(summary)
    }
}

/// Lists `<data_dir>/<encoded-cwd>/*.jsonl`, sorted by path. Files in deeper
/// directories (`<uuid>/subagents/`) are not sessions. A missing data
/// directory is an empty list; an unreadable project directory is skipped.
fn list_transcripts(data_dir: &Path) -> Result<Vec<FileMeta>> {
    let top = match fs::read_dir(data_dir) {
        Ok(top) => top,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(err).with_context(|| format!("reading {}", data_dir.display()));
        }
    };
    let mut files = Vec::new();
    for dir in top.filter_map(|e| e.ok()) {
        let dir_path = dir.path();
        if !dir_path.is_dir() {
            continue;
        }
        let dir_name = dir.file_name().to_string_lossy().into_owned();
        let inner = match fs::read_dir(&dir_path) {
            Ok(inner) => inner,
            Err(err) => {
                log::warn!("could not read {}: {err}", dir_path.display());
                continue;
            }
        };
        for entry in inner.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "jsonl") {
                continue;
            }
            let Some(id) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                continue;
            };
            let meta = match fs::metadata(&path) {
                Ok(meta) if meta.is_file() => meta,
                Ok(_) => continue,
                Err(err) => {
                    log::warn!("could not stat {}: {err}", path.display());
                    continue;
                }
            };
            let Ok(mtime) = meta.modified() else {
                log::warn!("no modification time for {}", path.display());
                continue;
            };
            files.push(FileMeta {
                path,
                dir_name: dir_name.clone(),
                id,
                size: meta.len(),
                mtime,
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn build_index(files: &[(&FileMeta, &FileScan)], aliases: &BTreeMap<PathBuf, String>) -> Index {
    // Project id -> (cwd, sessions).
    let mut groups: HashMap<ProjectId, (Option<PathBuf>, Vec<SessionSummary>)> = HashMap::new();
    for (file, scan) in files {
        let session = summarize(file, scan);
        let group = groups
            .entry(session.project.clone())
            .or_insert_with(|| (session.cwd.clone(), Vec::new()));
        group.1.push(session);
    }

    let mut projects = Vec::with_capacity(groups.len());
    let mut sessions = HashMap::with_capacity(groups.len());
    for (id, (cwd, mut list)) in groups {
        list.sort_by(|a, b| {
            b.started_at
                .cmp(&a.started_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        let last_activity = list
            .iter()
            .map(|s| s.last_activity)
            .max()
            .unwrap_or(DateTime::UNIX_EPOCH);
        projects.push(Project {
            name: display_name(&id, cwd.as_deref(), aliases),
            id: id.clone(),
            harness: HarnessId::Claude,
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
    Index { projects, sessions }
}

/// A file with no `cwd` anywhere belongs to the project named by its encoded
/// directory; it starts at the file's mtime and is untitled.
fn summarize(file: &FileMeta, scan: &FileScan) -> SessionSummary {
    let mtime = DateTime::<Utc>::from(file.mtime);
    let last_activity = scan.last_timestamp.map_or(mtime, |t| t.max(mtime));
    let (project, cwd, title, started_at) = match &scan.cwd {
        Some(raw) => (
            project_id(raw),
            // Component-normalised, as alias matching compares components.
            Some(Path::new(raw).components().collect::<PathBuf>()),
            session_title(scan),
            scan.started_at.unwrap_or(mtime),
        ),
        None => (
            ProjectId::from(file.dir_name.as_str()),
            None,
            UNTITLED.to_owned(),
            mtime,
        ),
    };
    SessionSummary {
        id: SessionId::from(file.id.as_str()),
        harness: HarnessId::Claude,
        project,
        cwd,
        title,
        first_prompt: scan.first_prompt.clone(),
        started_at,
        last_activity,
        path: file.path.clone(),
        idle: None,
    }
}

/// The project id for a cwd as Claude wrote it: the raw string with trailing
/// `/` and `\` removed, so `/a/b/` and `/a/b` are one project. No other
/// separator is touched, so the id is the same on every OS. A root (`/`,
/// `C:\`) is kept as it is.
fn project_id(cwd: &str) -> ProjectId {
    let trimmed = cwd.trim_end_matches(['/', '\\']);
    let is_drive = matches!(trimmed.as_bytes(), [letter, b':'] if letter.is_ascii_alphabetic());
    if trimmed.is_empty() || is_drive {
        ProjectId::from(cwd)
    } else {
        ProjectId::from(trimmed)
    }
}

/// The last `ai-title`, else the first prompt's first line, else
/// [`UNTITLED`].
fn session_title(scan: &FileScan) -> String {
    if let Some(title) = &scan.ai_title {
        return title.clone();
    }
    scan.first_prompt
        .as_deref()
        .and_then(|p| p.lines().map(str::trim).find(|l| !l.is_empty()))
        .map_or_else(|| UNTITLED.to_owned(), truncate_title)
}

/// The alias configured for the project's cwd, else the cwd's basename, else
/// the project id itself (a no-cwd project's encoded directory name, or a
/// cwd without a basename such as `/`).
fn display_name(id: &ProjectId, cwd: Option<&Path>, aliases: &BTreeMap<PathBuf, String>) -> String {
    let Some(cwd) = cwd else {
        return id.to_string();
    };
    if let Some(alias) = aliases.get(cwd) {
        return alias.clone();
    }
    cwd.file_name()
        .map_or_else(|| id.to_string(), |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_ids_trim_trailing_separators_only() {
        for (cwd, id) in [
            ("C:\\Users\\u\\p\\", "C:\\Users\\u\\p"),
            ("/a/b/", "/a/b"),
            ("/a/b", "/a/b"),
            ("/a/b//", "/a/b"),
            ("/", "/"),
            ("C:\\", "C:\\"),
            ("/a\\b/c", "/a\\b/c"),
        ] {
            assert_eq!(project_id(cwd).as_str(), id, "{cwd}");
        }
    }
}
