//! The index worker over stub harnesses: several harnesses at once, and
//! stats results that arrive late.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use chronoscope::harness::{Harness, Result, WatchHandle};
use chronoscope::index::{IndexEvent, SessionIndex, Timing};
use chronoscope::model::{
    HarnessId, ModelUpdate, Project, ProjectId, SessionId, SessionStats, SessionSummary,
    TokenUsage, ToolCallId,
};

const WAIT: Duration = Duration::from_secs(10);

/// A harness with a fixed set of sessions that cannot be watched, so the
/// worker polls it (slowly, unless told otherwise).
struct Stub {
    id: HarnessId,
    sessions: Vec<String>,
    /// Appended to every title; changing it changes the snapshot.
    version: AtomicU32,
    /// How long every `stats` call takes.
    delay: Duration,
    calls: AtomicUsize,
    /// When set, the first `stats` call reports on `entered`, then waits for
    /// `gate`.
    block_first: Option<(Sender<()>, Mutex<Receiver<()>>)>,
    /// The first `stats` call panics.
    panic_first: bool,
    /// `projects` fails.
    fail_projects: bool,
}

impl Stub {
    fn new(id: HarnessId, names: &[&str], delay: Duration) -> Arc<Self> {
        Arc::new(Self::plain(id, names, delay))
    }

    fn plain(id: HarnessId, names: &[&str], delay: Duration) -> Self {
        Self {
            id,
            sessions: names.iter().map(|n| (*n).to_owned()).collect(),
            version: AtomicU32::new(1),
            delay,
            calls: AtomicUsize::new(0),
            block_first: None,
            panic_first: false,
            fail_projects: false,
        }
    }

    /// Holds the first `stats` call until the returned sender is used; the
    /// receiver tells when that call has started.
    fn blocking_first(mut self) -> (Self, Receiver<()>, Sender<()>) {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (gate_tx, gate_rx) = mpsc::channel();
        self.block_first = Some((entered_tx, Mutex::new(gate_rx)));
        (self, entered_rx, gate_tx)
    }

    fn summary(&self, name: &str, rank: i64) -> SessionSummary {
        let time: DateTime<Utc> = DateTime::UNIX_EPOCH + TimeDelta::seconds(rank * 1000);
        SessionSummary {
            id: SessionId::from(name),
            harness: self.id,
            project: ProjectId::from("/p"),
            cwd: None,
            title: format!("{name} v{}", self.version.load(Ordering::SeqCst)),
            first_prompt: None,
            started_at: time,
            last_activity: time,
            path: PathBuf::from(name),
            idle: None,
        }
    }
}

impl Harness for Stub {
    fn id(&self) -> HarnessId {
        self.id
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
        if self.fail_projects {
            anyhow::bail!("the store cannot be read");
        }
        Ok(vec![Project {
            id: ProjectId::from("/p"),
            harness: self.id,
            name: "p".to_owned(),
            cwd: None,
            last_activity: DateTime::UNIX_EPOCH,
            session_count: self.sessions.len(),
        }])
    }
    fn sessions(&self, _: &ProjectId) -> Result<Vec<SessionSummary>> {
        Ok(self
            .sessions
            .iter()
            .enumerate()
            .map(|(i, name)| self.summary(name, i as i64))
            .collect())
    }
    fn load(&self, _: &SessionId, _: Sender<ModelUpdate>) -> Result<WatchHandle> {
        anyhow::bail!("not loadable")
    }
    fn subagent(&self, _: &SessionId, _: &ToolCallId) -> Option<SessionId> {
        None
    }
    fn resume_command(&self, _: &SessionSummary) -> Option<Command> {
        None
    }
    fn stats(&self, _: &SessionSummary) -> Result<SessionStats> {
        // The version this computation is for.
        let version = self.version.load(Ordering::SeqCst);
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            if self.panic_first {
                panic!("stats() panics on purpose");
            }
            if let Some((entered, gate)) = &self.block_first {
                entered.send(()).unwrap();
                gate.lock().unwrap().recv_timeout(WAIT).unwrap();
            }
        }
        std::thread::sleep(self.delay);
        Ok(SessionStats {
            turns: version as usize,
            agent_time: Duration::ZERO,
            run_time: Duration::ZERO,
            tokens: TokenUsage::default(),
        })
    }
    fn stats_key(&self, session: &SessionSummary) -> String {
        format!(
            "{}:{}:v{}",
            self.id.cache_prefix(),
            session.id,
            self.version.load(Ordering::SeqCst)
        )
    }
}

fn slow_poll() -> Timing {
    Timing {
        poll: Duration::from_secs(3600),
        ..Timing::default()
    }
}

#[test]
fn every_session_of_every_harness_gets_stats() {
    let names_a: Vec<String> = (0..12).map(|i| format!("a{i}")).collect();
    let refs_a: Vec<&str> = names_a.iter().map(String::as_str).collect();
    // More sessions than pool threads, each slow: harness A's queue cannot
    // drain before harness B is built.
    let a = Stub::new(HarnessId::Claude, &refs_a, Duration::from_millis(40));
    let b = Stub::new(HarnessId::OpenCode, &["b0", "b1", "b2"], Duration::ZERO);
    let cache = tempfile::tempdir().unwrap();
    let harnesses: Vec<Arc<dyn Harness>> = vec![a, b];
    let mut handle =
        SessionIndex::start_with_timing(harnesses, cache.path().to_owned(), slow_poll());
    let rx = handle.take_events().unwrap();

    let mut got: HashMap<HarnessId, HashSet<String>> = HashMap::new();
    let mut snapshots = HashSet::new();
    while got.values().map(HashSet::len).sum::<usize>() < 15 {
        match rx.recv_timeout(WAIT).expect("stats for every session") {
            IndexEvent::Stats {
                harness, session, ..
            } => {
                got.entry(harness).or_default().insert(session.to_string());
            }
            IndexEvent::Snapshot { harness, .. } => {
                snapshots.insert(harness);
            }
        }
    }
    assert_eq!(
        snapshots,
        HashSet::from([HarnessId::Claude, HarnessId::OpenCode])
    );
    assert_eq!(got[&HarnessId::Claude].len(), 12);
    assert_eq!(got[&HarnessId::OpenCode].len(), 3);
    handle.shutdown();
}

fn poll_50ms() -> Timing {
    Timing {
        poll: Duration::from_millis(50),
        ..Timing::default()
    }
}

/// Receives events until a snapshot lists a session titled `title`.
fn wait_for_title(rx: &Receiver<IndexEvent>, title: &str) {
    loop {
        match rx.recv_timeout(WAIT).unwrap() {
            IndexEvent::Snapshot { sessions, .. }
                if sessions.values().flatten().any(|s| s.title == title) =>
            {
                return;
            }
            IndexEvent::Stats { .. } => panic!("stats before the held computation ended"),
            IndexEvent::Snapshot { .. } => {}
        }
    }
}

fn next_stats(rx: &Receiver<IndexEvent>) -> SessionStats {
    loop {
        if let IndexEvent::Stats { stats, .. } = rx.recv_timeout(WAIT).unwrap() {
            return stats;
        }
    }
}

#[test]
fn a_stale_result_is_sent_so_a_busy_session_is_not_starved() {
    let (stub, entered, gate) =
        Stub::plain(HarnessId::Claude, &["s"], Duration::ZERO).blocking_first();
    let stub = Arc::new(stub);
    let cache = tempfile::tempdir().unwrap();
    let harness: Arc<dyn Harness> = stub.clone();
    let mut handle =
        SessionIndex::start_with_timing(vec![harness], cache.path().to_owned(), poll_50ms());
    let rx = handle.take_events().unwrap();

    // The computation for version 1 is running (and held).
    entered.recv_timeout(WAIT).unwrap();
    // The session changes twice meanwhile; no stats are due yet.
    stub.version.store(2, Ordering::SeqCst);
    wait_for_title(&rx, "s v2");
    stub.version.store(3, Ordering::SeqCst);
    wait_for_title(&rx, "s v3");
    // The session is not parsed twice at once.
    assert_eq!(stub.calls.load(Ordering::SeqCst), 1);

    gate.send(()).unwrap();
    // Version 1's result is outdated but the first the session gets, and it
    // comes without waiting for the current version.
    assert_eq!(next_stats(&rx).turns, 1);
    assert_eq!(next_stats(&rx).turns, 3);
    // Version 2 was replaced by version 3 before it ever ran.
    assert_eq!(stub.calls.load(Ordering::SeqCst), 2);
    handle.shutdown();
}

#[test]
fn a_stale_result_after_newer_stats_were_sent_is_dropped() {
    // The first harness' computation is held while its session changes to a
    // version whose stats are already cached. The other harness never builds
    // (so pruning does not remove the seeded entry).
    let (stub, entered, gate) =
        Stub::plain(HarnessId::Claude, &["s"], Duration::ZERO).blocking_first();
    let stub = Arc::new(stub);
    let mut broken = Stub::plain(HarnessId::OpenCode, &["x"], Duration::ZERO);
    broken.fail_projects = true;
    let cache = tempfile::tempdir().unwrap();
    std::fs::write(
        cache.path().join(chronoscope::index::CACHE_FILE),
        r#"{"claude:s:v2":{"turns":42,"agent_time":0,"run_time":0,"tokens":{"input":0,"output":0,"cache_read":0,"cache_write":0,"reasoning":0}}}"#,
    )
    .unwrap();
    let first: Arc<dyn Harness> = stub.clone();
    let harnesses = vec![first, Arc::new(broken) as Arc<dyn Harness>];
    let mut handle =
        SessionIndex::start_with_timing(harnesses, cache.path().to_owned(), poll_50ms());
    let rx = handle.take_events().unwrap();

    entered.recv_timeout(WAIT).unwrap();
    stub.version.store(2, Ordering::SeqCst);
    assert_eq!(next_stats(&rx).turns, 42);

    gate.send(()).unwrap();
    // The held version-1 result is older than the cached stats just sent.
    let end = std::time::Instant::now() + Duration::from_millis(500);
    while let Some(left) = end.checked_duration_since(std::time::Instant::now()) {
        if let Ok(IndexEvent::Stats {
            harness,
            session,
            stats,
        }) = rx.recv_timeout(left)
        {
            panic!("unexpected stats for {harness} {session}: {stats:?}");
        }
    }
    // The held result was recorded (version 3's job waited for it) and
    // dropped: version 2's stats would have come first.
    stub.version.store(3, Ordering::SeqCst);
    assert_eq!(next_stats(&rx).turns, 3);
    handle.shutdown();
}

#[test]
fn a_stale_result_after_a_revert_to_the_sent_key_is_dropped() {
    let (stub, entered, gate) =
        Stub::plain(HarnessId::Claude, &["s"], Duration::ZERO).blocking_first();
    let stub = Arc::new(stub);
    let mut broken = Stub::plain(HarnessId::OpenCode, &["x"], Duration::ZERO);
    broken.fail_projects = true;
    let cache = tempfile::tempdir().unwrap();
    std::fs::write(
        cache.path().join(chronoscope::index::CACHE_FILE),
        r#"{"claude:s:v1":{"turns":10,"agent_time":0,"run_time":0,"tokens":{"input":0,"output":0,"cache_read":0,"cache_write":0,"reasoning":0}}}"#,
    )
    .unwrap();
    let first: Arc<dyn Harness> = stub.clone();
    let harnesses = vec![first, Arc::new(broken) as Arc<dyn Harness>];
    let mut handle =
        SessionIndex::start_with_timing(harnesses, cache.path().to_owned(), poll_50ms());
    let rx = handle.take_events().unwrap();

    // Version 1 is cached and sent.
    assert_eq!(next_stats(&rx).turns, 10);
    // Version 2 is computed (held); then the session reverts to version 1,
    // whose stats are the ones already shown.
    stub.version.store(2, Ordering::SeqCst);
    entered.recv_timeout(WAIT).unwrap();
    stub.version.store(1, Ordering::SeqCst);
    wait_for_title(&rx, "s v1");

    gate.send(()).unwrap();
    // Version 2's result is outdated and older than what is shown. Version
    // 3's job waits for it, so its stats arrive after it was recorded: and
    // nothing else may come first.
    stub.version.store(3, Ordering::SeqCst);
    assert_eq!(next_stats(&rx).turns, 3);
    handle.shutdown();
}

#[test]
fn a_panicking_stats_call_does_not_block_the_session_or_the_pool() {
    let mut stub = Stub::plain(HarnessId::Claude, &["s"], Duration::ZERO);
    stub.panic_first = true;
    let stub = Arc::new(stub);
    let cache = tempfile::tempdir().unwrap();
    let harness: Arc<dyn Harness> = stub.clone();
    let mut handle =
        SessionIndex::start_with_timing(vec![harness], cache.path().to_owned(), poll_50ms());
    let rx = handle.take_events().unwrap();

    // Version 1 panicked: no stats. Once the session changes, it gets some.
    while stub.calls.load(Ordering::SeqCst) == 0 {
        std::thread::sleep(Duration::from_millis(10));
    }
    stub.version.store(2, Ordering::SeqCst);
    assert_eq!(next_stats(&rx).turns, 2);
    assert_eq!(stub.calls.load(Ordering::SeqCst), 2);
    handle.shutdown();
}

#[test]
fn cache_entries_of_harnesses_that_are_not_running_are_kept() {
    let cache = tempfile::tempdir().unwrap();
    let file = cache.path().join(chronoscope::index::CACHE_FILE);
    std::fs::write(
        &file,
        r#"{"opencode:old":{"turns":4,"agent_time":5000,"run_time":0,"tokens":{"input":0,"output":0,"cache_read":0,"cache_write":0,"reasoning":0}},"claude:gone":{"turns":1,"agent_time":0,"run_time":0,"tokens":{"input":0,"output":0,"cache_read":0,"cache_write":0,"reasoning":0}}}"#,
    )
    .unwrap();
    let stub = Stub::new(HarnessId::Claude, &["s"], Duration::ZERO);
    let mut handle =
        SessionIndex::start_with_timing(vec![stub], cache.path().to_owned(), slow_poll());
    let rx = handle.take_events().unwrap();
    while !matches!(rx.recv_timeout(WAIT).unwrap(), IndexEvent::Stats { .. }) {}
    handle.shutdown();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("opencode:old"), "{text}");
    assert!(text.contains("claude:s:v1"), "{text}");
    assert!(!text.contains("claude:gone"), "{text}");

    // With no harness at all, nothing is pruned.
    let handle = SessionIndex::start(Vec::new(), cache.path().to_owned());
    handle.shutdown();
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("opencode:old")
    );
}
