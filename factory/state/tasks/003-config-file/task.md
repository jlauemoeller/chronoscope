# Config file, app paths, log file and timestamps

## Goal
Load `config.toml` (§8.5) into a typed `Config`, with a default for every key and warnings (never errors) for unknown keys and invalid values. Resolve the per-platform application paths: config file, user themes directory, cache directory and log file. Set up the log file of §9.5. Format timestamps per §8.6. Everything is pure or file-based library code, tested without a terminal. Later tasks read these settings (theme, mouse, `show_system`, `dim_unselected`, timestamps, truncation, harness settings, project aliases, key bindings) from `Config`.

## Spec excerpts
From SPEC.md §8.5:

- Location: `$XDG_CONFIG_HOME/chronoscope/config.toml` (`~/.config/chronoscope/config.toml`), via `etcetera`. On macOS the XDG-style path is used as well, so dotfiles stay portable. On Windows: `%APPDATA%\chronoscope\config.toml`.
- All keys are optional. A missing file means defaults.
- Unknown keys produce a warning (shown once as a toast), not an error.
- Invalid values fall back to the default with a warning.
- Changes are picked up on restart; hot reload is future work.

```toml
theme = "tokyo-night"
transparent_background = false   # overrides the theme's setting when present
mouse = true
show_system = true
dim_unselected = true
timestamp_format = "%Y-%m-%d %H:%M:%S"   # strftime; displayed in local time
utc = false
truncate_output_lines = 200

[harness.claude]
enabled = true
data_dir = "~/.claude/projects"            # override auto-detection
resume_command = "claude --resume {id}"

[harness.opencode]
enabled = true
db_path = "~/.local/share/opencode/opencode.db"
resume_command = "opencode --session {id}"

[projects.aliases]
"/home/jacob/Development/projects/brio" = "BRIO"
"/home/jacob/Development/projects/camera-settings" = "CameraSettings"

[keys.browser]                 # action = key or [keys]; replaces the default for that action
fold_toggle = ["enter", "space"]
copy = "y"
copy_full = "Y"

[keys.selector]
search = "/"
```

Key syntax: `a`, `A`, `ctrl-d`, `shift-tab`, `enter`, `esc`, `pgdn`, `f5`, and two-key chords as space-separated keys (`"g g"`; only in user config, since no default binding is a chord). The help overlay is generated from the active keymap, so it always reflects the user's remappings.

From SPEC.md §8.6:

- Displayed in local time using `timestamp_format`. With `utc = true`, UTC is shown with a `Z` suffix.
- The selector's session rows use the same format without seconds.

From SPEC.md §4.1:

- Display name: basename of the cwd, unless an alias is configured (§8.5). Projects are sorted by most recent activity.

From SPEC.md §9.2:

- Session list stats are computed in the background and cached in `$XDG_CACHE_HOME/chronoscope/`, keyed by (path, size, mtime) or by (db, session id, `time_updated`).

From SPEC.md §9.5:

- Never writes to harness data. The SQLite database is opened read-only.
- Unknown entry types, unknown content blocks and malformed lines are skipped and counted, not fatal.
  - A muted `N entries could not be parsed` note appears in the browser header.
  - Session metadata entries (known ones such as `agent-name`, `queue-operation`, `continued-in`, `bridge-session`, `tools-used`, and unknown types without a `message`) carry no conversation content: they are skipped and logged, but not counted in the note.
  - Details are written to a log file at `$XDG_STATE_HOME/chronoscope/chronoscope.log`.

From SPEC.md §9.1:

Rules that keep tiers 2–3 cheap:
- No direct OS APIs; only cross-platform crates (crossterm, notify, arboard, rusqlite bundled, etcetera).
- All filesystem paths are `PathBuf`; no hard-coded `/` separators or `~` expansion by string concatenation.
- Harness data locations are resolved from a per-platform candidate list and can be overridden in config. The Claude location is `~/.claude/projects` on all platforms. The OpenCode location on macOS and Windows is **unverified** and must be confirmed before tier-2/3 release.
- Every harness adapter is tested against recorded fixture sessions (trimmed copies of real sessions), so parsing is verified on every OS in CI.

Planner notes (decisions not in the spec; binding for this task):
- **Crates.** Use `chrono` for timestamps and strftime formatting, and the `log` facade with a small file logger (`simplelog`'s `WriteLogger`, or an equivalent minimal implementation) for the log file. Add them to `Cargo.toml`. Add `tempfile` as a dev-dependency for file-based tests.
- **Paths.** A `Paths` value holds the config file, `themes/` directory, cache directory and log file.
  - Resolve them with `etcetera::choose_base_strategy()`. This gives XDG-style paths on Linux and macOS, and on Windows the config lives under `%APPDATA%\chronoscope\`.
  - The log file is `<state dir>/chronoscope/chronoscope.log`. Where the strategy has no state dir (Windows), use `<cache dir>/chronoscope/chronoscope.log`.
  - `Paths` can be constructed with an explicit config file path (used by `--config` in a later task).
  - Keep detection separate from construction: `Paths::from_dirs(config_dir, cache_dir, state_dir: Option<_>, home_dir)` is a pure constructor, and a thin `Paths::detect()` calls `etcetera` and passes its directories in. `~` expansion also takes the home directory as a parameter.
  - Tests use the pure constructor with temp directories. They never call `std::env::set_var`/`remove_var`: in edition 2024 those are `unsafe`, and tests run on parallel threads.
- **Config shape.** Keys and defaults:

  | Key | Default |
  |---|---|
  | `theme` | `"default"` (theme id string; resolved in a later task) |
  | `transparent_background` | `Option<bool>`, `None` |
  | `mouse` | `true` |
  | `show_system` | `true` |
  | `dim_unselected` | `true` |
  | `timestamp_format` | `"%Y-%m-%d %H:%M:%S"` |
  | `utc` | `false` |
  | `truncate_output_lines` | `200` (must be ≥ 1) |

  - `harness.claude`: `enabled` (`true`), `data_dir` (`None`), `resume_command` (`"claude --resume {id}"`).
  - `harness.opencode`: `enabled` (`true`), `db_path` (`None`), `resume_command` (`"opencode --session {id}"`).
  - `projects.aliases` maps a path to a display name.
  - `keys.<context>` maps an action name to a string or an array of strings. Keep it raw here; the keymap task validates it.
- **Path values.** `data_dir`, `db_path` and alias keys may start with `~`. Expand it by joining the remainder onto the home directory as `PathBuf` components, never by string concatenation.
- **Warnings.** Loading returns `(Config, Vec<ConfigWarning>)`. A warning names the key and the problem, e.g. `unknown key "harness.claude.colour"`, or `invalid value for "mouse": expected a boolean; using default true`.
  - Unknown keys anywhere except under `keys.*` and `projects.aliases` are warnings.
  - So are invalid values (wrong type, `truncate_output_lines = 0`, and a `timestamp_format` that chrono rejects); each falls back to its default.
  - Warnings are also written to the log.
  - Malformed TOML (syntax error) is one warning, and all defaults are used.
- **Logging.** `init_logging(&Path)` creates the parent directory and appends to the log file. Failure to open the log is not fatal: it returns an error the caller may ignore.
- **Timestamps.** A `TimestampFormatter` is built from `timestamp_format` and `utc`, and takes the display timezone as a parameter (local in production, a fixed offset in tests) so tests are deterministic.
  - `format(DateTime<Utc>)` renders local time with the format. With `utc = true` it renders UTC with a `Z` suffix.
  - `format_short(...)` is used by the selector's session rows. It uses the same format without seconds: remove `%S` together with the separator character before it, remove fractional-second specifiers (`%.f`, `%.3f`, `%.6f`, `%.9f`, `%3f`, …), and replace `%T` with `%H:%M`.

## Acceptance criteria
- [ ] `Paths::from_dirs` resolves the config file to `<config dir>/chronoscope/config.toml`, the themes dir to `<config dir>/chronoscope/themes`, the cache dir to `<cache dir>/chronoscope` and the log file per the planner notes, including the no-state-dir fallback. An explicit config path overrides only the config file. These are unit tests with temp directories and no process-environment mutation. `Paths::detect()` only gathers directories from `etcetera` and delegates to `from_dirs`.
- [ ] A missing config file yields `Config::default()` and no warnings (tested).
- [ ] Every key from the §8.5 example parses into `Config` with the documented types. A test loads the exact example file from §8.5 and asserts each value.
- [ ] Unknown keys produce one warning each, naming the full dotted key. Unknown keys under `keys.*` and `projects.aliases` produce none (tested).
- [ ] Invalid values fall back to their defaults with a warning naming the key: a wrong type for `mouse`, `truncate_output_lines = 0`, and an invalid `timestamp_format` such as `"%Q"`. Malformed TOML gives defaults plus one warning. Each case is tested.
- [ ] `~/foo` in `data_dir`, `db_path` and alias keys expands to `<home>/foo` built with `PathBuf::join`, using an injected home directory (tested).
- [ ] `init_logging` creates missing parent directories and appends lines to the log file (integration test in its own test binary, using a temp dir).
- [ ] `TimestampFormatter::format` with the default format and a fixed `+02:00` offset renders `2026-10-21T06:35:15Z` as `2026-10-21 08:35:15`. With `utc = true` it renders `2026-10-21 06:35:15Z` (tested).
- [ ] `format_short` with the default format renders `2026-10-21 08:35`. With `%d.%m.%Y %T` it renders `21.10.2026 08:35`. With `utc = true` it renders `2026-10-21 06:35Z` (tested).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 001-scaffold-crate
