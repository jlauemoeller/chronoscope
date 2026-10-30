//! Live watching of a Claude session (SPEC §7.1): appended lines, partial
//! lines, truncation and replacement, coalescing, activity, polling and
//! stopping. The files are generated in a temp directory and changed while a
//! worker watches them.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime};

use chronoscope::harness::claude::{Assembler, ClaudeHarness};
use chronoscope::harness::{Harness, WatchHandle};
use chronoscope::model::{Item, ModelUpdate, SessionId, ToolStatus, Turn};
use tempfile::TempDir;

/// Slow file-event delivery (macOS CI) needs room.
const WAIT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Lines
// ---------------------------------------------------------------------------

fn ts(n: usize) -> String {
    format!("2026-05-01T10:{:02}:{:02}.000Z", (n / 60) % 60, n % 60)
}

fn common(uuid: &str, n: usize) -> String {
    format!(
        "\"isSidechain\":false,\"cwd\":\"/home/user/p\",\"sessionId\":\"live\",\"version\":\"2.1.306\",\"uuid\":\"{uuid}\",\"timestamp\":\"{}\"",
        ts(n)
    )
}

/// A human prompt, which opens a User turn.
fn prompt(n: usize, text: &str) -> String {
    format!(
        "{{{},\"type\":\"user\",\"origin\":{{\"kind\":\"human\"}},\"message\":{{\"role\":\"user\",\"content\":\"{text}\"}}}}\n",
        common(&format!("u{n}"), n)
    )
}

/// One entry of the assistant message `msg`; `blocks` is the JSON content.
fn assistant(n: usize, msg: &str, blocks: &str) -> String {
    format!(
        "{{{},\"type\":\"assistant\",\"message\":{{\"id\":\"{msg}\",\"model\":\"claude-sonnet-5-5\",\"role\":\"assistant\",\"content\":[{blocks}]}}}}\n",
        common(&format!("a{n}"), n)
    )
}

fn text_block(text: &str) -> String {
    format!("{{\"type\":\"text\",\"text\":\"{text}\"}}")
}

fn tool_use(id: &str) -> String {
    format!(
        "{{\"type\":\"tool_use\",\"id\":\"{id}\",\"name\":\"Bash\",\"input\":{{\"command\":\"ls\",\"description\":\"list\"}}}}"
    )
}

fn tool_result(n: usize, id: &str, output: &str) -> String {
    format!(
        "{{{},\"type\":\"user\",\"message\":{{\"role\":\"user\",\"content\":[{{\"type\":\"tool_result\",\"tool_use_id\":\"{id}\",\"content\":\"{output}\"}}]}}}}\n",
        common(&format!("r{n}"), n)
    )
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Data {
    _tmp: TempDir,
    dir: PathBuf,
    file: PathBuf,
}

impl Data {
    fn new(content: &str) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("projects");
        let project = dir.join("-home-user-p");
        fs::create_dir_all(&project).unwrap();
        let file = project.join("live.jsonl");
        fs::write(&file, content).unwrap();
        Self {
            _tmp: tmp,
            dir,
            file,
        }
    }

    fn harness(&self) -> ClaudeHarness {
        ClaudeHarness::with_data_dir(self.dir.clone(), Default::default())
    }

    fn append(&self, bytes: &str) {
        let mut file = OpenOptions::new().append(true).open(&self.file).unwrap();
        file.write_all(bytes.as_bytes()).unwrap();
    }

    fn mtime(&self) -> SystemTime {
        fs::metadata(&self.file).unwrap().modified().unwrap()
    }

    /// The turns of a fresh parse of the file as it is now.
    fn fresh(&self) -> Vec<Arc<Turn>> {
        Assembler::from_file(&self.file).unwrap().into_turns()
    }
}

/// A running worker and everything it has sent.
struct Live {
    rx: Receiver<ModelUpdate>,
    handle: Option<WatchHandle>,
    seen: Vec<ModelUpdate>,
}

impl Live {
    /// Starts the worker and waits for the initial load, `Loaded` and the
    /// `Activity` after it.
    fn start(harness: &ClaudeHarness) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = harness.load(&SessionId::from("live"), tx).unwrap();
        let mut live = Self {
            rx,
            handle: Some(handle),
            seen: Vec::new(),
        };
        live.wait_for("Loaded and Activity", |seen| {
            seen.contains(&ModelUpdate::Loaded)
                && seen
                    .iter()
                    .skip_while(|u| **u != ModelUpdate::Loaded)
                    .any(|u| matches!(u, ModelUpdate::Activity { .. }))
        });
        live
    }

    /// Receives until `done` holds for everything seen so far.
    fn wait_for(&mut self, what: &str, done: impl Fn(&[ModelUpdate]) -> bool) {
        let deadline = Instant::now() + WAIT;
        while !done(&self.seen) {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(update) => self.seen.push(update),
                Err(RecvTimeoutError::Timeout) => {
                    panic!("timed out waiting for {what}; saw {:#?}", self.seen)
                }
                Err(RecvTimeoutError::Disconnected) => panic!("disconnected waiting for {what}"),
            }
        }
    }

    /// Forgets what was seen, keeping the position in the stream.
    fn clear(&mut self) {
        self.seen.clear();
    }

    /// Waits until the updates seen since the last `clear` build up exactly
    /// the turns of a fresh parse of the file as it is now. The updates from
    /// the last `Reset` on (all of them when there is none) are applied on
    /// top of `base`.
    fn wait_for_turns(&mut self, base: &[Arc<Turn>], expected: &[Arc<Turn>]) {
        let base = base.to_vec();
        let expected = expected.to_vec();
        self.wait_for("the expected turns", move |seen| {
            let mut turns = base.clone();
            for update in seen {
                update.apply(&mut turns);
            }
            turns == expected
        });
    }

    fn drop_handle(&mut self) {
        self.handle = None;
    }
}

fn turn_batches(updates: &[ModelUpdate]) -> usize {
    updates
        .iter()
        .filter(|u| matches!(u, ModelUpdate::Appended(_) | ModelUpdate::Updated(_)))
        .count()
}

fn applied(updates: &[ModelUpdate]) -> Vec<Arc<Turn>> {
    let mut turns = Vec::new();
    for update in updates {
        update.apply(&mut turns);
    }
    turns
}

fn call_status(turn: &Turn, call: &str) -> Option<ToolStatus> {
    let Turn::Assistant(turn) = turn else {
        return None;
    };
    turn.items.iter().find_map(|item| match item {
        Item::ToolCall(c) if c.id.as_str() == call => Some(c.status),
        _ => None,
    })
}

fn initial() -> String {
    prompt(1, "first") + &assistant(2, "msg_1", &text_block("hello"))
}

// ---------------------------------------------------------------------------
// Appends
// ---------------------------------------------------------------------------

#[test]
fn appended_lines_arrive_as_appended_turns() {
    let data = Data::new(&initial());
    let mut live = Live::start(&data.harness());
    let base = applied(&live.seen);
    assert_eq!(base.len(), 2);
    live.clear();

    data.append(&(prompt(3, "second") + &assistant(4, "msg_2", &text_block("again"))));
    let expected = data.fresh();
    assert_eq!(expected.len(), 4);
    live.wait_for_turns(&base, &expected);

    let appended: usize = live
        .seen
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Appended(t) => Some(t.len()),
            _ => None,
        })
        .sum();
    assert_eq!(appended, 2, "the two new turns were appended");
    assert!(
        !live
            .seen
            .iter()
            .any(|u| matches!(u, ModelUpdate::Reset | ModelUpdate::Progress(_))),
        "an append is neither a reload nor loading progress"
    );
}

#[test]
fn a_line_extending_a_message_updates_its_turn_in_place() {
    let data = Data::new(&initial());
    let mut live = Live::start(&data.harness());
    let base = applied(&live.seen);
    let id = base[1].id().clone();
    live.clear();

    // The same message id: a streamed message that grows.
    data.append(&assistant(3, "msg_1", &text_block("more of it")));
    live.wait_for("an Updated", |seen| {
        seen.iter().any(|u| matches!(u, ModelUpdate::Updated(_)))
    });

    let updated: Vec<&Arc<Turn>> = live
        .seen
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Updated(t) => Some(t),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].id(), &id, "the same TurnId");
    let Turn::Assistant(turn) = &**updated[0] else {
        panic!("an assistant turn");
    };
    assert_eq!(turn.items.len(), 2, "the new block is part of the turn");
    assert!(
        !live
            .seen
            .iter()
            .any(|u| matches!(u, ModelUpdate::Appended(_))),
        "no new turn"
    );
    live.wait_for_turns(&base, &data.fresh());
}

#[test]
fn a_tool_result_updates_the_turn_with_the_pending_call() {
    let data = Data::new(
        &(prompt(1, "run it")
            + &assistant(
                2,
                "msg_1",
                &(text_block("running") + "," + &tool_use("toolu_1")),
            )),
    );
    let mut live = Live::start(&data.harness());
    let base = applied(&live.seen);
    let id = base[1].id().clone();
    assert_eq!(call_status(&base[1], "toolu_1"), Some(ToolStatus::Pending));
    live.clear();

    data.append(&tool_result(3, "toolu_1", "file.txt"));
    live.wait_for("an Updated", |seen| {
        seen.iter().any(|u| matches!(u, ModelUpdate::Updated(_)))
    });

    let updated = live
        .seen
        .iter()
        .find_map(|u| match u {
            ModelUpdate::Updated(t) => Some(t.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].id(), &id, "the same TurnId");
    assert_eq!(call_status(&updated[0], "toolu_1"), Some(ToolStatus::Ok));
    live.wait_for_turns(&base, &data.fresh());
}

#[test]
fn a_line_written_in_two_parts_is_parsed_once_after_its_newline() {
    let data = Data::new(&initial());
    let mut live = Live::start(&data.harness());
    let base = applied(&live.seen);
    live.clear();

    let line = prompt(3, "in two parts");
    let (head, tail) = line.split_at(line.len() / 2);
    data.append(head);
    // Let the first part be noticed and processed: a Activity says it was.
    let before = data.mtime();
    live.wait_for("the first part to be noticed", |seen| {
        seen.iter()
            .any(|u| matches!(u, ModelUpdate::Activity { last_modified, .. } if *last_modified >= before))
    });
    assert_eq!(turn_batches(&live.seen), 0, "half a line is not parsed");
    assert!(
        !live
            .seen
            .iter()
            .any(|u| matches!(u, ModelUpdate::ParseWarnings(n) if *n > 0)),
        "half a line is not a warning"
    );

    data.append(tail);
    let expected = data.fresh();
    assert_eq!(expected.len(), 3);
    live.wait_for_turns(&base, &expected);
    assert_eq!(turn_batches(&live.seen), 1, "parsed exactly once");
    assert!(
        !live
            .seen
            .iter()
            .any(|u| matches!(u, ModelUpdate::ParseWarnings(n) if *n > 0))
    );
}

#[test]
fn a_partial_last_line_at_load_time_waits_for_its_newline() {
    let line = prompt(3, "cut off");
    let (head, tail) = line.split_at(line.len() - 7);
    let data = Data::new(&(initial() + head));
    let mut live = Live::start(&data.harness());
    let base = applied(&live.seen);
    assert_eq!(base.len(), 2, "the partial line is not a turn yet");
    assert!(
        !live
            .seen
            .iter()
            .any(|u| matches!(u, ModelUpdate::ParseWarnings(n) if *n > 0)),
        "nor an unparsable entry"
    );
    live.clear();

    data.append(tail);
    let expected = data.fresh();
    assert_eq!(expected.len(), 3);
    live.wait_for_turns(&base, &expected);
    assert_eq!(turn_batches(&live.seen), 1);
}

// ---------------------------------------------------------------------------
// Reloading
// ---------------------------------------------------------------------------

/// Waits for a `Reset`, then for the full re-parse to end in `Loaded`, and
/// checks that the turns equal a fresh parse. (A file that is rewritten in
/// place is seen empty for a moment: the re-parse may find nothing, and the
/// content then arrives as an append, so the turns are awaited.)
fn expect_reload(live: &mut Live, data: &Data) {
    let expected = data.fresh();
    live.wait_for("a Reset, a Loaded after it and the new turns", |seen| {
        seen.iter()
            .position(|u| *u == ModelUpdate::Reset)
            .is_some_and(|reset| {
                seen[reset..].contains(&ModelUpdate::Loaded) && applied(&seen[reset..]) == expected
            })
    });
    let reset = live
        .seen
        .iter()
        .position(|u| *u == ModelUpdate::Reset)
        .unwrap();
    let after = &live.seen[reset..];
    let loaded = after
        .iter()
        .position(|u| *u == ModelUpdate::Loaded)
        .unwrap();
    assert!(
        after[..loaded]
            .iter()
            .any(|u| matches!(u, ModelUpdate::Progress(p) if *p == 1.0)),
        "the re-parse reports its progress"
    );
}

#[test]
fn truncating_the_file_reloads_it() {
    let data = Data::new(&(initial() + &prompt(3, "second") + &prompt(4, "third")));
    let mut live = Live::start(&data.harness());
    assert_eq!(applied(&live.seen).len(), 4);
    live.clear();

    // Shorter than what was read.
    fs::write(&data.file, prompt(9, "only one")).unwrap();
    expect_reload(&mut live, &data);

    // The watch goes on: the new file grows like any other.
    live.clear();
    let base = data.fresh();
    data.append(&prompt(10, "after"));
    live.wait_for_turns(&base, &data.fresh());
    assert!(!live.seen.contains(&ModelUpdate::Reset));
}

#[cfg(unix)]
#[test]
fn replacing_the_file_by_a_rename_reloads_it() {
    use std::os::unix::fs::MetadataExt;

    let data = Data::new(&initial());
    let mut live = Live::start(&data.harness());
    live.clear();
    let old_inode = fs::metadata(&data.file).unwrap().ino();

    // Longer than the old file, so only the inode tells.
    let replacement = data.file.with_file_name("replacement.tmp");
    fs::write(
        &replacement,
        prompt(5, "new 1") + &prompt(6, "new 2") + &prompt(7, "new 3") + &prompt(8, "new 4"),
    )
    .unwrap();
    fs::rename(&replacement, &data.file).unwrap();
    assert_ne!(fs::metadata(&data.file).unwrap().ino(), old_inode);

    expect_reload(&mut live, &data);
}

// ---------------------------------------------------------------------------
// Coalescing, activity
// ---------------------------------------------------------------------------

#[test]
fn a_burst_of_appends_is_coalesced() {
    let data = Data::new(&initial());
    let mut live = Live::start(&data.harness());
    let base = applied(&live.seen);
    live.clear();

    let started = Instant::now();
    for i in 0..50 {
        data.append(&prompt(100 + i, &format!("burst {i}")));
        std::thread::sleep(Duration::from_millis(1));
    }
    let burst = started.elapsed();
    let expected = data.fresh();
    assert_eq!(expected.len(), 52);
    live.wait_for_turns(&base, &expected);

    // Let stragglers arrive, then count.
    std::thread::sleep(Duration::from_millis(300));
    while let Ok(update) = live.rx.try_recv() {
        live.seen.push(update);
    }
    let batches = turn_batches(&live.seen);
    assert!(
        batches < 10,
        "50 appends in {burst:?} made {batches} update batches"
    );
}

#[test]
fn activity_carries_the_file_mtime_after_the_load_and_after_each_change() {
    let data = Data::new(&initial());
    let mut live = Live::start(&data.harness());

    let after_load = live
        .seen
        .iter()
        .skip_while(|u| **u != ModelUpdate::Loaded)
        .find_map(|u| match u {
            ModelUpdate::Activity {
                last_modified,
                idle,
            } => Some((*last_modified, *idle)),
            _ => None,
        })
        .unwrap();
    assert_eq!(after_load, (data.mtime(), None));
    live.clear();

    // Make sure the mtime moves even on a file system with coarse stamps.
    std::thread::sleep(Duration::from_millis(20));
    data.append(&prompt(3, "later"));
    let mtime = data.mtime();
    assert_ne!(mtime, after_load.0, "the append moved the mtime");
    live.wait_for("an Activity with the new mtime", |seen| {
        seen.iter().any(|u| {
            matches!(u, ModelUpdate::Activity { last_modified, idle: None } if *last_modified == mtime)
        })
    });
}

// ---------------------------------------------------------------------------
// Polling, stopping, other files
// ---------------------------------------------------------------------------

#[test]
fn polling_detects_appends_when_watching_is_unavailable() {
    let data = Data::new(&initial());
    let mut live = Live::start(&data.harness().with_polling());
    let base = applied(&live.seen);
    live.clear();

    data.append(&prompt(3, "polled"));
    live.wait_for_turns(&base, &data.fresh());
    assert_eq!(turn_batches(&live.seen), 1);
    let mtime = data.mtime();
    live.wait_for("an Activity with the new mtime", |seen| {
        seen.iter().any(
            |u| matches!(u, ModelUpdate::Activity { last_modified, .. } if *last_modified == mtime),
        )
    });
}

#[test]
fn polling_reloads_a_truncated_file() {
    let data = Data::new(&(initial() + &prompt(3, "second")));
    let mut live = Live::start(&data.harness().with_polling());
    live.clear();

    fs::write(&data.file, prompt(9, "only")).unwrap();
    expect_reload(&mut live, &data);
}

#[test]
fn dropping_the_handle_stops_watching_and_disconnects() {
    let data = Data::new(&initial());
    let mut live = Live::start(&data.harness());
    live.drop_handle();

    let started = Instant::now();
    match live.rx.recv_timeout(Duration::from_secs(5)) {
        Err(RecvTimeoutError::Disconnected) => {}
        other => panic!("expected a disconnection, got {other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(2), "promptly");

    // Nothing is read any more: the channel stays shut.
    data.append(&prompt(3, "nobody listens"));
    assert_eq!(
        live.rx.recv_timeout(Duration::from_millis(500)),
        Err(RecvTimeoutError::Disconnected)
    );
}

#[test]
fn dropping_the_handle_stops_a_polling_worker_too() {
    let data = Data::new(&initial());
    let mut live = Live::start(&data.harness().with_polling());
    live.drop_handle();
    match live.rx.recv_timeout(Duration::from_secs(5)) {
        Err(RecvTimeoutError::Disconnected) => {}
        other => panic!("expected a disconnection, got {other:?}"),
    }
}

#[test]
fn other_sessions_and_subagent_files_do_not_touch_the_turns() {
    let data = Data::new(&initial());
    let mut live = Live::start(&data.harness());
    let base = applied(&live.seen);
    live.clear();

    let project = data.file.parent().unwrap();
    fs::write(project.join("other.jsonl"), prompt(1, "another session")).unwrap();
    let subagents = project.join("live").join("subagents");
    fs::create_dir_all(&subagents).unwrap();
    // Give the worker the time to start watching the new directory.
    std::thread::sleep(Duration::from_millis(1500));
    fs::write(subagents.join("agent-x.jsonl"), prompt(1, "a subagent")).unwrap();
    fs::write(subagents.join("agent-x.meta.json"), "{}").unwrap();
    std::thread::sleep(Duration::from_millis(600));
    while let Ok(update) = live.rx.try_recv() {
        live.seen.push(update);
    }
    assert_eq!(
        live.seen,
        Vec::<ModelUpdate>::new(),
        "nothing reached the parent session"
    );

    // And the session itself is still live.
    data.append(&prompt(3, "still here"));
    live.wait_for_turns(&base, &data.fresh());
}

#[test]
fn a_missing_session_still_ends_in_loaded_and_stops() {
    let data = Data::new("");
    let (tx, rx) = mpsc::channel();
    let _handle = data
        .harness()
        .load(&SessionId::from("nonexistent"), tx)
        .unwrap();
    assert_eq!(rx.recv_timeout(WAIT), Ok(ModelUpdate::Loaded));
    assert_eq!(
        rx.recv_timeout(WAIT),
        Err(RecvTimeoutError::Disconnected),
        "nothing to watch"
    );
    let _: &Path = &data.file;
}
