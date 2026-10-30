STATUS: DONE

## Changes
- `src/theme/registry.rs` (new): `ThemeRegistry` (`builtin()`, `load(themes_dir)`, `entries()`, `get(id)`) and `ThemeEntry` (`Valid { id, theme }` / `Invalid { id, label, error }` with `id()`, `label()`, `is_selectable()`, `theme()`, `error()`). Built-ins come first in §8.3 order; user `*.toml` files (id = file stem) replace a same-id built-in in place, other user themes are appended sorted by id. Missing dir = no user themes. Parse/read failures become invalid entries and are logged with `log::warn!`.
- `src/theme/resolve.rs` (new): `TerminalEnv { colorterm, no_color }` with `from_env()` (the only env read) and `truecolor()`; `resolve_by_id(registry, id, &env, transparent_override) -> Result<Theme, ResolveError>`; `resolve(registry, theme_id, transparent_background, &env) -> (Theme, Vec<ConfigWarning>)`. Order: pick theme, apply transparent override, then NO_COLOR (builtin Terminal theme through `without_colors()`) / truecolor (keep RGB) / otherwise `to_ansi256()`.
- `src/theme/builtin.rs`: added `TERMINAL_ID`. `src/theme/mod.rs`: module declarations and re-exports.
- `tests/theme_registry_log.rs` (new): checks the parse error reaches the log file (own binary, global logger).

Decisions on points the task left open:
- An invalid user file whose id equals a built-in replaces that built-in as an invalid entry (keeps the problem visible; the built-in is then unselectable).
- With `NO_COLOR`, `resolve` returns the Terminal theme without validating the configured id (no warning, per the planner note). `resolve_by_id` still errors for unknown/invalid ids first, then applies NO_COLOR.
- Fallback to `default` uses the registry's `default` entry, or the built-in default if a user `default.toml` is invalid. The fallback warning (key `theme`) is also logged.
- The configured `transparent_background` override is also applied under `NO_COLOR`.

## Self-check
- AC: seven built-ins in order with no user dir — `registry::tests::missing_directory_gives_exactly_the_seven_builtins_in_order`, `empty_directory_gives_only_the_builtins`
- AC: zeta/alpha appended; nord replaced in place with own display name — `user_themes_are_appended_alphabetically_by_id`, `user_theme_with_a_builtin_id_replaces_it_in_place`
- AC: missing base role -> invalid, listed by id, not selectable, resolve errors, error logged — `theme_with_a_missing_base_role_is_an_invalid_entry`, `resolve::tests::resolve_by_id_reports_unknown_and_invalid_ids`, `tests/theme_registry_log.rs::invalid_user_theme_error_is_written_to_the_log_file`
- AC: tokyo-night + truecolor keeps RGB — `truecolor_keeps_the_palette_with_rgb_intact`
- AC: COLORTERM unset -> indexed 16-255 — `unset_colorterm_maps_every_rgb_role_to_indexed_16_to_255`, `other_colorterm_values_also_map_to_256`
- AC: unknown / invalid id -> default + one warning naming the id — `unknown_theme_falls_back_to_default_with_one_warning_naming_the_id`, `invalid_user_theme_falls_back_to_default_with_one_warning_naming_the_id`
- AC: transparent override both ways — `config_transparent_true_overrides_a_theme_that_says_false`, `config_transparent_false_overrides_a_theme_that_says_true`
- AC: NO_COLOR -> Terminal theme, all Reset — `no_color_gives_the_terminal_theme_with_all_colours_reset`
- AC: only `TerminalEnv::from_env` reads env — `registry::tests::registry_never_reads_the_environment`, `resolve::tests::colorterm_and_no_color_are_parsed_from_the_environment_in_one_place`
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all passed, 0 failed)
