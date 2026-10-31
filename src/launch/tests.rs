//! The resolution of the command line against fixture indexes.

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};

use super::*;

const CLAUDE: HarnessId = HarnessId::Claude;
const OPENCODE: HarnessId = HarnessId::OpenCode;
const BOTH: [HarnessId; 2] = [CLAUDE, OPENCODE];

fn at(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0)
        .single()
        .unwrap()
}

fn project(harness: HarnessId, name: &str, cwd: &str) -> Project {
    Project {
        id: ProjectId::from(cwd),
        harness,
        name: name.to_owned(),
        cwd: Some(PathBuf::from(cwd)),
        last_activity: at(1, 0),
        session_count: 0,
    }
}

fn session(project: &Project, id: &str, started_at: DateTime<Utc>) -> SessionSummary {
    SessionSummary {
        id: SessionId::from(id),
        harness: project.harness,
        project: project.id.clone(),
        cwd: project.cwd.clone(),
        title: format!("Session {id}"),
        first_prompt: None,
        started_at,
        last_activity: started_at,
        path: PathBuf::from(format!("/sessions/{id}.jsonl")),
        idle: None,
    }
}

/// A project and the ids and start times of its sessions.
type Listing<'a> = (Project, Vec<(&'a str, DateTime<Utc>)>);

fn index_of(harness: HarnessId, projects: Vec<Listing<'_>>) -> HarnessIndex {
    let mut sessions = SessionMap::new();
    let mut listed = Vec::new();
    for (project, mut ids) in projects {
        ids.sort_by_key(|(_, started)| std::cmp::Reverse(*started));
        let summaries = ids
            .into_iter()
            .map(|(id, started)| session(&project, id, started))
            .collect();
        sessions.insert(project.id.clone(), summaries);
        listed.push(project);
    }
    HarnessIndex {
        harness,
        available: true,
        projects: Arc::new(listed),
        sessions: Arc::new(sessions),
    }
}

/// Claude: `BRIO` (`/work/brio`, sessions `85a86657`, `85a8aaaa`, `b2b2b2b2`),
/// `Tools` (`/work/brio/tools`, `c3c3c3c3`) and `Docs` (`/work/docs`,
/// `85a90000`). OpenCode: `Brio` (`/work/brio`, `ses_85a8zzzz`, `ses_new`)
/// and `Other` (`/other`, `ses_other`).
fn index() -> Vec<HarnessIndex> {
    vec![
        index_of(
            CLAUDE,
            vec![
                (
                    project(CLAUDE, "BRIO", "/work/brio"),
                    vec![
                        ("85a86657", at(20, 9)),
                        ("85a8aaaa", at(21, 9)),
                        ("b2b2b2b2", at(22, 9)),
                    ],
                ),
                (
                    project(CLAUDE, "Tools", "/work/brio/tools"),
                    vec![("c3c3c3c3", at(10, 9))],
                ),
                (
                    project(CLAUDE, "Docs", "/work/docs"),
                    vec![("85a90000", at(5, 9))],
                ),
            ],
        ),
        index_of(
            OPENCODE,
            vec![
                (
                    project(OPENCODE, "Brio", "/work/brio"),
                    vec![("ses_85a8zzzz", at(23, 9)), ("ses_new", at(25, 9))],
                ),
                (
                    project(OPENCODE, "Other", "/other"),
                    vec![("ses_other", at(24, 9))],
                ),
            ],
        ),
    ]
}

fn ask(request: &Request, dir: &str) -> Result<Resolution, LaunchError> {
    resolve(request, Path::new(dir), &BOTH, &index())
}

fn id_request(target: &str) -> Request {
    Request {
        target: Some(target.to_owned()),
        ..Request::default()
    }
}

fn opened(resolution: &Resolution) -> (HarnessId, &str) {
    match &resolution.target {
        Target::Open(harness, summary) => (*harness, summary.id.as_str()),
        other => panic!("{other:?}"),
    }
}

// -- no arguments

#[test]
fn no_arguments_start_the_selector_as_before() {
    let resolution = ask(&Request::default(), "/work/brio").unwrap();
    assert_eq!(resolution.target, Target::Selector);
    assert_eq!(resolution.preselect, Preselect::default());
    assert!(resolution.warnings.is_empty());
}

// -- ids

#[test]
fn an_exact_id_opens_that_session() {
    let resolution = ask(&id_request("b2b2b2b2"), "/nowhere").unwrap();
    assert_eq!(opened(&resolution), (CLAUDE, "b2b2b2b2"));
}

#[test]
fn an_exact_id_wins_over_longer_ids_that_start_with_it() {
    let mut claude = index();
    let brio = claude[0].projects[0].clone();
    let mut sessions = (*claude[0].sessions).clone();
    sessions
        .get_mut(&brio.id)
        .unwrap()
        .push(session(&brio, "85a86657-extra", at(1, 1)));
    claude[0].sessions = Arc::new(sessions);
    let resolution = resolve(&id_request("85a86657"), Path::new("/"), &BOTH, &claude).unwrap();
    assert_eq!(opened(&resolution), (CLAUDE, "85a86657"));
}

#[test]
fn a_unique_prefix_opens_that_session() {
    let resolution = ask(&id_request("b2b2"), "/nowhere").unwrap();
    assert_eq!(opened(&resolution), (CLAUDE, "b2b2b2b2"));
    let resolution = ask(&id_request("85a86"), "/nowhere").unwrap();
    assert_eq!(opened(&resolution), (CLAUDE, "85a86657"));
}

#[test]
fn a_unique_prefix_is_found_in_any_harness() {
    let resolution = ask(&id_request("ses_oth"), "/nowhere").unwrap();
    assert_eq!(opened(&resolution), (OPENCODE, "ses_other"));
}

#[test]
fn an_ambiguous_prefix_filters_the_selector_to_exactly_the_matches() {
    let resolution = ask(&id_request("85a8"), "/nowhere").unwrap();
    assert_eq!(
        resolution.target,
        Target::Filtered {
            prefix: "85a8".to_owned(),
            matches: vec![
                (CLAUDE, SessionId::from("85a8aaaa")),
                (CLAUDE, SessionId::from("85a86657")),
            ],
        }
    );
}

#[test]
fn an_ambiguous_prefix_matches_across_harnesses_lowest_number_first() {
    let mut claude = index();
    let other = claude[1].projects[0].clone();
    let mut sessions = (*claude[1].sessions).clone();
    sessions
        .get_mut(&other.id)
        .unwrap()
        .push(session(&other, "85a8opencode", at(26, 9)));
    claude[1].sessions = Arc::new(sessions);
    let resolution = resolve(&id_request("85a8"), Path::new("/"), &BOTH, &claude).unwrap();
    let Target::Filtered { matches, .. } = resolution.target else {
        panic!("not filtered");
    };
    let harnesses: Vec<HarnessId> = matches.iter().map(|(h, _)| *h).collect();
    assert_eq!(harnesses, [CLAUDE, CLAUDE, OPENCODE]);
}

#[test]
fn no_match_is_an_error_that_names_the_argument() {
    let err = ask(&id_request("zzzz"), "/nowhere").unwrap_err();
    assert_eq!(err, LaunchError::NoSessionMatches("zzzz".to_owned()));
    assert_eq!(err.to_string(), "no session matches \"zzzz\"");
}

#[test]
fn an_id_matches_from_its_start_only() {
    assert!(ask(&id_request("6657"), "/nowhere").is_err());
}

#[test]
fn harness_opencode_restricts_the_lookup() {
    let only = |target: &str, harness| Request {
        harness: Some(harness),
        ..id_request(target)
    };
    // `b2b2` is Claude's: with --harness opencode it is not found...
    let err = ask(&only("b2b2", OPENCODE), "/nowhere").unwrap_err();
    assert_eq!(err, LaunchError::NoSessionMatches("b2b2".to_owned()));
    let resolution = ask(&only("b2b2", CLAUDE), "/nowhere").unwrap();
    assert_eq!(opened(&resolution), (CLAUDE, "b2b2b2b2"));
    // ...and `ses_` matches several of OpenCode's, but none of Claude's.
    let resolution = ask(&only("ses_", OPENCODE), "/nowhere").unwrap();
    assert!(matches!(resolution.target, Target::Filtered { .. }));
    assert!(ask(&only("ses_", CLAUDE), "/nowhere").is_err());
}

#[test]
fn harness_restricts_an_ambiguity_to_one_harness() {
    let mut both = index();
    let other = both[1].projects[0].clone();
    let mut sessions = (*both[1].sessions).clone();
    sessions
        .get_mut(&other.id)
        .unwrap()
        .push(session(&other, "85a8opencode", at(26, 9)));
    both[1].sessions = Arc::new(sessions);
    let request = Request {
        harness: Some(OPENCODE),
        ..id_request("85a8")
    };
    let resolution = resolve(&request, Path::new("/"), &BOTH, &both).unwrap();
    assert_eq!(opened(&resolution), (OPENCODE, "85a8opencode"));
}

#[test]
fn an_empty_argument_matches_nothing() {
    assert!(ask(&id_request(""), "/nowhere").is_err());
}

// -- paths

fn temp_file(name: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    fs::write(&path, "{}\n").unwrap();
    (dir, path)
}

#[test]
fn a_jsonl_file_opens_directly_wherever_it_is() {
    let (_dir, path) = temp_file("1234.jsonl");
    let resolution = ask(&id_request(path.to_str().unwrap()), "/nowhere").unwrap();
    assert_eq!(resolution.target, Target::File(path));
}

#[test]
fn only_a_positional_argument_latest_or_project_needs_the_index() {
    let (_dir, path) = temp_file("1234.jsonl");
    assert!(id_request(path.to_str().unwrap()).needs_index());
    assert!(id_request("85a8").needs_index());
    assert!(latest(None, None).needs_index());
    assert!(project_request("brio", None).needs_index());
    assert!(!Request::default().needs_index());
    let harness_only = Request {
        harness: Some(OPENCODE),
        ..Request::default()
    };
    assert!(!harness_only.needs_index());
}

#[test]
fn a_missing_jsonl_path_is_an_error() {
    let err = ask(&id_request("/no/such/dir/1234.jsonl"), "/nowhere").unwrap_err();
    assert_eq!(
        err,
        LaunchError::NoSuchFile(PathBuf::from("/no/such/dir/1234.jsonl"))
    );
    assert_eq!(err.to_string(), "no such file: /no/such/dir/1234.jsonl");
}

#[test]
fn an_existing_file_that_is_not_jsonl_is_an_error() {
    let (_dir, path) = temp_file("notes.txt");
    let err = ask(&id_request(path.to_str().unwrap()), "/nowhere").unwrap_err();
    assert_eq!(err, LaunchError::NotATranscript(path));
}

#[test]
fn a_directory_is_an_id_not_a_path() {
    let dir = tempfile::tempdir().unwrap();
    let err = ask(&id_request(dir.path().to_str().unwrap()), "/nowhere").unwrap_err();
    assert!(matches!(err, LaunchError::NoSessionMatches(_)), "{err:?}");
}

#[test]
fn a_path_is_a_claude_session() {
    let (_dir, path) = temp_file("1234.jsonl");
    let request = Request {
        harness: Some(OPENCODE),
        ..id_request(path.to_str().unwrap())
    };
    assert_eq!(
        ask(&request, "/").unwrap_err(),
        LaunchError::PathNeedsClaude(OPENCODE)
    );
    let request = id_request(path.to_str().unwrap());
    let err = resolve(&request, Path::new("/"), &[OPENCODE], &[]).unwrap_err();
    assert_eq!(err, LaunchError::ClaudeDisabled);
}

// -- --latest

fn latest(project: Option<&str>, harness: Option<HarnessId>) -> Request {
    Request {
        latest: true,
        project: project.map(str::to_owned),
        harness,
        ..Request::default()
    }
}

#[test]
fn latest_opens_the_newest_session_of_the_working_directory_project() {
    // BRIO (Claude) and Brio (OpenCode) share the directory: the newest of
    // both is OpenCode's `ses_new`.
    let resolution = ask(&latest(None, None), "/work/brio").unwrap();
    assert_eq!(opened(&resolution), (OPENCODE, "ses_new"));
}

#[test]
fn latest_uses_the_deepest_project_that_contains_the_directory() {
    let resolution = ask(&latest(None, None), "/work/brio/tools/src").unwrap();
    assert_eq!(opened(&resolution), (CLAUDE, "c3c3c3c3"));
    let resolution = ask(&latest(None, None), "/work/brio/other").unwrap();
    assert_eq!(opened(&resolution), (OPENCODE, "ses_new"));
}

#[test]
fn latest_with_a_harness_stays_in_it() {
    let resolution = ask(&latest(None, Some(CLAUDE)), "/work/brio").unwrap();
    assert_eq!(opened(&resolution), (CLAUDE, "b2b2b2b2"));
}

#[test]
fn latest_with_a_project_ignores_the_working_directory() {
    let resolution = ask(&latest(Some("/work/docs"), None), "/work/brio").unwrap();
    assert_eq!(opened(&resolution), (CLAUDE, "85a90000"));
    let resolution = ask(&latest(Some("other"), None), "/work/brio").unwrap();
    assert_eq!(opened(&resolution), (OPENCODE, "ses_other"));
}

#[test]
fn latest_without_a_project_is_an_error() {
    let err = ask(&latest(None, None), "/elsewhere").unwrap_err();
    assert_eq!(err, LaunchError::NoProject("for /elsewhere".to_owned()));
    let err = ask(&latest(Some("nope"), None), "/work/brio").unwrap_err();
    assert_eq!(err, LaunchError::NoProject("matches \"nope\"".to_owned()));
    // The directory has a project in OpenCode only.
    let err = ask(&latest(None, Some(CLAUDE)), "/other").unwrap_err();
    assert!(matches!(err, LaunchError::NoProject(_)), "{err:?}");
}

#[test]
fn latest_in_a_project_without_sessions_is_an_error() {
    let mut empty = index();
    empty[0].sessions = Arc::new(SessionMap::new());
    let request = latest(None, Some(CLAUDE));
    let err = resolve(&request, Path::new("/work/docs"), &BOTH, &empty).unwrap_err();
    assert_eq!(err, LaunchError::NoSessions("Docs".to_owned()));
}

// -- --project

fn project_request(spec: &str, harness: Option<HarnessId>) -> Request {
    Request {
        project: Some(spec.to_owned()),
        harness,
        ..Request::default()
    }
}

fn picked(resolution: &Resolution) -> (HarnessId, &str) {
    let (harness, project) = resolution.preselect.project.as_ref().unwrap();
    (*harness, project.as_str())
}

#[test]
fn a_project_by_absolute_path_is_preselected() {
    let resolution = ask(&project_request("/work/docs", None), "/anywhere").unwrap();
    assert_eq!(picked(&resolution), (CLAUDE, "/work/docs"));
    assert_eq!(resolution.preselect.harness, Some(CLAUDE));
    assert_eq!(resolution.target, Target::Selector);
    assert!(resolution.warnings.is_empty());
}

#[test]
fn a_project_by_relative_path_is_taken_from_the_working_directory() {
    let resolution = ask(&project_request("docs", None), "/work").unwrap();
    assert_eq!(picked(&resolution), (CLAUDE, "/work/docs"));
    let resolution = ask(&project_request("../docs/", None), "/work/brio").unwrap();
    assert_eq!(picked(&resolution), (CLAUDE, "/work/docs"));
    let resolution = ask(&project_request(".", None), "/work/brio/tools").unwrap();
    assert_eq!(picked(&resolution), (CLAUDE, "/work/brio/tools"));
}

#[test]
fn a_path_is_compared_by_components() {
    let resolution = ask(&project_request("/work/doc", None), "/anywhere").unwrap();
    assert_eq!(resolution.preselect.project, None);
    assert_eq!(resolution.warnings.len(), 1);
}

#[test]
fn a_project_by_name_or_alias_is_preselected_case_insensitively() {
    for spec in ["docs", "DOCS", "Docs"] {
        let resolution = ask(&project_request(spec, None), "/anywhere").unwrap();
        assert_eq!(picked(&resolution), (CLAUDE, "/work/docs"), "{spec}");
    }
    // Claude comes first when both harnesses have the project.
    let resolution = ask(&project_request("brio", None), "/anywhere").unwrap();
    assert_eq!(picked(&resolution), (CLAUDE, "/work/brio"));
}

#[test]
fn a_project_with_a_harness_is_looked_up_in_that_harness() {
    let resolution = ask(&project_request("brio", Some(OPENCODE)), "/anywhere").unwrap();
    assert_eq!(picked(&resolution), (OPENCODE, "/work/brio"));
    assert_eq!(resolution.preselect.harness, Some(OPENCODE));
    let resolution = ask(&project_request("docs", Some(OPENCODE)), "/anywhere").unwrap();
    assert_eq!(resolution.preselect.project, None);
    assert_eq!(resolution.preselect.harness, Some(OPENCODE));
    assert_eq!(resolution.warnings.len(), 1);
}

#[test]
fn an_unknown_project_warns_and_falls_back() {
    let resolution = ask(&project_request("nope", None), "/anywhere").unwrap();
    assert_eq!(resolution.preselect, Preselect::default());
    assert_eq!(resolution.target, Target::Selector);
    assert_eq!(
        resolution.warnings,
        ["--project: no project matches \"nope\""]
    );
}

#[test]
fn a_harness_alone_preselects_its_tab() {
    let request = Request {
        harness: Some(OPENCODE),
        ..Request::default()
    };
    let resolution = resolve(&request, Path::new("/"), &BOTH, &[]).unwrap();
    assert_eq!(
        resolution.preselect,
        Preselect {
            harness: Some(OPENCODE),
            project: None
        }
    );
    assert!(resolution.warnings.is_empty());
}

#[test]
fn a_harness_that_is_not_enabled_warns() {
    let request = Request {
        harness: Some(OPENCODE),
        ..Request::default()
    };
    let resolution = resolve(&request, Path::new("/"), &[CLAUDE], &[]).unwrap();
    assert_eq!(resolution.warnings.len(), 1);
    assert!(resolution.warnings[0].contains("OpenCode"));
}

// -- the shared rule

#[test]
fn the_deepest_ancestor_wins_and_the_first_of_equals() {
    let projects = [
        project(CLAUDE, "A", "/a"),
        project(CLAUDE, "AB", "/a/b"),
        project(CLAUDE, "AB again", "/a/b"),
        project(CLAUDE, "AC", "/a/c"),
    ];
    let candidates = projects.iter().map(|p| ((), p));
    let (_, best) = deepest_ancestor(candidates, Path::new("/a/b/c")).unwrap();
    assert_eq!(best.name, "AB");
    let candidates = projects.iter().map(|p| ((), p));
    assert!(deepest_ancestor(candidates, Path::new("/abc")).is_none());
}
