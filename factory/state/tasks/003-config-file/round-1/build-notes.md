STATUS: DONE

## Changes
- `Cargo.toml`: added `chrono`, `log` (feature `std`), dev-dependency `tempfile`. No `simplelog`: a minimal file logger is implemented instead (allowed by the planner notes).
- `src/paths.rs`: `Paths` (`from_dirs` pure constructor, `with_config_file`, `detect` via `etcetera::choose_base_strategy`), `expand_tilde` (component-based, `PathBuf::join`). `Paths` also carries `home_dir`.
- `src/config/mod.rs`: `Config` and sub-structs with defaults; `Config::load(path, home)` and `Config::from_toml_str(text, home)` return `(Config, Vec<ConfigWarning>)`. Walks a `toml::Table`, removing consumed keys so leftovers are unknown-key warnings (not under `keys.*` / `projects.aliases`). Invalid values, `truncate_output_lines < 1`, invalid strftime (via chrono `Item::Error`) and malformed TOML warn and use defaults. Warnings are also sent to `log::warn!`. `keys` kept raw as `context -> action -> Vec<String>` (string normalised to one-element list).
- `src/config/tests.rs`: config unit tests (incl. verbatim §8.5 example loaded from a temp file).
- `src/logging.rs`: `init_logging(&Path)` creates parent dirs, appends, installs a `log` backend (Info level).
- `src/timestamp.rs`: `TimestampFormatter` with `DisplayTz::{Local, Fixed}`, `format`, `format_short` (tokenised removal of `%S` + separator, fractional specifiers, `%T` -> `%H:%M`).
- `tests/logging.rs`, `tests/logging_append.rs`: integration tests, each its own binary (global logger).
- `src/lib.rs`: new modules.

## Self-check
- AC: Paths::from_dirs / fallback / explicit config path — `paths::tests::from_dirs_resolves_all_locations`, `log_file_falls_back_to_cache_dir_without_state_dir`, `explicit_config_file_overrides_only_the_config_file`
- AC: missing file — `config::tests::missing_file_yields_defaults_without_warnings`
- AC: §8.5 example — `config::tests::spec_example_loads_from_a_file_with_every_value`
- AC: unknown keys — `unknown_keys_warn_once_each_with_the_full_dotted_key`, `unknown_keys_under_keys_and_aliases_do_not_warn`
- AC: invalid values — `wrong_type_falls_back_to_default_with_a_warning`, `zero_truncate_output_lines_falls_back_to_default`, `invalid_timestamp_format_falls_back_to_default`, `malformed_toml_gives_defaults_and_one_warning`
- AC: tilde expansion — `config::tests::tilde_expands_in_data_dir_db_path_and_alias_keys`, `paths::tests::tilde_expands_to_home`
- AC: init_logging — `tests/logging.rs`, `tests/logging_append.rs`
- AC: format — `timestamp::tests::format_renders_in_the_display_timezone`, `format_with_utc_adds_z_suffix`
- AC: format_short — `timestamp::tests::format_short_drops_seconds`, `short_format_strips_seconds_and_fractions`
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (78 passed, 0 failed)
