//! `build_harnesses`: which harnesses the config constructs (SPEC §6.6).

use std::path::Path;

use chronoscope::config::Config;
use chronoscope::harness::{build_harnesses, build_harnesses_with_home};
use chronoscope::model::HarnessId;

fn config_with_claude_dir(dir: &Path) -> Config {
    let mut config = Config::default();
    config.harness.claude.data_dir = Some(dir.to_path_buf());
    config
}

#[test]
fn claude_is_constructed_when_enabled() {
    let tmp = tempfile::tempdir().unwrap();
    let harnesses = build_harnesses(&config_with_claude_dir(tmp.path()));
    let ids: Vec<_> = harnesses.iter().map(|h| h.id()).collect();
    assert_eq!(ids, [HarnessId::Claude]);
    assert!(harnesses[0].available());
    assert_eq!(harnesses[0].watch_paths(), [tmp.path().to_path_buf()]);
}

#[test]
fn a_disabled_harness_is_not_constructed() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_with_claude_dir(tmp.path());
    config.harness.claude.enabled = false;
    assert!(build_harnesses(&config).is_empty());
    assert!(build_harnesses_with_home(&config, tmp.path()).is_empty());
}

#[test]
fn an_unavailable_harness_is_kept_and_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("missing");
    let harnesses = build_harnesses(&config_with_claude_dir(&missing));
    assert_eq!(harnesses.len(), 1);
    assert_eq!(harnesses[0].id(), HarnessId::Claude);
    assert!(!harnesses[0].available());
}

#[test]
fn the_default_data_dir_is_under_the_given_home() {
    let home = tempfile::tempdir().unwrap();
    let harnesses = build_harnesses_with_home(&Config::default(), home.path());
    assert_eq!(
        harnesses[0].watch_paths(),
        [home.path().join(".claude").join("projects")]
    );
    assert!(!harnesses[0].available());
}
