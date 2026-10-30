//! `build_harnesses`: which harnesses the config constructs (SPEC §6.6).

use std::path::Path;

use chronoscope::config::Config;
use chronoscope::harness::{Harness, build_harnesses, build_harnesses_with_home};
use chronoscope::model::HarnessId;

/// A config whose Claude data directory is `dir` and whose OpenCode database
/// is `dir/opencode.db` (which does not exist unless a test creates it), so
/// that nothing depends on the machine's real data.
fn config_in(dir: &Path) -> Config {
    let mut config = Config::default();
    config.harness.claude.data_dir = Some(dir.to_path_buf());
    config.harness.opencode.db_path = Some(dir.join("opencode.db"));
    config
}

fn ids(harnesses: &[std::sync::Arc<dyn Harness>]) -> Vec<HarnessId> {
    harnesses.iter().map(|h| h.id()).collect()
}

#[test]
fn both_harnesses_are_constructed_in_tab_order_when_enabled() {
    let tmp = tempfile::tempdir().unwrap();
    let harnesses = build_harnesses(&config_in(tmp.path()));
    assert_eq!(ids(&harnesses), [HarnessId::Claude, HarnessId::OpenCode]);
    assert!(harnesses[0].available());
    assert_eq!(harnesses[0].watch_paths(), [tmp.path().to_path_buf()]);
    // No database yet: OpenCode is kept, flagged unavailable.
    assert!(!harnesses[1].available());
    assert_eq!(
        harnesses[1].watch_paths()[0],
        tmp.path().join("opencode.db")
    );
}

#[test]
fn opencode_is_available_when_its_database_opens() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("opencode.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(include_str!("fixtures/opencode/fixture.sql"))
        .unwrap();
    drop(conn);
    let harnesses = build_harnesses(&config_in(tmp.path()));
    assert!(harnesses[1].available());
    let projects = harnesses[1].projects().unwrap();
    assert_eq!(projects.len(), 2);
}

#[test]
fn a_disabled_harness_is_not_constructed() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(tmp.path());
    config.harness.claude.enabled = false;
    assert_eq!(
        ids(&build_harnesses(&config)),
        [HarnessId::OpenCode],
        "the tab number of OpenCode stays 2"
    );
    config.harness.claude.enabled = true;
    config.harness.opencode.enabled = false;
    assert_eq!(ids(&build_harnesses(&config)), [HarnessId::Claude]);
    config.harness.claude.enabled = false;
    assert!(build_harnesses(&config).is_empty());
    assert!(build_harnesses_with_home(&config, tmp.path()).is_empty());
}

#[test]
fn an_unavailable_harness_is_kept_and_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("missing");
    let harnesses = build_harnesses(&config_in(&missing));
    assert_eq!(harnesses.len(), 2);
    assert_eq!(harnesses[0].id(), HarnessId::Claude);
    assert!(!harnesses[0].available());
    assert_eq!(harnesses[1].id(), HarnessId::OpenCode);
    assert!(!harnesses[1].available());
}

#[test]
fn the_default_data_locations_are_under_the_given_home() {
    let home = tempfile::tempdir().unwrap();
    let harnesses = build_harnesses_with_home(&Config::default(), home.path());
    assert_eq!(
        harnesses[0].watch_paths(),
        [home.path().join(".claude").join("projects")]
    );
    assert!(!harnesses[0].available());
    // OpenCode: `$XDG_DATA_HOME/opencode/opencode.db` when that variable is
    // set, else below the home directory.
    let watched = harnesses[1].watch_paths();
    assert_eq!(watched.len(), 2);
    assert!(watched[0].ends_with(Path::new("opencode").join("opencode.db")));
    assert!(watched[1].to_string_lossy().ends_with("opencode.db-wal"));
    if std::env::var_os("XDG_DATA_HOME").is_none_or(|v| v.is_empty()) {
        assert_eq!(
            watched[0],
            home.path()
                .join(".local")
                .join("share")
                .join("opencode")
                .join("opencode.db")
        );
        assert!(!harnesses[1].available());
    }
}
