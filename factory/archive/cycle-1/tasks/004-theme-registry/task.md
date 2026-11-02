# Theme registry and active theme resolution

## Goal
Build the theme registry and decide which theme is active at startup. The registry lists the built-in themes in §8.3 order, followed by user themes loaded from `<config dir>/chronoscope/themes/*.toml` in alphabetical order. A user theme with a built-in's name replaces that built-in. A user theme that fails to parse is kept as an invalid, unselectable entry, and the error is logged. Resolution applies the config's `theme` and `transparent_background` and then the terminal's capabilities: `COLORTERM` truecolor versus xterm-256 mapping, and `NO_COLOR`. The theme selector overlay and the app startup use this registry later.

## Spec excerpts
From SPEC.md §8.1:

Per-theme option `transparent_background` (bool): when true, `background` is not painted and the terminal's own background shows through. Users can override it per theme in config.

From SPEC.md §8.3:

- Palette values are ported from ratcn (MIT, with attribution).
- The id is used in `config.toml` and `--theme`. The display name comes from the theme's `name` field.
- The default theme is `default`.

User themes: `$XDG_CONFIG_HOME/chronoscope/themes/<name>.toml`. A user theme with the same name as a built-in one replaces it.

From SPEC.md §8.4:

- Lists the built-in themes in the order of §8.3, followed by user themes alphabetically. If there are more themes than fit, the list scrolls.

- A user theme that fails to parse is listed muted with `(invalid)` and can't be selected. The error goes to the log.

From SPEC.md §8.5 (config example, first lines):

```toml
theme = "tokyo-night"
transparent_background = false   # overrides the theme's setting when present
```

From SPEC.md §9.4:

- Colours: truecolor is assumed when `$COLORTERM` is `truecolor`/`24bit`. Otherwise hex colours are mapped to the nearest xterm-256 colour. The `terminal` theme uses only ANSI colours.
- Respects `NO_COLOR` by falling back to the `terminal` theme with styling (bold, underline, reverse) only.

Planner notes (decisions not in the spec; binding for this task):
- **Theme ids.** A user theme's id is its file stem (`themes/my-dark.toml` → `my-dark`), and its display name is its `name` field. "Same name as a built-in" means the same id. A replacing user theme keeps the built-in's position in the list. Only non-replacing user themes are appended, sorted alphabetically by id.
- **Invalid user themes.** Each registry entry is either valid (holds a `Theme`) or invalid (holds the id, a display label equal to the id, and the parse error). Invalid entries are listed but cannot be selected or resolved. Their error goes to the log (`log::warn!`). The parser comes from task 002.
- **Resolution inputs.** The resolver takes the registry, the config's `theme` id, the config's `transparent_background: Option<bool>`, and an explicit `TerminalEnv { colorterm: Option<String>, no_color: bool }`. Build `TerminalEnv` from the process environment in one small function, so tests never touch real env vars. It returns the resolved `Theme` plus any warnings (same warning type/style as the config warnings from task 003, so they can be shown together).
  - An unknown theme id, or an id naming an invalid user theme, falls back to `default` with a warning.
  - `transparent_background` from config, when present, overrides the theme's own value.
  - `NO_COLOR` set (any non-empty value, per no-color.org) wins over the configured theme. The result is the `terminal` theme passed through `without_colors()`, with no warning.
  - Otherwise, when `COLORTERM` is `truecolor` or `24bit` (case-insensitive), RGB colours are kept. In every other case the theme is passed through `to_ansi256()`.
- Keep the registry and resolver free of terminal I/O. Later tasks also resolve a theme by id when the user previews or selects one in the theme selector, and apply the same terminal transforms. Expose that as a function, e.g. `resolve_by_id(registry, id, &TerminalEnv, transparent_override)`.

## Acceptance criteria
- [ ] With no user themes directory, the registry lists exactly the seven built-ins in §8.3 order (tested).
- [ ] User themes `zeta.toml` and `alpha.toml` are appended after the built-ins as `alpha`, `zeta`. A user `nord.toml` replaces the built-in Nord at its position, with its own display name. Tests use a temp themes dir.
- [ ] A user theme file with a missing base role becomes an invalid entry. It is listed with its id, is reported as not selectable, and resolving it returns an error. The parse error is logged. Tests cover each of these.
- [ ] The config theme `tokyo-night` with `COLORTERM=truecolor` resolves to the Tokyo Night palette with RGB colours intact (tested).
- [ ] The same theme with `COLORTERM` unset resolves to indexed colours in the 16–255 range for every role that was RGB (tested).
- [ ] An unknown config theme id resolves to `default` with one warning naming the id. An id naming an invalid user theme does the same (tested).
- [ ] `transparent_background = true` in config makes the resolved theme transparent even if the theme says `false`, and `false` overrides a theme that says `true` (tested).
- [ ] `NO_COLOR=1` resolves to the Terminal theme with all colours `Color::Reset`, regardless of the configured theme (tested).
- [ ] No function in the registry or resolver reads environment variables except the single `TerminalEnv::from_env()` constructor.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 002-theme-model
- 003-config-file
