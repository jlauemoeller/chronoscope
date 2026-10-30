//! Claude project and session discovery against the fixture tree in
//! `tests/fixtures/claude/discovery` (a miniature `~/.claude/projects`).

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Utc};
use chronoscope::config::Config;
use chronoscope::harness::Harness;
use chronoscope::harness::claude::{ClaudeHarness, default_data_dir, truncate_title};
use chronoscope::model::{HarnessId, Project, ProjectId, SessionSummary};
use tempfile::TempDir;

/// Number of session transcripts in the fixture tree (the subagent transcript
/// is not one).
const SESSION_FILES: usize = 8;

const FIRST_LINE: &str = "Implement the incremental refresh so that a rebuild rescans only the files that changed, nothing more";

fn utc(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

fn set_mtime(path: &Path, when: &str) {
    let time = SystemTime::from(utc(when));
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
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

/// The fixture tree copied into a temp dir, with every transcript's mtime set
/// to its last entry's timestamp (so the mtime does not decide anything).
fn fixture() -> (TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude/discovery");
    let data = tmp.path().join("projects");
    copy_dir(&src, &data);
    for (rel, when) in [
        ("-home-user-alpha/aaaa0001.jsonl", "2026-03-01T09:05:00Z"),
        (
            "-home-user-alpha/aaaa0001/subagents/agent-a1.jsonl",
            "2026-03-09T09:01:00Z",
        ),
        ("-home-user-my-app/bbbb0001.jsonl", "2026-03-03T10:10:00Z"),
        ("-home-user-my.app/bbbb0002.jsonl", "2026-03-05T12:30:00Z"),
        ("-home-user-multi/cccc0001.jsonl", "2026-03-02T08:01:00Z"),
        ("-home-user-multi/cccc0002.jsonl", "2026-03-02T08:31:00Z"),
        ("-home-user-prompt/dddd0001.jsonl", "2026-03-04T07:01:00Z"),
        ("-home-user-prompt/dddd0002.jsonl", "2026-03-04T06:00:30Z"),
        ("-home-user-empty/ffff0001.jsonl", "2026-02-01T00:00:00Z"),
    ] {
        set_mtime(&data.join(rel), when);
    }
    (tmp, data)
}

fn harness(data: &Path) -> ClaudeHarness {
    ClaudeHarness::with_data_dir(data.to_path_buf(), BTreeMap::new())
}

fn project<'a>(projects: &'a [Project], id: &str) -> &'a Project {
    projects
        .iter()
        .find(|p| p.id.as_str() == id)
        .unwrap_or_else(|| panic!("no project {id}"))
}

fn session<'a>(sessions: &'a [SessionSummary], id: &str) -> &'a SessionSummary {
    sessions
        .iter()
        .find(|s| s.id.as_str() == id)
        .unwrap_or_else(|| panic!("no session {id}"))
}

fn all_sessions(h: &ClaudeHarness) -> Vec<SessionSummary> {
    h.projects()
        .unwrap()
        .iter()
        .flat_map(|p| h.sessions(&p.id).unwrap())
        .collect()
}

#[test]
fn harness_ids_have_stable_numbers_and_names() {
    let expected = [
        (HarnessId::Claude, 1, "Claude"),
        (HarnessId::OpenCode, 2, "OpenCode"),
        (HarnessId::Codex, 3, "Codex"),
        (HarnessId::Pi, 4, "Pi"),
    ];
    for (id, number, name) in expected {
        assert_eq!(id.number(), number);
        assert_eq!(HarnessId::from_number(number), Some(id));
        assert_eq!(id.display_name(), name);
        assert_eq!(id.to_string(), name);
    }
    assert_eq!(HarnessId::from_number(0), None);
    assert_eq!(HarnessId::from_number(5), None);
}

#[test]
fn claude_harness_implements_the_trait() {
    let (_tmp, data) = fixture();
    let h: Box<dyn Harness> = Box::new(harness(&data));
    assert_eq!(h.id(), HarnessId::Claude);
    assert!(h.available());
    assert!(!h.projects().unwrap().is_empty());
}

#[test]
fn available_follows_the_data_dir() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(!harness(&tmp.path().join("missing")).available());
    assert!(harness(tmp.path()).available());
    // A file is not a data dir.
    let file = tmp.path().join("file");
    fs::write(&file, "").unwrap();
    assert!(!harness(&file).available());
}

#[test]
fn default_data_dir_is_under_home_and_config_overrides_it() {
    let home = tempfile::tempdir().unwrap();
    let projects = home.path().join(".claude").join("projects");
    assert_eq!(default_data_dir(home.path()), projects);

    let default = ClaudeHarness::new(&Config::default(), home.path());
    assert_eq!(default.data_dir(), projects);
    assert!(!default.available());
    fs::create_dir_all(&projects).unwrap();
    assert!(default.available());

    let elsewhere = tempfile::tempdir().unwrap();
    let toml = format!(
        "[harness.claude]\ndata_dir = {:?}\n",
        elsewhere.path().to_str().unwrap()
    );
    let (config, warnings) = Config::from_toml_str(&toml, home.path());
    assert!(warnings.is_empty(), "{warnings:?}");
    let overridden = ClaudeHarness::new(&config, home.path());
    assert_eq!(overridden.data_dir(), elsewhere.path());
    assert!(overridden.available());
}

#[test]
fn a_missing_data_dir_has_no_projects_and_an_unknown_project_no_sessions() {
    let tmp = tempfile::tempdir().unwrap();
    let h = harness(&tmp.path().join("missing"));
    assert!(h.projects().unwrap().is_empty());
    assert!(h.sessions(&ProjectId::from("/nope")).unwrap().is_empty());

    let (_keep, data) = fixture();
    let h = harness(&data);
    assert!(h.sessions(&ProjectId::from("/nope")).unwrap().is_empty());
}

#[test]
fn sessions_in_different_encoded_dirs_with_one_cwd_are_one_project() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    let projects = h.projects().unwrap();
    let app = project(&projects, "/home/user/my.app");
    assert_eq!(app.session_count, 2);
    let sessions = h.sessions(&app.id).unwrap();
    let ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["bbbb0002", "bbbb0001"]);
    assert!(sessions.iter().all(|s| s.project == app.id));
    // No project is named after the lossy directory names.
    assert!(
        projects
            .iter()
            .all(|p| !p.id.as_str().starts_with("-home-user-my"))
    );
}

#[test]
fn sessions_in_one_encoded_dir_with_different_cwds_are_two_projects() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    let projects = h.projects().unwrap();
    let one = project(&projects, "/home/user/multi/one");
    let two = project(&projects, "/home/user/multi/two");
    assert_ne!(one.id, two.id);
    assert_eq!(h.sessions(&one.id).unwrap().len(), 1);
    assert_eq!(h.sessions(&two.id).unwrap().len(), 1);
    assert_eq!(h.sessions(&one.id).unwrap()[0].id.as_str(), "cccc0001");
    assert_eq!(h.sessions(&two.id).unwrap()[0].id.as_str(), "cccc0002");
    assert!(projects.iter().all(|p| p.id.as_str() != "-home-user-multi"));
}

#[test]
fn project_names_are_the_basename_or_the_configured_alias() {
    let (_tmp, data) = fixture();
    let plain = harness(&data).projects().unwrap();
    assert_eq!(project(&plain, "/home/user/my.app").name, "my.app");
    assert_eq!(project(&plain, "/home/user/alpha").name, "alpha");
    assert_eq!(project(&plain, "/home/user/multi/one").name, "one");
    assert_eq!(
        project(&plain, "/home/user/my.app").cwd.as_deref(),
        Some(Path::new("/home/user/my.app"))
    );

    let aliases = BTreeMap::from([
        (PathBuf::from("/home/user/my.app"), "MyApp".to_owned()),
        (PathBuf::from("/home/user/not-there"), "Ghost".to_owned()),
    ]);
    let aliased = ClaudeHarness::with_data_dir(data, aliases)
        .projects()
        .unwrap();
    assert_eq!(project(&aliased, "/home/user/my.app").name, "MyApp");
    assert_eq!(project(&aliased, "/home/user/alpha").name, "alpha");
}

#[test]
fn aliases_from_the_config_are_matched_on_the_expanded_path() {
    let home = tempfile::tempdir().unwrap();
    let data = home.path().join(".claude").join("projects");
    let dir = data.join("-enc");
    fs::create_dir_all(&dir).unwrap();
    let cwd = home.path().join("work").join("brio");
    let line = format!(
        "{{\"type\":\"user\",\"cwd\":{:?},\"timestamp\":\"2026-03-01T00:00:00Z\",\"message\":{{\"content\":\"hi\"}}}}\n",
        cwd.to_str().unwrap()
    );
    fs::write(dir.join("s1.jsonl"), line).unwrap();
    let (config, warnings) = Config::from_toml_str(
        "[projects.aliases]\n\"~/work/brio\" = \"BRIO\"\n",
        home.path(),
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let projects = ClaudeHarness::new(&config, home.path()).projects().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "BRIO");
}

#[test]
fn projects_are_sorted_by_most_recent_activity() {
    let (_tmp, data) = fixture();
    let projects = harness(&data).projects().unwrap();
    let ids: Vec<_> = projects.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "/home/user/my.app",
            "/home/user/prompt",
            "/home/user/multi/two",
            "/home/user/multi/one",
            "/home/user/alpha",
            "-home-user-empty",
        ]
    );
    assert_eq!(
        project(&projects, "/home/user/my.app").last_activity,
        utc("2026-03-05T12:30:00Z")
    );
    assert_eq!(
        project(&projects, "/home/user/prompt").last_activity,
        utc("2026-03-04T07:01:00Z")
    );
}

#[test]
fn a_newer_mtime_moves_the_project_up_and_is_the_last_activity() {
    let (_tmp, data) = fixture();
    set_mtime(
        &data.join("-home-user-alpha/aaaa0001.jsonl"),
        "2026-04-01T00:00:00Z",
    );
    // An mtime older than the last entry does not lower the last activity.
    set_mtime(
        &data.join("-home-user-my.app/bbbb0002.jsonl"),
        "2026-01-01T00:00:00Z",
    );
    let h = harness(&data);
    let projects = h.projects().unwrap();
    assert_eq!(projects[0].id.as_str(), "/home/user/alpha");
    let alpha = h.sessions(&projects[0].id).unwrap();
    assert_eq!(alpha[0].last_activity, utc("2026-04-01T00:00:00Z"));
    // The start time is the first entry's, whatever the mtime.
    assert_eq!(alpha[0].started_at, utc("2026-03-01T09:00:00Z"));
    let app = h.sessions(&ProjectId::from("/home/user/my.app")).unwrap();
    assert_eq!(
        session(&app, "bbbb0002").last_activity,
        utc("2026-03-05T12:30:00Z")
    );
}

#[test]
fn the_last_ai_title_wins() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    let alpha = h.sessions(&ProjectId::from("/home/user/alpha")).unwrap();
    assert_eq!(alpha.len(), 1);
    assert_eq!(alpha[0].title, "Final parser title");
    assert_eq!(alpha[0].first_prompt.as_deref(), Some("Add a parser"));
}

#[test]
fn without_a_title_the_first_prompt_gives_a_truncated_first_line() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    let sessions = h.sessions(&ProjectId::from("/home/user/prompt")).unwrap();
    let s = session(&sessions, "dddd0001");

    assert!(FIRST_LINE.chars().count() > 80);
    let expected: String = FIRST_LINE.chars().take(79).chain(['…']).collect();
    assert_eq!(s.title, expected);
    assert_eq!(s.title.chars().count(), 80);
    // Tool-result-only and `isMeta` entries before it are not prompts, and
    // the whole prompt is kept.
    assert_eq!(
        s.first_prompt.as_deref(),
        Some(format!("{FIRST_LINE}\nand a second line").as_str())
    );
    // The first entry (a tool_result) starts the session.
    assert_eq!(s.started_at, utc("2026-03-04T07:00:00Z"));

    let short = h.sessions(&ProjectId::from("/home/user/my.app")).unwrap();
    assert_eq!(session(&short, "bbbb0001").title, "Fix the build");
}

#[test]
fn truncation_keeps_80_characters_and_cuts_81() {
    let eighty = "é".repeat(80);
    assert_eq!(truncate_title(&eighty), eighty);
    let cut = truncate_title(&"é".repeat(81));
    assert_eq!(cut.chars().count(), 80);
    assert!(cut.ends_with('…'));
    assert_eq!(cut.chars().filter(|&c| c == 'é').count(), 79);
}

#[test]
fn without_title_or_prompt_a_session_is_untitled() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    let sessions = h.sessions(&ProjectId::from("/home/user/prompt")).unwrap();
    let s = session(&sessions, "dddd0002");
    assert_eq!(s.title, "(untitled)");
    assert_eq!(s.first_prompt, None);
    assert_eq!(s.cwd.as_deref(), Some(Path::new("/home/user/prompt")));
}

#[test]
fn sessions_are_sorted_newest_first_by_start_time() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    let sessions = h.sessions(&ProjectId::from("/home/user/prompt")).unwrap();
    let ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["dddd0001", "dddd0002"]);
    assert!(sessions[0].started_at > sessions[1].started_at);
}

#[test]
fn sessions_carry_their_summary_fields() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    let sessions = h.sessions(&ProjectId::from("/home/user/alpha")).unwrap();
    let s = &sessions[0];
    assert_eq!(s.id.as_str(), "aaaa0001");
    assert_eq!(s.harness, HarnessId::Claude);
    assert_eq!(s.project.as_str(), "/home/user/alpha");
    assert_eq!(s.cwd.as_deref(), Some(Path::new("/home/user/alpha")));
    assert_eq!(s.started_at, utc("2026-03-01T09:00:00Z"));
    assert_eq!(s.last_activity, utc("2026-03-01T09:05:00Z"));
    assert_eq!(s.path, data.join("-home-user-alpha").join("aaaa0001.jsonl"));
}

#[test]
fn subagent_transcripts_are_not_sessions() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    let sessions = all_sessions(&h);
    assert_eq!(sessions.len(), SESSION_FILES);
    assert!(
        sessions
            .iter()
            .all(|s| !s.id.as_str().starts_with("agent-"))
    );
    assert!(
        sessions
            .iter()
            .all(|s| !s.path.components().any(|c| c.as_os_str() == "subagents"))
    );
    // The subagent's (newer) timestamps do not leak into the project.
    let projects = h.projects().unwrap();
    assert_eq!(
        project(&projects, "/home/user/alpha").last_activity,
        utc("2026-03-01T09:05:00Z")
    );
}

#[test]
fn a_zero_byte_transcript_is_listed_under_its_directory_name_as_untitled() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    let projects = h.projects().unwrap();
    let p = project(&projects, "-home-user-empty");
    assert_eq!(p.name, "-home-user-empty");
    assert_eq!(p.cwd, None);
    assert_eq!(p.last_activity, utc("2026-02-01T00:00:00Z"));
    let sessions = h.sessions(&p.id).unwrap();
    assert_eq!(sessions.len(), 1);
    let s = &sessions[0];
    assert_eq!(s.id.as_str(), "ffff0001");
    assert_eq!(s.title, "(untitled)");
    assert_eq!(s.first_prompt, None);
    assert_eq!(s.cwd, None);
    assert_eq!(s.started_at, utc("2026-02-01T00:00:00Z"));
    assert_eq!(s.last_activity, utc("2026-02-01T00:00:00Z"));
}

#[test]
fn one_build_reads_each_file_exactly_once() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    assert_eq!(h.scan_count(), 0);
    let projects = h.projects().unwrap();
    for p in &projects {
        h.sessions(&p.id).unwrap();
    }
    assert_eq!(h.scan_count(), SESSION_FILES);
    // Reading the lists again never rescans.
    h.projects().unwrap();
    h.sessions(&projects[0].id).unwrap();
    assert_eq!(h.scan_count(), SESSION_FILES);
    let stats = h.last_build();
    assert_eq!(stats.files, SESSION_FILES);
    assert_eq!(stats.scanned, SESSION_FILES);
    assert_eq!(stats.reused, 0);
}

#[test]
fn sessions_alone_also_builds_once() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    h.sessions(&ProjectId::from("/home/user/alpha")).unwrap();
    h.projects().unwrap();
    assert_eq!(h.scan_count(), SESSION_FILES);
}

#[test]
fn a_second_build_without_changes_reads_nothing() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    let before = all_sessions(&h);
    h.refresh().unwrap();
    assert_eq!(h.scan_count(), SESSION_FILES);
    let stats = h.last_build();
    assert_eq!((stats.scanned, stats.reused, stats.dropped), (0, 8, 0));
    assert_eq!(stats.workers, 0);
    assert_eq!(all_sessions(&h), before);
}

#[test]
fn appending_to_one_file_rescans_only_that_file() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    h.refresh().unwrap();
    let path = data.join("-home-user-alpha/aaaa0001.jsonl");
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    writeln!(
        file,
        "{{\"type\":\"assistant\",\"cwd\":\"/home/user/alpha\",\"timestamp\":\"2026-03-07T10:00:00.000Z\"}}"
    )
    .unwrap();
    writeln!(file, "{{\"type\":\"ai-title\",\"aiTitle\":\"Renamed\"}}").unwrap();
    drop(file);
    set_mtime(&path, "2026-03-07T10:00:00Z");

    h.refresh().unwrap();
    assert_eq!(h.scan_count(), SESSION_FILES + 1);
    assert_eq!(h.last_build().scanned, 1);
    assert_eq!(h.last_build().reused, SESSION_FILES - 1);

    let projects = h.projects().unwrap();
    assert_eq!(projects[0].id.as_str(), "/home/user/alpha");
    let s = &h.sessions(&projects[0].id).unwrap()[0];
    assert_eq!(s.title, "Renamed");
    assert_eq!(s.last_activity, utc("2026-03-07T10:00:00Z"));
    assert_eq!(s.started_at, utc("2026-03-01T09:00:00Z"));
}

#[test]
fn a_changed_mtime_alone_rescans_the_file() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    h.refresh().unwrap();
    set_mtime(
        &data.join("-home-user-multi/cccc0001.jsonl"),
        "2026-03-10T00:00:00Z",
    );
    h.refresh().unwrap();
    assert_eq!(h.scan_count(), SESSION_FILES + 1);
}

#[test]
fn a_new_file_is_scanned_and_listed() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    h.refresh().unwrap();
    let dir = data.join("-home-user-new");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("nnnn0001.jsonl"),
        "{\"type\":\"user\",\"cwd\":\"/home/user/new\",\"timestamp\":\"2026-03-20T00:00:00Z\",\"message\":{\"content\":\"hello\"}}\n",
    )
    .unwrap();
    h.refresh().unwrap();
    assert_eq!(h.scan_count(), SESSION_FILES + 1);
    let projects = h.projects().unwrap();
    assert_eq!(projects.len(), 7);
    assert_eq!(projects[0].id.as_str(), "/home/user/new");
}

#[test]
fn a_deleted_file_loses_its_session() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    assert_eq!(all_sessions(&h).len(), SESSION_FILES);
    fs::remove_file(data.join("-home-user-my.app/bbbb0002.jsonl")).unwrap();
    h.refresh().unwrap();
    assert_eq!(h.scan_count(), SESSION_FILES);
    assert_eq!(h.last_build().dropped, 1);
    let sessions = all_sessions(&h);
    assert_eq!(sessions.len(), SESSION_FILES - 1);
    assert!(sessions.iter().all(|s| s.id.as_str() != "bbbb0002"));
    let app = h.sessions(&ProjectId::from("/home/user/my.app")).unwrap();
    assert_eq!(app.len(), 1);
    assert_eq!(app[0].id.as_str(), "bbbb0001");
    assert_eq!(
        project(&h.projects().unwrap(), "/home/user/my.app").last_activity,
        utc("2026-03-03T10:10:00Z")
    );

    // A project whose only file is gone disappears.
    fs::remove_file(data.join("-home-user-alpha/aaaa0001.jsonl")).unwrap();
    h.refresh().unwrap();
    assert!(
        h.projects()
            .unwrap()
            .iter()
            .all(|p| p.id.as_str() != "/home/user/alpha")
    );
}

#[test]
fn a_file_that_comes_back_unchanged_is_scanned_again() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    h.refresh().unwrap();
    let path = data.join("-home-user-multi/cccc0001.jsonl");
    let bytes = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    h.refresh().unwrap();
    // Dropped results are forgotten, so the file is read afresh.
    fs::write(&path, bytes).unwrap();
    set_mtime(&path, "2026-03-02T08:01:00Z");
    h.refresh().unwrap();
    assert_eq!(h.scan_count(), SESSION_FILES + 1);
}

#[test]
fn scanning_uses_several_threads() {
    let (_tmp, data) = fixture();
    // Each scan waits (up to 5 s) until two scans have been in flight at once, so the
    // maximum concurrency only reaches 2 when two workers run at once.
    let in_flight = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (f, p) = (Arc::clone(&in_flight), Arc::clone(&peak));
    let h = harness(&data).with_max_workers(4).with_scan_hook(move |_| {
        let now = f.fetch_add(1, Ordering::SeqCst) + 1;
        p.fetch_max(now, Ordering::SeqCst);
        let start = Instant::now();
        while p.load(Ordering::SeqCst) < 2 && start.elapsed() < Duration::from_secs(5) {
            std::thread::yield_now();
        }
        f.fetch_sub(1, Ordering::SeqCst);
    });
    h.refresh().unwrap();
    assert!(peak.load(Ordering::SeqCst) >= 2, "scans never overlapped");
    assert_eq!(h.last_build().workers, 4);
    assert_eq!(h.scan_count(), SESSION_FILES);
}

#[test]
fn the_worker_count_never_exceeds_the_files_to_scan() {
    let (_tmp, data) = fixture();
    let h = harness(&data).with_max_workers(64);
    h.refresh().unwrap();
    assert_eq!(h.last_build().workers, SESSION_FILES);

    let single = harness(&data).with_max_workers(1);
    single.refresh().unwrap();
    assert_eq!(single.last_build().workers, 1);
}

#[test]
fn the_default_worker_count_follows_available_parallelism() {
    let (_tmp, data) = fixture();
    let h = harness(&data);
    h.refresh().unwrap();
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    assert_eq!(h.last_build().workers, cores.min(SESSION_FILES));
}
