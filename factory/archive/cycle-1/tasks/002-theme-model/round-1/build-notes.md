STATUS: DONE

## Changes
- `src/theme/mod.rs`: `Theme` (name, `transparent_background`, `Palette` of the 16 colour base roles, per-token overrides), `Role`/`Token` enums (with `ALL` and `key()`), `TokenOverride`, `Theme::role`, `Theme::style` (fg only; fg+bg for `search_match`/`toast`), builders `with_role`/`with_token`/`with_transparent_background`, `to_ansi256()`, `without_colors()`, `from_toml()`. Tokens are resolved lazily from roles, so changing a role moves every non-overridden derived token. Colours are only readable through `role()`/`style()`.
- `src/theme/color.rs`: `parse_color` (`#rrggbb` any case, 16 ANSI names case-insensitive, 0-255, `reset`; error names the value) and the xterm-256 nearest mapping (cube 16-231 + grey ramp 232-255, never 0-15).
- `src/theme/parse.rs`: TOML parsing (`ThemeError`: Syntax / MissingKey / WrongType / InvalidColor, each naming the key). `[tokens]`, `[tokens.syntax]`, `{ fg, bg }` for `search_match`/`toast`; unknown keys ignored.
- `src/theme/builtin.rs`: the 7 built-ins (`IDS`, `all()`, `get(id)`, `default_theme()`, `DEFAULT_ID`), palettes ported from ratcn 0.0.6 with ratcn's `dim` arithmetic for `ring`/`surface`; Terminal uses ANSI-only substitutions. Module doc credits ratcn (MIT).
- `THIRD-PARTY-NOTICES.md`: ratcn MIT licence and copyright.
- `src/lib.rs`: `pub mod theme;`.
- Note: the spec's "17 base roles" includes `name`; there are 16 colour roles (`Role::ALL`) plus the `name` field.
- Syntax token defaults follow the planner notes (comment uses `muted_foreground`).

## Self-check
- AC: Theme type holds name, roles, transparent_background, tokens + syntax tokens — `theme::tests::defaults_table_covers_every_token_once`, `every_token_resolves_to_its_default_role`
- AC: default token resolution incl. search_match/toast, table-driven — `every_token_resolves_to_its_default_role`, `search_match_and_toast_have_their_explicit_backgrounds`
- AC: token override affects only that token; role change propagates — `overriding_a_token_changes_only_that_token`, `overriding_one_part_of_a_two_colour_token_keeps_the_other`, `changing_a_role_changes_every_derived_token_that_is_not_overridden`
- AC: colour parsing accept/reject — `theme::color::tests::parses_valid_colours`, `rejects_malformed_colours_naming_the_value`
- AC: seven built-ins, order, ids, names, default id — `builtin::tests::seven_builtins_in_order_with_ids_and_display_names`, `default_theme_is_the_default_id`, `unknown_id_is_none`
- AC: every role of every built-in vs literals — `builtin::tests::every_role_of_every_builtin_matches_ratcn`, `dim_matches_ratcn_rounding`
- AC: Terminal ANSI-only — `builtin::tests::terminal_theme_uses_only_ansi_colours_and_reset`
- AC: TOML parsing, missing role named, token / syntax / search_match overrides — `theme::parse::tests::*` (`complete_document_parses_to_the_expected_theme`, `each_missing_base_role_or_name_is_named_in_the_error`, `simple_token_and_syntax_overrides_are_applied`, `search_match_accepts_fg_and_bg_and_either_may_be_omitted`, ...)
- AC: to_ansi256 values — `color::tests::ansi256_mapping_of_known_colours`, `ansi256_never_uses_the_first_sixteen_and_only_touches_rgb`, `theme::tests::to_ansi256_maps_rgb_in_roles_and_overrides`, `to_ansi256_keeps_the_terminal_theme_unchanged`
- AC: without_colors — `theme::tests::without_colors_resets_everything_and_reverses_search_match`
- AC: THIRD-PARTY-NOTICES.md and module doc — `THIRD-PARTY-NOTICES.md`, doc comment atop `src/theme/builtin.rs`
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (45 passed, 0 failed)
