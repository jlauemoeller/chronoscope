//! The on-disk stats cache (SPEC §9.2): one JSON file, `stats-v1.json`,
//! mapping a harness's `stats_key` to the [`SessionStats`] computed for it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::model::SessionStats;

/// The cache file's name inside the cache directory.
pub const CACHE_FILE: &str = "stats-v1.json";

/// The stats cache: loaded at start, saved on demand.
#[derive(Debug)]
pub(super) struct StatsCache {
    dir: PathBuf,
    entries: HashMap<String, SessionStats>,
    dirty: bool,
}

impl StatsCache {
    /// Loads `<dir>/stats-v1.json`. A missing file gives an empty cache; a
    /// file that cannot be read or parsed is logged and ignored (it is
    /// overwritten by the next save).
    pub fn load(dir: &Path) -> Self {
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

    fn stats(turns: usize, ms: u64) -> SessionStats {
        SessionStats {
            turns,
            duration: Duration::from_millis(ms),
        }
    }

    #[test]
    fn round_trips_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = StatsCache::load(dir.path());
        assert!(!cache.is_dirty());
        cache.insert("a".into(), stats(2, 300_500));
        assert!(cache.is_dirty());
        cache.save();
        assert!(!cache.is_dirty());
        let leftovers = fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(leftovers, 1, "only the cache file is left");

        let loaded = StatsCache::load(dir.path());
        assert_eq!(loaded.get("a"), Some(stats(2, 300_500)));
        assert_eq!(loaded.get("b"), None);
        let text = fs::read_to_string(dir.path().join(CACHE_FILE)).unwrap();
        assert_eq!(text, r#"{"a":{"turns":2,"duration":300500}}"#);
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
