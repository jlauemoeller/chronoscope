//! The on-disk stats cache (base product §8.2, SPEC cycle 3 §3.3): one JSON file, `stats-v2.json`,
//! mapping a harness's `stats_key` to the [`SessionStats`] computed for it.
//! The file of the version before, `stats-v1.json`, is removed when the cache
//! loads and never read: its entries have another shape and their keys still
//! match, so every session's stats are computed again.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::model::SessionStats;

/// The cache file's name inside the cache directory.
pub const CACHE_FILE: &str = "stats-v2.json";

/// The file of the previous version, removed when the cache loads.
const OLD_CACHE_FILE: &str = "stats-v1.json";

/// Removes the previous version's cache file. A missing file is the normal
/// case and logs nothing; any other failure is logged and not fatal.
fn remove_old(dir: &Path) {
    let path = dir.join(OLD_CACHE_FILE);
    match fs::remove_file(&path) {
        Ok(()) => log::info!("removed the old stats cache {}", path.display()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => log::warn!(
            "could not remove the old stats cache {}: {err}",
            path.display()
        ),
    }
}

/// The stats cache: loaded at start, saved on demand.
#[derive(Debug)]
pub(super) struct StatsCache {
    dir: PathBuf,
    entries: HashMap<String, SessionStats>,
    dirty: bool,
}

impl StatsCache {
    /// Loads `<dir>/stats-v2.json`. A missing file gives an empty cache; a
    /// file that cannot be read or parsed is logged and ignored (it is
    /// overwritten by the next save). A `stats-v1.json` is removed, best
    /// effort, and never read.
    pub fn load(dir: &Path) -> Self {
        remove_old(dir);
        let path = dir.join(CACHE_FILE);
        let entries = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<HashMap<String, SessionStats>>(&bytes) {
                Ok(entries) => entries,
                Err(err) => {
                    log::warn!("ignoring corrupt stats cache {}: {err}", path.display());
                    HashMap::new()
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => HashMap::new(),
            Err(err) => {
                log::warn!("ignoring unreadable stats cache {}: {err}", path.display());
                HashMap::new()
            }
        };
        Self {
            dir: dir.to_path_buf(),
            entries,
            dirty: false,
        }
    }

    pub fn get(&self, key: &str) -> Option<SessionStats> {
        self.entries.get(key).copied()
    }

    pub fn insert(&mut self, key: String, stats: SessionStats) {
        if self.entries.insert(key, stats) != Some(stats) {
            self.dirty = true;
        }
    }

    /// Whether there is anything to save.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Drops the entries starting with one of the `owned` prefixes whose key
    /// is not in `live`. Other entries (of harnesses that are not running)
    /// are kept.
    pub fn retain_live(&mut self, live: &HashSet<&str>, owned: &[String]) {
        let before = self.entries.len();
        self.entries.retain(|key, _| {
            live.contains(key.as_str()) || !owned.iter().any(|p| key.starts_with(p.as_str()))
        });
        if self.entries.len() != before {
            self.dirty = true;
        }
    }

    /// Writes the cache to a temporary file and renames it over the old one.
    /// Failure is logged; the cache stays dirty so the next save retries.
    pub fn save(&mut self) {
        match self.write() {
            Ok(()) => self.dirty = false,
            Err(err) => log::warn!(
                "could not save the stats cache in {}: {err}",
                self.dir.display()
            ),
        }
    }

    fn write(&self) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let sorted: BTreeMap<&String, &SessionStats> = self.entries.iter().collect();
        let json = serde_json::to_vec(&sorted).map_err(io::Error::other)?;
        // A name of its own, so that two instances saving at once cannot mix
        // their writes in one file.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let tmp = self.dir.join(format!(
            "{CACHE_FILE}.{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let result =
            fs::write(&tmp, json).and_then(|()| fs::rename(&tmp, self.dir.join(CACHE_FILE)));
        if result.is_err() {
            // Best effort: do not leave the temporary file behind.
            let _ = fs::remove_file(&tmp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::model::TokenUsage;

    fn stats(turns: usize, ms: u64) -> SessionStats {
        SessionStats {
            turns,
            agent_time: Duration::from_millis(ms),
            run_time: Duration::from_millis(ms * 2),
            tokens: TokenUsage::default(),
        }
    }

    /// Every field different, so that a swapped one is caught.
    fn full_stats() -> SessionStats {
        SessionStats {
            turns: 3,
            agent_time: Duration::from_millis(61_500),
            run_time: Duration::from_millis(7_200_250),
            tokens: TokenUsage {
                input: 11,
                output: 22,
                cache_read: 33,
                cache_write: 44,
                reasoning: 5,
            },
        }
    }

    #[test]
    fn round_trips_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = StatsCache::load(dir.path());
        assert!(!cache.is_dirty());
        cache.insert("a".into(), full_stats());
        assert!(cache.is_dirty());
        cache.save();
        assert!(!cache.is_dirty());
        let leftovers = fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(leftovers, 1, "only the cache file is left");

        let loaded = StatsCache::load(dir.path());
        assert_eq!(loaded.get("a"), Some(full_stats()));
        assert_eq!(loaded.get("b"), None);
        let text = fs::read_to_string(dir.path().join(CACHE_FILE)).unwrap();
        assert_eq!(
            text,
            r#"{"a":{"turns":3,"agent_time":61500,"run_time":7200250,"tokens":{"input":11,"output":22,"cache_read":33,"cache_write":44,"reasoning":5}}}"#
        );
    }

    #[test]
    fn the_cache_file_is_stats_v2() {
        assert_eq!(CACHE_FILE, "stats-v2.json");
    }

    #[test]
    fn an_old_cache_file_is_removed_and_never_read() {
        let dir = tempfile::tempdir().unwrap();
        // In the new shape and under the key being asked for, so that
        // reading it would show.
        let old = dir.path().join(OLD_CACHE_FILE);
        let json = serde_json::to_string(&HashMap::from([("a", full_stats())])).unwrap();
        fs::write(&old, json).unwrap();
        let cache = StatsCache::load(dir.path());
        assert_eq!(cache.get("a"), None);
        assert!(!old.exists(), "the old file is removed");
        assert!(!cache.is_dirty());
    }

    #[test]
    fn an_old_cache_file_does_not_hide_a_new_one() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(OLD_CACHE_FILE), "{}").unwrap();
        let mut cache = StatsCache::load(dir.path());
        cache.insert("a".into(), full_stats());
        cache.save();
        fs::write(dir.path().join(OLD_CACHE_FILE), "{}").unwrap();
        let loaded = StatsCache::load(dir.path());
        assert_eq!(loaded.get("a"), Some(full_stats()));
        assert!(!dir.path().join(OLD_CACHE_FILE).exists());
    }

    #[test]
    fn an_old_cache_that_cannot_be_removed_is_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        // A directory of that name cannot be removed as a file.
        fs::create_dir(dir.path().join(OLD_CACHE_FILE)).unwrap();
        let cache = StatsCache::load(dir.path());
        assert_eq!(cache.get("a"), None);
    }

    #[test]
    fn inserting_the_same_value_is_not_a_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = StatsCache::load(dir.path());
        cache.insert("a".into(), stats(1, 1));
        cache.save();
        cache.insert("a".into(), stats(1, 1));
        assert!(!cache.is_dirty());
    }

    #[test]
    fn a_missing_cache_directory_is_created_on_save() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("x").join("y");
        let mut cache = StatsCache::load(&nested);
        cache.insert("a".into(), stats(1, 1));
        cache.save();
        assert!(nested.join(CACHE_FILE).is_file());
    }

    #[test]
    fn a_corrupt_file_gives_an_empty_cache() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(CACHE_FILE), "{ not json").unwrap();
        let cache = StatsCache::load(dir.path());
        assert_eq!(cache.get("a"), None);
        fs::write(dir.path().join(CACHE_FILE), r#"{"a":{"turns":"x"}}"#).unwrap();
        assert_eq!(StatsCache::load(dir.path()).get("a"), None);
    }

    #[test]
    fn retain_live_drops_stale_entries() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = StatsCache::load(dir.path());
        cache.insert("a".into(), stats(1, 1));
        cache.insert("b".into(), stats(1, 1));
        cache.save();
        let owned = [String::new()];
        cache.retain_live(&HashSet::from(["a"]), &owned);
        assert!(cache.is_dirty());
        assert_eq!(cache.get("a"), Some(stats(1, 1)));
        assert_eq!(cache.get("b"), None);
        cache.save();
        cache.retain_live(&HashSet::from(["a"]), &owned);
        assert!(!cache.is_dirty());
    }

    #[test]
    fn retain_live_keeps_the_entries_of_harnesses_it_does_not_own() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = StatsCache::load(dir.path());
        cache.insert("claude:gone".into(), stats(1, 1));
        cache.insert("claude:live".into(), stats(1, 1));
        cache.insert("opencode:other".into(), stats(1, 1));
        cache.retain_live(&HashSet::from(["claude:live"]), &["claude:".to_owned()]);
        assert_eq!(cache.get("claude:gone"), None);
        assert!(cache.get("claude:live").is_some());
        assert!(cache.get("opencode:other").is_some());
        // With no harness running, nothing is dropped.
        cache.retain_live(&HashSet::new(), &[]);
        assert!(cache.get("claude:live").is_some());
    }
}
