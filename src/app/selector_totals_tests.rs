//! The session row shows the agent time (SPEC cycle 3 §3), also for a session
//! that holds only local commands (§2.3).

use std::path::Path;

use super::fixtures;
use super::selector_tests::{empty_app, loaded, send, session_rows, text, theme};
use crate::components::format_stats;
use crate::harness::Harness;
use crate::harness::claude::ClaudeHarness;
use crate::index::IndexEvent;
use crate::model::{HarnessId, SessionId, SessionStats};

const CLAUDE: HarnessId = HarnessId::Claude;

/// The stats the Claude adapter computes for the local-commands-only fixture.
fn local_commands_only_stats() -> SessionStats {
    let data = tempfile::tempdir().unwrap();
    let harness = ClaudeHarness::with_data_dir(data.path().to_owned(), Default::default());
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude/local_commands_only.jsonl");
    let summary = harness.open_file(&path).unwrap();
    harness.stats(&summary).unwrap()
}

/// An app with one project and one session, `Local-1`, whose stats are those
/// of the local-commands-only fixture.
fn app_with_local_commands_only_row() -> super::App {
    let mut app = empty_app(&[CLAUDE], theme("default"));
    let project = fixtures::project(CLAUDE, "Local", fixtures::now());
    send(&mut app, fixtures::snapshot_of(CLAUDE, vec![project]));
    send(
        &mut app,
        IndexEvent::Stats {
            harness: CLAUDE,
            session: SessionId::from("Local-1"),
            stats: local_commands_only_stats(),
        },
    );
    app
}

#[test]
fn a_local_commands_only_session_has_no_turns_and_no_agent_time() {
    let stats = local_commands_only_stats();
    assert_eq!(stats.turns, 0);
    assert_eq!(stats.agent_time, std::time::Duration::ZERO);
    // Its commands span ten seconds, and they count towards the run time.
    assert!(stats.run_time > std::time::Duration::ZERO);
    assert_eq!(format_stats(&stats), "0 turns · 0s");
    let app = app_with_local_commands_only_row();
    let row = &session_rows(&app).rows[0];
    assert_eq!(row.stats, Some(stats));
}

#[test]
fn the_row_of_a_stats_event_shows_the_agent_time() {
    let mut app = loaded();
    send(
        &mut app,
        IndexEvent::Stats {
            harness: CLAUDE,
            session: SessionId::from("s1"),
            stats: SessionStats {
                run_time: std::time::Duration::from_secs(81 * 3600 + 33 * 60),
                ..match fixtures::stats_event(CLAUDE, "s1", 6, 17 * 3600 + 56 * 60) {
                    IndexEvent::Stats { stats, .. } => stats,
                    other => panic!("{other:?}"),
                }
            },
        },
    );
    let screen = text(&mut app, 120, 40);
    assert!(screen.contains("6 turns · 17h 56m"), "{screen}");
    assert!(!screen.contains("81h"), "{screen}");
}

#[test]
fn a_local_commands_only_row_120x40_text() {
    let mut app = app_with_local_commands_only_row();
    insta::assert_snapshot!(text(&mut app, 120, 40));
}

/// At 80x24 the Sessions pane leaves a title fewer than 10 cells beside
/// `0 turns · 0s`, so the row drops its stats (base §4.1), as every row does
/// at that size.
#[test]
fn a_local_commands_only_row_80x24_text() {
    let mut app = app_with_local_commands_only_row();
    let screen = text(&mut app, 80, 24);
    assert!(screen.contains("A session of Local"), "{screen}");
    assert!(!screen.contains("turns"), "{screen}");
    insta::assert_snapshot!(screen);
}
