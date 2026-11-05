//! Adapter tests against the fixture database (SPEC §9.6).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, UNIX_EPOCH};

use chrono::{DateTime, TimeZone, Utc};

use super::testing::TempDb;
use super::*;
use crate::config::Config;
use crate::harness::Harness;
use crate::model::{
    HarnessId, Item, ModelUpdate, ProjectId, SessionId, SessionSummary, TokenUsage, ToolKind,
    ToolStatus, Turn,
};

fn at(day: u32, h: u32, m: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, h, m, s)
        .single()
        .unwrap()
}

fn harness(db: &TempDb) -> OpenCodeHarness {
    OpenCodeHarness::with_db_path(db.path.clone(), Default::default())
}

fn sid(id: &str) -> SessionId {
    SessionId::from(id)
}

/// Every update of a load, up to and including `Activity`.
fn load_updates(h: &OpenCodeHarness, id: &str) -> Vec<ModelUpdate> {
    let (tx, rx) = mpsc::channel();
    let _handle = h.load(&sid(id), tx).unwrap();
    let mut out = Vec::new();
    while let Ok(update) = rx.recv_timeout(Duration::from_secs(30)) {
        let last = matches!(update, ModelUpdate::Activity { .. });
        let loaded_without_activity = out.last() == Some(&ModelUpdate::Loaded)
            && !matches!(update, ModelUpdate::Activity { .. });
        out.push(update);
        if last || loaded_without_activity {
            break;
        }
    }
    out
}

fn turns_of(updates: &[ModelUpdate]) -> Vec<Arc<Turn>> {
    let mut turns = Vec::new();
    for update in updates {
        update.apply(&mut turns);
    }
    turns
}

fn load_turns(h: &OpenCodeHarness, id: &str) -> Vec<Arc<Turn>> {
    turns_of(&load_updates(h, id))
}

fn assistant(turn: &Turn) -> &crate::model::AssistantTurn {
    match turn {
        Turn::Assistant(t) => t,
        other => panic!("not an assistant turn: {other:?}"),
    }
}

fn user(turn: &Turn) -> &crate::model::UserTurn {
    match turn {
        Turn::User(t) => t,
        other => panic!("not a user turn: {other:?}"),
    }
}

fn calls(turn: &crate::model::AssistantTurn) -> Vec<&crate::model::ToolCall> {
    turn.items
        .iter()
        .filter_map(|item| match item {
            Item::ToolCall(call) => Some(call),
            _ => None,
        })
        .collect()
}

// -- the read-only helper

#[test]
fn an_insert_through_the_helper_is_rejected_and_nothing_is_written() {
    let db = TempDb::new();
    let before = std::fs::read(&db.path).unwrap();
    let conn = Db::open(&db.path).unwrap();
    for sql in [
        "INSERT INTO migration VALUES ('29990101000000_x', 1)",
        "UPDATE session_v2 SET title = 'x'",
        "DELETE FROM session_message",
        "DROP TABLE project",
        "PRAGMA journal_mode = WAL",
        "  insert into migration values ('x', 1)",
        "WITH x AS (SELECT 1) INSERT INTO migration SELECT 'y', 1",
    ] {
        let err = conn.select(sql, [], |_| Ok(())).unwrap_err();
        assert!(
            err.to_string().contains("refusing"),
            "{sql}: unexpected error {err:#}"
        );
    }
    let count: i64 = conn
        .select_one("SELECT count(*) FROM migration", [], |r| r.get(0))
        .unwrap()
        .unwrap();
    assert_eq!(count, 3);
    drop(conn);
    assert_eq!(std::fs::read(&db.path).unwrap(), before);
}

#[test]
fn a_full_load_leaves_the_database_file_and_its_directory_unchanged() {
    let db = TempDb::new();
    let before = std::fs::read(&db.path).unwrap();
    let files = db.files();
    let h = harness(&db);
    assert!(h.available());
    h.refresh().unwrap();
    for project in h.projects().unwrap() {
        for session in h.sessions(&project.id).unwrap() {
            h.stats(&session).unwrap();
        }
    }
    for id in ["ses_main", "ses_child1", "ses_bad"] {
        load_updates(&h, id);
    }
    h.subagent(&sid("ses_main"), &"call-sub-1".into());
    h.subagent_info(&sid("ses_child1"));
    assert_eq!(std::fs::read(&db.path).unwrap(), before);
    assert_eq!(db.files(), files, "no -wal, -shm or journal file appeared");
}

#[test]
fn a_write_ahead_log_database_is_read_through_its_wal_and_the_main_file_is_unchanged() {
    // OpenCode's real database is in WAL mode, with a writer connection open.
    let db = TempDb::new();
    let writer = rusqlite::Connection::open(&db.path).unwrap();
    writer
        .query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))
        .unwrap();
    // A change that only lives in the -wal file so far.
    writer
        .execute(
            "UPDATE session_v2 SET title = 'Renamed in the wal' WHERE id = 'ses_main'",
            [],
        )
        .unwrap();
    let before = std::fs::read(&db.path).unwrap();
    let files = db.files();
    assert!(files.contains(&"opencode.db-wal".to_owned()), "{files:?}");

    let h = harness(&db);
    assert!(h.available());
    assert_eq!(summary(&h, "ses_main").title, "Renamed in the wal");
    for id in ["ses_main", "ses_child1", "ses_bad"] {
        load_updates(&h, id);
    }
    drop(h);
    assert_eq!(std::fs::read(&db.path).unwrap(), before);
    assert_eq!(db.files(), files, "no side file appeared or went away");
    drop(writer);
}

#[cfg(unix)]
#[test]
fn a_database_that_cannot_be_opened_read_only_is_read_as_immutable() {
    use std::os::unix::fs::PermissionsExt;

    // A write-ahead-log database without its -wal and -shm files, in a
    // directory nobody can write to: the read-only open cannot create them.
    let db = TempDb::new();
    db.exec("PRAGMA journal_mode = WAL;");
    let dir = db.dir().to_path_buf();
    assert!(!dir.join("opencode.db-wal").exists());
    let mut perms = std::fs::metadata(&dir).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&dir, perms.clone()).unwrap();
    let can_write = std::fs::write(dir.join("probe"), b"").is_ok();
    let opened = Db::open(&db.path);
    perms.set_mode(0o755);
    std::fs::set_permissions(&dir, perms).unwrap();
    let opened = opened.unwrap();
    if can_write {
        eprintln!(
            "skipping the Immutable assertion: the directory stays writable (running as root?)"
        );
    } else {
        assert_eq!(opened.mode(), OpenMode::Immutable);
        let err = opened
            .select("DELETE FROM project", [], |_| Ok(()))
            .unwrap_err();
        assert!(err.to_string().contains("refusing"), "{err:#}");
    }
    let count: i64 = opened
        .select_one("SELECT count(*) FROM project", [], |r| r.get(0))
        .unwrap()
        .unwrap();
    assert_eq!(count, 3);
}

#[test]
fn a_file_that_is_not_a_database_is_not_available() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("opencode.db");
    std::fs::write(&path, b"this is not sqlite").unwrap();
    let h = OpenCodeHarness::with_db_path(path, Default::default());
    assert!(!h.available());
    assert!(h.refresh().is_err());
}

#[test]
fn a_missing_database_is_unavailable_with_empty_lists() {
    let tmp = tempfile::tempdir().unwrap();
    let h = OpenCodeHarness::with_db_path(tmp.path().join("none.db"), Default::default());
    assert!(!h.available());
    assert_eq!(h.id(), HarnessId::OpenCode);
    h.refresh().unwrap();
    assert!(h.projects().unwrap().is_empty());
    assert!(h.sessions(&ProjectId::from("x")).unwrap().is_empty());
}

#[test]
fn watch_paths_are_the_database_and_its_wal() {
    let h = OpenCodeHarness::with_db_path(PathBuf::from("/d/opencode.db"), Default::default());
    assert_eq!(
        h.watch_paths(),
        [
            PathBuf::from("/d/opencode.db"),
            PathBuf::from("/d/opencode.db-wal")
        ]
    );
}

#[test]
fn the_config_chooses_the_database() {
    let db = TempDb::new();
    let mut config = Config::default();
    config.harness.opencode.db_path = Some(db.path.clone());
    config.harness.opencode.resume_command = "oc -s {id}".to_owned();
    config
        .projects
        .aliases
        .insert(PathBuf::from("/home/user/alpha"), "Mine".to_owned());
    let h = OpenCodeHarness::new(&config, std::path::Path::new("/home/u"));
    assert_eq!(h.db_path(), db.path);
    assert_eq!(h.projects().unwrap()[0].name, "Mine");
    let command = h.resume_command(&summary(&h, "ses_main")).unwrap();
    assert_eq!(command.get_program(), "oc");
    assert_eq!(command.get_args().collect::<Vec<_>>(), ["-s", "ses_main"]);
}

// -- projects and sessions

#[test]
fn projects_have_a_name_else_the_basename_and_are_sorted_by_activity() {
    let db = TempDb::new();
    let h = harness(&db);
    let projects = h.projects().unwrap();
    let got: Vec<_> = projects
        .iter()
        .map(|p| (p.id.as_str(), p.name.as_str(), p.session_count))
        .collect();
    // `prj_empty` has no session, so it is not listed.
    assert_eq!(
        got,
        [("prj_alpha", "Alpha App", 2), ("prj_beta", "beta-tool", 3)]
    );
    assert!(projects.iter().all(|p| p.harness == HarnessId::OpenCode));
    assert_eq!(projects[0].cwd, Some(PathBuf::from("/home/user/alpha")));
    assert_eq!(
        projects[1].cwd,
        Some(PathBuf::from("/home/user/work/beta-tool"))
    );
    assert_eq!(projects[0].last_activity, at(28, 9, 30, 0));
    assert_eq!(projects[1].last_activity, at(26, 10, 10, 0));
}

#[test]
fn a_configured_alias_beats_the_name_and_the_basename() {
    let db = TempDb::new();
    let aliases = [
        (PathBuf::from("/home/user/alpha"), "Alpha!".to_owned()),
        // Matched by component, so a trailing slash does not matter.
        (
            PathBuf::from("/home/user/work/beta-tool/"),
            "Beta!".to_owned(),
        ),
    ]
    .into_iter()
    .collect();
    let h = OpenCodeHarness::with_db_path(db.path.clone(), aliases);
    let names: Vec<_> = h.projects().unwrap().into_iter().map(|p| p.name).collect();
    assert_eq!(names, ["Alpha!", "Beta!"]);
}

#[test]
fn only_top_level_sessions_are_listed_newest_first() {
    let db = TempDb::new();
    let h = harness(&db);
    let ids = |project: &str| -> Vec<String> {
        h.sessions(&ProjectId::from(project))
            .unwrap()
            .into_iter()
            .map(|s| s.id.to_string())
            .collect()
    };
    // The two child sessions of `ses_main` are not listed.
    assert_eq!(ids("prj_alpha"), ["ses_main", "ses_untitled"]);
    assert_eq!(ids("prj_beta"), ["ses_bad", "ses_long", "ses_blank"]);
    assert!(ids("prj_empty").is_empty());
    assert!(ids("nope").is_empty());
}

fn summary(h: &OpenCodeHarness, id: &str) -> SessionSummary {
    for project in h.projects().unwrap() {
        for session in h.sessions(&project.id).unwrap() {
            if session.id.as_str() == id {
                return session;
            }
        }
    }
    panic!("no session {id}");
}

#[test]
fn session_summaries_carry_times_directory_and_idleness() {
    let db = TempDb::new();
    let h = harness(&db);
    let main = summary(&h, "ses_main");
    assert_eq!(main.title, "Add the settings screen");
    assert_eq!(main.started_at, at(28, 9, 0, 0));
    assert_eq!(main.last_activity, at(28, 9, 30, 0));
    assert_eq!(main.cwd, Some(PathBuf::from("/home/user/alpha")));
    assert_eq!(main.project, ProjectId::from("prj_alpha"));
    assert_eq!(main.harness, HarnessId::OpenCode);
    assert_eq!(main.path, db.path);
    assert_eq!(main.idle, Some(true), "time_idle is set");
    assert_eq!(
        main.first_prompt.as_deref(),
        Some("Add a settings screen with a dark mode toggle.")
    );
    // `time_idle IS NULL`: the session is running.
    assert_eq!(summary(&h, "ses_long").idle, Some(false));
}

#[test]
fn the_title_falls_back_to_the_first_prompt_then_untitled() {
    let db = TempDb::new();
    let h = harness(&db);
    // First non-empty line of the first prompt.
    assert_eq!(summary(&h, "ses_untitled").title, "Rename the config key");
    // A long prompt is cut to 80 characters with an ellipsis.
    let long = summary(&h, "ses_long").title;
    assert_eq!(long.chars().count(), 80);
    assert!(
        long.starts_with("Investigate why the continuous integration"),
        "{long}"
    );
    assert!(long.ends_with('…'));
    // Neither title nor prompt.
    assert_eq!(summary(&h, "ses_blank").title, "(untitled)");
    assert_eq!(summary(&h, "ses_blank").first_prompt, None);
    // A title wins over the prompt.
    assert_eq!(summary(&h, "ses_bad").title, "Broken rows");
}

// -- turn assembly

#[test]
fn users_and_assistants_alternate_with_injected_content_placed_like_claudes() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    let kinds: Vec<&str> = turns
        .iter()
        .map(|t| match &**t {
            Turn::User(_) => "user",
            Turn::Assistant(_) => "assistant",
            Turn::Compaction(_) => "compaction",
        })
        .collect();
    assert_eq!(kinds, ["user", "assistant", "user", "assistant"]);
    assert_eq!(turns[0].id().as_str(), "msg_main_02");
    assert_eq!(turns[1].id().as_str(), "msg_main_04");
    assert_eq!(turns[2].id().as_str(), "msg_main_07");
    assert_eq!(turns[3].id().as_str(), "msg_main_08");

    let first = user(&turns[0]);
    assert_eq!(first.text, "Add a settings screen with a dark mode toggle.");
    assert_eq!(first.timestamp, at(28, 9, 0, 1));
    // The `synthetic` row before the prompt, then the `system` row after it.
    assert_eq!(first.items.len(), 2);
    let Item::Injected { id, entries } = &first.items[0] else {
        panic!("{:?}", first.items[0]);
    };
    assert_eq!(id.as_str(), "injected:msg_main_01");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, "synthetic");
    assert!(
        entries[0]
            .text
            .contains("The user opened the file src/settings.rs")
    );
    let Item::Injected { entries, .. } = &first.items[1] else {
        panic!("{:?}", first.items[1]);
    };
    assert_eq!(entries[0].kind, "system");
    // The description comes first, then the text.
    assert!(
        entries[0]
            .text
            .starts_with("Instructions updated: core/skill-guidance\n\nNew skills are available"),
        "{}",
        entries[0].text
    );
    assert_eq!(entries[0].timestamp, at(28, 9, 0, 2));
    assert_eq!(user(&turns[2]).text, "Now delegate the review.");
}

#[test]
fn idle_rows_make_no_turn_and_no_item() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    // The `idle` row sits between the first assistant turn and the second
    // prompt: nothing of it is anywhere.
    let first = assistant(&turns[1]);
    assert_eq!(first.stats.as_ref().unwrap().message_count, 2);
    assert!(
        first
            .items
            .iter()
            .all(|i| !matches!(i, Item::Injected { .. }))
    );
}

#[test]
fn text_reasoning_and_tool_parts_become_items_in_order() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    let first = assistant(&turns[1]);
    assert_eq!(first.timestamp, at(28, 9, 0, 3));
    assert_eq!(first.model, "acme/coder-1");
    let ids: Vec<&str> = first.items.iter().map(|i| i.id().as_str()).collect();
    // Whitespace-only text parts are dropped.
    assert_eq!(
        ids,
        [
            "msg_main_04:0",
            "msg_main_04:2",
            "call-read-1",
            "call-shell-1",
            "call-shell-2",
            "call-write-1"
        ]
    );
    assert!(matches!(
        &first.items[0],
        Item::Thinking { thinking: crate::model::Thinking::Text(t), .. }
            if t == "I should look at the existing screens first."
    ));
    assert!(matches!(
        &first.items[1],
        Item::Text { text, .. } if text == "Let me look at the repo layout."
    ));
}

#[test]
fn tool_parts_carry_kind_description_input_result_status_and_duration() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    let first = assistant(&turns[1]);
    let calls = calls(first);

    let read = calls[0];
    assert_eq!(read.tool, ToolKind::Read);
    assert_eq!(
        read.description, "Read src/main.rs",
        "relative to the directory"
    );
    assert_eq!(read.input["path"], "/home/user/alpha/src/main.rs");
    assert_eq!(read.status, ToolStatus::Ok);
    assert_eq!(read.duration, Some(Duration::from_secs(1)));
    let text = read
        .result
        .as_ref()
        .unwrap()
        .output
        .text()
        .unwrap()
        .into_owned();
    assert!(
        text.starts_with("Read file /home/user/alpha/src/main.rs"),
        "{text}"
    );

    let shell = calls[1];
    assert_eq!(shell.tool, ToolKind::Shell);
    assert_eq!(
        shell.description, "List the sources",
        "the input's description"
    );
    assert_eq!(shell.status, ToolStatus::Ok);
    let result = shell.result.as_ref().unwrap();
    assert!(!result.is_error);
    assert_eq!(result.output.text().unwrap(), "main.rs\nsettings.rs\n");
    assert_eq!(shell.duration, Some(Duration::from_secs(2)));
}

#[test]
fn a_failed_tool_is_an_error_with_its_content_else_its_error_message() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    let first = assistant(&turns[1]);
    let calls = calls(first);
    let failed = calls[2];
    assert_eq!(failed.tool, ToolKind::Shell);
    assert_eq!(failed.description, "shell");
    assert_eq!(failed.status, ToolStatus::Error);
    let result = failed.result.as_ref().unwrap();
    assert!(result.is_error);
    assert_eq!(result.output.text().unwrap(), "error: test failed\n");
    // Without content parts, `state.error.message` is the result.
    let rejected = calls[3];
    assert_eq!(rejected.tool, ToolKind::Write);
    assert_eq!(rejected.status, ToolStatus::Error);
    let result = rejected.result.as_ref().unwrap();
    assert!(result.is_error);
    assert_eq!(result.output.text().unwrap(), "Permission denied: edit");
}

#[test]
fn a_tool_that_has_not_finished_is_pending_without_a_result() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    let second = assistant(&turns[3]);
    let pending = calls(second).into_iter().last().unwrap();
    assert_eq!(pending.id.as_str(), "call-shell-3");
    assert_eq!(pending.status, ToolStatus::Pending);
    assert!(pending.result.is_none());
    assert_eq!(pending.duration, None);
    assert_eq!(pending.input["command"], "sleep 60");
}

#[test]
fn effort_is_the_variant_unless_it_is_default() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    assert_eq!(assistant(&turns[1]).effort.as_deref(), Some("high"));
    assert_eq!(assistant(&turns[3]).effort, None, "variant `default`");
    let child = load_turns(&harness(&db), "ses_child1");
    assert_eq!(assistant(&child[1]).effort, None);
}

#[test]
fn a_model_change_inside_a_turn_adds_a_note() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    let second = assistant(&turns[3]);
    // The turn's model is the first row's; the third row used another one.
    assert_eq!(second.model, "acme/coder-1");
    let notes: Vec<_> = second
        .items
        .iter()
        .filter_map(|i| match i {
            Item::ModelChange { id, model } => Some((id.as_str(), model.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(notes, [("model:msg_main_09", "acme/coder-2")]);
    // The note comes before that row's content.
    let pos = |id: &str| second.items.iter().position(|i| i.id().as_str() == id);
    assert!(pos("model:msg_main_09") < pos("call-shell-3"));
    assert!(pos("call-sub-2") < pos("model:msg_main_09"));
    // The first turn has one model throughout.
    assert!(
        !assistant(&turns[1])
            .items
            .iter()
            .any(|i| matches!(i, Item::ModelChange { .. }))
    );
}

#[test]
fn an_assistant_row_that_ended_in_error_loads_with_its_notice() {
    let db = TempDb::new();
    db.exec(
        r#"INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES
        ('msg_main_90', 'ses_main', 'user', 90, 1793179000000, 1793179000000, '{"time":{"created":1793179000000},"text":"One more"}'),
        ('msg_main_91', 'ses_main', 'assistant', 91, 1793179001000, 1793179001000, '{"time":{"created":1793179001000,"completed":1793179002000},"model":{"id":"acme/coder-1"},"content":[],"finish":"error","error":{"type":"provider.transport","message":"connection reset"}}');"#,
    );
    let turns = load_turns(&harness(&db), "ses_main");
    let last = assistant(turns.last().unwrap());
    assert_eq!(
        last.items,
        [Item::Notice {
            id: "notice:msg_main_91".into(),
            level: crate::model::NoticeLevel::Error,
            text: "provider.transport: connection reset".to_owned(),
        }]
    );
    // The rows of the fixture end normally: no other turn has a notice.
    let others = turns[..turns.len() - 1].iter().filter(|t| match &***t {
        Turn::Assistant(a) => a.items.iter().any(|i| matches!(i, Item::Notice { .. })),
        _ => false,
    });
    assert_eq!(others.count(), 0);
}

#[test]
fn injected_rows_after_the_last_row_go_into_the_current_turn() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    let second = assistant(&turns[3]);
    let Some(Item::Injected { id, entries }) = second.items.last() else {
        panic!("{:?}", second.items.last());
    };
    assert_eq!(id.as_str(), "injected:msg_main_10");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, "synthetic");
    assert_eq!(
        entries[0].text,
        "<system-reminder>Reminder at the end.</system-reminder>"
    );
}

#[test]
fn the_stats_footer_sums_duration_messages_tokens_and_cost() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    // Two rows: 09:00:03 .. 09:00:20, 1000+100 and 2000+200 input+output tokens.
    let stats = assistant(&turns[1]).stats.clone().unwrap();
    assert_eq!(stats.duration, Duration::from_secs(17));
    assert_eq!(stats.message_count, 2);
    assert_eq!(
        stats.tokens,
        Some(TokenUsage {
            input: 3000,
            output: 300,
            ..TokenUsage::default()
        })
    );
    assert!(
        (stats.cost.unwrap() - 0.03).abs() < 1e-9,
        "{:?}",
        stats.cost
    );
    // Two rows: 09:05:01 .. 09:05:40 (the last row's completion).
    let stats = assistant(&turns[3]).stats.clone().unwrap();
    assert_eq!(stats.duration, Duration::from_secs(39));
    assert_eq!(stats.message_count, 2);
    assert_eq!(
        stats.tokens,
        Some(TokenUsage {
            input: 800,
            output: 80,
            ..TokenUsage::default()
        })
    );
    assert!(
        (stats.cost.unwrap() - 0.008).abs() < 1e-9,
        "{:?}",
        stats.cost
    );
}

#[test]
fn subagent_tool_parts_are_subagent_calls_with_the_agent_in_the_input() {
    let db = TempDb::new();
    let turns = load_turns(&harness(&db), "ses_main");
    let call = calls(assistant(&turns[3]))[0];
    assert_eq!(call.tool, ToolKind::Subagent);
    assert_eq!(call.description, "Explore the repo");
    assert_eq!(call.input["agent"], "explore");
    assert_eq!(call.input["prompt"], "Look around the repository");
}

#[test]
fn rows_whose_data_does_not_parse_are_skipped_and_counted() {
    let db = TempDb::new();
    let updates = load_updates(&harness(&db), "ses_bad");
    assert!(
        updates.contains(&ModelUpdate::ParseWarnings(2)),
        "{updates:?}"
    );
    let turns = turns_of(&updates);
    // Hello, then (two bad rows) "Still here.".
    assert_eq!(turns.len(), 2);
    assert_eq!(user(&turns[0]).text, "Hello");
    assert!(matches!(
        &assistant(&turns[1]).items[..],
        [Item::Text { text, .. }] if text == "Still here."
    ));
}

#[test]
fn a_session_without_rows_loads_to_nothing() {
    let db = TempDb::new();
    let updates = load_updates(&harness(&db), "ses_blank");
    assert_eq!(
        updates[..3],
        [
            ModelUpdate::Progress(1.0),
            ModelUpdate::ParseWarnings(0),
            ModelUpdate::Loaded
        ]
    );
    assert!(turns_of(&updates).is_empty());
}

// -- load

#[test]
fn load_streams_batches_progress_warnings_loaded_and_activity() {
    let db = TempDb::new();
    let h = harness(&db).with_page_size(3);
    let updates = load_updates(&h, "ses_main");
    // The first update is a batch of turns (the first page).
    assert!(
        matches!(&updates[0], ModelUpdate::Appended(t) if !t.is_empty()),
        "{updates:?}"
    );
    let progress: Vec<f32> = updates
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Progress(p) => Some(*p),
            _ => None,
        })
        .collect();
    // 10 rows in pages of 3: four pages, the last Progress is 1.0.
    assert_eq!(progress.len(), 4, "{progress:?}");
    assert!(progress.windows(2).all(|w| w[0] < w[1]), "{progress:?}");
    assert!((progress[0] - 0.3).abs() < 1e-6);
    assert_eq!(*progress.last().unwrap(), 1.0);
    // Each Progress follows a batch; the ending is fixed.
    let tail = &updates[updates.len() - 3..];
    assert_eq!(tail[0], ModelUpdate::ParseWarnings(0));
    assert_eq!(tail[1], ModelUpdate::Loaded);
    assert_eq!(
        tail[2],
        ModelUpdate::Activity {
            last_modified: UNIX_EPOCH
                + Duration::from_secs(u64::try_from(at(28, 9, 30, 0).timestamp()).unwrap()),
            idle: Some(true),
        }
    );
    assert_eq!(
        updates[updates.len() - 4],
        ModelUpdate::Progress(1.0),
        "{updates:?}"
    );
    // Turns that grew on a later page arrive as updates.
    assert!(updates.iter().any(|u| matches!(u, ModelUpdate::Updated(_))));
    assert_eq!(
        updates
            .iter()
            .filter(|u| matches!(u, ModelUpdate::Loaded))
            .count(),
        1
    );
}

#[test]
fn the_page_size_does_not_change_the_turns() {
    let db = TempDb::new();
    let reference = load_turns(&harness(&db), "ses_main");
    for page in [1, 2, 3, 4, 5, 7, 9, 10, 11] {
        let paged = load_turns(&harness(&db).with_page_size(page), "ses_main");
        assert_eq!(paged, reference, "page size {page}");
    }
}

#[test]
fn a_page_never_has_more_rows_than_asked_for() {
    let db = TempDb::new();
    let conn = Db::open(&db.path).unwrap();
    let mut after = i64::MIN;
    let mut seen = Vec::new();
    loop {
        let page = super::load::fetch_page(&conn, "ses_main", after, 4).unwrap();
        assert!(page.len() <= 4);
        let Some(last) = page.last() else { break };
        after = last.seq;
        seen.extend(page.iter().map(|r| r.seq));
    }
    assert_eq!(seen, (1..=10).collect::<Vec<_>>());
    // Another session's rows are never mixed in.
    assert!(
        super::load::fetch_page(&conn, "ses_child1", i64::MIN, 500)
            .unwrap()
            .iter()
            .all(|r| r.id.starts_with("msg_child1_"))
    );
}

#[test]
fn the_default_page_size_is_500() {
    assert_eq!(super::load::PAGE_SIZE, 500);
}

#[test]
fn a_session_of_1200_rows_streams_in_pages_of_at_most_500() {
    let db = TempDb::new();
    // 600 prompts and 600 replies in the session that has no rows yet.
    db.exec(
        "INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) \
         WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1200) \
         SELECT 'big_' || i, 'ses_blank', CASE WHEN i % 2 = 1 THEN 'user' ELSE 'assistant' END, i, \
                1793000000000 + i, 1793000000000 + i, \
                CASE WHEN i % 2 = 1 \
                     THEN '{\"time\":{\"created\":' || (1793000000000 + i) || '},\"text\":\"q' || i || '\"}' \
                     ELSE '{\"time\":{\"created\":' || (1793000000000 + i) || '},\"model\":{\"id\":\"m\"},\"content\":[{\"type\":\"text\",\"text\":\"a' || i || '\"}]}' \
                END FROM n;",
    );
    let updates = load_updates(&harness(&db), "ses_blank");
    let batches: Vec<usize> = updates
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Appended(turns) => Some(turns.len()),
            _ => None,
        })
        .collect();
    // 500 rows are 500 turns here, and every batch is one page.
    assert_eq!(batches, [500, 500, 200]);
    let progress: Vec<f32> = updates
        .iter()
        .filter_map(|u| match u {
            ModelUpdate::Progress(p) => Some(*p),
            _ => None,
        })
        .collect();
    assert_eq!(progress.len(), 3, "{progress:?}");
    assert!((progress[0] - 500.0 / 1200.0).abs() < 1e-6);
    assert!((progress[1] - 1000.0 / 1200.0).abs() < 1e-6);
    assert_eq!(progress[2], 1.0);
    assert_eq!(turns_of(&updates).len(), 1200);
}

#[test]
fn the_activity_of_a_running_session_is_not_idle() {
    let db = TempDb::new();
    let updates = load_updates(&harness(&db), "ses_long");
    assert!(
        matches!(
            updates.last(),
            Some(ModelUpdate::Activity {
                idle: Some(false),
                ..
            })
        ),
        "{updates:?}"
    );
}

#[test]
fn an_unknown_session_ends_in_loaded() {
    let db = TempDb::new();
    let updates = load_updates(&harness(&db), "ses_nope");
    assert_eq!(updates, [ModelUpdate::Loaded]);
}

#[test]
fn dropping_the_handle_stops_the_worker() {
    let db = TempDb::new();
    let h = harness(&db);
    let (tx, rx) = mpsc::channel();
    let handle = h.load(&sid("ses_main"), tx).unwrap();
    drop(rx);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !handle.is_finished() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(handle.is_finished());
}

// -- subagents

#[test]
fn a_subagent_call_resolves_to_its_child_session_through_the_metadata() {
    let db = TempDb::new();
    let h = harness(&db);
    assert_eq!(
        h.subagent(&sid("ses_main"), &"call-sub-1".into()),
        Some(sid("ses_child1"))
    );
    // The child's transcript streams like any session.
    let turns = load_turns(&h, "ses_child1");
    assert_eq!(turns.len(), 2);
    assert_eq!(user(&turns[0]).text, "Look around the repository");
    assert!(matches!(
        &assistant(&turns[1]).items[..],
        [Item::Text { text, .. }] if text == "Two crates: a lib and a binary."
    ));
}

#[test]
fn without_metadata_the_child_is_found_by_its_title() {
    let db = TempDb::new();
    let h = harness(&db);
    // `call-sub-2` has no `state.metadata`: its description is the title.
    assert_eq!(
        h.subagent(&sid("ses_main"), &"call-sub-2".into()),
        Some(sid("ses_child2"))
    );
}

#[test]
fn metadata_naming_a_session_that_is_not_a_child_is_not_trusted() {
    let db = TempDb::new();
    // `ses_untitled` exists but its parent is not `ses_main`.
    db.exec(
        "UPDATE session_message SET data = replace(data, 'ses_child1', 'ses_untitled') \
         WHERE id = 'msg_main_08';",
    );
    let h = harness(&db);
    // Falls back to the title match ("Explore the repo").
    assert_eq!(
        h.subagent(&sid("ses_main"), &"call-sub-1".into()),
        Some(sid("ses_child1"))
    );
}

#[test]
fn an_unknown_call_or_parent_resolves_to_nothing() {
    let db = TempDb::new();
    let h = harness(&db);
    assert_eq!(h.subagent(&sid("ses_main"), &"call-nope".into()), None);
    assert_eq!(h.subagent(&sid("ses_untitled"), &"call-sub-1".into()), None);
    assert_eq!(h.subagent(&sid("ses_nope"), &"call-sub-1".into()), None);
    // A call that is not a subagent call has no child.
    assert_eq!(h.subagent(&sid("ses_main"), &"call-read-1".into()), None);
}

#[test]
fn a_call_with_neither_metadata_nor_a_matching_title_has_no_transcript() {
    let db = TempDb::new();
    db.exec("UPDATE session_v2 SET title = 'Something else' WHERE id = 'ses_child2';");
    let h = harness(&db);
    assert_eq!(h.subagent(&sid("ses_main"), &"call-sub-2".into()), None);
}

#[test]
fn subagent_info_names_the_agent_and_the_description() {
    let db = TempDb::new();
    let h = harness(&db);
    let info = h.subagent_info(&sid("ses_child1")).unwrap();
    assert_eq!(info.agent_type, "explore");
    assert_eq!(info.description, "Explore the repo");
    // From the child's own row when the parent's call does not name it.
    let info = h.subagent_info(&sid("ses_child2")).unwrap();
    assert_eq!(info.agent_type, "review");
    assert_eq!(info.description, "Review the code");
    // A top-level session is not a subagent.
    assert_eq!(h.subagent_info(&sid("ses_main")), None);
    assert_eq!(h.subagent_info(&sid("ses_nope")), None);
}

// -- stats, stats key, resume

#[test]
fn the_stats_key_changes_with_time_updated() {
    let db = TempDb::new();
    let h = harness(&db);
    let mut session = summary(&h, "ses_main");
    let key = h.stats_key(&session);
    assert_eq!(
        key,
        format!(
            "opencode:{}:ses_main:{}",
            db.path.display(),
            at(28, 9, 30, 0).timestamp_millis()
        )
    );
    assert!(key.starts_with(&format!("{}:", HarnessId::OpenCode.cache_prefix())));
    assert_eq!(h.stats_key(&session), key, "stable");
    session.last_activity += chrono::Duration::milliseconds(1);
    assert_ne!(h.stats_key(&session), key);
    // Another session of the same database has another key.
    assert_ne!(h.stats_key(&summary(&h, "ses_untitled")), key);
}

/// A child session of `parent` that was last updated at `updated`.
fn child_sql(id: &str, parent: &str, updated: i64) -> String {
    format!(
        "INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, \
         agent, model, time_created, time_updated, time_idle) VALUES ('{id}', 'prj_alpha', \
         '{parent}', '{id}', '/home/user/alpha', 'Nested', '1.0.0', NULL, NULL, 1793178400000, \
         {updated}, NULL);"
    )
}

#[test]
fn the_stats_key_follows_the_newest_time_updated_of_the_descendants() {
    let db = TempDb::new();
    let h = harness(&db);
    let session = summary(&h, "ses_main");
    let key = h.stats_key(&session);
    let other = h.stats_key(&summary(&h, "ses_untitled"));
    let key_with = |updated: i64| format!("opencode:{}:ses_main:{updated}", db.path.display());
    let main_updated = session.last_activity.timestamp_millis();

    // A child written after its parent changes the key and nothing else.
    db.exec("UPDATE session_v2 SET time_updated = 1793179900000 WHERE id = 'ses_child1'");
    h.refresh().unwrap();
    assert_eq!(summary(&h, "ses_main"), session, "the summary is unchanged");
    assert_ne!(h.stats_key(&session), key);
    assert_eq!(h.stats_key(&session), key_with(1_793_179_900_000));
    // Another session's key does not change with it.
    assert_eq!(h.stats_key(&summary(&h, "ses_untitled")), other);

    // A grandchild older than the newest descendant changes nothing ...
    db.exec(&child_sql("ses_grand", "ses_child1", 1_793_178_500_000));
    h.refresh().unwrap();
    assert_eq!(h.stats_key(&session), key_with(1_793_179_900_000));
    // ... and a newer one changes the key.
    db.exec("UPDATE session_v2 SET time_updated = 1793180000000 WHERE id = 'ses_grand'");
    h.refresh().unwrap();
    assert_eq!(summary(&h, "ses_main"), session, "the summary is unchanged");
    assert_eq!(h.stats_key(&session), key_with(1_793_180_000_000));

    // A descendant older than the session's own time does not lower the key.
    db.exec("UPDATE session_v2 SET time_updated = 1 WHERE id IN ('ses_child1', 'ses_grand')");
    h.refresh().unwrap();
    assert_eq!(h.stats_key(&session), key_with(main_updated));
    assert_eq!(h.stats_key(&session), key);
}

#[test]
fn a_child_whose_time_updated_is_not_an_integer_does_not_fail_the_index() {
    let db = TempDb::new();
    let h = harness(&db);
    let session = summary(&h, "ses_main");
    let key = h.stats_key(&session);
    let main_updated = session.last_activity.timestamp_millis();
    db.exec(&child_sql("ses_grand", "ses_child1", 1_793_180_000_000));
    for bad in ["'abc'", "1793179900000.5"] {
        // Text and REAL are not counted.
        db.exec(&format!(
            "UPDATE session_v2 SET time_updated = {bad} WHERE id = 'ses_child1'"
        ));
        h.refresh().unwrap();
        assert_eq!(summary(&h, "ses_main"), session, "{bad}: still listed");
        // The grandchild's valid time still counts.
        assert_eq!(
            h.stats_key(&session),
            format!("opencode:{}:ses_main:1793180000000", db.path.display()),
            "{bad}"
        );
    }
    // With no valid descendant time the key falls back to the session's own.
    db.exec("UPDATE session_v2 SET time_updated = 'x' WHERE id IN ('ses_child1', 'ses_grand')");
    h.refresh().unwrap();
    assert_eq!(summary(&h, "ses_main"), session);
    assert_eq!(h.stats_key(&session), key);
    assert!(key.ends_with(&format!(":{main_updated}")));
}

#[test]
fn a_cycle_in_parent_id_does_not_hang_the_index() {
    let db = TempDb::new();
    let h = harness(&db);
    let session = summary(&h, "ses_main");
    let key = h.stats_key(&session);
    // Two sessions that are each other's parent: no top-level session
    // reaches them, so the build ends and the keys stay.
    db.exec(&format!(
        "{}{}",
        child_sql("ses_x", "ses_y", 1_793_190_000_000),
        child_sql("ses_y", "ses_x", 1_793_190_000_000)
    ));
    h.refresh().unwrap();
    assert_eq!(h.stats_key(&session), key);
    // A session has one parent, so a cycle cannot hang off a root: the
    // recursion starts at the top-level sessions and never meets one. `UNION`
    // is for the stats of a session inside a cycle (see `stats_tests`).
    db.exec("UPDATE session_v2 SET time_updated = 1793190000001 WHERE id = 'ses_x'");
    h.refresh().unwrap();
    assert_eq!(h.stats_key(&session), key);
    assert!(h.stats(&session).is_ok());
}

#[test]
fn the_resume_command_runs_in_the_session_directory() {
    let db = TempDb::new();
    let h = harness(&db);
    let command = h.resume_command(&summary(&h, "ses_main")).unwrap();
    assert_eq!(command.get_program(), "opencode");
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args, ["--session", "ses_main"]);
    assert_eq!(
        command.get_current_dir(),
        Some(std::path::Path::new("/home/user/alpha"))
    );
    assert_eq!(DEFAULT_RESUME_COMMAND, "opencode --session {id}");
}

#[test]
fn an_unusable_resume_template_gives_no_command() {
    let db = TempDb::new();
    let h = harness(&db).with_resume_command("");
    assert!(h.resume_command(&summary(&h, "ses_main")).is_none());
}

// -- schema version

#[test]
fn a_database_with_a_newer_migration_still_parses() {
    let db = TempDb::with_newer_migration();
    let h = harness(&db);
    assert_eq!(h.projects().unwrap().len(), 2);
    let turns = load_turns(&h, "ses_main");
    assert_eq!(turns.len(), 4);
    let conn = Db::open(&db.path).unwrap();
    assert_eq!(
        conn.newest_migration().unwrap().as_deref(),
        Some("20270101000000_future_change")
    );
}

#[test]
fn the_known_migration_is_the_newest_of_the_fixture() {
    let db = TempDb::new();
    let conn = Db::open(&db.path).unwrap();
    assert_eq!(
        conn.newest_migration().unwrap().as_deref(),
        Some(NEWEST_KNOWN_MIGRATION)
    );
}

// -- the selector's turn count against the sidebar

#[test]
fn for_every_session_the_selectors_turn_count_is_the_number_of_user_rows() {
    use crate::components::{EntryKind, TurnRows};

    let db = TempDb::new();
    let h = harness(&db);
    let mut checked = 0;
    for project in h.projects().unwrap() {
        for session in h.sessions(&project.id).unwrap() {
            let turns = load_turns(&h, session.id.as_str());
            let rows = TurnRows::from_turns(turns.iter().map(|t| &**t));
            let user_rows = (0..rows.entry_count())
                .filter(|i| rows.entry(*i).unwrap().1.kind == EntryKind::User)
                .count();
            let counted = h.stats(&session).unwrap().turns;
            if session.id.as_str() == "ses_bad" {
                // The known exception: the SQL count sees 2 `user` rows, but
                // the second row's `data` is `[]`, which the assembler skips
                // and counts (SPEC §9.5), so the sidebar shows 1.
                assert_eq!((counted, user_rows), (2, 1));
            } else {
                assert_eq!(counted, user_rows, "{}", session.id);
            }
            checked += 1;
        }
    }
    assert!(checked >= 5, "{checked}");
}
