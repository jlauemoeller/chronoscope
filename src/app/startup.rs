//! Building the application's domain state at startup.

use crate::config::{Config, ConfigWarning};
use crate::keymap::Keymap;
use crate::paths::Paths;
use crate::theme::{TerminalEnv, Theme, ThemeRegistry, resolve};

/// The domain state the application starts with.
#[derive(Debug, Clone)]
pub struct AppInit {
    pub config: Config,
    pub registry: ThemeRegistry,
    pub theme: Theme,
    pub keymap: Keymap,
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
        let mut warnings = Vec::new();
        let (config, registry) = match paths {
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
        let (theme, theme_warnings) =
            resolve(&registry, &config.theme, config.transparent_background, env);
        warnings.extend(theme_warnings);
        let (keymap, keymap_warnings) = Keymap::from_config(&config.keys);
        warnings.extend(keymap_warnings);
        Self {
            config,
            registry,
            theme,
            keymap,
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
    fn no_paths_uses_defaults_with_a_warning() {
        let init = AppInit::load(None, &env());
        assert_eq!(init.config, Config::default());
        assert_eq!(init.warnings.len(), 1);
        assert!(init.warnings[0].message.contains("home directory"));
    }
}
