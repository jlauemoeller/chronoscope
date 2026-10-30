//! The on-disk stats cache (SPEC §9.2): one JSON file, `stats-v1.json`,
//! mapping a harness's `stats_key` to the [`SessionStats`] computed for it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

    /// Drops the entries whose key is not in `live`.
    pub fn retain_live(&mut self, live: &HashSet<&str>) {
        let before = self.entries.len();
        self.entries.retain(|key, _| live.contains(key.as_str()));
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
        let tmp = self.dir.join(format!("{CACHE_FILE}.tmp"));
        fs::write(&tmp, json)?;
        fs::rename(&tmp, self.dir.join(CACHE_FILE))
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
        assert!(!dir.path().join("stats-v1.json.tmp").exists());

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
        cache.retain_live(&HashSet::from(["a"]));
        assert!(cache.is_dirty());
        assert_eq!(cache.get("a"), Some(stats(1, 1)));
        assert_eq!(cache.get("b"), None);
        cache.save();
        cache.retain_live(&HashSet::from(["a"]));
        assert!(!cache.is_dirty());
    }
}
