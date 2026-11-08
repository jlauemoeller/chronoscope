//! Deciding which theme is active (product §7.1, §7.5, §8.4).
//!
//! Resolution picks a theme from the [`ThemeRegistry`], applies the
//! `transparent_background` override from the config and then adapts it to the
//! terminal: `NO_COLOR` gives the colourless Terminal theme, a truecolor
//! terminal keeps RGB colours, anything else maps them to xterm-256.
//!
//! Nothing here touches the terminal. The environment is read only by
//! [`TerminalEnv::from_env`]; everything else takes a [`TerminalEnv`].

use std::fmt;

use crate::config::ConfigWarning;

use super::{Theme, ThemeEntry, ThemeError, ThemeRegistry, builtin};

/// The terminal capabilities that affect colours.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalEnv {
    /// The value of `$COLORTERM`, if set.
    pub colorterm: Option<String>,
    /// Whether `$NO_COLOR` is set to a non-empty value (no-color.org).
    pub no_color: bool,
}

impl TerminalEnv {
    /// Reads `COLORTERM` and `NO_COLOR` from the process environment. This is
    /// the only place that does.
    pub fn from_env() -> Self {
        Self {
            colorterm: std::env::var("COLORTERM").ok(),
            no_color: std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()),
        }
    }

    /// Whether `COLORTERM` announces 24-bit colour (`truecolor` or `24bit`,
    /// case-insensitive).
    pub fn truecolor(&self) -> bool {
        self.colorterm
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("truecolor") || v.eq_ignore_ascii_case("24bit"))
    }
}

/// Why a theme id could not be resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolveError {
    /// No theme has this id.
    Unknown(String),
    /// The id names a user theme that failed to load.
    Invalid { id: String, error: ThemeError },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "unknown theme \"{id}\""),
            Self::Invalid { id, error } => write!(f, "theme \"{id}\" is invalid: {error}"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Resolves the theme with the given id for the terminal.
///
/// `transparent_override`, when present, replaces the theme's own
/// `transparent_background`. Then the terminal transforms apply: `NO_COLOR`
/// gives the built-in Terminal theme without colours (whatever the id, once it
/// is known to be selectable; a user theme replacing `terminal` is not used,
/// as it would be colourless anyway), a truecolor terminal keeps RGB colours,
/// and anything else maps them to xterm-256.
///
/// Fails for an unknown id and for an invalid user theme.
pub fn resolve_by_id(
    registry: &ThemeRegistry,
    id: &str,
    env: &TerminalEnv,
    transparent_override: Option<bool>,
) -> Result<Theme, ResolveError> {
    match registry.get(id) {
        Some(ThemeEntry::Valid { theme, .. }) => {
            let base = if env.no_color {
                builtin::terminal_theme()
            } else {
                theme.clone()
            };
            Ok(adapt(base, env, transparent_override))
        }
        Some(ThemeEntry::Invalid { error, .. }) => Err(ResolveError::Invalid {
            id: id.to_owned(),
            error: error.clone(),
        }),
        None => Err(ResolveError::Unknown(id.to_owned())),
    }
}

/// Resolves the active theme from the config's `theme` and
/// `transparent_background`.
///
/// `NO_COLOR` wins over the configured theme and gives no warning (see
/// [`resolve_by_id`] for which theme it yields). Otherwise an unknown id, or
/// one naming an invalid user theme, falls back to `default` with a warning
/// (also written to the log).
pub fn resolve(
    registry: &ThemeRegistry,
    theme_id: &str,
    transparent_background: Option<bool>,
    env: &TerminalEnv,
) -> (Theme, Vec<ConfigWarning>) {
    if env.no_color {
        let theme = adapt(builtin::terminal_theme(), env, transparent_background);
        return (theme, Vec::new());
    }
    match resolve_by_id(registry, theme_id, env, transparent_background) {
        Ok(theme) => (theme, Vec::new()),
        Err(err) => {
            let warning = ConfigWarning {
                key: "theme".to_owned(),
                message: format!("{err}; using \"{}\"", builtin::DEFAULT_ID),
            };
            log::warn!("config: {warning}");
            let fallback =
                resolve_by_id(registry, builtin::DEFAULT_ID, env, transparent_background)
                    .unwrap_or_else(|_| {
                        adapt(builtin::default_theme(), env, transparent_background)
                    });
            (fallback, vec![warning])
        }
    }
}

/// Applies the transparency override and the terminal transforms to `theme`.
/// Under `NO_COLOR` the caller passes the Terminal theme.
fn adapt(theme: Theme, env: &TerminalEnv, transparent_override: Option<bool>) -> Theme {
    let theme = match transparent_override {
        Some(transparent) => theme.with_transparent_background(transparent),
        None => theme,
    };
    if env.no_color {
        theme.without_colors()
    } else if env.truecolor() {
        theme
    } else {
        theme.to_ansi256()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ratatui::style::Color;

    use super::*;
    use crate::theme::{Role, Token};

    fn toml_with(name: &str, transparent: bool) -> String {
        let mut out = format!("transparent_background = {transparent}\n");
        for (i, role) in Role::ALL.iter().enumerate() {
            let v = u8::try_from(i).unwrap() + 1;
            out.push_str(&format!("{} = \"#{v:02x}{v:02x}{v:02x}\"\n", role.key()));
        }
        out.push_str(&format!("name = \"{name}\"\n"));
        out
    }

    fn truecolor() -> TerminalEnv {
        TerminalEnv {
            colorterm: Some("truecolor".into()),
            no_color: false,
        }
    }

    fn plain() -> TerminalEnv {
        TerminalEnv::default()
    }

    fn no_color() -> TerminalEnv {
        TerminalEnv {
            colorterm: Some("truecolor".into()),
            no_color: true,
        }
    }

    /// A registry with user themes `opaque` (transparent = false),
    /// `see-through` (true) and `broken` (invalid).
    fn registry() -> (tempfile::TempDir, ThemeRegistry) {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("opaque.toml"), toml_with("Opaque", false)).unwrap();
        fs::write(
            dir.path().join("see-through.toml"),
            toml_with("See Through", true),
        )
        .unwrap();
        fs::write(dir.path().join("broken.toml"), "name = \"Broken\"\n").unwrap();
        let registry = ThemeRegistry::load(dir.path());
        (dir, registry)
    }

    fn resolved(id: &str, env: &TerminalEnv, transparent: Option<bool>) -> Theme {
        let (_dir, registry) = registry();
        let (theme, warnings) = resolve(&registry, id, transparent, env);
        assert!(warnings.is_empty(), "{warnings:?}");
        theme
    }

    #[test]
    fn truecolor_keeps_the_palette_with_rgb_intact() {
        for colorterm in ["truecolor", "24bit", "TrueColor", "24BIT"] {
            let env = TerminalEnv {
                colorterm: Some(colorterm.into()),
                no_color: false,
            };
            let theme = resolved("tokyo-night", &env, None);
            assert_eq!(theme, builtin::get("tokyo-night").unwrap(), "{colorterm}");
            assert!(matches!(theme.role(Role::Background), Color::Rgb(..)));
        }
    }

    #[test]
    fn unset_colorterm_maps_every_rgb_role_to_indexed_16_to_255() {
        let original = builtin::get("tokyo-night").unwrap();
        let theme = resolved("tokyo-night", &plain(), None);
        let mut rgb_roles = 0;
        for role in Role::ALL {
            if let Color::Rgb(..) = original.role(role) {
                rgb_roles += 1;
                match theme.role(role) {
                    Color::Indexed(n) => assert!(n >= 16, "{role:?} -> {n}"),
                    other => panic!("{role:?} stayed {other:?}"),
                }
            }
        }
        assert!(rgb_roles > 0);
        for token in Token::ALL {
            let style = theme.style(token);
            assert!(!matches!(style.fg, Some(Color::Rgb(..))), "{token:?}");
            assert!(!matches!(style.bg, Some(Color::Rgb(..))), "{token:?}");
        }
    }

    #[test]
    fn other_colorterm_values_also_map_to_256() {
        for colorterm in ["", "yes", "256color"] {
            let env = TerminalEnv {
                colorterm: Some(colorterm.into()),
                no_color: false,
            };
            let theme = resolved("tokyo-night", &env, None);
            assert!(
                matches!(theme.role(Role::Foreground), Color::Indexed(n) if n >= 16),
                "{colorterm:?}"
            );
        }
    }

    #[test]
    fn terminal_theme_is_unchanged_by_the_256_mapping() {
        let theme = resolved("terminal", &plain(), None);
        assert_eq!(theme, builtin::get("terminal").unwrap());
    }

    #[test]
    fn unknown_theme_falls_back_to_default_with_one_warning_naming_the_id() {
        let (_dir, registry) = registry();
        let (theme, warnings) = resolve(&registry, "no-such-theme", None, &truecolor());
        assert_eq!(theme, builtin::default_theme());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].key, "theme");
        assert!(warnings[0].message.contains("no-such-theme"));
    }

    #[test]
    fn invalid_user_theme_falls_back_to_default_with_one_warning_naming_the_id() {
        let (_dir, registry) = registry();
        let (theme, warnings) = resolve(&registry, "broken", None, &truecolor());
        assert_eq!(theme, builtin::default_theme());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("broken"));
    }

    #[test]
    fn fallback_is_terminal_transformed_too() {
        let (_dir, registry) = registry();
        let (theme, warnings) = resolve(&registry, "nope", None, &plain());
        assert_eq!(warnings.len(), 1);
        assert_eq!(theme, builtin::default_theme().to_ansi256());
    }

    #[test]
    fn invalid_user_default_falls_back_to_the_builtin_default() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("default.toml"), "name = \"x\"\n").unwrap();
        let registry = ThemeRegistry::load(dir.path());
        let (theme, warnings) = resolve(&registry, "default", None, &truecolor());
        assert_eq!(warnings.len(), 1);
        assert_eq!(theme, builtin::default_theme());
    }

    #[test]
    fn user_theme_resolves_by_its_file_stem() {
        let theme = resolved("opaque", &truecolor(), None);
        assert_eq!(theme.name(), "Opaque");
    }

    #[test]
    fn config_transparent_true_overrides_a_theme_that_says_false() {
        assert!(!resolved("opaque", &truecolor(), None).transparent_background());
        assert!(resolved("opaque", &truecolor(), Some(true)).transparent_background());
        assert!(resolved("tokyo-night", &plain(), Some(true)).transparent_background());
    }

    #[test]
    fn config_transparent_false_overrides_a_theme_that_says_true() {
        assert!(resolved("see-through", &truecolor(), None).transparent_background());
        assert!(!resolved("see-through", &truecolor(), Some(false)).transparent_background());
    }

    #[test]
    fn no_color_gives_the_terminal_theme_with_all_colours_reset() {
        for id in [
            "tokyo-night",
            "default",
            "opaque",
            "no-such-theme",
            "broken",
        ] {
            let (_dir, registry) = registry();
            let (theme, warnings) = resolve(&registry, id, None, &no_color());
            assert!(warnings.is_empty(), "{id}: {warnings:?}");
            assert_eq!(theme.name(), "Terminal", "{id}");
            assert_eq!(
                theme,
                builtin::get("terminal").unwrap().without_colors(),
                "{id}"
            );
            for role in Role::ALL {
                assert_eq!(theme.role(role), Color::Reset, "{id} {role:?}");
            }
            for token in Token::ALL {
                let style = theme.style(token);
                assert!(matches!(style.fg, Some(Color::Reset)), "{id} {token:?}");
                assert!(
                    matches!(style.bg, None | Some(Color::Reset)),
                    "{id} {token:?}"
                );
            }
        }
    }

    #[test]
    fn no_color_still_honours_the_transparent_override() {
        let theme = resolved("tokyo-night", &no_color(), Some(true));
        assert!(theme.transparent_background());
    }

    #[test]
    fn resolve_by_id_reports_unknown_and_invalid_ids() {
        let (_dir, registry) = registry();
        let unknown = resolve_by_id(&registry, "nope", &plain(), None).unwrap_err();
        assert_eq!(unknown, ResolveError::Unknown("nope".into()));
        let invalid = resolve_by_id(&registry, "broken", &plain(), None).unwrap_err();
        assert!(matches!(&invalid, ResolveError::Invalid { id, .. } if id == "broken"));
        assert!(invalid.to_string().contains("broken"));
        // NO_COLOR does not turn an unselectable id into a valid one.
        assert!(resolve_by_id(&registry, "broken", &no_color(), None).is_err());
    }

    #[test]
    fn resolve_by_id_applies_the_terminal_transforms() {
        let (_dir, registry) = registry();
        let theme = resolve_by_id(&registry, "nord", &plain(), Some(true)).unwrap();
        assert_eq!(
            theme,
            builtin::get("nord")
                .unwrap()
                .with_transparent_background(true)
                .to_ansi256()
        );
    }

    #[test]
    fn default_theme_id_is_shared_with_the_config() {
        assert_eq!(crate::config::DEFAULT_THEME, builtin::DEFAULT_ID);
    }

    #[test]
    fn colorterm_and_no_color_are_parsed_from_the_environment_in_one_place() {
        let source = include_str!("resolve.rs").replace("\r\n", "\n");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert_eq!(production.matches("env::var").count(), 2);
        assert_eq!(production.matches("fn from_env").count(), 1);
        let (before, after) = production.split_once("pub fn from_env").unwrap();
        assert!(!before.contains("env::var"));
        let body_end = after.find("\n    }\n").unwrap();
        assert!(!after[body_end..].contains("env::var"));
    }
}
