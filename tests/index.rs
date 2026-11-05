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
use chronoscope::model::{HarnessId, SessionId, SessionStats, SessionSummary, TokenUsage};
use tempfile::TempDir;

const WAIT: Duration = Duration::from_secs(10);

/// The sessions of the fixture tree, and their expected stats (user turns,
/// last minus first entry timestamp in seconds as the run time), read off the
/// fixture files. None of them has a `turn_duration` entry or a `usage`, so
/// their agent time is zero and their tokens are empty.
const EXPECTED: [(&str, usize, u64); 8] = [
    // Its subagent has no usage either.
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

/// Stats with no footer and no usage: the span is the run time.
fn stats_of(turns: usize, run_secs: u64) -> SessionStats {
    SessionStats {
        turns,
        agent_time: Duration::ZERO,
        run_time: Duration::from_secs(run_secs),
        tokens: TokenUsage::default(),
    }
}

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

/// Timing under which a harness that can be watched is only rebuilt on file
/// events: an hour between polls.
fn watching_only() -> Timing {
    Timing {
        poll: Duration::from_secs(3600),
        ..Timing::default()
    }
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
        .map(|&(id, turns, secs)| (SessionId::from(id), stats_of(turns, secs)))
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
    // Polling is out of the picture: only file events can deliver this.
    let (_handle, rx) = start_with(&claude(&env.data), &env.cache, watching_only());
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
    let (_handle, rx) = start_with(&claude(&env.data), &env.cache, watching_only());
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
    let (_handle, rx) = start_with(&claude(&env.data), &env.cache, watching_only());
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
    assert_eq!(stats[&SessionId::from("mt01")], stats_of(3, 12 * 60 + 34));
}

#[test]
fn a_stats_v1_file_is_removed_and_none_of_its_entries_is_used() {
    let env = env();
    let harness = claude(&env.data);
    // Entries under the current keys, in the new shape and with values no
    // transcript gives: reading the file would show.
    let bogus = SessionStats {
        turns: 99,
        agent_time: Duration::from_secs(99),
        run_time: Duration::from_secs(999),
        tokens: TokenUsage {
            input: 1,
            output: 2,
            cache_read: 3,
            cache_write: 4,
            reasoning: 5,
        },
    };
    let mut entries = BTreeMap::new();
    for project in harness.projects().unwrap() {
        for session in harness.sessions(&project.id).unwrap() {
            entries.insert(harness.stats_key(&session), bogus);
        }
    }
    assert_eq!(entries.len(), EXPECTED.len());
    fs::create_dir_all(&env.cache).unwrap();
    let old = env.cache.join("stats-v1.json");
    fs::write(&old, serde_json::to_vec(&entries).unwrap()).unwrap();

    let (handle, rx) = start(&harness, &env.cache);
    let stats = collect_stats(&rx, EXPECTED.len());
    handle.shutdown();
    assert_eq!(stats, expected_stats());
    assert_eq!(harness.stats_parse_count(), EXPECTED.len());
    assert!(!old.exists(), "the old file is removed");
    assert!(env.cache.join(CACHE_FILE).is_file());
}

/// A transcript with two footers, a message that continues after a prompt,
/// a turn without a footer, a subagent and a nested workflow subagent.
#[test]
fn stats_cover_agent_time_run_time_and_the_subagents_tokens() {
    let env = env();
    let dir = env.data.join("-home-user-totals");
    fs::create_dir_all(dir.join("tot01/subagents/workflows/run1")).unwrap();
    let ts = |h: u32, m: u32, s: u32| format!("2026-05-01T{h:02}:{m:02}:{s:02}.000Z");
    let line =
        |body: &str| format!(r#"{{"cwd":"/home/user/tot","sessionId":"tot01",{body}}}"#) + "\n";
    let user = |uuid: &str, t: String| {
        line(&format!(
            r#""type":"user","uuid":"{uuid}","timestamp":"{t}","origin":{{"kind":"human"}},"message":{{"role":"user","content":"go"}}"#
        ))
    };
    let assistant = |id: &str, t: String, usage: &str| {
        line(&format!(
            r#""type":"assistant","uuid":"u-{id}","timestamp":"{t}","message":{{"id":"{id}","model":"x","role":"assistant","content":[{{"type":"text","text":"ok"}}],"usage":{usage}}}"#
        ))
    };
    let footer = |t: String, ms: u64| {
        line(&format!(
            r#""type":"system","subtype":"turn_duration","uuid":"d-{ms}","timestamp":"{t}","durationMs":{ms}"#
        ))
    };
    let mut text = String::new();
    text += &user("u1", ts(10, 0, 0));
    text += &assistant(
        "m1",
        ts(10, 0, 10),
        r#"{"input_tokens":10,"output_tokens":20,"cache_read_input_tokens":30,"cache_creation_input_tokens":40,"output_tokens_details":{"thinking_tokens":6}}"#,
    );
    text += &footer(ts(10, 0, 20), 20_000);
    // Hours later: idle time between the turns.
    text += &user("u2", ts(14, 0, 0));
    text += &assistant(
        "m2",
        ts(14, 0, 5),
        r#"{"input_tokens":1,"output_tokens":2}"#,
    );
    text += &footer(ts(14, 0, 30), 30_000);
    // A turn that has no footer: its tokens count, its time does not.
    text += &user("u3", ts(15, 0, 0));
    text += &assistant(
        "m3",
        ts(15, 0, 5),
        r#"{"input_tokens":100,"output_tokens":200}"#,
    );
    fs::write(dir.join("tot01.jsonl"), text).unwrap();
    let sub = |id: &str, tokens: &str| {
        assistant(id, ts(11, 0, 0), tokens).replace(
            r#""sessionId":"tot01""#,
            r#""sessionId":"tot01","isSidechain":true"#,
        )
    };
    fs::write(
        dir.join("tot01/subagents/agent-a1.jsonl"),
        sub("s1", r#"{"input_tokens":1000,"output_tokens":2000}"#),
    )
    .unwrap();
    fs::write(
        dir.join("tot01/subagents/workflows/run1/agent-w1.jsonl"),
        sub("s2", r#"{"input_tokens":10000,"output_tokens":20000}"#),
    )
    .unwrap();
    fs::write(
        dir.join("tot01/subagents/workflows/run1/journal.jsonl"),
        "{}\n",
    )
    .unwrap();

    let (_handle, rx) = start(&claude(&env.data), &env.cache);
    let stats = collect_stats(&rx, EXPECTED.len() + 1);
    assert_eq!(
        stats[&SessionId::from("tot01")],
        SessionStats {
            turns: 3,
            agent_time: Duration::from_secs(50),
            run_time: Duration::from_secs(5 * 3600 + 5),
            tokens: TokenUsage {
                input: 10 + 1 + 100 + 1000 + 10_000,
                output: 20 + 2 + 200 + 2000 + 20_000,
                cache_read: 30,
                cache_write: 40,
                reasoning: 6,
            },
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
    let leftovers = fs::read_dir(&env.cache).unwrap().count();
    assert_eq!(leftovers, 1, "only the cache file is left");

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
    expected.insert(SessionId::from("aaaa0001"), stats_of(2, 30 * 60));
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
    assert_eq!(stats, stats_of(2, 3600));
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
    // The worker saved the cache before it ended.
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

    // A further change arrives too (by polling or, once the directory is
    // watched, by file events; the new-session tests with polling off cover those).
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

#[test]
fn claude_stats_keys_start_with_the_cache_prefix_and_change_with_the_file() {
    let env = env();
    let harness = claude(&env.data);
    let path = env.data.join("-home-user-alpha").join("aaaa0001.jsonl");
    let summary = harness
        .sessions(&"/home/user/alpha".into())
        .unwrap()
        .into_iter()
        .find(|s| s.path == path)
        .unwrap();
    let key = harness.stats_key(&summary);
    assert!(key.starts_with("claude:"), "{key}");
    assert!(key.starts_with(&format!("{}:", HarnessId::Claude.cache_prefix())));
    append(&path, "\n");
    assert_ne!(harness.stats_key(&summary), key);
}

/// An assistant line of a subagent with `input` input tokens.
fn subagent_line(id: &str, input: u64) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"sub-{id}","isSidechain":true,"cwd":"/home/user/alpha","sessionId":"aaaa0001","timestamp":"2026-03-01T09:10:00.000Z","message":{{"id":"{id}","model":"x","role":"assistant","content":[{{"type":"text","text":"ok"}}],"usage":{{"input_tokens":{input},"output_tokens":1}}}}}}"#
    ) + "\n"
}

#[test]
fn appending_to_only_a_subagent_file_updates_the_stats_with_the_same_sessions() {
    let env = env();
    let harness = claude(&env.data);
    let (_handle, rx) = start_with(&harness, &env.cache, watching_only());
    let first = wait_for(&rx, WAIT, snap);
    let before = collect_stats(&rx, EXPECTED.len());
    let alpha = SessionId::from("aaaa0001");
    assert_eq!(before[&alpha].tokens, TokenUsage::default());
    let sub = env
        .data
        .join("-home-user-alpha/aaaa0001/subagents/agent-a1.jsonl");
    let main = env.data.join("-home-user-alpha/aaaa0001.jsonl");
    let main_before = fs::metadata(&main).unwrap().modified().unwrap();
    let key = harness.stats_key(
        &first
            .sessions
            .iter()
            .find(|s| s.id == alpha)
            .unwrap()
            .clone(),
    );

    append(&sub, &subagent_line("s1", 70));
    // The snapshot of the rebuild comes first, then the stats.
    let mut snapshots = Vec::new();
    let after = wait_for(&rx, WAIT, |event| match event {
        IndexEvent::Stats { session, stats, .. } if *session == alpha => Some(*stats),
        other => {
            snapshots.extend(snap(other).map(|s| s.sessions));
            None
        }
    });
    assert_eq!(after.tokens.input, 70);
    assert_eq!(after.tokens.output, 1);
    // Only the subagent file changed: not the main transcript, so no summary.
    assert_eq!(
        fs::metadata(&main).unwrap().modified().unwrap(),
        main_before
    );
    let session = first.sessions.iter().find(|s| s.id == alpha).unwrap();
    assert_ne!(harness.stats_key(session), key);
    let sorted = |mut sessions: Vec<SessionSummary>| {
        sessions.sort_by(|a, b| a.id.cmp(&b.id));
        sessions
    };
    assert!(!snapshots.is_empty(), "the rebuild sent its snapshot");
    for sessions in snapshots {
        assert_eq!(sorted(sessions), sorted(first.sessions.clone()));
    }
    // The other sessions were not parsed again.
    assert_eq!(harness.stats_parse_count(), EXPECTED.len() + 1);
}
