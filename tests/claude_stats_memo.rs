//! The Claude stats key and the subagent usage memo (SPEC cycle 3 §3.3,
//! §3.5): the key sees the subagent transcripts without reading any file, and
//! a recomputation reads only the subagent files that changed.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use chronoscope::harness::Harness;
use chronoscope::harness::claude::ClaudeHarness;
use chronoscope::model::{SessionStats, SessionSummary, TokenUsage};
use tempfile::TempDir;

/// The main transcript's line: a prompt and an answer with some usage.
const MAIN: &str = concat!(
    r#"{"type":"user","uuid":"u1","cwd":"/p","timestamp":"2026-05-01T10:00:00.000Z","origin":{"kind":"human"},"message":{"role":"user","content":"go"}}"#,
    "\n",
    r#"{"type":"assistant","uuid":"a1","cwd":"/p","timestamp":"2026-05-01T10:00:05.000Z","message":{"id":"m0","model":"x","role":"assistant","content":[{"type":"text","text":"ok"}],"usage":{"input_tokens":1,"output_tokens":2}}}"#,
    "\n"
);

/// A subagent line: message `id` with `input` input tokens.
fn sub_line(id: &str, input: u64) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"u-{id}","isSidechain":true,"cwd":"/p","timestamp":"2026-05-01T10:01:00.000Z","message":{{"id":"{id}","model":"x","role":"assistant","content":[{{"type":"text","text":"ok"}}],"usage":{{"input_tokens":{input},"output_tokens":1}}}}}}"#
    ) + "\n"
}

fn tokens(input: u64, output: u64) -> TokenUsage {
    TokenUsage {
        input,
        output,
        ..TokenUsage::default()
    }
}

struct Rig {
    dir: TempDir,
    harness: ClaudeHarness,
    /// The subagent files the harness read, in order.
    reads: Arc<Mutex<Vec<PathBuf>>>,
}

impl Rig {
    /// A session `ses` in project directory `-p`, with the main transcript
    /// written and the subagents directory not.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let reads = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&reads);
        let harness = ClaudeHarness::with_data_dir(dir.path().to_owned(), Default::default())
            .with_subagent_read_hook(move |path| log.lock().unwrap().push(path.to_owned()));
        let rig = Self {
            dir,
            harness,
            reads,
        };
        fs::create_dir_all(rig.main().parent().unwrap()).unwrap();
        fs::write(rig.main(), MAIN).unwrap();
        rig
    }

    fn main(&self) -> PathBuf {
        self.dir.path().join("-p").join("ses.jsonl")
    }

    fn subagents(&self) -> PathBuf {
        self.dir.path().join("-p").join("ses").join("subagents")
    }

    fn write_sub(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.subagents().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    /// The session as the index lists it.
    fn session(&self) -> SessionSummary {
        self.harness.refresh().unwrap();
        let project = self.harness.projects().unwrap().remove(0);
        self.harness.sessions(&project.id).unwrap().remove(0)
    }

    /// Takes the subagent files read since the last call.
    fn take_reads(&self) -> Vec<PathBuf> {
        std::mem::take(&mut *self.reads.lock().unwrap())
    }

    /// The stats a harness with an empty memo computes.
    fn fresh_stats(&self, session: &SessionSummary) -> SessionStats {
        let fresh = ClaudeHarness::with_data_dir(self.dir.path().to_owned(), Default::default());
        fresh.stats(session).unwrap()
    }
}

/// The three subagent fields of a key: count, total size, newest mtime.
fn subagent_fields(key: &str) -> (u64, u64, u128) {
    let mut fields = key.rsplitn(4, ':');
    let newest = fields.next().unwrap().parse().unwrap();
    let total = fields.next().unwrap().parse().unwrap();
    let count = fields.next().unwrap().parse().unwrap();
    (count, total, newest)
}

fn set_mtime(path: &Path, time: SystemTime) {
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

fn nanos(path: &Path) -> u128 {
    fs::metadata(path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

#[test]
fn the_key_without_subagents_ends_in_zeros() {
    let rig = Rig::new();
    let session = rig.session();
    let key = rig.harness.stats_key(&session);
    assert!(key.starts_with("claude:"), "{key}");
    assert!(key.ends_with(":0:0:0"), "{key}");
    let meta = fs::metadata(rig.main()).unwrap();
    assert!(
        key.contains(&format!(":{}:", meta.len())),
        "the main file's size is in the key: {key}"
    );
}

#[test]
fn the_key_changes_with_the_number_total_size_and_newest_mtime_of_the_subagents() {
    let rig = Rig::new();
    let session = rig.session();
    let none = rig.harness.stats_key(&session);

    // The number: a first and a second transcript, one of them nested.
    let a = rig.write_sub("agent-a.jsonl", &sub_line("s1", 10));
    let one = rig.harness.stats_key(&session);
    assert_ne!(one, none);
    let (count, total, newest) = subagent_fields(&one);
    assert_eq!(count, 1);
    assert_eq!(total, fs::metadata(&a).unwrap().len());
    assert_eq!(newest, nanos(&a));

    let b = rig.write_sub("workflows/run1/agent-b.jsonl", &sub_line("s2", 20));
    let two = rig.harness.stats_key(&session);
    assert_ne!(two, one);
    let (count, total, _) = subagent_fields(&two);
    assert_eq!(count, 2);
    assert_eq!(
        total,
        fs::metadata(&a).unwrap().len() + fs::metadata(&b).unwrap().len()
    );

    // The newest mtime alone: same count, same sizes.
    let before = nanos(&a).max(nanos(&b));
    let later = SystemTime::UNIX_EPOCH
        + Duration::from_nanos(u64::try_from(before).unwrap())
        + Duration::from_secs(60);
    set_mtime(&a, later);
    let touched = rig.harness.stats_key(&session);
    assert_ne!(touched, two);
    let (count2, total2, newest2) = subagent_fields(&touched);
    assert_eq!((count2, total2), (2, subagent_fields(&two).1));
    assert_eq!(newest2, nanos(&a));

    // The total size alone: another size, the newest mtime put back.
    fs::write(&b, sub_line("s2", 20) + &sub_line("s3", 5)).unwrap();
    set_mtime(&b, SystemTime::UNIX_EPOCH + Duration::from_secs(1));
    let grown = rig.harness.stats_key(&session);
    assert_ne!(grown, touched);
    let (count3, total3, newest3) = subagent_fields(&grown);
    assert_eq!(count3, 2);
    assert!(total3 > total2);
    assert_eq!(newest3, newest2, "the newest mtime is still a's");
}

#[test]
fn the_key_ignores_the_journal_the_meta_files_and_other_names() {
    let rig = Rig::new();
    rig.write_sub("agent-a.jsonl", &sub_line("s1", 10));
    let session = rig.session();
    let key = rig.harness.stats_key(&session);
    rig.write_sub("agent-a.meta.json", r#"{"toolUseId":"t"}"#);
    rig.write_sub("journal.jsonl", "{}\n");
    rig.write_sub("workflows/run1/journal.jsonl", "{}\n");
    rig.write_sub("notes.jsonl", "{}\n");
    rig.write_sub("workflows/run1/agent-b.meta.json", "{}");
    assert_eq!(rig.harness.stats_key(&session), key);
}

#[test]
fn the_key_of_a_missing_main_file_keeps_its_missing_form() {
    let rig = Rig::new();
    let session = rig.session();
    rig.write_sub("agent-a.jsonl", &sub_line("s1", 10));
    fs::remove_file(rig.main()).unwrap();
    assert_eq!(
        rig.harness.stats_key(&session),
        format!("claude:{}:missing", session.path.display())
    );
}

#[test]
fn the_key_reads_no_file() {
    let rig = Rig::new();
    rig.write_sub("agent-a.jsonl", &sub_line("s1", 10));
    rig.write_sub("workflows/run1/agent-b.jsonl", &sub_line("s2", 20));
    let session = rig.session();
    let (scans, parses) = (rig.harness.scan_count(), rig.harness.stats_parse_count());
    for _ in 0..3 {
        rig.harness.stats_key(&session);
    }
    assert_eq!(rig.harness.scan_count(), scans);
    assert_eq!(rig.harness.stats_parse_count(), parses);
    assert_eq!(rig.harness.subagent_read_count(), 0);
    assert!(rig.take_reads().is_empty());
}

#[test]
fn a_recomputation_reads_only_the_subagent_file_that_changed() {
    let rig = Rig::new();
    let a = rig.write_sub("agent-a.jsonl", &sub_line("s1", 10));
    let b = rig.write_sub("workflows/run1/agent-b.jsonl", &sub_line("s2", 20));
    rig.write_sub("agent-c.jsonl", &sub_line("s3", 40));
    let session = rig.session();

    let first = rig.harness.stats(&session).unwrap();
    assert_eq!(first.tokens, tokens(1 + 10 + 20 + 40, 2 + 3));
    assert_eq!(rig.harness.subagent_read_count(), 3);
    assert_eq!(rig.take_reads().len(), 3);
    assert_eq!(rig.harness.subagent_memo_len(), 3);

    // Nothing changed: the main transcript is parsed, no subagent is read.
    let parses = rig.harness.stats_parse_count();
    assert_eq!(rig.harness.stats(&session).unwrap(), first);
    assert_eq!(rig.harness.stats_parse_count(), parses + 1);
    assert!(rig.take_reads().is_empty());

    // One file grows: it is the only subagent file read.
    fs::write(&b, sub_line("s2", 20) + &sub_line("s4", 300)).unwrap();
    let scans = rig.harness.scan_count();
    let second = rig.harness.stats(&session).unwrap();
    assert_eq!(rig.take_reads(), std::slice::from_ref(&b));
    assert_eq!(rig.harness.stats_parse_count(), parses + 2);
    assert_eq!(rig.harness.scan_count(), scans, "no discovery scan either");
    assert_eq!(second.tokens, tokens(1 + 10 + 20 + 300 + 40, 2 + 4));
    // What the memo serves is what a harness without one computes.
    assert_eq!(second, rig.fresh_stats(&session));

    // The same size, another mtime and content: read again.
    fs::write(&a, sub_line("s1", 99)).unwrap();
    assert_eq!(
        fs::metadata(&a).unwrap().len(),
        sub_line("s1", 10).len() as u64
    );
    set_mtime(&a, SystemTime::now() + Duration::from_secs(120));
    let third = rig.harness.stats(&session).unwrap();
    assert_eq!(rig.take_reads(), [a]);
    assert_eq!(third.tokens.input, 1 + 99 + 20 + 300 + 40);
    assert_eq!(third, rig.fresh_stats(&session));
}

#[test]
fn a_file_of_another_size_is_read_again_even_with_its_old_mtime() {
    let rig = Rig::new();
    let a = rig.write_sub("agent-a.jsonl", &sub_line("s1", 10));
    let session = rig.session();
    rig.harness.stats(&session).unwrap();
    rig.take_reads();
    // The memo is keyed by the stat the first read was made after: another
    // size misses it, whatever the clock says.
    let mtime = fs::metadata(&a).unwrap().modified().unwrap();
    fs::write(&a, sub_line("s1", 10) + &sub_line("s2", 5)).unwrap();
    set_mtime(&a, mtime);
    let stats = rig.harness.stats(&session).unwrap();
    assert_eq!(rig.take_reads(), [a]);
    assert_eq!(stats.tokens.input, 1 + 10 + 5);
}

#[test]
fn a_removed_subagent_file_loses_its_entry_and_its_tokens() {
    let rig = Rig::new();
    let a = rig.write_sub("agent-a.jsonl", &sub_line("s1", 10));
    rig.write_sub("agent-b.jsonl", &sub_line("s2", 20));
    let session = rig.session();
    assert_eq!(
        rig.harness.stats(&session).unwrap().tokens,
        tokens(1 + 10 + 20, 2 + 2)
    );
    assert_eq!(rig.harness.subagent_memo_len(), 2);

    fs::remove_file(&a).unwrap();
    rig.take_reads();
    let stats = rig.harness.stats(&session).unwrap();
    assert_eq!(stats.tokens, tokens(1 + 20, 2 + 1));
    assert_eq!(rig.harness.subagent_memo_len(), 1);
    assert!(rig.take_reads().is_empty(), "the other file is memoised");
    assert_eq!(stats, rig.fresh_stats(&session));
}

#[test]
fn the_entries_of_a_session_that_is_gone_are_dropped_at_the_next_build() {
    let rig = Rig::new();
    rig.write_sub("agent-a.jsonl", &sub_line("s1", 10));
    // A second session in the same project.
    let other = rig.dir.path().join("-p").join("two.jsonl");
    fs::write(&other, MAIN).unwrap();
    let two_sub = rig.dir.path().join("-p").join("two").join("subagents");
    fs::create_dir_all(&two_sub).unwrap();
    fs::write(two_sub.join("agent-z.jsonl"), sub_line("z1", 7)).unwrap();

    rig.harness.refresh().unwrap();
    let project = rig.harness.projects().unwrap().remove(0);
    let sessions = rig.harness.sessions(&project.id).unwrap();
    assert_eq!(sessions.len(), 2);
    for session in &sessions {
        rig.harness.stats(session).unwrap();
    }
    assert_eq!(rig.harness.subagent_memo_len(), 2);

    // Its subagents stay on disk; the session itself is gone.
    fs::remove_file(&other).unwrap();
    rig.harness.refresh().unwrap();
    assert_eq!(rig.harness.subagent_memo_len(), 1);
    fs::remove_file(rig.main()).unwrap();
    rig.harness.refresh().unwrap();
    assert_eq!(rig.harness.subagent_memo_len(), 0);
}

#[cfg(unix)]
#[test]
fn a_file_whose_read_failed_is_tried_again_next_time() {
    let rig = Rig::new();
    rig.write_sub("agent-a.jsonl", &sub_line("s1", 10));
    // A dangling link is listed, and reading it fails.
    let link = rig.subagents().join("agent-b.jsonl");
    let target = rig.dir.path().join("target.jsonl");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let session = rig.session();

    let first = rig.harness.stats(&session).unwrap();
    assert_eq!(first.tokens, tokens(1 + 10, 2 + 1));
    assert_eq!(rig.take_reads().len(), 2);
    assert_eq!(
        rig.harness.subagent_memo_len(),
        1,
        "the failure is not kept"
    );

    // Tried again, and fails again; the good file is not read again.
    rig.harness.stats(&session).unwrap();
    assert_eq!(rig.take_reads(), std::slice::from_ref(&link));

    // Once the target exists the file counts.
    fs::write(&target, sub_line("s2", 20)).unwrap();
    let stats = rig.harness.stats(&session).unwrap();
    assert_eq!(rig.take_reads(), [link]);
    assert_eq!(stats.tokens, tokens(1 + 10 + 20, 2 + 2));
    assert_eq!(rig.harness.subagent_memo_len(), 2);
    assert_eq!(stats, rig.fresh_stats(&session));
}
