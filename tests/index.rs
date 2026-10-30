//! The background session index over a copy of the Claude discovery fixtures
//! (`tests/fixtures/claude/discovery`) with a real `ClaudeHarness`.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use chronoscope::harness::Harness;
use chronoscope::harness::claude::ClaudeHarness;
use chronoscope::index::{CACHE_FILE, IndexEvent, IndexHandle, SessionIndex, Timing};
use chronoscope::model::{HarnessId, SessionId, SessionStats, SessionSummary};
use tempfile::TempDir;

const WAIT: Duration = Duration::from_secs(10);

/// The sessions of the fixture tree, and their expected stats (user turns,
/// last minus first entry timestamp in seconds), read off the fixture files.
const EXPECTED: [(&str, usize, u64); 8] = [
    ("aaaa0001", 1, 300),
    ("bbbb0001", 1, 600),
    ("bbbb0002", 1, 1800),
    ("cccc0001", 1, 60),
    ("cccc0002", 1, 60),
    // A tool result, an injected prompt, then a human prompt, then the answer.
    ("dddd0001", 1, 60),
    // Starts with an assistant entry; the only user entry is a tool result.
    ("dddd0002", 0, 30),
    // An empty transcript.
    ("ffff0001", 0, 0),
];

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

struct Env {
    _tmp: TempDir,
    data: PathBuf,
    cache: PathBuf,
}

/// The fixture tree copied into a temp dir; the cache dir is outside it.
fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude/discovery");
    let data = tmp.path().join("projects");
    copy_dir(&src, &data);
    let cache = tmp.path().join("cache");
    Env {
        _tmp: tmp,
        data,
        cache,
    }
}

fn claude(data: &Path) -> Arc<ClaudeHarness> {
    Arc::new(ClaudeHarness::with_data_dir(
        data.to_path_buf(),
        BTreeMap::new(),
    ))
}

fn start(h: &Arc<ClaudeHarness>, cache: &Path) -> (IndexHandle, Receiver<IndexEvent>) {
    start_with(h, cache, Timing::default())
}

fn start_with(
    h: &Arc<ClaudeHarness>,
    cache: &Path,
    timing: Timing,
) -> (IndexHandle, Receiver<IndexEvent>) {
    let harness: Arc<dyn Harness> = h.clone();
    let mut handle = SessionIndex::start_with_timing(vec![harness], cache.to_path_buf(), timing);
    let rx = handle.take_events().unwrap();
    (handle, rx)
}

/// A snapshot, unpacked.
struct Snap {
    available: bool,
    project_ids: Vec<String>,
    sessions: Vec<SessionSummary>,
}

fn snap(event: &IndexEvent) -> Option<Snap> {
    let IndexEvent::Snapshot {
        harness,
        available,
        projects,
        sessions,
    } = event
    else {
        return None;
    };
    assert_eq!(*harness, HarnessId::Claude);
    Some(Snap {
        available: *available,
        project_ids: projects.iter().map(|p| p.id.to_string()).collect(),
        sessions: sessions.values().flatten().cloned().collect(),
    })
}

/// Receives events until `pick` returns a value; panics after `timeout`.
fn wait_for<T>(
    rx: &Receiver<IndexEvent>,
    timeout: Duration,
    mut pick: impl FnMut(&IndexEvent) -> Option<T>,
) -> T {
    let end = Instant::now() + timeout;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(event) => {
                if let Some(found) = pick(&event) {
                    return found;
                }
            }
            Err(err) => panic!("no matching event within {timeout:?}: {err}"),
        }
    }
}

/// Receives events until `count` distinct sessions have stats.
fn collect_stats(rx: &Receiver<IndexEvent>, count: usize) -> HashMap<SessionId, SessionStats> {
    let mut stats = HashMap::new();
    wait_for(rx, WAIT, |event| {
        if let IndexEvent::Stats {
            session, stats: s, ..
        } = event
        {
            stats.insert(session.clone(), *s);
        }
        (stats.len() >= count).then_some(())
    });
    stats
}

fn expected_stats() -> HashMap<SessionId, SessionStats> {
    EXPECTED
        .iter()
        .map(|&(id, turns, secs)| {
            (
                SessionId::from(id),
                SessionStats {
                    turns,
                    duration: Duration::from_secs(secs),
                },
            )
        })
        .collect()
}

fn append(path: &Path, text: &str) {
    let mut file = OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(text.as_bytes()).unwrap();
}

#[test]
fn the_first_snapshot_lists_the_projects_and_sessions() {
    let env = env();
    let (_handle, rx) = start(&claude(&env.data), &env.cache);
    let first = snap(&rx.recv_timeout(WAIT).unwrap()).expect("the first event is a snapshot");
    assert!(first.available);
    let mut projects = first.project_ids.clone();
    projects.sort();
    assert_eq!(
        projects,
        [
            "-home-user-empty",
            "/home/user/alpha",
            "/home/user/multi/one",
            "/home/user/multi/two",
            "/home/user/my.app",
            "/home/user/prompt",
        ]
    );
    let mut ids: Vec<_> = first.sessions.iter().map(|s| s.id.to_string()).collect();
    ids.sort();
    let expected: Vec<_> = EXPECTED.iter().map(|e| e.0).collect();
    assert_eq!(ids, expected);
    let titled = |id: &str| {
        first
            .sessions
            .iter()
            .find(|s| s.id.as_str() == id)
            .unwrap()
            .title
            .clone()
    };
    assert_eq!(titled("aaaa0001"), "Final parser title");
    assert_eq!(titled("ffff0001"), "(untitled)");
}

const NEW_SESSION: &str = concat!(
    r#"{"type":"user","uuid":"n1","timestamp":"2026-04-01T10:00:00.000Z","cwd":"/home/user/alpha","#,
    r#""sessionId":"eeee0001","origin":{"kind":"human"},"message":{"role":"user","content":"Brand new"}}"#,
    "\n"
);

#[test]
fn a_new_session_file_delivers_a_new_snapshot() {
    let env = env();
    let (_handle, rx) = start(&claude(&env.data), &env.cache);
    let first = wait_for(&rx, WAIT, snap);
    assert!(first.sessions.iter().all(|s| s.id.as_str() != "eeee0001"));

    fs::write(
        env.data.join("-home-user-alpha").join("eeee0001.jsonl"),
        NEW_SESSION,
    )
    .unwrap();
    let next = wait_for(&rx, WAIT, |e| {
        snap(e).filter(|s| s.sessions.iter().any(|s| s.id.as_str() == "eeee0001"))
    });
    assert_eq!(next.sessions.len(), first.sessions.len() + 1);
    let new = next.sessions.iter().find(|s| s.id.as_str() == "eeee0001");
    assert_eq!(new.unwrap().title, "Brand new");
}

#[test]
fn a_new_project_directory_delivers_a_new_snapshot() {
    let env = env();
    let (_handle, rx) = start(&claude(&env.data), &env.cache);
    wait_for(&rx, WAIT, snap);
    let dir = env.data.join("-home-user-fresh");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("eeee0002.jsonl"),
        NEW_SESSION.replace("/home/user/alpha", "/home/user/fresh"),
    )
    .unwrap();
    let next = wait_for(&rx, WAIT, |e| {
        snap(e).filter(|s| s.project_ids.contains(&"/home/user/fresh".to_owned()))
    });
    assert!(next.sessions.iter().any(|s| s.id.as_str() == "eeee0002"));
}

#[test]
fn modifying_a_file_updates_its_title_in_the_next_snapshot() {
    let env = env();
    let (_handle, rx) = start(&claude(&env.data), &env.cache);
    wait_for(&rx, WAIT, snap);
    append(
        &env.data.join("-home-user-alpha").join("aaaa0001.jsonl"),
        "{\"type\":\"ai-title\",\"aiTitle\":\"Renamed live\",\"sessionId\":\"aaaa0001\"}\n",
    );
    let next = wait_for(&rx, WAIT, |e| {
        snap(e).filter(|s| {
            s.sessions
                .iter()
                .any(|s| s.id.as_str() == "aaaa0001" && s.title == "Renamed live")
        })
    });
    assert_eq!(next.sessions.len(), EXPECTED.len());
}

#[test]
fn the_index_does_not_rebuild_in_a_loop_on_its_own_reads() {
    let env = env();
    let (_handle, rx) = start(&claude(&env.data), &env.cache);
    wait_for(&rx, WAIT, snap);
    collect_stats(&rx, EXPECTED.len());
    // Scanning and parsing read every file; none of that may count as a
    // change (a rebuild would follow within the 500 ms coalescing window).
    match rx.recv_timeout(Duration::from_millis(1800)) {
        Err(RecvTimeoutError::Timeout) => {}
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn stats_arrive_for_every_session() {
    let env = env();
    let (_handle, rx) = start(&claude(&env.data), &env.cache);
    let stats = collect_stats(&rx, EXPECTED.len());
    assert_eq!(stats, expected_stats());
}

#[test]
fn stats_count_user_turns_and_span_first_to_last_entry() {
    let env = env();
    let dir = env.data.join("-home-user-multi-turn");
    fs::create_dir_all(&dir).unwrap();
    let line = |kind: &str, uuid: &str, time: &str, message: &str| {
        format!(
            r#"{{"type":"{kind}","uuid":"{uuid}","timestamp":"2026-05-01T{time}.000Z","cwd":"/home/user/mt","sessionId":"mt01","origin":{{"kind":"human"}},"message":{message}}}"#
        ) + "\n"
    };
    let user = |text: &str| format!(r#"{{"role":"user","content":"{text}"}}"#);
    let assistant =
        r#"{"id":"m","role":"assistant","model":"x","content":[{"type":"text","text":"ok"}]}"#;
    let mut text = String::new();
    text += &line("user", "u1", "10:00:00", &user("one"));
    text += &line("assistant", "a1", "10:00:30", assistant);
    text += &line("user", "u2", "10:05:00", &user("two"));
    text += &line("user", "u3", "10:06:00", &user("three"));
    text += &line(
        "assistant",
        "a2",
        "10:12:34",
        &assistant.replace(r#""id":"m""#, r#""id":"n""#),
    );
    fs::write(dir.join("mt01.jsonl"), text).unwrap();

    let (_handle, rx) = start(&claude(&env.data), &env.cache);
    let stats = collect_stats(&rx, EXPECTED.len() + 1);
    assert_eq!(
        stats[&SessionId::from("mt01")],
        SessionStats {
            turns: 3,
            duration: Duration::from_secs(12 * 60 + 34),
        }
    );
}

#[test]
fn stats_are_cached_on_disk_and_reused_by_the_next_index() {
    let env = env();
    let first_harness = claude(&env.data);
    let (handle, rx) = start(&first_harness, &env.cache);
    let first = collect_stats(&rx, EXPECTED.len());
    assert_eq!(first, expected_stats());
    assert_eq!(first_harness.stats_parse_count(), EXPECTED.len());
    handle.shutdown();
    assert!(env.cache.join(CACHE_FILE).is_file());
    assert!(!env.cache.join("stats-v1.json.tmp").exists());

    let second_harness = claude(&env.data);
    let (handle, rx) = start(&second_harness, &env.cache);
    // Right after the first snapshot, in that order.
    assert!(snap(&rx.recv_timeout(WAIT).unwrap()).is_some());
    let second = collect_stats(&rx, EXPECTED.len());
    assert_eq!(second, first);
    handle.shutdown();
    assert_eq!(second_harness.stats_parse_count(), 0);
}

#[test]
fn a_changed_file_gets_fresh_stats() {
    let env = env();
    let (handle, rx) = start(&claude(&env.data), &env.cache);
    collect_stats(&rx, EXPECTED.len());
    handle.shutdown();

    append(
        &env.data.join("-home-user-alpha").join("aaaa0001.jsonl"),
        concat!(
            r#"{"type":"user","uuid":"a0000003","timestamp":"2026-03-01T09:30:00.000Z","cwd":"/home/user/alpha","#,
            r#""sessionId":"aaaa0001","origin":{"kind":"human"},"message":{"role":"user","content":"More"}}"#,
            "\n"
        ),
    );
    let harness = claude(&env.data);
    let (handle, rx) = start(&harness, &env.cache);
    let stats = collect_stats(&rx, EXPECTED.len());
    handle.shutdown();
    assert_eq!(harness.stats_parse_count(), 1);
    let mut expected = expected_stats();
    expected.insert(
        SessionId::from("aaaa0001"),
        SessionStats {
            turns: 2,
            duration: Duration::from_secs(30 * 60),
        },
    );
    assert_eq!(stats, expected);
}

#[test]
fn a_growing_file_gets_fresh_stats_while_the_index_runs() {
    let env = env();
    let (_handle, rx) = start(&claude(&env.data), &env.cache);
    collect_stats(&rx, EXPECTED.len());
    append(
        &env.data.join("-home-user-multi").join("cccc0001.jsonl"),
        concat!(
            r#"{"type":"user","uuid":"c3","timestamp":"2026-03-02T09:00:00.000Z","cwd":"/home/user/multi/one","#,
            r#""sessionId":"cccc0001","origin":{"kind":"human"},"message":{"role":"user","content":"More"}}"#,
            "\n"
        ),
    );
    let stats = wait_for(&rx, WAIT, |e| match e {
        IndexEvent::Stats { session, stats, .. } if session.as_str() == "cccc0001" => Some(*stats),
        _ => None,
    });
    assert_eq!(
        stats,
        SessionStats {
            turns: 2,
            duration: Duration::from_secs(3600),
        }
    );
}

#[test]
fn the_cache_is_saved_while_running_and_pruned_of_gone_sessions() {
    let env = env();
    let timing = Timing {
        save_interval: Duration::from_millis(50),
        ..Timing::default()
    };
    let (handle, rx) = start_with(&claude(&env.data), &env.cache, timing);
    collect_stats(&rx, EXPECTED.len());
    let file = env.cache.join(CACHE_FILE);
    let end = Instant::now() + WAIT;
    while !file.is_file() {
        assert!(
            Instant::now() < end,
            "the cache was not saved while running"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    handle.shutdown();
    let text = fs::read_to_string(&file).unwrap();
    assert_eq!(text.matches("claude:").count(), EXPECTED.len());

    // A session that is gone leaves the cache at the next save.
    fs::remove_file(env.data.join("-home-user-alpha").join("aaaa0001.jsonl")).unwrap();
    let (handle, rx) = start(&claude(&env.data), &env.cache);
    wait_for(&rx, WAIT, snap);
    handle.shutdown();
    let text = fs::read_to_string(&file).unwrap();
    assert_eq!(text.matches("claude:").count(), EXPECTED.len() - 1);
    assert!(!text.contains("aaaa0001"));
}

#[test]
fn dropping_the_handle_stops_the_worker_and_disconnects_the_receiver() {
    let env = env();
    let (handle, rx) = start(&claude(&env.data), &env.cache);
    collect_stats(&rx, EXPECTED.len());
    drop(handle);
    let end = Instant::now() + WAIT;
    loop {
        match rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
            Ok(_) => {}
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => panic!("the receiver never disconnected"),
        }
    }
    // The watcher is gone too: changes cause nothing, and the cache saved at
    // the end of the run is there.
    assert!(env.cache.join(CACHE_FILE).is_file());
}

#[test]
fn the_worker_ends_when_the_receiver_is_dropped() {
    let env = env();
    let (handle, rx) = start(&claude(&env.data), &env.cache);
    wait_for(&rx, WAIT, snap);
    drop(rx);
    let end = Instant::now() + WAIT;
    // The next send fails; trigger one with a change.
    append(
        &env.data.join("-home-user-alpha").join("aaaa0001.jsonl"),
        "{\"type\":\"ai-title\",\"aiTitle\":\"x\",\"sessionId\":\"aaaa0001\"}\n",
    );
    while !handle.is_finished() {
        assert!(Instant::now() < end, "the worker kept running");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_missing_data_dir_gives_an_unavailable_snapshot_and_is_polled() {
    let env = env();
    let missing = env.data.join("later");
    let timing = Timing {
        poll: Duration::from_millis(100),
        ..Timing::default()
    };
    let (_handle, rx) = start_with(&claude(&missing), &env.cache, timing);
    let first = wait_for(&rx, WAIT, snap);
    assert!(!first.available);
    assert!(first.project_ids.is_empty());
    assert!(first.sessions.is_empty());

    // Polling sends nothing while nothing changes.
    match rx.recv_timeout(Duration::from_millis(600)) {
        Err(RecvTimeoutError::Timeout) => {}
        other => panic!("unexpected event: {other:?}"),
    }

    let project = missing.join("-home-user-alpha");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("eeee0001.jsonl"), NEW_SESSION).unwrap();
    let next = wait_for(&rx, WAIT, snap);
    assert!(next.available);
    assert_eq!(next.sessions.len(), 1);
    assert_eq!(next.sessions[0].id.as_str(), "eeee0001");

    // Once the directory exists it is watched: a further change arrives.
    append(&project.join("eeee0001.jsonl"), "\n");
    wait_for(&rx, WAIT, snap);
}

#[test]
fn an_index_without_harnesses_ends_quietly() {
    let env = env();
    let mut handle = SessionIndex::start(Vec::new(), env.cache.clone());
    let rx = handle.take_events().unwrap();
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    assert!(handle.events().is_none());
    handle.shutdown();
    assert_eq!(rx.recv_timeout(WAIT), Err(RecvTimeoutError::Disconnected));
    assert!(!env.cache.join(CACHE_FILE).exists());
}
