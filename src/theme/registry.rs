//! The theme registry (SPEC §8.3, §8.4).
//!
//! The registry lists the built-in themes in their §8.3 order, followed by the
//! user themes found in the themes directory. A user theme's id is its file
//! stem. A user theme with a built-in's id replaces that built-in and keeps
//! its position; all other user themes are appended, sorted by id.
//!
//! A user theme that fails to parse is kept as an invalid entry: it is listed
//! but cannot be selected or resolved, and the error is logged. That includes a
//! broken file that shadows a built-in: the built-in is replaced by the invalid
//! entry, so the problem stays visible in the list.

use std::path::Path;

use super::{Theme, ThemeError, builtin};

/// One theme in the registry.
#[derive(Debug, Clone, PartialEq)]
pub enum ThemeEntry {
    /// A theme that can be selected.
    Valid { id: String, theme: Theme },
    /// A user theme file that could not be loaded.
    Invalid {
        id: String,
        /// What the list shows for it: the id.
        label: String,
        error: ThemeError,
    },
}

impl ThemeEntry {
    fn valid(id: impl Into<String>, theme: Theme) -> Self {
        Self::Valid {
            id: id.into(),
            theme,
        }
    }

    fn invalid(id: impl Into<String>, error: ThemeError) -> Self {
        let id = id.into();
        Self::Invalid {
            label: id.clone(),
            id,
            error,
        }
    }

    /// The id used in `config.toml` and `--theme`.
    pub fn id(&self) -> &str {
        match self {
            Self::Valid { id, .. } | Self::Invalid { id, .. } => id,
        }
    }

    /// The text shown in the list: the theme's `name`, or the id for an
    /// invalid entry.
    pub fn label(&self) -> &str {
        match self {
            Self::Valid { theme, .. } => theme.name(),
            Self::Invalid { label, .. } => label,
        }
    }

    /// Whether the entry can be selected.
    pub fn is_selectable(&self) -> bool {
        matches!(self, Self::Valid { .. })
    }

    /// The theme, for a valid entry.
    pub fn theme(&self) -> Option<&Theme> {
        match self {
            Self::Valid { theme, .. } => Some(theme),
            Self::Invalid { .. } => None,
        }
    }

    /// The parse error, for an invalid entry.
    pub fn error(&self) -> Option<&ThemeError> {
        match self {
            Self::Valid { .. } => None,
            Self::Invalid { error, .. } => Some(error),
        }
    }
}

/// The ordered list of available themes.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeRegistry {
    entries: Vec<ThemeEntry>,
}

impl ThemeRegistry {
    /// A registry holding only the built-in themes.
    pub fn builtin() -> Self {
        Self {
            entries: builtin::all()
                .into_iter()
                .map(|b| ThemeEntry::valid(b.id, b.theme))
                .collect(),
        }
    }

    /// The built-in themes plus the user themes in `themes_dir`
    /// (`*.toml`, non-recursive).
    ///
    /// A missing directory means no user themes. Parse failures become
    /// invalid entries and are logged; this never fails.
    pub fn load(themes_dir: &Path) -> Self {
        Self::builtin().with_user_themes(read_user_themes(themes_dir))
    }

    /// Merges user themes (`(id, parse result)`) into the registry.
    fn with_user_themes(mut self, mut user: Vec<(String, Result<Theme, ThemeError>)>) -> Self {
        // Alphabetical ignoring case; the raw id breaks ties.
        user.sort_by_cached_key(|(id, _)| (id.to_lowercase(), id.clone()));
        for (id, result) in user {
            let entry = match result {
                Ok(theme) => ThemeEntry::valid(id, theme),
                Err(error) => ThemeEntry::invalid(id, error),
            };
            match self.entries.iter_mut().find(|e| e.id() == entry.id()) {
                Some(slot) => *slot = entry,
                None => self.entries.push(entry),
            }
        }
        self
    }

    /// All entries, in list order.
    pub fn entries(&self) -> &[ThemeEntry] {
        &self.entries
    }

    /// The entry with the given id.
    pub fn get(&self, id: &str) -> Option<&ThemeEntry> {
        self.entries.iter().find(|e| e.id() == id)
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

/// Reads every `*.toml` file in `dir`. Files that cannot be read or parsed
/// give an `Err`, which is logged here.
fn read_user_themes(dir: &Path) -> Vec<(String, Result<Theme, ThemeError>)> {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            log::warn!("themes: could not read directory {}: {err}", dir.display());
            return Vec::new();
        }
    };
    let mut themes = Vec::new();
    for entry in read_dir {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(err) => {
                log::warn!(
                    "themes: could not read an entry of {}: {err}",
                    dir.display()
                );
                continue;
            }
        };
        // Exactly `*.toml`, so two files never share an id.
        let is_toml = path.extension().is_some_and(|ext| ext == "toml");
        if !is_toml || !path.is_file() {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            log::warn!(
                "themes: ignoring {}: file name is not UTF-8",
                path.display()
            );
            continue;
        };
        let result = std::fs::read_to_string(&path)
            .map_err(|err| ThemeError::Io(err.to_string()))
            .and_then(|text| Theme::from_toml(&text));
        if let Err(err) = &result {
            log::warn!("themes: invalid theme {} ({id}): {err}", path.display());
        }
        themes.push((id.to_owned(), result));
    }
    themes
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ratatui::style::Color;

    use super::*;
    use crate::theme::Role;

    /// A complete theme document where role `i` is `#0i0i0i`-style grey.
    fn theme_toml(name: &str, skip: Option<Role>) -> String {
        let mut out = format!("name = \"{name}\"\n");
        for (i, role) in Role::ALL.iter().enumerate() {
            if Some(*role) == skip {
                continue;
            }
            let v = u8::try_from(i).unwrap() + 1;
            out.push_str(&format!("{} = \"#{v:02x}{v:02x}{v:02x}\"\n", role.key()));
        }
        out
    }

    fn ids(registry: &ThemeRegistry) -> Vec<&str> {
        registry.entries().iter().map(ThemeEntry::id).collect()
    }

    #[test]
    fn missing_directory_gives_exactly_the_seven_builtins_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let registry = ThemeRegistry::load(&dir.path().join("does-not-exist"));
        assert_eq!(
            ids(&registry),
            [
                "default",
                "terminal",
                "catppuccin",
                "gruvbox",
                "nord",
                "tokyo-night",
                "solarized"
            ]
        );
        assert!(registry.entries().iter().all(ThemeEntry::is_selectable));
        assert_eq!(registry, ThemeRegistry::builtin());
    }

    #[test]
    fn with_user_themes_sorts_its_input_by_id_ignoring_case() {
        let theme = |name: &str| Theme::from_toml(&theme_toml(name, None));
        let registry = ThemeRegistry::builtin().with_user_themes(vec![
            ("zeta".into(), theme("Z")),
            ("Mid".into(), theme("M")),
            ("alpha".into(), theme("A")),
        ]);
        assert_eq!(&ids(&registry)[7..], ["alpha", "Mid", "zeta"]);
    }

    #[test]
    fn file_that_cannot_be_read_is_an_io_error_entry() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("binary.toml"), [0xff, 0xfe, 0x00]).unwrap();
        let registry = ThemeRegistry::load(dir.path());
        let entry = registry.get("binary").unwrap();
        assert!(!entry.is_selectable());
        assert!(matches!(entry.error(), Some(ThemeError::Io(_))));
    }

    #[test]
    fn only_lowercase_toml_extension_is_read() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("shout.TOML"), theme_toml("Shout", None)).unwrap();
        assert!(ThemeRegistry::load(dir.path()).get("shout").is_none());
    }

    #[test]
    fn empty_directory_gives_only_the_builtins() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(ThemeRegistry::load(dir.path()).entries().len(), 7);
    }

    #[test]
    fn user_themes_are_appended_alphabetically_by_id() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("zeta.toml"), theme_toml("Zeta Theme", None)).unwrap();
        fs::write(
            dir.path().join("alpha.toml"),
            theme_toml("Alpha Theme", None),
        )
        .unwrap();
        let registry = ThemeRegistry::load(dir.path());
        let ids = ids(&registry);
        assert_eq!(ids.len(), 9);
        assert_eq!(&ids[7..], ["alpha", "zeta"]);
        assert_eq!(ids[..7], builtin::IDS);
        assert_eq!(registry.get("alpha").unwrap().label(), "Alpha Theme");
        assert_eq!(registry.get("zeta").unwrap().label(), "Zeta Theme");
    }

    #[test]
    fn user_theme_with_a_builtin_id_replaces_it_in_place() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("nord.toml"), theme_toml("My Nord", None)).unwrap();
        fs::write(dir.path().join("aaa.toml"), theme_toml("Aaa", None)).unwrap();
        let registry = ThemeRegistry::load(dir.path());
        let ids = ids(&registry);
        assert_eq!(ids.len(), 8);
        assert_eq!(ids[..7], builtin::IDS);
        assert_eq!(ids[7], "aaa");
        let nord = registry.get("nord").unwrap();
        assert_eq!(nord.label(), "My Nord");
        assert_eq!(registry.entries()[4].id(), "nord");
        assert_eq!(
            nord.theme().unwrap().role(Role::Foreground),
            Color::Rgb(1, 1, 1)
        );
    }

    #[test]
    fn non_toml_files_and_directories_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("notes.txt"), "hello").unwrap();
        fs::create_dir(dir.path().join("folder.toml")).unwrap();
        assert_eq!(ThemeRegistry::load(dir.path()).entries().len(), 7);
    }

    #[test]
    fn theme_with_a_missing_base_role_is_an_invalid_entry() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("broken.toml"),
            theme_toml("Broken", Some(Role::Border)),
        )
        .unwrap();
        let registry = ThemeRegistry::load(dir.path());
        let entry = registry.get("broken").expect("listed");
        assert_eq!(registry.entries().last().unwrap().id(), "broken");
        assert_eq!(entry.label(), "broken");
        assert!(!entry.is_selectable());
        assert!(entry.theme().is_none());
        assert_eq!(
            entry.error(),
            Some(&ThemeError::MissingKey("border".into()))
        );
    }

    #[test]
    fn unparsable_file_is_an_invalid_entry() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("junk.toml"), "name = [").unwrap();
        let registry = ThemeRegistry::load(dir.path());
        let entry = registry.get("junk").unwrap();
        assert!(matches!(entry.error(), Some(ThemeError::Syntax(_))));
    }

    #[test]
    fn invalid_user_theme_replaces_the_builtin_with_the_same_id() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("nord.toml"), "name = \"Nord\"\n").unwrap();
        let registry = ThemeRegistry::load(dir.path());
        assert_eq!(registry.entries().len(), 7);
        assert_eq!(registry.entries()[4].id(), "nord");
        assert!(!registry.get("nord").unwrap().is_selectable());
    }

    #[test]
    fn registry_never_reads_the_environment() {
        let source = include_str!("registry.rs").replace("\r\n", "\n");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(!production.contains("env::var"));
        assert!(!production.contains("env::args"));
    }
}
