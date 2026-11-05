//! Fixed index data for the selector's tests: fixed times, a fixed clock and
//! UTC timestamps, so nothing depends on the machine.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};

use super::{IndexUpdate, Msg};
use crate::index::{IndexEvent, SessionMap};
use crate::model::{
    HarnessId, Project, ProjectId, SessionId, SessionStats, SessionSummary, TokenUsage,
};

/// The time the tests' clock shows.
pub fn now() -> DateTime<Utc> {
    at(2026, 10, 30, 12, 0, 0)
}

pub fn at(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, min, sec)
        .single()
        .unwrap()
}

pub fn project(harness: HarnessId, name: &str, last_activity: DateTime<Utc>) -> Project {
    let cwd = PathBuf::from(format!("/home/jacob/Development/projects/{name}"));
    Project {
        id: ProjectId::from(cwd.display().to_string()),
        harness,
        name: name.to_owned(),
        cwd: Some(cwd),
        last_activity,
        session_count: 0,
    }
}

pub fn session(
    project: &Project,
    id: &str,
    title: &str,
    started_at: DateTime<Utc>,
    last_activity: DateTime<Utc>,
) -> SessionSummary {
    SessionSummary {
        id: SessionId::from(id),
        harness: project.harness,
        project: project.id.clone(),
        cwd: project.cwd.clone(),
        title: title.to_owned(),
        first_prompt: None,
        started_at,
        last_activity,
        path: PathBuf::from(format!("/sessions/{id}.jsonl")),
        idle: None,
    }
}

/// The 6 projects of the screen snapshots, newest activity first.
pub fn projects(harness: HarnessId) -> Vec<Project> {
    [
        ("Agentmon", at(2026, 10, 30, 11, 59, 50)),
        ("BRIO", at(2026, 10, 28, 9, 30, 0)),
        ("Calira", at(2026, 10, 1, 12, 45, 0)),
        ("CameraSettings", at(2026, 9, 22, 14, 12, 0)),
        ("FactoryFight", at(2026, 1, 12, 17, 10, 0)),
        ("Nayra", at(2025, 6, 12, 8, 35, 0)),
    ]
    .into_iter()
    .map(|(name, last)| project(harness, name, last))
    .collect()
}

/// The 5 sessions of the first fixture project, newest first. The newest is
/// active (its last activity is 10 seconds before [`now`]).
pub fn first_project_sessions(project: &Project) -> Vec<SessionSummary> {
    let titles = [
        (
            "s1",
            "Fix the flaky login test",
            at(2026, 10, 30, 11, 40, 0),
            at(2026, 10, 30, 11, 59, 50),
        ),
        (
            "s2",
            "Refactor the session index worker",
            at(2026, 10, 28, 9, 15, 0),
            at(2026, 10, 28, 9, 30, 0),
        ),
        (
            "s3",
            "(untitled)",
            at(2026, 10, 12, 17, 2, 0),
            at(2026, 10, 12, 17, 20, 0),
        ),
        (
            "s4",
            "Add dark mode to the settings screen",
            at(2026, 9, 22, 14, 12, 0),
            at(2026, 9, 22, 15, 0, 0),
        ),
        (
            "s5",
            "Investigate the memory spike in the transcript parser",
            at(2026, 6, 12, 8, 35, 0),
            at(2026, 6, 12, 9, 0, 0),
        ),
    ];
    titles
        .into_iter()
        .map(|(id, title, start, last)| session(project, id, title, start, last))
        .collect()
}

/// `count` sessions of `project`, newest first, one day apart, long ago.
pub fn many_sessions(project: &Project, count: usize) -> Vec<SessionSummary> {
    (0..count)
        .map(|n| {
            let start = at(2026, 5, 1, 10, 0, 0) - chrono::Duration::days(n as i64);
            session(
                project,
                &format!("m{n:02}"),
                &format!("Session number {n}"),
                start,
                start + chrono::Duration::minutes(5),
            )
        })
        .collect()
}

/// A snapshot of the 6 fixture projects: the first has 5 sessions, the
/// others one each.
pub fn fixture_snapshot(harness: HarnessId) -> IndexEvent {
    let projects = projects(harness);
    let mut sessions = SessionMap::new();
    sessions.insert(projects[0].id.clone(), first_project_sessions(&projects[0]));
    for (n, project) in projects.iter().enumerate().skip(1) {
        let session = session(
            project,
            &format!("other{n}"),
            &format!("A session of {}", project.name),
            project.last_activity - chrono::Duration::minutes(30),
            project.last_activity,
        );
        sessions.insert(project.id.clone(), vec![session]);
    }
    snapshot_event(harness, true, projects, sessions)
}

pub fn snapshot_event(
    harness: HarnessId,
    available: bool,
    projects: Vec<Project>,
    sessions: SessionMap,
) -> IndexEvent {
    IndexEvent::Snapshot {
        harness,
        available,
        projects: projects.into(),
        sessions: sessions.into(),
    }
}

/// Wraps an event as the message `GlobalEvents` makes of it, with the next
/// sequence number.
pub fn message(event: IndexEvent) -> Msg {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    Msg::Index(IndexUpdate::new(SEQ.fetch_add(1, Ordering::Relaxed), event))
}

pub fn stats_event(harness: HarnessId, session: &str, turns: usize, secs: u64) -> IndexEvent {
    IndexEvent::Stats {
        harness,
        session: SessionId::from(session),
        stats: SessionStats {
            turns,
            agent_time: Duration::from_secs(secs),
            run_time: Duration::ZERO,
            tokens: TokenUsage::default(),
        },
    }
}

/// The stats of the worked example (SPEC §4.2): 81h 33m in all and 17h 56m
/// of agent time, 15.8k input, 6.3M output (2.2M of it thinking), 862.3M read
/// from and 30.7M written to the cache.
pub fn worked_example_stats() -> SessionStats {
    SessionStats {
        turns: 6,
        agent_time: Duration::from_secs(64_560),
        run_time: Duration::from_secs(293_580),
        tokens: TokenUsage {
            input: 15_800,
            output: 6_300_000,
            reasoning: 2_200_000,
            cache_read: 862_300_000,
            cache_write: 30_700_000,
        },
    }
}

/// An event that brings `stats` for a session.
pub fn full_stats_event(harness: HarnessId, session: &str, stats: SessionStats) -> IndexEvent {
    IndexEvent::Stats {
        harness,
        session: SessionId::from(session),
        stats,
    }
}

/// A project of `harness` with an explicit working directory.
pub fn project_in(
    harness: HarnessId,
    name: &str,
    cwd: &str,
    last_activity: DateTime<Utc>,
) -> Project {
    Project {
        id: ProjectId::from(cwd),
        harness,
        name: name.to_owned(),
        cwd: Some(PathBuf::from(cwd)),
        last_activity,
        session_count: 0,
    }
}

/// A snapshot of `projects`, each with one session `<name>-1`.
pub fn snapshot_of(harness: HarnessId, projects: Vec<Project>) -> IndexEvent {
    snapshot_with_availability(harness, true, projects)
}

/// Like [`snapshot_of`], for a harness that says it is not available.
pub fn unavailable_snapshot_of(harness: HarnessId, projects: Vec<Project>) -> IndexEvent {
    snapshot_with_availability(harness, false, projects)
}

fn snapshot_with_availability(
    harness: HarnessId,
    available: bool,
    projects: Vec<Project>,
) -> IndexEvent {
    let mut sessions = SessionMap::new();
    for project in &projects {
        let id = format!("{}-1", project.name);
        let title = format!("A session of {}", project.name);
        let last = project.last_activity;
        let one = session(
            project,
            &id,
            &title,
            last - chrono::Duration::minutes(30),
            last,
        );
        sessions.insert(project.id.clone(), vec![one]);
    }
    snapshot_event(harness, available, projects, sessions)
}

/// The 12 projects of the preselection snapshots, newest activity first. The
/// 9th is `Quill`, in `/home/jacob/Development/projects/Quill`.
pub fn twelve_projects(harness: HarnessId) -> Vec<Project> {
    [
        "Agentmon",
        "BRIO",
        "Calira",
        "CameraSettings",
        "FactoryFight",
        "Nayra",
        "Orbit",
        "Parsley",
        "Quill",
        "Relay",
        "Sundial",
        "Tessera",
    ]
    .into_iter()
    .enumerate()
    .map(|(n, name)| {
        project(
            harness,
            name,
            at(2026, 10, 30, 11, 0, 0) - chrono::Duration::days(n as i64),
        )
    })
    .collect()
}

/// Like [`fixture_snapshot`], with sessions for the search tests spread over
/// the projects. `brio` fuzzy-matches (as `b`, `r`, `i`, `o` in order) the
/// session titles and first prompts below, and the name of the project
/// `BRIO`; no other session has a `b` followed by an `r`, an `i` and an `o`.
///
/// - `Agentmon` (5 sessions, `s1` the newest and active): `s2` by its title,
///   `s3` by its first prompt.
/// - `BRIO` (2 sessions): both by the project's name.
/// - `Calira` (2 sessions): `c2` by its title.
/// - The other projects: one session each, none of which matches.
pub fn search_snapshot(harness: HarnessId) -> IndexEvent {
    let projects = projects(harness);
    let mut sessions = SessionMap::new();
    let mut agentmon = first_project_sessions(&projects[0]);
    agentmon[1].title = "Brio importer: handle empty files".to_owned();
    agentmon[2].first_prompt = Some("can you debug the brio ingest step".to_owned());
    sessions.insert(projects[0].id.clone(), agentmon);
    let brio = [
        ("b1", "Plan the sprint", at(2026, 10, 27, 9, 0, 0)),
        ("b2", "Tidy the backlog", at(2026, 10, 20, 15, 30, 0)),
    ];
    sessions.insert(
        projects[1].id.clone(),
        brio.into_iter()
            .map(|(id, title, start)| {
                session(
                    &projects[1],
                    id,
                    title,
                    start,
                    start + chrono::Duration::hours(1),
                )
            })
            .collect(),
    );
    let calira = [
        (
            "c1",
            "Update the camera firmware notes",
            at(2026, 9, 30, 10, 0, 0),
        ),
        ("c2", "Bring up the old pipeline", at(2026, 9, 12, 8, 45, 0)),
    ];
    sessions.insert(
        projects[2].id.clone(),
        calira
            .into_iter()
            .map(|(id, title, start)| {
                session(
                    &projects[2],
                    id,
                    title,
                    start,
                    start + chrono::Duration::hours(1),
                )
            })
            .collect(),
    );
    for (n, project) in projects.iter().enumerate().skip(3) {
        let start = project.last_activity - chrono::Duration::minutes(30);
        let one = session(
            project,
            &format!("other{n}"),
            &format!("Tune the {} settings", project.name),
            start,
            project.last_activity,
        );
        sessions.insert(project.id.clone(), vec![one]);
    }
    snapshot_event(harness, true, projects, sessions)
}

/// 50 projects of Claude holding `count` sessions between them, with
/// first prompts of a realistic length (a few hundred characters).
pub fn big_data(count: usize) -> (Vec<Project>, SessionMap) {
    let base = at(2026, 1, 1, 0, 0, 0);
    let projects: Vec<Project> = (0..50)
        .map(|n| project(HarnessId::Claude, &format!("project-{n:02}"), base))
        .collect();
    let mut sessions = SessionMap::new();
    for n in 0..count {
        let p = &projects[n % projects.len()];
        let started = base + chrono::Duration::minutes(n as i64);
        let mut s = session(
            p,
            &format!("session-{n}"),
            &format!("Investigate the flaky test number {n} in the importer"),
            started,
            started,
        );
        s.first_prompt = Some(format!(
            "Please look at failure {n}: the nightly run of the importer \
             crashes with a timeout after about {} seconds when the cache \
             is cold. Check the retry logic in the worker, the way the \
             backoff is computed, and whether the lock file is released \
             on every error path. Also compare with how the exporter \
             handles the same situation, and write down what differs.",
            n % 97
        ));
        sessions.entry(p.id.clone()).or_default().push(s);
    }
    (projects, sessions)
}
