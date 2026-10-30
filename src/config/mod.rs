//! The user config file (`config.toml`, SPEC §8.5).
//!
//! Every key is optional. Loading never fails: a missing file means defaults,
//! and unknown keys, invalid values and malformed TOML each produce a
//! [`ConfigWarning`] while the affected settings fall back to their defaults.

mod edit;

pub use edit::{SaveThemeError, save_theme};

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

use crate::paths::expand_tilde;
use crate::timestamp::{DEFAULT_TIMESTAMP_FORMAT, is_valid_format};

/// Default theme id.
pub const DEFAULT_THEME: &str = "default";
/// Default for `truncate_output_lines`.
pub const DEFAULT_TRUNCATE_OUTPUT_LINES: usize = 200;

/// A non-fatal problem found while loading the config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigWarning {
    /// The full dotted key the warning is about (empty for file-level problems).
    pub key: String,
    /// Human-readable description, naming the key and the problem.
    pub message: String,
}

impl fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Settings for the Claude Code harness (`[harness.claude]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeConfig {
    pub enabled: bool,
    /// Overrides the auto-detected projects directory.
    pub data_dir: Option<PathBuf>,
    pub resume_command: String,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            data_dir: None,
            resume_command: "claude --resume {id}".to_owned(),
        }
    }
}

/// Settings for the OpenCode harness (`[harness.opencode]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCodeConfig {
    pub enabled: bool,
    /// Overrides the auto-detected database path.
    pub db_path: Option<PathBuf>,
    pub resume_command: String,
}

impl Default for OpenCodeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            db_path: None,
            resume_command: "opencode --session {id}".to_owned(),
        }
    }
}

/// The `[harness.*]` tables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HarnessConfig {
    pub claude: ClaudeConfig,
    pub opencode: OpenCodeConfig,
}

/// The `[projects]` table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectsConfig {
    /// Maps a project path (`~` expanded) to its display name.
    pub aliases: BTreeMap<PathBuf, String>,
}

/// Raw key bindings: context (`browser`, `selector`, ...) -> action name ->
/// key strings. A single string and a one-element array are the same here.
/// The keymap task validates the key syntax.
pub type RawKeys = BTreeMap<String, BTreeMap<String, Vec<String>>>;

/// The typed configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Theme id, resolved by the theme loader.
    pub theme: String,
    /// Overrides the theme's own setting when present.
    pub transparent_background: Option<bool>,
    pub mouse: bool,
    pub show_system: bool,
    pub dim_unselected: bool,
    /// strftime format for timestamps.
    pub timestamp_format: String,
    /// Show UTC (with a `Z` suffix) instead of local time.
    pub utc: bool,
    /// Tool output is truncated to this many lines (at least 1).
    pub truncate_output_lines: usize,
    pub harness: HarnessConfig,
    pub projects: ProjectsConfig,
    pub keys: RawKeys,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: DEFAULT_THEME.to_owned(),
            transparent_background: None,
            mouse: true,
            show_system: true,
            dim_unselected: true,
            timestamp_format: DEFAULT_TIMESTAMP_FORMAT.to_owned(),
            utc: false,
            truncate_output_lines: DEFAULT_TRUNCATE_OUTPUT_LINES,
            harness: HarnessConfig::default(),
            projects: ProjectsConfig::default(),
            keys: RawKeys::new(),
        }
    }
}

impl Config {
    /// Loads the config file at `path`. `home` is used to expand `~`.
    ///
    /// A missing file gives the defaults and no warnings. A file that cannot
    /// be read or parsed gives the defaults and one warning. All warnings are
    /// also written to the log, so call
    /// [`init_logging`](crate::logging::init_logging) before loading or the
    /// warnings only reach the returned list.
    pub fn load(path: &Path, home: &Path) -> (Self, Vec<ConfigWarning>) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), Vec::new());
            }
            Err(err) => {
                let warnings = vec![ConfigWarning {
                    key: String::new(),
                    message: format!(
                        "could not read config file {}: {err}; using defaults",
                        path.display()
                    ),
                }];
                log_warnings(&warnings);
                return (Self::default(), warnings);
            }
        };
        Self::from_toml_str(&text, home)
    }

    /// Parses config text. See [`Config::load`] for the error handling.
    pub fn from_toml_str(text: &str, home: &Path) -> (Self, Vec<ConfigWarning>) {
        let mut warnings = Vec::new();
        let config = match text.parse::<Table>() {
            Ok(table) => Self::from_table(table, home, &mut warnings),
            Err(err) => {
                warnings.push(ConfigWarning {
                    key: String::new(),
                    message: format!("malformed config file: {err}; using defaults"),
                });
                Self::default()
            }
        };
        log_warnings(&warnings);
        (config, warnings)
    }

    fn from_table(mut table: Table, home: &Path, warnings: &mut Vec<ConfigWarning>) -> Self {
        let mut config = Self::default();
        let mut r = Reader { warnings };

        if let Some(v) = r.string(&mut table, "", "theme", format!("{:?}", config.theme)) {
            config.theme = v;
        }
        if let Some(v) = r.boolean(&mut table, "", "transparent_background", "unset") {
            config.transparent_background = Some(v);
        }
        if let Some(v) = r.boolean(&mut table, "", "mouse", config.mouse) {
            config.mouse = v;
        }
        if let Some(v) = r.boolean(&mut table, "", "show_system", config.show_system) {
            config.show_system = v;
        }
        if let Some(v) = r.boolean(&mut table, "", "dim_unselected", config.dim_unselected) {
            config.dim_unselected = v;
        }
        if let Some(v) = r.string(
            &mut table,
            "",
            "timestamp_format",
            format!("{:?}", config.timestamp_format),
        ) {
            if is_valid_format(&v) {
                config.timestamp_format = v;
            } else {
                r.invalid(
                    "timestamp_format",
                    "not a valid strftime format",
                    &format!("{:?}", config.timestamp_format),
                );
            }
        }
        if let Some(v) = r.boolean(&mut table, "", "utc", config.utc) {
            config.utc = v;
        }
        if let Some(v) = r.integer(
            &mut table,
            "",
            "truncate_output_lines",
            config.truncate_output_lines,
        ) {
            match usize::try_from(v) {
                Ok(n) if n >= 1 => config.truncate_output_lines = n,
                _ => r.invalid(
                    "truncate_output_lines",
                    "must be at least 1",
                    &config.truncate_output_lines.to_string(),
                ),
            }
        }

        if let Some(mut harness) = r.table(&mut table, "", "harness") {
            if let Some(mut claude) = r.table(&mut harness, "harness", "claude") {
                let p = "harness.claude";
                if let Some(v) = r.boolean(&mut claude, p, "enabled", config.harness.claude.enabled)
                {
                    config.harness.claude.enabled = v;
                }
                if let Some(v) = r.string(&mut claude, p, "data_dir", "unset") {
                    config.harness.claude.data_dir = Some(expand_tilde(&v, home));
                }
                if let Some(v) = r.string(
                    &mut claude,
                    p,
                    "resume_command",
                    format!("{:?}", config.harness.claude.resume_command),
                ) {
                    config.harness.claude.resume_command = v;
                }
                r.unknown(&claude, p);
            }
            if let Some(mut opencode) = r.table(&mut harness, "harness", "opencode") {
                let p = "harness.opencode";
                if let Some(v) =
                    r.boolean(&mut opencode, p, "enabled", config.harness.opencode.enabled)
                {
                    config.harness.opencode.enabled = v;
                }
                if let Some(v) = r.string(&mut opencode, p, "db_path", "unset") {
                    config.harness.opencode.db_path = Some(expand_tilde(&v, home));
                }
                if let Some(v) = r.string(
                    &mut opencode,
                    p,
                    "resume_command",
                    format!("{:?}", config.harness.opencode.resume_command),
                ) {
                    config.harness.opencode.resume_command = v;
                }
                r.unknown(&opencode, p);
            }
            r.unknown(&harness, "harness");
        }

        if let Some(mut projects) = r.table(&mut table, "", "projects") {
            if let Some(aliases) = r.table(&mut projects, "projects", "aliases") {
                for (path, value) in aliases {
                    match value {
                        Value::String(name) => {
                            config
                                .projects
                                .aliases
                                .insert(expand_tilde(&path, home), name);
                        }
                        _ => r.skipped(
                            &format!("projects.aliases.{path}"),
                            "expected a string; ignoring this alias",
                        ),
                    }
                }
            }
            r.unknown(&projects, "projects");
        }

        if let Some(keys) = r.table(&mut table, "", "keys") {
            for (context, value) in keys {
                let Value::Table(actions) = value else {
                    r.skipped(
                        &format!("keys.{context}"),
                        "expected a table of actions; ignoring it",
                    );
                    continue;
                };
                let mut bindings = BTreeMap::new();
                for (action, value) in actions {
                    let key = format!("keys.{context}.{action}");
                    match raw_key_list(&value) {
                        Some(list) => {
                            bindings.insert(action, list);
                        }
                        None => r.skipped(
                            &key,
                            "expected a string or an array of strings; ignoring this binding",
                        ),
                    }
                }
                config.keys.insert(context, bindings);
            }
        }

        r.unknown(&table, "");
        config
    }
}

/// Converts a string or an array of strings into a list of key strings.
fn raw_key_list(value: &Value) -> Option<Vec<String>> {
    match value {
        Value::String(s) => Some(vec![s.clone()]),
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => None,
    }
}

fn log_warnings(warnings: &[ConfigWarning]) {
    for warning in warnings {
        log::warn!("config: {warning}");
    }
}

/// The TOML type a key must have.
#[derive(Clone, Copy)]
enum Expected {
    Boolean,
    String,
    Integer,
    Table,
}

impl Expected {
    fn matches(self, value: &Value) -> bool {
        match self {
            Self::Boolean => value.is_bool(),
            Self::String => value.is_str(),
            Self::Integer => value.is_integer(),
            Self::Table => value.is_table(),
        }
    }
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Boolean => "a boolean",
            Self::String => "a string",
            Self::Integer => "an integer",
            Self::Table => "a table",
        })
    }
}

/// Helpers that take values out of a table (so what remains is unknown) and
/// record warnings.
struct Reader<'a> {
    warnings: &'a mut Vec<ConfigWarning>,
}

impl Reader<'_> {
    fn dotted(prefix: &str, key: &str) -> String {
        if prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{prefix}.{key}")
        }
    }

    /// Warns that the value for `key` is invalid and `default` is used.
    fn invalid(&mut self, key: &str, problem: &str, default: &str) {
        self.warnings.push(ConfigWarning {
            key: key.to_owned(),
            message: format!("invalid value for \"{key}\": {problem}; using default {default}"),
        });
    }

    /// Warns that an entry is ignored.
    fn skipped(&mut self, key: &str, problem: &str) {
        self.warnings.push(ConfigWarning {
            key: key.to_owned(),
            message: format!("invalid value for \"{key}\": {problem}"),
        });
    }

    fn unknown(&mut self, table: &Table, prefix: &str) {
        for key in table.keys() {
            let key = Self::dotted(prefix, key);
            self.warnings.push(ConfigWarning {
                message: format!("unknown key \"{key}\""),
                key,
            });
        }
    }

    fn take(
        &mut self,
        table: &mut Table,
        prefix: &str,
        key: &str,
        expected: Expected,
        default: &str,
    ) -> Option<Value> {
        let value = table.remove(key)?;
        if expected.matches(&value) {
            Some(value)
        } else {
            let full = Self::dotted(prefix, key);
            self.warnings.push(ConfigWarning {
                message: format!(
                    "invalid value for \"{full}\": expected {expected}; using default {default}"
                ),
                key: full,
            });
            None
        }
    }

    fn boolean(
        &mut self,
        table: &mut Table,
        prefix: &str,
        key: &str,
        default: impl fmt::Display,
    ) -> Option<bool> {
        self.take(table, prefix, key, Expected::Boolean, &default.to_string())?
            .as_bool()
    }

    fn string(
        &mut self,
        table: &mut Table,
        prefix: &str,
        key: &str,
        default: impl fmt::Display,
    ) -> Option<String> {
        match self.take(table, prefix, key, Expected::String, &default.to_string())? {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    fn integer(
        &mut self,
        table: &mut Table,
        prefix: &str,
        key: &str,
        default: impl fmt::Display,
    ) -> Option<i64> {
        self.take(table, prefix, key, Expected::Integer, &default.to_string())?
            .as_integer()
    }

    fn table(&mut self, table: &mut Table, prefix: &str, key: &str) -> Option<Table> {
        match self.take(table, prefix, key, Expected::Table, "settings")? {
            Value::Table(t) => Some(t),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
