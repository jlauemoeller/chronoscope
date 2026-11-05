//! The session stats of the Claude adapter (SPEC cycle 3 §3): turns, agent
//! time, run time and the whole token use, subagents included.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat};
use chronoscope::components::format_stats;
use chronoscope::duration::format_duration;
use chronoscope::harness::Harness;
use chronoscope::harness::claude::ClaudeHarness;
use chronoscope::model::{ModelUpdate, SessionStats, TokenUsage, Turn};
use tempfile::TempDir;

const BASE: i64 = 1_777_000_000;
const WAIT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------- lines

fn ts(secs: i64) -> String {
    DateTime::from_timestamp(BASE + secs, 0)
        .unwrap()
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn prompt(secs: i64) -> String {
    format!(
        r#"{{"type":"user","uuid":"u{secs}","cwd":"/p","timestamp":"{}","origin":{{"kind":"human"}},"message":{{"role":"user","content":"go"}}}}"#,
        ts(secs)
    ) + "\n"
}

/// A line of message `id`, with `usage` as the JSON of its `usage` object.
fn message(secs: i64, id: &str, usage: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"a{secs}","cwd":"/p","timestamp":"{}","message":{{"id":"{id}","model":"claude-sonnet-5-5","role":"assistant","content":[{{"type":"text","text":"ok"}}],"usage":{usage}}}}}"#,
        ts(secs)
    ) + "\n"
}

fn usage(input: u64, output: u64) -> String {
    format!(r#"{{"input_tokens":{input},"output_tokens":{output}}}"#)
}

fn footer(secs: i64, ms: u64) -> String {
    format!(
        r#"{{"type":"system","subtype":"turn_duration","uuid":"d{secs}","cwd":"/p","timestamp":"{}","durationMs":{ms}}}"#,
        ts(secs)
    ) + "\n"
}

/// The echo of a local command: a `user` entry without `origin`.
fn local_command(secs: i64) -> String {
    format!(
        r#"{{"type":"user","uuid":"e{secs}","cwd":"/p","timestamp":"{}","message":{{"role":"user","content":"<command-name>/clear</command-name>\n<command-message>clear</command-message>\n<command-args></command-args>"}}}}"#,
        ts(secs)
    ) + "\n"
}

// ------------------------------------------------------------------ rig

struct Rig {
    dir: TempDir,
    harness: ClaudeHarness,
}

impl Rig {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let harness = ClaudeHarness::with_data_dir(dir.path().to_owned(), Default::default());
        Self { dir, harness }
    }

    /// The transcript `ses.jsonl`.
    fn transcript(&self) -> PathBuf {
        self.dir.path().join("ses.jsonl")
    }

    /// The directory of the session's subagents.
    fn subagents(&self) -> PathBuf {
        self.dir.path().join("ses").join("subagents")
    }

    fn write_main(&self, text: &str) {
        fs::write(self.transcript(), text).unwrap();
    }

    fn write_sub(&self, relative: &str, text: &str) {
        let path = self.subagents().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn stats(&self) -> SessionStats {
        let summary = self.harness.open_file(&self.transcript()).unwrap();
        self.harness.stats(&summary).unwrap()
    }

    /// The turns the browser shows for the transcript.
    fn turns(&self) -> Vec<Arc<Turn>> {
        let summary = self.harness.open_file(&self.transcript()).unwrap();
        let (tx, rx) = mpsc::channel();
        let _handle = self.harness.load(&summary.id, tx).unwrap();
        let mut turns = Vec::new();
        loop {
            let update = rx.recv_timeout(WAIT).unwrap();
            update.apply(&mut turns);
            if update == ModelUpdate::Loaded {
                return turns;
            }
        }
    }
}

/// What the sidebar shows: the footer durations of the Assistant turns.
fn sidebar(turns: &[Arc<Turn>]) -> Vec<Duration> {
    turns
        .iter()
        .filter_map(|turn| match &**turn {
            Turn::Assistant(t) => t.stats.as_ref().map(|s| s.duration),
            _ => None,
        })
        .collect()
}

fn tokens(input: u64, output: u64) -> TokenUsage {
    TokenUsage {
        input,
        output,
        ..TokenUsage::default()
    }
}

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

// ------------------------------------------------------- times and turns

#[test]
fn agent_time_is_the_sum_of_the_footers_and_run_time_the_span_local_commands_included() {
    let rig = Rig::new();
    rig.write_main(
        &(prompt(0)
            + &message(10, "m1", &usage(1, 1))
            + &footer(40, 40_000)
            // Hours of idle time.
            + &prompt(3_600)
            + &message(3_610, "m2", &usage(1, 1))
            + &footer(3_625, 25_000)
            // A local command is the last entry.
            + &local_command(7_200)),
    );
    let stats = rig.stats();
    assert_eq!(stats.turns, 2);
    assert_eq!(stats.agent_time, secs(65));
    assert_eq!(stats.run_time, secs(7_200));
    assert_eq!(format_stats(&stats), "2 turns · 1m 5s");
}

#[test]
fn the_agent_time_is_the_sum_of_the_sidebar_durations_taken_before_formatting() {
    let rig = Rig::new();
    // 1h 0m 59s twice: each shows as `1h 0m`, together 2h 1m 58s.
    let each = 3_600 + 59;
    rig.write_main(
        &(prompt(0)
            + &message(1, "m1", &usage(1, 1))
            + &footer(each, 0)
            + &prompt(5_000)
            + &message(5_001, "m2", &usage(1, 1))
            + &footer(5_000 + each, 0)),
    );
    let durations = sidebar(&rig.turns());
    assert_eq!(durations, [secs(3_659), secs(3_659)]);
    for duration in &durations {
        assert_eq!(format_duration(*duration), "1h 0m");
    }
    let stats = rig.stats();
    assert_eq!(stats.agent_time, durations.iter().sum::<Duration>());
    assert_eq!(format_duration(stats.agent_time), "2h 1m");
    assert_eq!(format_stats(&stats), "2 turns · 2h 1m");
}

#[test]
fn a_turn_without_a_footer_adds_no_time_but_its_tokens_count() {
    let rig = Rig::new();
    rig.write_main(
        &(prompt(0)
            + &message(5, "m1", &usage(10, 20))
            + &footer(30, 30_000)
            + &prompt(100)
            + &message(105, "m2", &usage(100, 200))),
    );
    let turns = rig.turns();
    assert_eq!(sidebar(&turns), [secs(30)], "the second turn has no footer");
    let stats = rig.stats();
    assert_eq!(stats.agent_time, secs(30));
    assert_eq!(stats.tokens, tokens(110, 220));
}

#[test]
fn a_session_of_local_commands_only_has_no_turns_and_no_time() {
    let rig = Rig::new();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude/local_commands_only.jsonl");
    rig.write_main(&fs::read_to_string(fixture).unwrap());
    let stats = rig.stats();
    assert_eq!(format_stats(&stats), "0 turns · 0s");
    assert_eq!(stats.run_time, Duration::from_millis(10_100));
    assert_eq!(stats.tokens, TokenUsage::default());
}

#[test]
fn the_agent_time_of_every_real_fixture_is_the_sum_of_its_loaded_footers() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude");
    let mut with_footers = 0;
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let rig = Rig::new();
        rig.write_main(&fs::read_to_string(&path).unwrap());
        let durations = sidebar(&rig.turns());
        with_footers += usize::from(!durations.is_empty());
        assert_eq!(
            rig.stats().agent_time,
            durations.iter().sum::<Duration>(),
            "{}",
            path.display()
        );
    }
    assert!(with_footers >= 2, "the fixtures have footers to compare");
}

#[test]
fn absurd_footers_saturate_the_agent_time_instead_of_panicking() {
    let rig = Rig::new();
    // No timestamp on the footer: its own `durationMs` is the duration.
    let footer = r#"{"type":"system","subtype":"turn_duration","uuid":"d","cwd":"/p","durationMs":18446744073709551615}"#;
    let mut text = String::new();
    for i in 0..1_100 {
        text += &prompt(i * 2);
        text += &message(i * 2 + 1, &format!("m{i}"), &usage(1, 1));
        text += footer;
        text += "\n";
    }
    rig.write_main(&text);
    let stats = rig.stats();
    assert_eq!(stats.turns, 1_100);
    assert_eq!(stats.agent_time, Duration::MAX);
}

// ------------------------------------------------------------- tokens

#[test]
fn output_includes_reasoning_and_reasoning_is_not_added_again() {
    let rig = Rig::new();
    rig.write_main(&(prompt(0)
        + &message(
            1,
            "m1",
            r#"{"input_tokens":10,"output_tokens":20,"cache_read_input_tokens":30,"cache_creation_input_tokens":40,"output_tokens_details":{"thinking_tokens":6}}"#,
        )));
    assert_eq!(
        rig.stats().tokens,
        TokenUsage {
            input: 10,
            output: 20,
            cache_read: 30,
            cache_write: 40,
            reasoning: 6,
        }
    );
}

#[test]
fn a_repeated_message_id_counts_once_with_its_latest_usage() {
    let rig = Rig::new();
    rig.write_main(
        &(prompt(0)
            + &message(1, "m1", &usage(1, 1))
            + &message(2, "m1", &usage(5, 7))
            + &message(3, "other", &usage(100, 100))),
    );
    assert_eq!(rig.stats().tokens, tokens(105, 107));
}

#[test]
fn a_message_id_that_continues_after_a_prompt_counts_once() {
    let rig = Rig::new();
    rig.write_main(
        &(prompt(0)
            + &message(1, "m1", &usage(5, 5))
            + &prompt(10)
            // The same message again, in the next turn, with its final usage.
            + &message(11, "m1", &usage(9, 9))),
    );
    assert_eq!(rig.stats().tokens, tokens(9, 9));
}

#[test]
fn malformed_lines_are_skipped() {
    let rig = Rig::new();
    rig.write_main(
        &(prompt(0)
            + "{not json\n"
            + &message(1, "m1", &usage(2, 3))
            + "[]\n"
            + r#"{"type":"assistant","timestamp":5,"message":{"id":"bad","usage":{"input_tokens":99}}}"#
            + "\n"),
    );
    assert_eq!(rig.stats().tokens, tokens(2, 3));
}

// ---------------------------------------------------------- subagents

#[test]
fn a_missing_subagents_directory_means_no_subagents() {
    let rig = Rig::new();
    rig.write_main(&(prompt(0) + &message(1, "m1", &usage(1, 2))));
    assert!(!rig.subagents().exists());
    assert_eq!(rig.stats().tokens, tokens(1, 2));
}

#[test]
fn subagents_and_nested_workflow_subagents_count_and_the_journal_does_not() {
    let rig = Rig::new();
    rig.write_main(&(prompt(0) + &message(1, "m1", &usage(1, 2))));
    rig.write_sub("agent-a1.jsonl", &message(2, "s1", &usage(10, 20)));
    rig.write_sub(
        "workflows/run1/agent-w1.jsonl",
        &message(3, "s2", &usage(100, 200)),
    );
    rig.write_sub(
        "workflows/run1/deeper/agent-w2.jsonl",
        &message(4, "s3", &usage(1_000, 2_000)),
    );
    // Not transcripts, although they look like some.
    rig.write_sub("journal.jsonl", &message(5, "j", &usage(9_999, 9_999)));
    rig.write_sub(
        "workflows/run1/journal.jsonl",
        &message(6, "j", &usage(9_999, 9_999)),
    );
    rig.write_sub("agent-a1.meta.json", r#"{"agentType":"x"}"#);
    let stats = rig.stats();
    assert_eq!(
        stats.tokens,
        tokens(1 + 10 + 100 + 1_000, 2 + 20 + 200 + 2_000)
    );
    // Total time and turns are those of the main session.
    assert_eq!(stats.turns, 1);
    assert_eq!(stats.run_time, secs(1));
    assert_eq!(stats.agent_time, Duration::ZERO);
}

#[test]
fn a_subagent_message_counts_once_per_file_and_malformed_lines_are_skipped() {
    let rig = Rig::new();
    rig.write_main(&(prompt(0) + &message(1, "m1", &usage(1, 1))));
    rig.write_sub(
        "agent-a1.jsonl",
        &(message(2, "s1", &usage(10, 10))
            + "{broken\n"
            + &message(3, "s1", &usage(30, 40))
            + &message(4, "s2", &usage(5, 5))),
    );
    assert_eq!(rig.stats().tokens, tokens(1 + 30 + 5, 1 + 40 + 5));
}

#[cfg(unix)]
#[test]
fn an_unreadable_subagent_file_leaves_the_other_tokens_counted() {
    let rig = Rig::new();
    rig.write_main(&(prompt(0) + &message(1, "m1", &usage(1, 2))));
    rig.write_sub("agent-a1.jsonl", &message(2, "s1", &usage(10, 20)));
    // A dangling link: listed, and reading it fails.
    std::os::unix::fs::symlink(
        rig.dir.path().join("nowhere"),
        rig.subagents().join("agent-gone.jsonl"),
    )
    .unwrap();
    rig.write_sub(
        "workflows/run1/agent-w1.jsonl",
        &message(3, "s2", &usage(100, 200)),
    );
    assert_eq!(rig.stats().tokens, tokens(111, 222));
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_directory_is_not_followed() {
    let rig = Rig::new();
    rig.write_main(&(prompt(0) + &message(1, "m1", &usage(1, 2))));
    let elsewhere = rig.dir.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(
        elsewhere.join("agent-z.jsonl"),
        message(2, "z", &usage(777, 777)),
    )
    .unwrap();
    fs::create_dir_all(rig.subagents()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, rig.subagents().join("linked")).unwrap();
    assert_eq!(rig.stats().tokens, tokens(1, 2));
}
