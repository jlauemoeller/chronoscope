//! Opening a Claude transcript by its path (`chronoscope <path>.jsonl`),
//! including one outside the data directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use chronoscope::harness::Harness;
use chronoscope::harness::claude::ClaudeHarness;
use chronoscope::model::{HarnessId, ModelUpdate, ProjectId, SessionId, ToolCallId, Turn};

fn fixture_tree() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude/subagents")
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

/// Everything a load sends until `Loaded`.
fn load(harness: &dyn Harness, id: &SessionId) -> Vec<ModelUpdate> {
    let (tx, rx) = mpsc::channel();
    let _handle = harness.load(id, tx).unwrap();
    let mut updates = Vec::new();
    loop {
        let update = rx
            .recv_timeout(Duration::from_secs(30))
            .expect("the load finishes");
        let done = update == ModelUpdate::Loaded;
        updates.push(update);
        if done {
            return updates;
        }
    }
}

fn user_turns(updates: &[ModelUpdate]) -> usize {
    let mut turns = Vec::new();
    for update in updates {
        update.apply(&mut turns);
    }
    turns
        .iter()
        .filter(|t| matches!(***t, Turn::User(_)))
        .count()
}

/// A tree outside the (empty) data directory the harness is built with.
struct Outside {
    _tmp: tempfile::TempDir,
    file: PathBuf,
    harness: ClaudeHarness,
}

fn outside() -> Outside {
    let tmp = tempfile::tempdir().unwrap();
    let tree = tmp.path().join("elsewhere");
    copy_dir(&fixture_tree(), &tree);
    let data = tmp.path().join("projects");
    fs::create_dir_all(&data).unwrap();
    Outside {
        file: tree.join("-home-user-loading/pppp0001.jsonl"),
        harness: ClaudeHarness::with_data_dir(data, Default::default()),
        _tmp: tmp,
    }
}

#[test]
fn a_file_outside_the_data_directory_opens_as_a_claude_session() {
    let o = outside();
    let summary = o.harness.open_file(&o.file).unwrap();
    assert_eq!(summary.id, SessionId::from("pppp0001"));
    assert_eq!(summary.harness, HarnessId::Claude);
    // The project and the cwd come from the entries, not from the directory.
    assert_eq!(summary.project, ProjectId::from("/home/user/loading"));
    assert_eq!(
        summary.cwd.as_deref(),
        Some(Path::new("/home/user/loading"))
    );
    assert_eq!(summary.path, o.file);
    assert_eq!(summary.title, "Explore the repo and write the story");

    let updates = load(&o.harness, &summary.id);
    assert!(user_turns(&updates) >= 1, "{updates:?}");
}

#[test]
fn the_session_is_not_loadable_before_it_is_opened() {
    let o = outside();
    let updates = load(&o.harness, &SessionId::from("pppp0001"));
    assert_eq!(user_turns(&updates), 0);
}

#[test]
fn subagents_of_a_file_outside_the_data_directory_are_found_next_to_it() {
    let o = outside();
    let summary = o.harness.open_file(&o.file).unwrap();
    let agent = o
        .harness
        .subagent(&summary.id, &ToolCallId::from("toolu_meta01"))
        .expect("the subagent is found in <stem>/subagents/");
    assert_eq!(agent, SessionId::from("pppp0001/subagents/agent-m1"));
    let updates = load(&o.harness, &agent);
    assert!(user_turns(&updates) >= 1, "{updates:?}");
    let info = o.harness.subagent_info(&agent).expect("its meta file");
    assert!(!info.agent_type.is_empty());
}

#[test]
fn a_missing_file_is_an_error() {
    let o = outside();
    let err = o.harness.open_file(&o.file.with_file_name("nope.jsonl"));
    assert!(err.is_err());
}

#[test]
fn a_file_inside_the_data_directory_opens_the_same_way() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("projects");
    copy_dir(&fixture_tree(), &data);
    let harness = ClaudeHarness::with_data_dir(data.clone(), Default::default());
    let summary = harness
        .open_file(&data.join("-home-user-loading/pppp0001.jsonl"))
        .unwrap();
    let listed = harness
        .sessions(&ProjectId::from("/home/user/loading"))
        .unwrap();
    let listed = listed.iter().find(|s| s.id == summary.id).unwrap();
    assert_eq!(&summary, listed);
}
