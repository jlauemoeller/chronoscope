# Theme selector overlay with live preview

## Goal
Add the `ThemeSelector` overlay (§8.4), opened with `t` from the browser or the selector. It is a small centred list of the registry's themes:
- built-ins in §8.3 order, then user themes alphabetically;
- invalid user themes shown muted as `(invalid)` and not selectable.

The cursor starts on the active theme. Moving it applies the highlighted theme immediately as a preview to everything on screen. `Enter` keeps it and saves `theme = "<id>"` to `config.toml` with `toml_edit`, preserving the user's comments and formatting, or creating the file if missing. `Esc` restores the theme that was active when the overlay opened.

## Spec excerpts
From SPEC.md §8.4:

Layout per [theme-selector.md](spec/theme-selector.md):

```
╭─ Themes ──────────────────────────────╮
│                                       │
│    Default                            │
│  > Terminal                           │
│    Catppuccin                         │
│    Gruvbox                            │
│    Nord                               │
│    Tokyo Night                        │
│    Solarized                          │
│                                       │
│ ENTER to select | ESC to cancel       │
╰───────────────────────────────────────╯
```

- Opened with `t` from the browser or the selector. It is a small centred overlay drawn on top of the current screen, so the preview is visible on real content around it.
- Lists the built-in themes in the order of §8.3, followed by user themes alphabetically. If there are more themes than fit, the list scrolls.
- On open, the cursor is on the active theme.
- `↑` / `↓` or `j` / `k` move the cursor. The highlighted theme is **applied immediately as a preview**, to the overlay and to everything behind it.
- `ENTER` selects the highlighted theme, closes the overlay and saves the choice to `config.toml`.
- `ESC` closes the overlay and restores the theme that was active when it opened.
- The selected row follows §8.2 (`>` marker in `primary`, bright bold text, not inverted); other rows use `muted_foreground`. The help line uses `help_key` / `help_text`.
- A user theme that fails to parse is listed muted with `(invalid)` and can't be selected. The error goes to the log.
- The config file is updated with `toml_edit`, so the user's comments and formatting survive. If no config file exists, one is created containing just `theme = "…"`.

From theme-selector.md:

### Theme Selector

- Up / Down or j / k changes highlighted line and previews the highlighted theme. 
- ENTER selects and closes overlay
- ESC cancels and closes overlay without changing theme.

```
  ╭─ Themes ──────────────────────────────╮ 
  │                                       │
  │    Default                            │
  │  > Terminal                           │
  │    Catpucchin                         │
  │    Gruvbox                            │
  │    Nord                               │
  │    Tokio Night                        │
  │    Solarized                          │
  │                                       │ 
  │ ENTER to select | ESC to cancel       │
  ╰───────────────────────────────────────╯   
```

From SPEC.md §8.3:

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

User themes: `$XDG_CONFIG_HOME/chronoscope/themes/<name>.toml`. A user theme with the same name as a built-in one replaces it.

From SPEC.md §4.3:

- **Theme selector** (`t`): see §8.4.

Overlays are centred, use rounded borders with `ring` colour, and paint `surface` behind them (unless the background is transparent, in which case `Clear` is still used so underlying text doesn't bleed through).

From SPEC.md §1.4:

- **Overlays and prompts:** opening one mounts its component and gives it focus. Closing it unmounts it and returns focus to the component that had it before.

- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, §6; `Esc` / `?` closes help). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (§6.6).

From SPEC.md §8.5:

- Location: `$XDG_CONFIG_HOME/chronoscope/config.toml` (`~/.config/chronoscope/config.toml`), via `etcetera`. On macOS the XDG-style path is used as well, so dotfiles stay portable. On Windows: `%APPDATA%\chronoscope\config.toml`.
- All keys are optional. A missing file means defaults.

From SPEC.md §8.2:

Selection is shown with **text brightness and markers**. The one background band is the session selector's focused selection:

- Unselected list rows and transcript sections: `muted_foreground`.
- Selected transcript section and theme-selector row: `foreground`, bold, with a marker (`>` in the theme list, `▒` over the left border in the transcript) in `primary`.
- Session selector (Projects and Sessions lists):
  - Selected row in the **focused** list: the `>` marker in `primary`, never inverted. Everything after the marker is one **inverted** band (`foreground` with the reverse-video modifier, not bold), running up to the row's right margin (two cells before the pane border). Inside the band, the timestamp, live dot, title, gaps and stats all take the band's style.
  - Selected row in the **unfocused** list (e.g. Projects while Sessions has focus): text **bold** in `secondary_foreground`, not inverted; the `>` marker in the same `secondary_foreground` (not bold), so the context stays visible.
- Focused pane border: `ring`; unfocused borders: `border`.
- Selected harness tab: `primary`, underlined; other tabs: `muted_foreground`.
- In the transcript, dimming of unselected sections is controlled by `dim_unselected` (default `true`). When `false`, all sections use their normal token colours and only the marker + bold indicate selection.

From SPEC.md §1.3:

**Every custom UI element is its own component.** The code is organised around components, not around long render functions or a central key handler.

- A component implements tui-realm's `Component` trait (`view`, `query`/`attr`, `state`, `perform(Cmd) -> CmdResult`), plus `AppComponent` (`on(&Event) -> Option<Msg>`) if it handles events. An element that appears on several screens (e.g. the help line) is one component type, mounted with different props.
- A component draws only itself. Its `view` draws its own border and content, and leaves sub-elements to child types: a child component it owns, or a small `Widget`/`StatefulWidget` type.
- **No long render functions.** A render function (`view`, or a helper it calls) that draws more than one distinct element, or grows past about 50 lines, is split into smaller components or widgets. Rendering has no side effects.
- Components are `pub` types that can be built from plain data (props and the theme) and rendered into a `TestBackend` frame on their own, without an `Application` or a terminal (§9.6).
- All colours and modifiers come from theme tokens (§8.1), passed in as a prop. Components never hard-code colours.
- `tui-realm-stdlib` is not used. The look in §4 and §8 is specific enough that wrapping stdlib components would mean overriding their rendering. Prompt text editing uses `tui-input` inside the `Prompt` component.

Components (each is its own type and module):

| Component | Where | Draws / handles | Takes focus |
|---|---|---|---|
| `SelectorFrame` | Selector | Overlay border, sizing (§4.1) and background; nothing inside | No |
| `HarnessTabs` | Selector | Harness tabs; click switches tab (§6.6) | No |
| `ProjectList` | Selector | Projects pane, selection rules of §4.1 | Yes |
| `SessionList` | Selector | Sessions pane, including search results with the project prefix and the `●` marker | Yes |
| `WorkingDir` | Selector | Working Directory box | No |
| `BrowserHeader` | Browser | Project and session titles, cwd, `● live`, breadcrumb, parse-warning note | No |
| `Transcript` | Browser | The turn blocks: selection, folding, navigation, search highlights (§5, §6.1–§6.3) | Yes |
| `HelpLine` | Both | Key hints and status items (`3/17`, `↓ N new`, `Loading… 34%`) | No |
| `Prompt` | Both | Search and path prompts and `y/n` confirmations, drawn in place of the help line while active | Yes (input mode, §6) |
| `Toast` | Both | Bottom-right toast; expires on `Tick` | No |
| `HelpOverlay` | Overlay | §4.3 | Yes |
| `ThemeSelector` | Overlay | §8.4 | Yes |
| `GlobalEvents` | Both | Nothing drawn; turns global keys and worker events into `Msg`s (§1.4) | No |
| `EmptyState` | Both | "Terminal too small", "No sessions for this harness", "Session file is empty" | No |

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (§9.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (§5.2), one body renderer per `ToolKind` (§5.3), the markdown renderer (§5.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (§5.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **SPEC.md overrides theme-selector.md.** The display names are `Catppuccin` and `Tokyo Night`, not "Catpucchin" and "Tokio Night". The list content comes from the registry (task 004).
- **Layout.**
  - Centred overlay. Width `max(41, longest label + 10)`, capped at `w − 4`. Height `min(h − 2, rows + 5)`.
  - Rounded border in `ring`, with title `╭─ Themes ─…╮` in `foreground` and `surface` fill (`Clear` only when transparent).
  - Inner rows: blank; theme rows (marker `>` at column 3, label from column 5); blank; the help row; then the bottom border.
  - The help row starts at column 2: `<overlay.select> to select | <overlay.close> to cancel` (default `ENTER to select | ESC to cancel`), keys in `help_key` and text in `help_text`.
  - The list scrolls to keep the cursor visible when it has more rows than fit.
- **Row styles (§8.2).**
  - The cursor row has `>` in `primary`, and its label in bold `foreground`, not inverted.
  - Other rows are `muted_foreground`.
  - Invalid entries show `<id> (invalid)` in `muted_foreground` and are skipped by the cursor.
- **Behaviour.**
  - The global `theme` key (`t`) returns `Msg::OpenThemeSelector`. The `App` mounts the overlay with the cursor on the active theme's id, remembers that id, and pushes focus.
  - `overlay.up`/`overlay.down` move the cursor (no wrap) and return `Msg::PreviewTheme(id)`. The `App` resolves it with task 004's `resolve_by_id` (applying the same `TerminalEnv` and transparent override as at startup), makes it the active theme, and pushes the theme prop to every mounted component, the overlay included.
  - `overlay.select` (`Enter`) returns `Msg::SelectTheme(id)`. The `App` keeps the theme, saves it to config, and closes the overlay, restoring focus.
  - `overlay.close` (`Esc`) returns `Msg::CancelTheme`. The `App` re-applies the remembered theme and closes.
  - Global keys don't fire while the overlay has focus, except `Ctrl-c`.
  - Under `NO_COLOR`, previews still switch the theme id, but every theme renders colourless (task 004 rules).
- **Saving.**
  - Use `toml_edit::DocumentMut`. If the config file exists, set or replace the top-level `theme` value and keep everything else byte-identical, comments included.
  - If it doesn't exist, create the parent directory and write exactly `theme = "<id>"\n`.
  - If the existing file is not valid TOML, don't touch it; show a toast `Could not save theme: config.toml is invalid`.
  - On a write error, toast `Could not save theme: <error>`. The theme stays applied for this run either way.
  - The config path is the one in use, including `--config` later.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] The list shows the built-ins in §8.3 order with display names, then user themes alphabetically. An invalid user theme shows `<id> (invalid)` muted, and the cursor skips it (component tests with a temp themes dir).
- [ ] On open, the cursor is on the active theme (test).
- [ ] Moving the cursor emits `PreviewTheme`, and the `App` applies it to all mounted components: a screen snapshot shows the new colours behind the overlay (update test plus styled snapshot).
- [ ] `Esc` restores the original theme exactly. `Enter` keeps the new one and closes (update tests).
- [ ] Saving into an existing config with comments and other keys changes only the `theme` value: a byte-for-byte comparison of everything else (test). A missing config is created with exactly `theme = "<id>"\n` (test). An invalid TOML config is left untouched and a toast is shown (test).
- [ ] `t` opens the overlay from both the browser and the selector. `q` does nothing while it is open, and `Ctrl-c` quits (update tests).
- [ ] With more themes than fit (12 themes at 60x20), the list scrolls to keep the cursor visible (component test plus snapshot).
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
A new centred overlay, as in theme-selector.md (with the corrected names):

```text
╭─ Themes ──────────────────────────────╮
│                                       │
│    Default                            │
│  > Terminal                           │
│    Catppuccin                         │
│    Gruvbox                            │
│    Nord                               │
│    Tokyo Night                        │
│    Solarized                          │
│                                       │
│ ENTER to select | ESC to cancel       │
╰───────────────────────────────────────╯
```

- Border in `ring`, with `surface` fill.
- The cursor row has `>` in `primary` and a bold `foreground` label. Other rows are `muted_foreground`. Invalid user themes are `my-theme (invalid)` in `muted_foreground`.
- The help row uses `help_key`/`help_text`.
- Drawn over the current screen, which uses the previewed theme.

Snapshots (screen through `App::view`, browser fixture behind unless noted, `utc = true`):
1. Opened over the browser at 120x40 with the cursor on `Default` (text + styled).
2. After `j` ×3: cursor on `Gruvbox`, with the whole screen (overlay and browser behind it) rendered in Gruvbox colours, at 120x40 (styled).
3. Opened over the selector at 80x24 (text).
4. With two user themes, one of them invalid, at 120x40 (text + styled: `(invalid)` muted).
5. 12 themes at 60x20, with the cursor near the end and the list scrolled (text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 004-theme-registry
- 021-browser-screen
