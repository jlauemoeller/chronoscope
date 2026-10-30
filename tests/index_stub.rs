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
    HarnessId, ModelUpdate, Project, ProjectId, SessionId, SessionStats, SessionSummary, ToolCallId,
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
}

impl Stub {
    fn new(id: HarnessId, names: &[&str], delay: Duration) -> Arc<Self> {
        Arc::new(Self {
            id,
            sessions: names.iter().map(|n| (*n).to_owned()).collect(),
            version: AtomicU32::new(1),
            delay,
            calls: AtomicUsize::new(0),
            block_first: None,
        })
    }

    fn prefix(&self) -> String {
        self.id.display_name().to_lowercase()
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
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0
            && let Some((entered, gate)) = &self.block_first
        {
            entered.send(()).unwrap();
            gate.lock().unwrap().recv_timeout(WAIT).unwrap();
        }
        std::thread::sleep(self.delay);
        Ok(SessionStats {
            turns: version as usize,
            duration: Duration::ZERO,
        })
    }
    fn stats_key(&self, session: &SessionSummary) -> String {
        format!(
            "{}:{}:v{}",
            self.prefix(),
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

#[test]
fn a_late_result_for_an_old_key_is_dropped_and_the_new_key_is_computed_after_it() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (gate_tx, gate_rx) = mpsc::channel();
    let stub = Arc::new(Stub {
        id: HarnessId::Claude,
        sessions: vec!["s".to_owned()],
        version: AtomicU32::new(1),
        delay: Duration::ZERO,
        calls: AtomicUsize::new(0),
        block_first: Some((entered_tx, Mutex::new(gate_rx))),
    });
    let cache = tempfile::tempdir().unwrap();
    let timing = Timing {
        poll: Duration::from_millis(50),
        ..Timing::default()
    };
    let harness: Arc<dyn Harness> = stub.clone();
    let mut handle =
        SessionIndex::start_with_timing(vec![harness], cache.path().to_owned(), timing);
    let rx = handle.take_events().unwrap();

    // The computation for version 1 is running (and held).
    entered_rx.recv_timeout(WAIT).unwrap();
    // The session changes: a snapshot with the new title is sent while the
    // old computation is still running.
    stub.version.store(2, Ordering::SeqCst);
    loop {
        match rx.recv_timeout(WAIT).unwrap() {
            IndexEvent::Snapshot { sessions, .. }
                if sessions.values().flatten().any(|s| s.title == "s v2") =>
            {
                break;
            }
            IndexEvent::Stats { .. } => panic!("stats before the held computation ended"),
            IndexEvent::Snapshot { .. } => {}
        }
    }
    // The session is not parsed twice at once.
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(stub.calls.load(Ordering::SeqCst), 1);

    gate_tx.send(()).unwrap();
    // Version 1's result is dropped; the first stats are those of version 2.
    loop {
        if let IndexEvent::Stats { stats, .. } = rx.recv_timeout(WAIT).unwrap() {
            assert_eq!(stats.turns, 2);
            break;
        }
    }
    assert_eq!(stub.calls.load(Ordering::SeqCst), 2);
    handle.shutdown();
}

#[test]
fn cache_entries_of_harnesses_that_are_not_running_are_kept() {
    let cache = tempfile::tempdir().unwrap();
    let file = cache.path().join(chronoscope::index::CACHE_FILE);
    std::fs::write(
        &file,
        r#"{"opencode:old":{"turns":4,"duration":5000},"claude:gone":{"turns":1,"duration":1}}"#,
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
