//! A corrupt stats cache is logged and ignored. Own test binary: the logger
//! is process-global.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chronoscope::harness::Harness;
use chronoscope::harness::claude::ClaudeHarness;
use chronoscope::index::{CACHE_FILE, IndexEvent, SessionIndex};
use chronoscope::logging::init_logging;
use chronoscope::model::SessionStats;

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

#[test]
fn a_corrupt_cache_is_ignored_with_a_log_entry_and_stats_are_recomputed() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("chronoscope.log");
    init_logging(&log).expect("logging initialises");

    let data = tmp.path().join("projects");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude/discovery"),
        &data,
    );
    let cache = tmp.path().join("cache");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join(CACHE_FILE), "{ this is not json").unwrap();

    let harness = Arc::new(ClaudeHarness::with_data_dir(data, BTreeMap::new()));
    let dynamic: Arc<dyn Harness> = harness.clone();
    let mut handle = SessionIndex::start(vec![dynamic], cache.clone());
    let rx = handle.take_events().unwrap();
    let mut stats = BTreeMap::new();
    while stats.len() < 8 {
        match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
            IndexEvent::Stats {
                session, stats: s, ..
            } => {
                stats.insert(session.to_string(), s);
            }
            IndexEvent::Snapshot { .. } => {}
        }
    }
    handle.shutdown();

    let text = fs::read_to_string(&log).unwrap();
    let warnings: Vec<_> = text
        .lines()
        .filter(|l| l.contains("WARN") && l.contains("corrupt stats cache"))
        .collect();
    assert_eq!(warnings.len(), 1, "{text}");
    assert!(warnings[0].contains(CACHE_FILE), "{}", warnings[0]);

    // Everything was recomputed, and the cache is valid JSON again.
    assert_eq!(harness.stats_parse_count(), 8);
    assert_eq!(stats["aaaa0001"].turns, 1);
    let saved: BTreeMap<String, SessionStats> =
        serde_json::from_str(&fs::read_to_string(cache.join(CACHE_FILE)).unwrap()).unwrap();
    assert_eq!(saved.len(), 8);
}
