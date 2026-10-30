//! Fixed index data for the selector's tests: fixed times, a fixed clock and
//! UTC timestamps, so nothing depends on the machine.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};

use super::{IndexUpdate, Msg};
use crate::index::{IndexEvent, SessionMap};
use crate::model::{HarnessId, Project, ProjectId, SessionId, SessionStats, SessionSummary};

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
            duration: Duration::from_secs(secs),
        },
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
    snapshot_event(harness, true, projects, sessions)
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
