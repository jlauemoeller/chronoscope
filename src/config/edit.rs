//! Writing the chosen theme back to the config file (SPEC §8.4).
//!
//! The file is edited with `toml_edit`, so the user's comments and formatting
//! survive: only the value of the top-level `theme` key changes.

use std::fmt;
use std::path::Path;

use toml_edit::{Document, Item, Table, Value};

/// Why the theme could not be saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveThemeError {
    /// The existing file is not valid TOML. It is left as it is.
    InvalidToml,
    /// The file could not be read or written.
    Io(String),
}

impl fmt::Display for SaveThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidToml => f.write_str("config.toml is invalid"),
            Self::Io(err) => f.write_str(err),
        }
    }
}

impl std::error::Error for SaveThemeError {}

/// Sets `theme = "<id>"` in the config file at `path`.
///
/// An existing file keeps everything but the value of `theme` byte for byte
/// (a comment on the same line stays); without a `theme` key one is added to
/// the top level. A missing file is created, with its directory, holding just
/// `theme = "<id>"`. A file that is not valid TOML is not touched.
pub fn save_theme(path: &Path, id: &str) -> Result<(), SaveThemeError> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let edited = with_theme(&text, id)?;
            std::fs::write(path, edited).map_err(io_error)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir).map_err(io_error)?;
            }
            std::fs::write(path, with_theme("", id)?).map_err(io_error)
        }
        Err(err) => Err(io_error(err)),
    }
}

/// `text` with the top-level `theme` set to `id`.
///
/// Everything outside the old value (or the new line) is copied from `text`
/// as it is, so comments, spacing and line endings of any kind survive.
fn with_theme(text: &str, id: &str) -> Result<String, SaveThemeError> {
    let parsed = Document::parse(text).map_err(|_| SaveThemeError::InvalidToml)?;
    let value = Value::from(id).to_string();
    match parsed.get("theme") {
        Some(Item::Value(old)) => {
            let Some(range) = old.span() else {
                // Parsed values always have a span; never add a second key.
                return Ok(replaced_by_document(parsed, id));
            };
            let mut edited = text.to_owned();
            edited.replace_range(range, &value);
            return Ok(edited);
        }
        Some(_) => {
            log::warn!(
                "config: replacing the `theme` table with the chosen theme; its contents are lost"
            );
            return Ok(replaced_by_document(parsed, id));
        }
        None => {}
    }
    let at = insertion_point(text, &parsed);
    Ok(insert_line(text, at, &format!("theme = {value}")))
}

/// The document with its `theme` replaced by `id`, written out again (what
/// is around the key is kept, line endings are normalised).
fn replaced_by_document(parsed: Document<&str>, id: &str) -> String {
    let mut doc = parsed.into_mut();
    doc["theme"] = toml_edit::value(id);
    doc.to_string()
}

/// Where a new top-level key goes: after the line of the last top-level
/// key-value; else before the first table (the top of the file); else at the
/// end of the file.
fn insertion_point(text: &str, doc: &Document<&str>) -> usize {
    match root_values_end(doc.as_table()) {
        Some(end) => text[end..].find('\n').map_or(text.len(), |n| end + n + 1),
        None if doc.as_table().iter().next().is_some() => 0,
        None => text.len(),
    }
}

/// The end of the last value that is a top-level key-value (dotted keys
/// included), if any.
fn root_values_end(table: &Table) -> Option<usize> {
    table
        .iter()
        .filter_map(|(_, item)| match item {
            Item::Value(value) => value.span().map(|span| span.end),
            Item::Table(table) if table.is_dotted() => root_values_end(table),
            _ => None,
        })
        .max()
}

/// `text` with `line` as a line of its own at byte `at`, which is at the
/// start of a line or the end of the text. The line ends like the text does.
fn insert_line(text: &str, at: usize, line: &str) -> String {
    let eol = match text.find('\n') {
        Some(n) if text[..n].ends_with('\r') => "\r\n",
        _ => "\n",
    };
    let mut out = String::with_capacity(text.len() + line.len() + 2);
    out.push_str(&text[..at]);
    let at_end = at == text.len();
    if at_end && !text.is_empty() && !text.ends_with('\n') {
        // The last line has no end; the new one gets none either.
        out.push_str(eol);
        out.push_str(line);
        return out;
    }
    out.push_str(line);
    out.push_str(eol);
    out.push_str(&text[at..]);
    out
}

fn io_error(err: std::io::Error) -> SaveThemeError {
    SaveThemeError::Io(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn saved(text: &str, id: &str) -> String {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, text).unwrap();
        save_theme(&path, id).unwrap();
        fs::read_to_string(&path).unwrap()
    }

    /// `text` without the lines that start with `theme`.
    fn without_theme(text: &str) -> String {
        text.split_inclusive('\n')
            .filter(|line| !line.starts_with("theme"))
            .collect()
    }

    #[test]
    fn a_missing_file_is_created_with_exactly_the_theme() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        save_theme(&path, "nord").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "theme = \"nord\"\n");
    }

    #[test]
    fn a_missing_directory_is_created_too() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("config.toml");
        save_theme(&path, "tokyo-night").unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "theme = \"tokyo-night\"\n"
        );
    }

    #[test]
    fn replacing_the_theme_changes_only_its_value() {
        let before = "# My config\n\n# the look\ntheme = \"default\"\n\nmouse = false # no mice\n\n[keys.global]\n# quit with x\nquit = \"x\"\n";
        let after = saved(before, "gruvbox");
        assert_eq!(
            after,
            "# My config\n\n# the look\ntheme = \"gruvbox\"\n\nmouse = false # no mice\n\n[keys.global]\n# quit with x\nquit = \"x\"\n"
        );
    }

    #[test]
    fn a_comment_after_the_value_and_odd_spacing_stay() {
        let after = saved(
            "theme   =   \"default\"   # my favourite\nutc = true\n",
            "nord",
        );
        assert_eq!(after, "theme   =   \"nord\"   # my favourite\nutc = true\n");
    }

    #[test]
    fn a_theme_after_other_keys_is_replaced_in_place() {
        let before = "mouse = true\n# comment\ntheme = 'solarized'\nutc = true";
        let after = saved(before, "catppuccin");
        assert_eq!(
            after,
            "mouse = true\n# comment\ntheme = \"catppuccin\"\nutc = true"
        );
    }

    #[test]
    fn without_a_theme_key_one_is_added_at_the_top_level() {
        let before = "# settings\nmouse = false\n\n[keys.global]\nquit = \"x\"\n\n[harness.claude]\nenabled = false\n";
        let after = saved(before, "nord");
        assert_eq!(without_theme(&after), before);
        let parsed: toml::Table = after.parse().unwrap();
        assert_eq!(parsed["theme"].as_str(), Some("nord"));
        assert_eq!(parsed["keys"]["global"]["quit"].as_str(), Some("x"));
        assert!(parsed["keys"]["global"].get("theme").is_none());
        assert!(parsed["harness"]["claude"].get("theme").is_none());
    }

    #[test]
    fn only_tables_in_the_file_still_get_a_root_theme() {
        let before = "[keys.global]\nquit = \"x\"\n";
        let after = saved(before, "default");
        assert_eq!(without_theme(&after), before);
        let parsed: toml::Table = after.parse().unwrap();
        assert_eq!(parsed["theme"].as_str(), Some("default"));
    }

    #[test]
    fn a_file_without_a_final_newline_still_gets_a_line_of_its_own() {
        let after = saved("mouse = false", "nord");
        let parsed: toml::Table = after.parse().unwrap();
        assert_eq!(parsed["theme"].as_str(), Some("nord"));
        assert_eq!(parsed["mouse"].as_bool(), Some(false));
        assert!(after.starts_with("mouse = false\n"), "{after:?}");
    }

    #[test]
    fn an_empty_file_gets_the_theme() {
        assert_eq!(saved("", "nord"), "theme = \"nord\"\n");
    }

    #[test]
    fn saving_the_same_theme_changes_nothing() {
        let before = "# c\ntheme = \"nord\" # x\n[keys.global]\nquit = \"x\"\n";
        assert_eq!(saved(before, "nord"), before);
    }

    #[test]
    fn crlf_line_endings_are_kept() {
        let before = "mouse = true\r\ntheme = \"default\"\r\nutc = true\r\n";
        assert_eq!(
            saved(before, "nord"),
            "mouse = true\r\ntheme = \"nord\"\r\nutc = true\r\n"
        );
    }

    #[test]
    fn crlf_line_endings_are_kept_when_a_theme_is_added() {
        let before = "mouse = true\r\n[keys.global]\r\nquit = \"x\"\r\n";
        let after = saved(before, "nord");
        assert_eq!(without_theme(&after), before);
        assert!(!after.replace("\r\n", "").contains('\n'), "{after:?}");
        assert!(after.contains("theme = \"nord\"\r\n"), "{after:?}");
    }

    #[test]
    fn mixed_line_endings_stay_as_they_are() {
        let before = "mouse = true\r\n# note\nutc = true\r\n[keys.global]\nquit = \"x\"\r\n";
        let added = saved(before, "nord");
        assert_eq!(without_theme(&added), before);
        let replaced = saved(&format!("theme = \"default\"\n{before}"), "nord");
        assert_eq!(replaced, format!("theme = \"nord\"\n{before}"));
    }

    #[test]
    fn the_theme_goes_after_the_last_top_level_key_not_into_a_table() {
        let before = "a = 1\nkeys.global.quit = \"x\" # dotted\n# about the table\n[harness.claude]\nenabled = false\n";
        let after = saved(before, "nord");
        assert_eq!(without_theme(&after), before);
        let parsed: toml::Table = after.parse().unwrap();
        assert_eq!(parsed["theme"].as_str(), Some("nord"));
        assert_eq!(
            after,
            "a = 1\nkeys.global.quit = \"x\" # dotted\ntheme = \"nord\"\n# about the table\n[harness.claude]\nenabled = false\n"
        );
    }

    #[test]
    fn a_file_of_only_comments_gets_the_theme_at_the_end() {
        assert_eq!(
            saved("# nothing yet\n", "nord"),
            "# nothing yet\ntheme = \"nord\"\n"
        );
        assert_eq!(
            saved("# nothing yet", "nord"),
            "# nothing yet\ntheme = \"nord\""
        );
    }

    #[test]
    fn a_theme_that_is_not_a_value_is_replaced() {
        let after = saved("[theme]\nname = \"x\"\n", "nord");
        let parsed: toml::Table = after.parse().unwrap();
        assert_eq!(parsed["theme"].as_str(), Some("nord"));
    }

    #[test]
    fn an_id_that_needs_escaping_is_escaped() {
        let after = saved("", "my \"theme\"");
        let parsed: toml::Table = after.parse().unwrap();
        assert_eq!(parsed["theme"].as_str(), Some("my \"theme\""));
    }

    #[test]
    fn a_file_that_is_not_toml_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let before = "theme = \"default\"\nthis is = = not toml [\n";
        fs::write(&path, before).unwrap();
        let err = save_theme(&path, "nord").unwrap_err();
        assert_eq!(err, SaveThemeError::InvalidToml);
        assert_eq!(err.to_string(), "config.toml is invalid");
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn a_path_below_a_file_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        fs::write(&file, "x").unwrap();
        let err = save_theme(&file.join("config.toml"), "nord").unwrap_err();
        assert!(matches!(err, SaveThemeError::Io(_)), "{err:?}");
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn a_directory_in_place_of_the_file_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = save_theme(dir.path(), "nord").unwrap_err();
        assert!(matches!(err, SaveThemeError::Io(_)), "{err:?}");
    }
}
