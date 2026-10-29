# Theme model and built-in themes

## Goal
Implement the theme model of §8.1 as pure library code. That covers the base roles, the component tokens (each derived from a base role unless overridden), colour parsing, the `transparent_background` option, parsing a theme from a TOML document, and the seven built-in themes of §8.3 with palettes ported from ratcn. It also adds two whole-theme transforms that later tasks apply based on the terminal: mapping to the xterm-256 palette, and the colourless `NO_COLOR` variant. Every component built later takes a resolved theme as a prop and reads all its colours and modifiers from it.

## Spec excerpts
From SPEC.md §8.1:

Two layers, modelled on ratcn's shadcn-inspired `Theme`:

1. **Base roles** (required in a theme file): `name`, `foreground`, `muted_foreground`, `background`, `surface`, `field`, `primary`, `primary_foreground`, `secondary`, `secondary_foreground`, `accent`, `destructive`, `destructive_foreground`, `warning`, `border`, `ring`, `cursor`.
2. **Component tokens** (optional overrides; each derived from a base role by default): see the table below.

| Token | Default | Used for |
|---|---|---|
| `user_title` | `primary` | `User` block title |
| `assistant_title` | `accent` | `Assistant` block title, model @ effort |
| `system_title` | `muted_foreground` | System / compaction block titles |
| `timestamp` | `muted_foreground` | Right-aligned block titles |
| `tool_description` | `foreground` | `> description` lines |
| `label` | `muted_foreground` | `Command:` / `Result:` labels |
| `command` | `foreground` | Command / tool input text |
| `output` | `muted_foreground` | Tool results |
| `error_output` | `destructive` | Failed tool results |
| `diff_added` / `diff_removed` | `primary` / `destructive` | Edit diffs |
| `thinking` | `muted_foreground` | Thinking text |
| `selection_marker` | `primary` | `▒` and `>` markers |
| `search_match` | fg `primary_foreground` on `primary` | Search hits (the only explicit background colour; the selector's focused selection uses reverse video, §8.2) |
| `help_key` / `help_text` | `primary` / `muted_foreground` | Help line and help overlay |
| `toast` | fg `foreground` on `surface` | Toast notifications |
| `syntax.*` | derived from `accent`, `primary`, `secondary`, `warning` | Code-block syntax highlighting |

Colours accept `#rrggbb`, ANSI names (`red`, `bright_blue`, …), ANSI indices (`0`–`255`) and `reset` (terminal default).

Per-theme option `transparent_background` (bool): when true, `background` is not painted and the terminal's own background shows through. Users can override it per theme in config.

From SPEC.md §8.3:

Compiled into the binary:

| Id | Display name | Source |
|---|---|---|
| `default` | Default | ratcn `default_dark` (shadcn palette) |
| `terminal` | Terminal | Terminal's own ANSI palette, with `reset` for background and foreground |
| `catppuccin` | Catppuccin | ratcn `catppuccin` (Mocha) |
| `gruvbox` | Gruvbox | ratcn `gruvbox` (dark) |
| `nord` | Nord | ratcn `nord` |
| `tokyo-night` | Tokyo Night | ratcn `tokyo_night` (night) |
| `solarized` | Solarized | ratcn `solarized` (dark) |

- Palette values are ported from ratcn (MIT, with attribution).
- The id is used in `config.toml` and `--theme`. The display name comes from the theme's `name` field.
- The default theme is `default`.

From SPEC.md §9.4:

- Colours: truecolor is assumed when `$COLORTERM` is `truecolor`/`24bit`. Otherwise hex colours are mapped to the nearest xterm-256 colour. The `terminal` theme uses only ANSI colours.
- Respects `NO_COLOR` by falling back to the `terminal` theme with styling (bold, underline, reverse) only.

From SPEC.md §1.3:

- All colours and modifiers come from theme tokens (§8.1), passed in as a prop. Components never hard-code colours.

From SPEC.md §2:

**Decision:** Rust + [tui-realm](https://github.com/veeso/tui-realm) 4.x (`tuirealm`, built on Ratatui 0.30 + crossterm) as the application framework, with custom components for every UI element (§1.3). ratcn is not a runtime dependency; its theme role model is adopted (see §8).

Planner notes (decisions not in the spec; binding for this task):
- **Palette source.** ratcn is not a dependency. Copy its values. The source is in the local cargo registry at `~/.cargo/registry/src/index.crates.io-*/ratcn-0.0.6/src/theme.rs` (the constructors `default_dark`, `terminal`, `catppuccin`, `gruvbox`, `nord`, `tokyo_night`, `solarized`) and `src/color.rs`. If it is missing, `cargo info ratcn@0.0.6` downloads it. Some roles (`ring` everywhere, `surface` in some themes) are computed with ratcn's `dim(color, toward, amount)` (with `RING_DIM = 10`). Port that arithmetic or evaluate it to literal colours, so the values match exactly.
- **Attribution.** Add `THIRD-PARTY-NOTICES.md` at the repo root with ratcn's MIT licence text and copyright line ("Copyright (c) 2026 Kristofer Lund"). Also put a doc comment on the built-in themes module stating that the palettes are ported from ratcn (MIT).
- **Terminal theme is ANSI-only.** ratcn's `terminal()` uses RGB values for `surface`, `field` and `secondary`. Replace them with `black`, `black` and `bright_black` respectively, so the theme uses only named ANSI colours and `reset` (§9.4). Every other value is as in ratcn.
- **Tokens resolve to styles.** Each component token resolves to a `ratatui::style::Style`: fg only, except `search_match` and `toast`, which set fg and bg. Code asks the theme for a token's style or a role's colour (e.g. `theme.style(Token::UserTitle)`, `theme.role(Role::Ring)`). There is no public way to get a colour that bypasses the theme.
- **Syntax tokens.** Define `syntax.keyword` (default `accent`), `syntax.type` (`accent`), `syntax.function` (`primary`), `syntax.string` (`warning`), `syntax.number` (`warning`), `syntax.comment` (`muted_foreground`) and `syntax.plain` (`foreground`). §8.1 lists `secondary` as a source, but `secondary` is a fill colour in every built-in palette (e.g. `#262626` in Default) and is unreadable as text on `background`, so comments use `muted_foreground` instead.
- **Colour names.** Accepted names (case-insensitive) are `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `bright_black`, `bright_red`, `bright_green`, `bright_yellow`, `bright_blue`, `bright_magenta`, `bright_cyan` and `bright_white`. They follow ANSI index semantics 0–15. Map them to Ratatui's named colours: `white` → `Color::Gray`, `bright_black` → `Color::DarkGray`, `bright_white` → `Color::White`, `bright_red` → `Color::LightRed`, and so on.
  - Indices `0`–`255` map to `Color::Indexed`.
  - `#rrggbb` (hex digits case-insensitive) maps to `Color::Rgb`.
  - `reset` maps to `Color::Reset`.
  - Anything else is an error naming the offending value.
- **Theme TOML format** (used by user themes in a later task):
  - Top-level keys: `name` (display name, required), every base role (required), and `transparent_background` (optional, default `false`).
  - An optional `[tokens]` table overrides component tokens by name. Simple tokens take a colour string. `search_match` and `toast` take an inline table `{ fg = "…", bg = "…" }`, and either key may be omitted to keep the default. `diff_added` and `diff_removed` are separate keys. Syntax tokens go in a `[tokens.syntax]` sub-table (`keyword = "…"`).
  - A missing base role is an error naming the role. An invalid colour is an error naming the key. Unknown keys are ignored.
- **256-colour mapping.** `Theme::to_ansi256()` returns a copy in which every `Color::Rgb` is replaced by the nearest xterm-256 colour. Use only the 6×6×6 cube (16–231) and the grey ramp (232–255), never indices 0–15, which terminals redefine. Named, indexed and reset colours are unchanged.
- **NO_COLOR variant.** `Theme::without_colors()` returns a copy in which every role and token colour is `Color::Reset`. `search_match` then carries the `REVERSED` modifier so search hits stay visible. Bold, underline and reverse applied by components keep working. (The environment detection that chooses these transforms is a later task.)

## Acceptance criteria
- [ ] A `Theme` type holds `name`, the 17 base roles listed in §8.1, `transparent_background`, and the component tokens of the §8.1 table plus the `syntax.*` tokens from the planner notes.
- [ ] With no overrides, every token resolves to the default role given in the §8.1 table (and planner notes for `syntax.*`). `search_match` resolves to fg `primary_foreground` on bg `primary`, and `toast` to fg `foreground` on bg `surface`. A table-driven unit test covers every token.
- [ ] Overriding a token changes only that token. Changing a base role changes every token derived from it that is not overridden. Both are unit-tested.
- [ ] Colour parsing accepts `#rrggbb` (either case), all 16 ANSI names, indices `0`–`255` and `reset`. It rejects malformed input (`#12345`, `256`, `purple`, empty string) with an error naming the value. Table-driven unit tests cover these.
- [ ] Seven built-in themes exist, in this order, with these ids and display names: `default`/Default, `terminal`/Terminal, `catppuccin`/Catppuccin, `gruvbox`/Gruvbox, `nord`/Nord, `tokyo-night`/Tokyo Night, `solarized`/Solarized. The default theme id is `default`.
- [ ] Each built-in's 17 base-role values equal ratcn 0.0.6's values (with the Terminal substitutions from the planner notes), including the `dim(...)`-derived ones. A unit test asserts every role of every built-in against literal expected values.
- [ ] The Terminal theme's resolved roles and tokens contain only named ANSI colours or `reset`, never `Rgb` or `Indexed`. A test asserts this.
- [ ] Parsing a complete theme TOML document produces the expected theme. A missing base role gives an error naming it. A token override in `[tokens]` / `[tokens.syntax]` and an `{ fg, bg }` override of `search_match` are applied. Unit tests cover all of these.
- [ ] `to_ansi256()` maps `#ff0000` → 196, `#000000` → 16, `#ffffff` → 231 and `#808080` → 244, and leaves named and `reset` colours unchanged (unit-tested).
- [ ] `without_colors()` yields only `Color::Reset` colours, and `search_match` has the `REVERSED` modifier (unit-tested).
- [ ] `THIRD-PARTY-NOTICES.md` contains ratcn's MIT licence and copyright, and the built-in themes module's doc comment credits ratcn.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 001-scaffold-crate
