//! Per-platform application paths: config file, user themes directory, cache
//! directory and log file.
//!
//! Detection ([`Paths::detect`]) is kept apart from construction
//! ([`Paths::from_dirs`]) so the path logic is testable without touching the
//! process environment.

use std::path::{Path, PathBuf};

/// Name of the per-application subdirectory in every base directory.
const APP_DIR: &str = "chronoscope";

/// The application's filesystem locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// The user's home directory, used for `~` expansion.
    pub home_dir: PathBuf,
    /// The config file, `<config dir>/chronoscope/config.toml` by default.
    pub config_file: PathBuf,
    /// The directory of user themes, `<config dir>/chronoscope/themes`.
    pub themes_dir: PathBuf,
    /// The cache directory, `<cache dir>/chronoscope`.
    pub cache_dir: PathBuf,
    /// The log file, `<state dir>/chronoscope/chronoscope.log`, or
    /// `<cache dir>/chronoscope/chronoscope.log` where there is no state dir.
    pub log_file: PathBuf,
}

impl Paths {
    /// Builds the paths from base directories (without the `chronoscope`
    /// component, which is appended here).
    pub fn from_dirs(
        config_dir: &Path,
        cache_dir: &Path,
        state_dir: Option<&Path>,
        home_dir: &Path,
    ) -> Self {
        let app_config = config_dir.join(APP_DIR);
        let app_cache = cache_dir.join(APP_DIR);
        let log_dir = match state_dir {
            Some(state) => state.join(APP_DIR),
            None => app_cache.clone(),
        };
        Self {
            home_dir: home_dir.to_path_buf(),
            config_file: app_config.join("config.toml"),
            themes_dir: app_config.join("themes"),
            cache_dir: app_cache,
            log_file: log_dir.join("chronoscope.log"),
        }
    }

    /// Uses `config_file` instead of the default config file. Nothing else
    /// changes (the themes directory stays where it is).
    #[must_use]
    pub fn with_config_file(mut self, config_file: PathBuf) -> Self {
        self.config_file = config_file;
        self
    }

    /// Detects the directories of the current platform through `etcetera`.
    ///
    /// Returns `None` when the home directory cannot be determined.
    pub fn detect() -> Option<Self> {
        use etcetera::BaseStrategy;
        let strategy = etcetera::choose_base_strategy().ok()?;
        Some(Self::from_dirs(
            &strategy.config_dir(),
            &strategy.cache_dir(),
            strategy.state_dir().as_deref(),
            strategy.home_dir(),
        ))
    }
}

/// Expands a leading `~` component to `home`.
///
/// `~` alone is the home directory and `~/foo` is `<home>/foo`, built with
/// [`PathBuf::join`]. Anything else (including `~user/foo`) is returned
/// unchanged.
pub fn expand_tilde(value: &str, home: &Path) -> PathBuf {
    let path = Path::new(value);
    match path.strip_prefix("~") {
        Ok(rest) if rest.as_os_str().is_empty() => home.to_path_buf(),
        Ok(rest) => home.join(rest),
        Err(_) => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(state: Option<&Path>) -> Paths {
        Paths::from_dirs(
            Path::new("/tmp/cfg"),
            Path::new("/tmp/cache"),
            state,
            Path::new("/tmp/home"),
        )
    }

    #[test]
    fn from_dirs_resolves_all_locations() {
        let p = paths(Some(Path::new("/tmp/state")));
        assert_eq!(p.config_file, Path::new("/tmp/cfg/chronoscope/config.toml"));
        assert_eq!(p.themes_dir, Path::new("/tmp/cfg/chronoscope/themes"));
        assert_eq!(p.cache_dir, Path::new("/tmp/cache/chronoscope"));
        assert_eq!(
            p.log_file,
            Path::new("/tmp/state/chronoscope/chronoscope.log")
        );
        assert_eq!(p.home_dir, Path::new("/tmp/home"));
    }

    #[test]
    fn log_file_falls_back_to_cache_dir_without_state_dir() {
        let p = paths(None);
        assert_eq!(
            p.log_file,
            Path::new("/tmp/cache/chronoscope/chronoscope.log")
        );
    }

    #[test]
    fn explicit_config_file_overrides_only_the_config_file() {
        let default = paths(Some(Path::new("/tmp/state")));
        let p = default
            .clone()
            .with_config_file(PathBuf::from("/elsewhere/my.toml"));
        assert_eq!(p.config_file, Path::new("/elsewhere/my.toml"));
        assert_eq!(p.themes_dir, default.themes_dir);
        assert_eq!(p.cache_dir, default.cache_dir);
        assert_eq!(p.log_file, default.log_file);
        assert_eq!(p.home_dir, default.home_dir);
    }

    #[test]
    fn tilde_expands_to_home() {
        let home = Path::new("/h");
        assert_eq!(
            expand_tilde("~/foo/bar", home),
            home.join("foo").join("bar")
        );
        assert_eq!(expand_tilde("~", home), home);
        assert_eq!(expand_tilde("/abs/~/x", home), Path::new("/abs/~/x"));
        assert_eq!(expand_tilde("~user/x", home), Path::new("~user/x"));
        assert_eq!(expand_tilde("rel/x", home), Path::new("rel/x"));
    }
}
