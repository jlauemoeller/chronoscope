//! Claude session loading, subagent linkage and the resume command, against
//! fixtures and generated files.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use chronoscope::config::Config;
use chronoscope::harness::Harness;
use chronoscope::harness::claude::{Assembler, ClaudeHarness};
use chronoscope::model::{
    HarnessId, Item, ModelUpdate, ProjectId, SessionId, SessionSummary, ToolKind, Turn,
};
use tempfile::TempDir;

const WAIT: Duration = Duration::from_secs(60);

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude")
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

/// A data directory with one project directory.
struct Data {
    _tmp: TempDir,
    dir: PathBuf,
}

impl Data {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("projects");
        fs::create_dir_all(dir.join("-home-user-p")).unwrap();
        Self { _tmp: tmp, dir }
    }

    /// The subagent fixture tree.
    fn subagents() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("projects");
        copy_dir(&fixtures().join("subagents"), &dir);
        Self { _tmp: tmp, dir }
    }

    /// Writes `<project>/<name>.jsonl` and returns its path.
    fn session(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.dir.join("-home-user-p").join(format!("{name}.jsonl"));
        fs::write(&path, bytes).unwrap();
        path
    }

    fn copy_fixture(&self, name: &str, fixture: &str) -> PathBuf {
        self.session(name, fs::read(fixtures().join(fixture)).unwrap())
    }

    fn harness(&self) -> ClaudeHarness {
        ClaudeHarness::with_data_dir(self.dir.clone(), Default::default())
    }
}

/// Everything the worker sends, until `Loaded` or until the channel closes.
fn collect(rx: &Receiver<ModelUpdate>) -> Vec<ModelUpdate> {
    let deadline = Instant::now() + WAIT;
    let mut out = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(update) => {
                let done = update == ModelUpdate::Loaded;
                out.push(update);
                if done {
                    return out;
                }
            }
            Err(RecvTimeoutError::Disconnected) => return out,
            Err(RecvTimeoutError::Timeout) => panic!("the worker did not finish in {WAIT:?}"),
        }
    }
}

fn load_all(h: &dyn Harness, id: &str) -> Vec<ModelUpdate> {
    let (tx, rx) = mpsc::channel();
    let _handle = h.load(&SessionId::from(id), tx).unwrap();
    collect(&rx)
}

/// The turns the updates build up, applied in order.
fn applied(updates: &[ModelUpdate]) -> Vec<Arc<Turn>> {
    let mut turns = Vec::new();
    for update in updates {
        update.apply(&mut turns);
    }
    turns
}

fn appended(updates: &[ModelUpdate]) -> Vec<Arc<Turn>> {
    updates
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Appended(t) => Some(t.iter().cloned()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn progress(updates: &[ModelUpdate]) -> Vec<f32> {
    updates
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Progress(p) => Some(*p),
            _ => None,
        })
        .collect()
}

fn full_parse(path: &Path) -> (Vec<Arc<Turn>>, usize) {
    let asm = Assembler::from_file(path).unwrap();
    let n = asm.unparsed_count();
    (asm.into_turns(), n)
}

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

#[test]
fn load_streams_the_fixture_then_warnings_and_loaded() {
    for fixture in ["session.jsonl", "injected.jsonl", "compaction.jsonl"] {
        let data = Data::new();
        let path = data.copy_fixture("sess", fixture);
        let (expected, unparsed) = full_parse(&path);
        let updates = load_all(&data.harness(), "sess");

        assert_eq!(appended(&updates), expected, "{fixture}");
        assert!(
            !updates.iter().any(|u| matches!(u, ModelUpdate::Updated(_))),
            "{fixture}: a single batch needs no updates"
        );
        let n = updates.len();
        assert_eq!(updates[n - 1], ModelUpdate::Loaded, "{fixture}");
        assert_eq!(updates[n - 2], ModelUpdate::ParseWarnings(unparsed));
        assert_eq!(updates[n - 3], ModelUpdate::Progress(1.0));
        if fixture != "compaction.jsonl" {
            assert!(unparsed > 0, "{fixture} has unparsable entries");
        }
    }
}

#[test]
fn load_returns_before_the_parse_and_parses_on_another_thread() {
    let data = Data::new();
    data.copy_fixture("sess", "session.jsonl");
    let threads = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&threads);
    let h = data
        .harness()
        .with_load_hook(move || seen.lock().unwrap().push(std::thread::current().id()));
    let updates = load_all(&h, "sess");
    assert_eq!(updates.last(), Some(&ModelUpdate::Loaded));
    let threads = threads.lock().unwrap();
    assert!(!threads.is_empty());
    assert!(threads.iter().all(|t| *t != std::thread::current().id()));
}

#[test]
fn load_does_not_wait_for_a_slow_worker() {
    // The worker is held up in its hook; `load` must return all the same.
    let data = Data::new();
    data.copy_fixture("sess", "session.jsonl");
    let (gate_tx, gate_rx) = mpsc::channel::<()>();
    let gate_rx = Mutex::new(gate_rx);
    let h = data.harness().with_load_hook(move || {
        let _ = gate_rx.lock().unwrap().recv_timeout(WAIT);
    });
    let (tx, rx) = mpsc::channel();
    let handle = h.load(&SessionId::from("sess"), tx).unwrap();
    assert!(
        rx.try_recv().is_err(),
        "nothing is sent while the hook waits"
    );
    assert!(!handle.is_finished());
    gate_tx.send(()).unwrap();
    assert_eq!(collect(&rx).last(), Some(&ModelUpdate::Loaded));
}

/// A tool-using exchange of about 11 KiB: a prompt, an assistant message with
/// a `Bash` call, its (lazily stored) result and a closing assistant message
/// in the same turn.
fn exchange(out: &mut String, i: usize) {
    let filler = |c: char, n: usize| c.to_string().repeat(n);
    let ts = format!("2026-05-01T10:{:02}:{:02}.000Z", (i / 60) % 60, i % 60);
    let common = |uuid: &str| {
        format!(
            "\"isSidechain\":false,\"cwd\":\"/home/user/p\",\"sessionId\":\"big\",\"version\":\"2.1.306\",\"uuid\":\"{uuid}\",\"timestamp\":\"{ts}\""
        )
    };
    writeln!(
        out,
        "{{{},\"type\":\"user\",\"origin\":{{\"kind\":\"human\"}},\"message\":{{\"role\":\"user\",\"content\":\"prompt {i} {}\"}}}}",
        common(&format!("u{i}")),
        filler('p', 200)
    )
    .unwrap();
    writeln!(
        out,
        "{{{},\"type\":\"assistant\",\"message\":{{\"id\":\"msg_a{i}\",\"model\":\"claude-sonnet-5-5\",\"role\":\"assistant\",\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}},{{\"type\":\"tool_use\",\"id\":\"toolu_{i}\",\"name\":\"Bash\",\"input\":{{\"command\":\"ls\",\"description\":\"list {i}\"}}}}]}}}}",
        common(&format!("a{i}")),
        filler('a', 2000)
    )
    .unwrap();
    writeln!(
        out,
        "{{{},\"type\":\"user\",\"message\":{{\"role\":\"user\",\"content\":[{{\"type\":\"tool_result\",\"tool_use_id\":\"toolu_{i}\",\"content\":\"{}\"}}]}}}}",
        common(&format!("r{i}")),
        filler('r', 6000)
    )
    .unwrap();
    writeln!(
        out,
        "{{{},\"type\":\"assistant\",\"message\":{{\"id\":\"msg_b{i}\",\"model\":\"claude-sonnet-5-5\",\"role\":\"assistant\",\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}]}}}}",
        common(&format!("b{i}")),
        filler('b', 2000)
    )
    .unwrap();
}

fn big_session(min_bytes: usize) -> String {
    let mut out = String::new();
    let mut i = 0;
    while out.len() < min_bytes {
        exchange(&mut out, i);
        i += 1;
    }
    out
}

#[test]
fn a_20_mib_file_shows_its_first_turns_early_and_progress_is_monotonic() {
    let data = Data::new();
    let path = data.session("big", big_session(20 * 1024 * 1024));
    assert!(fs::metadata(&path).unwrap().len() >= 20 * 1024 * 1024);
    let updates = load_all(&data.harness(), "big");

    let first_appended = updates
        .iter()
        .position(|u| matches!(u, ModelUpdate::Appended(_)))
        .expect("an Appended batch");
    let first_half = updates
        .iter()
        .position(|u| matches!(u, ModelUpdate::Progress(p) if *p >= 0.5))
        .expect("progress reaches one half");
    assert!(first_appended < first_half);
    let ModelUpdate::Appended(first) = &updates[first_appended] else {
        unreachable!()
    };
    assert!(
        first.len() < 100,
        "the first batch is small: {}",
        first.len()
    );

    let p = progress(&updates);
    assert!(p.len() > 5, "{} progress updates", p.len());
    assert!(p.windows(2).all(|w| w[0] <= w[1]), "{p:?}");
    assert!(p.iter().all(|p| (0.0..=1.0).contains(p)));
    assert_eq!(p.last(), Some(&1.0));
    assert!(p[0] < 0.1, "the first progress is early: {}", p[0]);

    let n = updates.len();
    assert_eq!(updates[n - 1], ModelUpdate::Loaded);
    assert_eq!(updates[n - 2], ModelUpdate::ParseWarnings(0));
    assert!(updates[..n - 2].iter().all(|u| *u != ModelUpdate::Loaded));

    // Early updates do not change the result.
    let (expected, _) = full_parse(&path);
    assert_eq!(applied(&updates), expected);
}

#[test]
fn updates_to_already_sent_turns_arrive_as_updated() {
    let data = Data::new();
    let path = data.session("big", big_session(3 * 1024 * 1024));
    // Small batches, so that batches end in the middle of turns.
    let h = data.harness().with_batch_limits(5, 4096, 40_000);
    let updates = load_all(&h, "big");

    let (expected, _) = full_parse(&path);
    assert_eq!(applied(&updates), expected);

    let mut sent: HashSet<String> = HashSet::new();
    let mut updated = 0;
    for update in &updates {
        match update {
            ModelUpdate::Appended(turns) => {
                for t in turns {
                    assert!(sent.insert(t.id().to_string()), "{} sent twice", t.id());
                }
            }
            ModelUpdate::Updated(turns) => {
                updated += turns.len();
                for t in turns {
                    assert!(sent.contains(t.id().as_str()), "{} not sent yet", t.id());
                }
                let ids: HashSet<_> = turns.iter().map(|t| t.id()).collect();
                assert_eq!(ids.len(), turns.len(), "an update lists a turn once");
            }
            _ => {}
        }
    }
    assert!(updated > 0, "some batch ended inside a turn");
}

/// `count` prompts of `size` bytes each, one turn per line.
fn prompts(count: usize, size: usize) -> String {
    let mut out = String::new();
    for i in 0..count {
        writeln!(
            out,
            "{{\"isSidechain\":false,\"cwd\":\"/home/user/p\",\"sessionId\":\"s\",\"uuid\":\"u{i}\",\"timestamp\":\"2026-05-01T10:00:00.000Z\",\"type\":\"user\",\"origin\":{{\"kind\":\"human\"}},\"message\":{{\"role\":\"user\",\"content\":\"{}\"}}}}",
            "x".repeat(size)
        )
        .unwrap();
    }
    out
}

#[test]
fn the_first_batch_goes_out_after_50_turns() {
    let data = Data::new();
    data.session("s", prompts(130, 100));
    let updates = load_all(&data.harness(), "s");
    let sizes: Vec<usize> = updates
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Appended(t) => Some(t.len()),
            _ => None,
        })
        .collect();
    // The rest is far below 2 MiB: it goes out at the end.
    assert_eq!(sizes, [50, 80]);
    // Each batch is followed by its progress.
    assert!(matches!(updates[1], ModelUpdate::Progress(p) if p > 0.0 && p < 1.0));
}

#[test]
fn the_first_batch_goes_out_after_256_kib_of_input() {
    let data = Data::new();
    data.session("s", prompts(20, 30_000));
    let updates = load_all(&data.harness(), "s");
    let sizes: Vec<usize> = updates
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Appended(t) => Some(t.len()),
            _ => None,
        })
        .collect();
    // 9 lines of about 30 KB pass 256 KiB.
    assert_eq!(sizes, [9, 11]);
}

#[test]
fn later_batches_go_out_every_2_mib() {
    let data = Data::new();
    data.session("s", prompts(60, 100_000));
    let updates = load_all(&data.harness(), "s");
    let sizes: Vec<usize> = updates
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Appended(t) => Some(t.len()),
            _ => None,
        })
        .collect();
    // 3 lines pass 256 KiB; then 21 lines (2.1 MB) pass 2 MiB; the rest at the end.
    assert_eq!(sizes.iter().sum::<usize>(), 60);
    assert_eq!(sizes[0], 3, "{sizes:?}");
    assert!(sizes.len() >= 3, "{sizes:?}");
    assert!(sizes[1] >= 20, "{sizes:?}");
}

#[test]
fn an_empty_file_loads_to_complete_progress() {
    let data = Data::new();
    data.session("empty", "");
    let updates = load_all(&data.harness(), "empty");
    assert_eq!(
        updates,
        [
            ModelUpdate::Progress(1.0),
            ModelUpdate::ParseWarnings(0),
            ModelUpdate::Loaded
        ]
    );
}

#[test]
fn a_last_line_without_a_newline_is_parsed() {
    let data = Data::new();
    let text = prompts(3, 10);
    let path = data.session("s", text.trim_end());
    let updates = load_all(&data.harness(), "s");
    let (expected, _) = full_parse(&path);
    assert_eq!(appended(&updates), expected);
    assert_eq!(expected.len(), 3);
}

// ---------------------------------------------------------------------------
// Stopping
// ---------------------------------------------------------------------------

/// A hook that holds the worker at its first batch until the gate opens.
fn gated(h: ClaudeHarness) -> (ClaudeHarness, mpsc::Sender<()>, Arc<Mutex<usize>>) {
    let (gate_tx, gate_rx) = mpsc::channel::<()>();
    let gate_rx = Mutex::new(gate_rx);
    let calls = Arc::new(Mutex::new(0));
    let counter = Arc::clone(&calls);
    let h = h.with_load_hook(move || {
        *counter.lock().unwrap() += 1;
        let _ = gate_rx.lock().unwrap().recv_timeout(WAIT);
    });
    (h, gate_tx, calls)
}

#[test]
fn dropping_the_handle_stops_the_worker() {
    let data = Data::new();
    data.session("big", big_session(2 * 1024 * 1024));
    let (h, gate, _) = gated(data.harness());
    let (tx, rx) = mpsc::channel();
    let handle = h.load(&SessionId::from("big"), tx).unwrap();
    drop(handle);
    gate.send(()).unwrap();

    // The sender is dropped: the receiver sees the disconnection, and never
    // sees a batch or `Loaded`.
    match rx.recv_timeout(Duration::from_secs(5)) {
        Err(RecvTimeoutError::Disconnected) => {}
        other => panic!("expected a disconnection, got {other:?}"),
    }
}

#[test]
fn dropping_the_handle_does_not_block() {
    let data = Data::new();
    data.session("big", big_session(2 * 1024 * 1024));
    let (h, gate, _) = gated(data.harness());
    let (tx, _rx) = mpsc::channel();
    let handle = h.load(&SessionId::from("big"), tx).unwrap();
    // The worker is stuck in its hook; dropping must not wait for it.
    let started = Instant::now();
    drop(handle);
    assert!(started.elapsed() < Duration::from_secs(5));
    drop(gate);
}

#[test]
fn a_worker_whose_receiver_hung_up_stops_by_itself() {
    let data = Data::new();
    data.session("big", big_session(3 * 1024 * 1024));
    let (h, gate, calls) = gated(data.harness().with_batch_limits(5, 4096, 40_000));
    let (tx, rx) = mpsc::channel();
    let handle = h.load(&SessionId::from("big"), tx).unwrap();
    drop(rx);
    gate.send(()).unwrap();
    drop(gate);

    let deadline = Instant::now() + Duration::from_secs(5);
    while !handle.is_finished() {
        assert!(Instant::now() < deadline, "the worker is still running");
        std::thread::sleep(Duration::from_millis(5));
    }
    // It stopped at its first batch instead of parsing on to the end.
    assert_eq!(*calls.lock().unwrap(), 1);
}

// ---------------------------------------------------------------------------
// Failures
// ---------------------------------------------------------------------------

#[test]
fn a_missing_session_ends_in_loaded() {
    let data = Data::new();
    for id in [
        "nope",
        "../etc/passwd",
        "",
        "a/b",
        "/abs",
        "x/subagents/agent-q",
    ] {
        assert_eq!(
            load_all(&data.harness(), id),
            [ModelUpdate::Loaded],
            "{id:?}"
        );
    }
}

#[test]
fn a_missing_data_directory_ends_in_loaded() {
    let h = ClaudeHarness::with_data_dir(PathBuf::from("/definitely/not/here"), Default::default());
    assert_eq!(load_all(&h, "x"), [ModelUpdate::Loaded]);
}

#[test]
fn a_read_error_ends_in_loaded() {
    let data = Data::new();
    // A directory opens, but reading it fails (or it does not even open).
    fs::create_dir(data.dir.join("-home-user-p").join("dir.jsonl")).unwrap();
    assert_eq!(load_all(&data.harness(), "dir"), [ModelUpdate::Loaded]);
}

#[test]
fn damaged_lines_are_counted_not_fatal() {
    let data = Data::new();
    let mut text = prompts(2, 10);
    text.push_str("not json\n{\"type\":\"user\",\"message\":{\"content\":7}}\n");
    text.push_str(&prompts(1, 10));
    data.session("s", text);
    let updates = load_all(&data.harness(), "s");
    let n = updates.len();
    assert_eq!(updates[n - 2], ModelUpdate::ParseWarnings(2));
    assert_eq!(applied(&updates).len(), 3);
}

// ---------------------------------------------------------------------------
// Subagents
// ---------------------------------------------------------------------------

fn call(id: &str) -> chronoscope::model::ToolCallId {
    id.into()
}

fn sid(id: &str) -> SessionId {
    id.into()
}

#[test]
fn subagent_is_found_through_the_meta_file() {
    let data = Data::subagents();
    let h = data.harness();
    assert_eq!(
        h.subagent(&sid("pppp0001"), &call("toolu_meta01")),
        Some(sid("pppp0001/subagents/agent-m1"))
    );
}

#[test]
fn subagent_meta_linkage_does_not_need_the_parent_transcript() {
    let data = Data::subagents();
    // Only the meta file can name the agent now.
    fs::write(data.dir.join("-home-user-loading/pppp0001.jsonl"), "").unwrap();
    assert_eq!(
        data.harness()
            .subagent(&sid("pppp0001"), &call("toolu_meta01")),
        Some(sid("pppp0001/subagents/agent-m1"))
    );
}

#[test]
fn subagent_falls_back_to_the_tool_use_result_agent_id() {
    let data = Data::subagents();
    let h = data.harness();
    // `agent-r2.meta.json` names another tool call.
    assert_eq!(
        h.subagent(&sid("pppp0001"), &call("toolu_res02")),
        Some(sid("pppp0001/subagents/agent-r2"))
    );
}

#[test]
fn subagent_is_none_without_a_link() {
    let data = Data::subagents();
    let h = data.harness();
    for id in [
        "toolu_unknown",
        // Its result names an agent that has no transcript.
        "toolu_gone03",
        // Its result has no `toolUseResult`.
        "toolu_plain04",
        "",
    ] {
        assert_eq!(h.subagent(&sid("pppp0001"), &call(id)), None, "{id:?}");
    }
    assert_eq!(h.subagent(&sid("unknown"), &call("toolu_meta01")), None);
    assert_eq!(h.subagent(&sid("../x"), &call("toolu_meta01")), None);
}

#[test]
fn a_meta_file_without_its_transcript_does_not_link() {
    let data = Data::subagents();
    fs::remove_file(
        data.dir
            .join("-home-user-loading/pppp0001/subagents/agent-m1.jsonl"),
    )
    .unwrap();
    assert_eq!(
        data.harness()
            .subagent(&sid("pppp0001"), &call("toolu_meta01")),
        None
    );
}

#[test]
fn load_streams_the_subagent_with_its_prompt_as_the_first_user_turn() {
    let data = Data::subagents();
    let h = data.harness();
    let id = h.subagent(&sid("pppp0001"), &call("toolu_meta01")).unwrap();
    let updates = load_all(&h, id.as_str());
    let turns = applied(&updates);
    assert_eq!(turns.len(), 2, "{turns:#?}");
    let Turn::User(prompt) = &*turns[0] else {
        panic!("first turn is {:?}", turns[0]);
    };
    assert_eq!(prompt.text, "Look around the repository");
    assert!(matches!(&*turns[1], Turn::Assistant(_)));
    let n = updates.len();
    assert_eq!(updates[n - 2], ModelUpdate::ParseWarnings(0));
    assert_eq!(updates[n - 1], ModelUpdate::Loaded);

    let id = h.subagent(&sid("pppp0001"), &call("toolu_res02")).unwrap();
    let turns = applied(&load_all(&h, id.as_str()));
    let Turn::User(prompt) = &*turns[0] else {
        panic!("first turn is {:?}", turns[0]);
    };
    assert_eq!(prompt.text, "Write a short story");
}

#[test]
fn every_subagent_call_of_a_loaded_parent_resolves() {
    let data = Data::subagents();
    let h = data.harness();
    let turns = applied(&load_all(&h, "pppp0001"));
    let mut found = Vec::new();
    for turn in &turns {
        let Turn::Assistant(turn) = &**turn else {
            continue;
        };
        for item in &turn.items {
            if let Item::ToolCall(c) = item
                && c.tool == ToolKind::Subagent
            {
                found.push((c.id.to_string(), h.subagent(&sid("pppp0001"), &c.id)));
            }
        }
    }
    let ids: Vec<_> = found
        .iter()
        .map(|(c, s)| (c.as_str(), s.as_ref().map(SessionId::as_str)))
        .collect();
    assert_eq!(
        ids,
        [
            ("toolu_meta01", Some("pppp0001/subagents/agent-m1")),
            ("toolu_res02", Some("pppp0001/subagents/agent-r2")),
            ("toolu_gone03", None),
            ("toolu_plain04", None),
        ]
    );
}

#[test]
fn subagent_info_reads_the_meta_file() {
    let data = Data::subagents();
    let h = data.harness();
    let info = h
        .subagent_info(&sid("pppp0001/subagents/agent-m1"))
        .unwrap();
    assert_eq!(info.agent_type, "Explore");
    assert_eq!(info.description, "Explore the repo");
    let info = h
        .subagent_info(&sid("pppp0001/subagents/agent-r2"))
        .unwrap();
    assert_eq!(info.agent_type, "story-writer");
    assert_eq!(info.description, "Write the story");
}

#[test]
fn subagent_info_is_none_without_a_subagent_or_a_meta_file() {
    let data = Data::subagents();
    let h = data.harness();
    assert_eq!(h.subagent_info(&sid("pppp0001")), None);
    assert_eq!(h.subagent_info(&sid("pppp0001/subagents/agent-zz")), None);
    assert_eq!(h.subagent_info(&sid("../pppp0001")), None);
    fs::remove_file(
        data.dir
            .join("-home-user-loading/pppp0001/subagents/agent-m1.meta.json"),
    )
    .unwrap();
    assert_eq!(h.subagent_info(&sid("pppp0001/subagents/agent-m1")), None);
}

#[test]
fn the_trait_object_loads_and_links() {
    let data = Data::subagents();
    let h: Box<dyn Harness> = Box::new(data.harness());
    assert!(
        h.subagent(&sid("pppp0001"), &call("toolu_meta01"))
            .is_some()
    );
    assert_eq!(load_all(&*h, "pppp0001").last(), Some(&ModelUpdate::Loaded));
}

// ---------------------------------------------------------------------------
// Resume command
// ---------------------------------------------------------------------------

fn summary(id: &str, cwd: Option<&str>) -> SessionSummary {
    let t: DateTime<Utc> = DateTime::UNIX_EPOCH;
    SessionSummary {
        id: sid(id),
        harness: HarnessId::Claude,
        project: ProjectId::from("/home/user/p"),
        cwd: cwd.map(PathBuf::from),
        title: "t".to_owned(),
        first_prompt: None,
        started_at: t,
        last_activity: t,
        path: PathBuf::from("/x.jsonl"),
        idle: None,
    }
}

fn parts(c: &std::process::Command) -> (String, Vec<String>, Option<PathBuf>) {
    (
        c.get_program().to_string_lossy().into_owned(),
        c.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect(),
        c.get_current_dir().map(Path::to_path_buf),
    )
}

fn harness_with(template: &str) -> ClaudeHarness {
    ClaudeHarness::with_data_dir(PathBuf::from("/none"), Default::default())
        .with_resume_command(template)
}

#[test]
fn the_default_resume_command_resumes_in_the_session_cwd() {
    let h = ClaudeHarness::with_data_dir(PathBuf::from("/none"), Default::default());
    let c = h
        .resume_command(&summary("abc-123", Some("/home/user/p")))
        .unwrap();
    assert_eq!(
        parts(&c),
        (
            "claude".to_owned(),
            vec!["--resume".to_owned(), "abc-123".to_owned()],
            Some(PathBuf::from("/home/user/p"))
        )
    );
}

#[test]
fn the_config_template_is_used_by_new() {
    let mut config = Config::default();
    assert_eq!(config.harness.claude.resume_command, "claude --resume {id}");
    config.harness.claude.resume_command = "wrapper --id {id}".to_owned();
    let h = ClaudeHarness::new(&config, Path::new("/home/u"));
    let c = h.resume_command(&summary("s1", None)).unwrap();
    assert_eq!(
        parts(&c),
        (
            "wrapper".to_owned(),
            vec!["--id".to_owned(), "s1".to_owned()],
            None
        )
    );
}

#[test]
fn a_custom_template_substitutes_inside_arguments() {
    let h = harness_with("my-claude --resume={id} --flag");
    let c = h.resume_command(&summary("s9", Some("/w"))).unwrap();
    assert_eq!(
        parts(&c),
        (
            "my-claude".to_owned(),
            vec!["--resume=s9".to_owned(), "--flag".to_owned()],
            Some(PathBuf::from("/w"))
        )
    );
}

#[test]
fn quoting_is_honoured_and_every_placeholder_is_replaced() {
    let h = harness_with("'my claude' \"--name={id} {id}\" {id}");
    let c = h.resume_command(&summary("s9", None)).unwrap();
    assert_eq!(
        parts(&c),
        (
            "my claude".to_owned(),
            vec!["--name=s9 s9".to_owned(), "s9".to_owned()],
            None
        )
    );
}

#[test]
fn an_empty_or_unparsable_template_gives_no_command() {
    for template in ["", "   ", "claude --resume 'unclosed {id}"] {
        let h = harness_with(template);
        assert!(
            h.resume_command(&summary("s", Some("/w"))).is_none(),
            "{template:?}"
        );
    }
}
