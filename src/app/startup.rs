//! Building the application's domain state at startup.

use std::path::PathBuf;

use crate::config::{Config, ConfigWarning};
use crate::harness::registered_harnesses;
use crate::keymap::Keymap;
use crate::model::HarnessId;
use crate::paths::Paths;
use crate::theme::{TerminalEnv, Theme, ThemeEntry, ThemeRegistry, builtin, resolve};

/// The domain state the application starts with.
#[derive(Debug, Clone)]
pub struct AppInit {
    pub config: Config,
    pub registry: ThemeRegistry,
    pub theme: Theme,
    /// The id of `theme` in the registry (the theme selector starts on it).
    pub theme_id: String,
    /// The terminal the theme was adapted to; previews are adapted the same
    /// way.
    pub env: TerminalEnv,
    /// The config file the theme selector saves the choice to; none without
    /// a home directory.
    pub config_path: Option<PathBuf>,
    pub keymap: Keymap,
    /// The harnesses with a tab, in tab order (those the config enables).
    pub harnesses: Vec<HarnessId>,
    /// Every config, theme and keymap warning, in the order they arose.
    pub warnings: Vec<ConfigWarning>,
}

impl AppInit {
    /// Loads the config, builds the theme registry and resolves the theme,
    /// builds the keymap, and collects all their warnings.
    ///
    /// Logging should already be initialised, so the warnings reach the log
    /// as well. Without `paths` (no home directory) the defaults are used and
    /// a warning says so.
    pub fn load(paths: Option<&Paths>, env: &TerminalEnv) -> Self {
        Self::load_with_theme(paths, env, None)
    }

    /// Like [`load`](Self::load), with `theme_override` (the id given with `--theme`)
    /// used for this run instead of the configured one. The config file is
    /// not touched. An unknown id gives a warning and the default theme.
    pub fn load_with_theme(
        paths: Option<&Paths>,
        env: &TerminalEnv,
        theme_override: Option<&str>,
    ) -> Self {
        let mut warnings = Vec::new();
        let (mut config, registry) = match paths {
            Some(paths) => {
                let (config, config_warnings) = Config::load(&paths.config_file, &paths.home_dir);
                warnings.extend(config_warnings);
                (config, ThemeRegistry::load(&paths.themes_dir))
            }
            None => {
                warnings.push(ConfigWarning {
                    key: String::new(),
                    message: "could not determine the home directory; using the default \
                              settings and no user themes"
                        .to_owned(),
                });
                (Config::default(), ThemeRegistry::builtin())
            }
        };
        if let Some(id) = theme_override {
            config.theme = id.to_owned();
        }
        let (theme, theme_warnings) =
            resolve(&registry, &config.theme, config.transparent_background, env);
        warnings.extend(theme_warnings.into_iter().map(|mut warning| {
            if theme_override.is_some() {
                warning.key = "--theme".to_owned();
                warning.message = format!("--theme: {}", warning.message);
            }
            warning
        }));
        // The configured theme if it can be used, else the one `resolve`
        // fell back to. (`NO_COLOR` shows the Terminal theme whatever the id.)
        let theme_id = match registry.get(&config.theme) {
            Some(ThemeEntry::Valid { id, .. }) => id.clone(),
            _ => builtin::DEFAULT_ID.to_owned(),
        };
        let (keymap, keymap_warnings) = Keymap::from_config(&config.keys);
        warnings.extend(keymap_warnings);
        let harnesses = registered_harnesses(&config);
        Self {
            config,
            registry,
            theme,
            theme_id,
            env: env.clone(),
            config_path: paths.map(|paths| paths.config_file.clone()),
            keymap,
            harnesses,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{Action, Context};
    use std::fs;
    use std::path::Path;

    fn env() -> TerminalEnv {
        TerminalEnv {
            colorterm: Some("truecolor".to_owned()),
            no_color: false,
        }
    }

    fn paths_in(dir: &Path) -> Paths {
        Paths::from_dirs(
            &dir.join("config"),
            &dir.join("cache"),
            Some(&dir.join("state")),
            dir,
        )
    }

    #[test]
    fn empty_home_gives_defaults_without_warnings() {
        let dir = tempfile::tempdir().unwrap();
        let init = AppInit::load(Some(&paths_in(dir.path())), &env());
        assert_eq!(init.config, Config::default());
        assert!(init.warnings.is_empty(), "{:?}", init.warnings);
        assert_eq!(init.keymap, Keymap::default());
        assert_eq!(init.theme.name(), "Default");
        assert_eq!(init.harnesses, [HarnessId::Claude, HarnessId::OpenCode]);
    }

    #[test]
    fn a_disabled_harness_has_no_tab() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        fs::create_dir_all(paths.config_file.parent().unwrap()).unwrap();
        fs::write(&paths.config_file, "[harness.claude]\nenabled = false\n").unwrap();
        let init = AppInit::load(Some(&paths), &env());
        assert_eq!(init.harnesses, [HarnessId::OpenCode]);
        fs::write(
            &paths.config_file,
            "[harness.claude]\nenabled = false\n[harness.opencode]\nenabled = false\n",
        )
        .unwrap();
        let init = AppInit::load(Some(&paths), &env());
        assert!(init.harnesses.is_empty());
    }

    #[test]
    fn warnings_of_config_theme_and_keymap_are_collected_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        fs::create_dir_all(paths.config_file.parent().unwrap()).unwrap();
        fs::write(
            &paths.config_file,
            "theme = \"no-such-theme\"\nmouse = \"yes\"\n[keys.global]\nquit = \"x\"\nbogus = \"y\"\n",
        )
        .unwrap();
        let init = AppInit::load(Some(&paths), &env());
        let keys: Vec<&str> = init.warnings.iter().map(|w| w.key.as_str()).collect();
        assert_eq!(keys.len(), 3, "{:?}", init.warnings);
        assert_eq!(keys[0], "mouse");
        assert_eq!(keys[1], "theme");
        assert!(keys[2].starts_with("keys.global"), "{keys:?}");
        assert_eq!(
            init.keymap.label(Context::Global, Action::Quit).as_deref(),
            Some("x")
        );
    }

    #[test]
    fn the_theme_id_is_the_configured_one_when_it_can_be_used() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        fs::create_dir_all(paths.config_file.parent().unwrap()).unwrap();
        fs::write(&paths.config_file, "theme = \"nord\"\n").unwrap();
        let init = AppInit::load(Some(&paths), &env());
        assert_eq!(init.theme_id, "nord");
        assert_eq!(init.theme.name(), "Nord");
        assert_eq!(
            init.config_path.as_deref(),
            Some(paths.config_file.as_path())
        );
        assert_eq!(init.env, env());
    }

    #[test]
    fn the_theme_id_falls_back_to_default_with_the_theme() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        fs::create_dir_all(paths.config_file.parent().unwrap()).unwrap();
        fs::write(&paths.config_file, "theme = \"no-such-theme\"\n").unwrap();
        let init = AppInit::load(Some(&paths), &env());
        assert_eq!(init.theme_id, "default");
        assert_eq!(init.theme.name(), "Default");
        // An invalid user theme is not usable either.
        fs::create_dir_all(&paths.themes_dir).unwrap();
        fs::write(paths.themes_dir.join("mine.toml"), "name = 1\n").unwrap();
        fs::write(&paths.config_file, "theme = \"mine\"\n").unwrap();
        let init = AppInit::load(Some(&paths), &env());
        assert_eq!(init.theme_id, "default");
    }

    #[test]
    fn the_config_path_is_the_one_in_use() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("elsewhere.toml");
        let paths = paths_in(dir.path()).with_config_file(other.clone());
        let init = AppInit::load(Some(&paths), &env());
        assert_eq!(init.config_path, Some(other));
        assert_eq!(AppInit::load(None, &env()).config_path, None);
    }

    #[test]
    fn no_paths_uses_defaults_with_a_warning() {
        let init = AppInit::load(None, &env());
        assert_eq!(init.config, Config::default());
        assert_eq!(init.warnings.len(), 1);
        assert!(init.warnings[0].message.contains("home directory"));
    }

    #[test]
    fn theme_override_applies_the_theme_without_touching_the_config() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        fs::create_dir_all(paths.config_file.parent().unwrap()).unwrap();
        let text = "theme = \"nord\"\n";
        fs::write(&paths.config_file, text).unwrap();
        let init = AppInit::load_with_theme(Some(&paths), &env(), Some("gruvbox"));
        assert_eq!(init.theme_id, "gruvbox");
        assert_eq!(init.theme.name(), "Gruvbox");
        assert!(init.warnings.is_empty(), "{:?}", init.warnings);
        assert_eq!(fs::read_to_string(&paths.config_file).unwrap(), text);
        // Without the override the configured theme is used, as before.
        let init = AppInit::load_with_theme(Some(&paths), &env(), None);
        assert_eq!(init.theme_id, "nord");
    }

    #[test]
    fn theme_override_works_without_a_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let init = AppInit::load_with_theme(Some(&paths), &env(), Some("gruvbox"));
        assert_eq!(init.theme.name(), "Gruvbox");
        assert!(!paths.config_file.exists(), "nothing is written");
    }

    #[test]
    fn an_unknown_theme_override_warns_and_uses_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        fs::create_dir_all(paths.config_file.parent().unwrap()).unwrap();
        fs::write(&paths.config_file, "theme = \"nord\"\n").unwrap();
        let init = AppInit::load_with_theme(Some(&paths), &env(), Some("no-such-theme"));
        assert_eq!(init.theme_id, "default");
        assert_eq!(init.theme.name(), "Default");
        assert_eq!(init.warnings.len(), 1);
        let message = &init.warnings[0].message;
        assert!(message.contains("--theme"), "{message}");
        assert!(message.contains("no-such-theme"), "{message}");
        let toast = crate::app::hints::startup_toast_text(&init.warnings).unwrap();
        assert!(toast.starts_with("--theme: unknown theme"), "{toast}");
    }

    #[test]
    fn the_config_option_loads_that_file_instead_of_the_default_one() {
        let dir = tempfile::tempdir().unwrap();
        let default = paths_in(dir.path());
        fs::create_dir_all(default.config_file.parent().unwrap()).unwrap();
        fs::write(&default.config_file, "theme = \"nord\"\n").unwrap();
        let other = dir.path().join("other.toml");
        fs::write(&other, "theme = \"solarized\"\nutc = true\n").unwrap();
        let paths = default.with_config_file(other.clone());
        let init = AppInit::load(Some(&paths), &env());
        assert_eq!(init.theme_id, "solarized");
        assert!(init.config.utc);
        assert_eq!(init.config_path, Some(other));
    }

    #[test]
    fn a_missing_config_file_at_the_given_path_means_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path()).with_config_file(dir.path().join("missing.toml"));
        let init = AppInit::load(Some(&paths), &env());
        assert_eq!(init.config, Config::default());
        assert!(init.warnings.is_empty(), "{:?}", init.warnings);
    }
}
