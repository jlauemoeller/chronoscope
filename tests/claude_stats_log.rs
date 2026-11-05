//! A subagent transcript that cannot be read is logged and skipped (SPEC
//! cycle 3 §3.2). Own test binary: the logger is process-global.

use std::fs;

use chronoscope::harness::Harness;
use chronoscope::harness::claude::ClaudeHarness;
use chronoscope::logging::init_logging;

const MAIN: &str = concat!(
    r#"{"type":"user","uuid":"u1","cwd":"/p","timestamp":"2026-05-01T10:00:00.000Z","origin":{"kind":"human"},"message":{"role":"user","content":"go"}}"#,
    "\n",
    r#"{"type":"assistant","uuid":"a1","cwd":"/p","timestamp":"2026-05-01T10:00:01.000Z","message":{"id":"m1","model":"x","role":"assistant","content":[{"type":"text","text":"ok"}],"usage":{"input_tokens":1,"output_tokens":2}}}"#,
    "\n"
);

#[cfg(unix)]
#[test]
fn an_unreadable_subagent_file_is_logged_and_the_rest_still_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("chronoscope.log");
    init_logging(&log).expect("logging initialises");

    let data = tmp.path().join("data");
    let subagents = data.join("ses").join("subagents");
    fs::create_dir_all(&subagents).unwrap();
    fs::write(data.join("ses.jsonl"), MAIN).unwrap();
    fs::write(
        subagents.join("agent-a1.jsonl"),
        MAIN.replace("\"m1\"", "\"s1\""),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        tmp.path().join("nowhere"),
        subagents.join("agent-gone.jsonl"),
    )
    .unwrap();

    let harness = ClaudeHarness::with_data_dir(data.clone(), Default::default());
    let summary = harness.open_file(&data.join("ses.jsonl")).unwrap();
    let stats = harness.stats(&summary).unwrap();
    assert_eq!(
        stats.tokens.input, 2,
        "the main session and the readable one"
    );
    assert_eq!(stats.tokens.output, 4);

    let text = fs::read_to_string(&log).unwrap();
    let warnings: Vec<_> = text
        .lines()
        .filter(|l| l.contains("WARN") && l.contains("agent-gone.jsonl"))
        .collect();
    assert_eq!(warnings.len(), 1, "{text}");
    assert!(
        !text.contains("agent-a1.jsonl"),
        "the readable file is not mentioned: {text}"
    );
}
