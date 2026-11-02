# Help overlay

## Goal
Add the `HelpOverlay` (§4.3), opened with `?` from the browser or the selector. It lists keybindings grouped by purpose, generated from the active keymap so that remappings always show. It is a centred rounded overlay with a `ring` border on `surface`. It scrolls when taller than the screen, closes with `Esc` or `?` (focus returns to where it was), and mentions that holding `Shift` allows native text selection while mouse capture is on.

## Spec excerpts
From SPEC.md §4.3:

- **Help overlay** (`?`): grouped keybindings as in ui-sketch.md, generated from the active keymap. `Esc` / `?` closes it.

Overlays are centred, use rounded borders with `ring` colour, and paint `surface` behind them (unless the background is transparent, in which case `Clear` is still used so underlying text doesn't bleed through).

From ui-sketch.md (help overlay; see the overrides in the planner notes):

```text
 Help Overlay                                           ┃
                                                        ▼
╭ Keybindings ───────────────────────────────────────────────────╮
│                                                                │
│  Navigation                                                    │
│                                                                │
│    Up / Down / j / k      Previous or next item                │
│    [ / ]                  Previous or next section             │
│                                                                │
│  Sections                                                      │
│                                                                │
│    ENTER                  Fold or unfold current section       │
│    Shift+ENTER            Unfold all sections                  │
│    Ctrl-ENTER             Fold all sections                    │
│    y / Ctrl-c             Copy current selection to clipboard  │
│    w / Ctrl-s             Save current selection to file       │
│                                                                │
│  Search                                                        │
│                                                                │
│    /                      Search for text in transcript        │
│    n / N                  Next or previous match               │
│    g / Home               Jump to start                        │
│    G / End                Jump to end                          │
│    ESC                    Clear search                         │
│                                                                │
│  Sessions                                                      │
│                                                                │
│    o                      Open another session                 │
│                                                                │
│  Application                                                   │
│                                                                │
│    e / Ctrl-e             Export current session to markdown   │
│    Ctrl-q / Ctrl-w        Quit application                     │
│    ?                      Show this screen                     │
│                                                                │
╰────────────────────────────────────────────────────────────────╯
```

From SPEC.md §1.4:

- **Overlays and prompts:** opening one mounts its component and gives it focus. Closing it unmounts it and returns focus to the component that had it before.

- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, §6; `Esc` / `?` closes help). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (§6.6).

From SPEC.md §6.5:

| Key | Action |
|---|---|
| `?` | Help overlay |

From SPEC.md §6.9:

- Mouse capture can be disabled in config (`mouse = false`). Most terminals also bypass mouse capture while `Shift` is held, which allows native text selection; the help overlay mentions this.

From SPEC.md §8.5:

Key syntax: `a`, `A`, `ctrl-d`, `shift-tab`, `enter`, `esc`, `pgdn`, `f5`, and two-key chords as space-separated keys (`"g g"`; only in user config, since no default binding is a chord). The help overlay is generated from the active keymap, so it always reflects the user's remappings.

From SPEC.md §8.1:

| Token | Default | Used for |
|---|---|---|
| `help_key` / `help_text` | `primary` / `muted_foreground` | Help line and help overlay |

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
- **SPEC.md and the keymap override the sketch's content.** The overlay is generated from task 005's `help_entries`, so:
  - unfold/fold all show `+ / =` and `-`, not `Shift+ENTER` and `Ctrl-ENTER` (no default uses modifier+Enter);
  - opening another session shows `Ctrl-o`, not `o`;
  - copy shows `y` only: `Ctrl-c` always quits and is never copy;
  - `[` and `]` are "Previous turn" / "Next turn", which jump between turn blocks;
  - there is one row per action, using the descriptions and groups of task 005, rather than the sketch's merged rows.

  For consistency with the other blocks, the title is drawn as `╭─ Keybindings ─…╮`, not the sketch's `╭ Keybindings`.
- **Content by context.**
  - Opened from the browser, the groups are Navigation, Sections, Search, Sessions and Application: the browser-context entries plus the global ones in their groups (`open_selector` in Sessions; `theme`, `help` and `quit` in Application).
  - Opened from the selector, the groups are Session selector (with `harness_N` rows only for registered harnesses) and Application (`theme`, `help`, `quit`).
  - The `quit` row's keys are the keymap keys followed by ` / Ctrl-c`.
  - When mouse capture is enabled, a final row after the last group reads `Hold Shift to select text with the mouse` in `help_text`.
- **Layout.**
  - The overlay is centred, with a rounded border in `ring`, the interior filled with `surface` (or `Clear` only when transparent), and the title `Keybindings` in `foreground`.
  - Rows:
    - row 1 is blank;
    - each group has a heading at column 3 in bold `foreground`, a blank row, then its entries;
    - an entry has its keys at column 5 in `help_key`, padded to the widest keys label + 6, followed by the description in `help_text`;
    - a blank row follows each group's entries.
  - Width: `min(w − 4, widest entry + 8)`, at least 40. Height: `min(h − 2, content + 2)`.
  - When the content overflows, the top border shows ` ↑ more ` and/or the bottom border shows ` ↓ more `, right-aligned with one `─` before the corner, in `muted_foreground`, as applicable.
- **Keys.**
  - The global `help` key returns `Msg::OpenHelp(context)`, and the `App` mounts the overlay and pushes focus.
  - In the overlay, `overlay.up`/`overlay.down` scroll by one row, `PgUp`/`PgDn` by a page, and `overlay.close` (`Esc`) or the global `help` key (`?`) returns `Msg::CloseHelp`.
  - Other global keys do not fire while it has focus (subscription clause), except `Ctrl-c`, which quits.
  - Mouse handling is a later task.
- The overlay is drawn last, over the current screen (browser or selector), which stays visible around it.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] From the browser, the overlay lists every browser and relevant global action in the planner-notes groups and order, with keys from `Keymap::labels` (component test on the entries model).
- [ ] Remapping `[keys.browser] copy = "c"` shows `c` on the copy row (test).
- [ ] From the selector, it lists the Session selector group (only registered harness rows) and Application (test).
- [ ] The quit row shows `q / Ctrl-q / Ctrl-w / Ctrl-c` (test).
- [ ] The Shift note appears only when `mouse = true` (test).
- [ ] `?` opens it and focus moves to it. `Esc` and `?` close it and restore the previous focus. `q` and `t` do nothing while it is open. `Ctrl-c` quits (update tests).
- [ ] Scrolling with `j`/`k`/`↓`/`↑`/`PgDn`/`PgUp` works and clamps. The `↑ more`/`↓ more` indicators appear accordingly (component tests).
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
A new centred overlay:

```text
╭─ Keybindings ──────────────────────────────────────────╮
│                                                        │
│  Navigation                                            │
│                                                        │
│    Up / k            Previous section                  │
│    Down / j          Next section                      │
│    [                 Previous turn                     │
│    …                                                   │
│                                                        │
│  Sections                                              │
│    …                                                   │
╰─────────────────────────────────────────────── ↓ more ─╯
```

- Border in `ring`, with `surface` behind it.
- Group headings bold `foreground` at column 3.
- Keys in `help_key` at column 5, descriptions in `help_text`.
- ` ↑ more ` / ` ↓ more ` in `muted_foreground` on the border when scrollable.

Snapshots (screen through `App::view`):
1. From the browser at 120x40: top of the list with `↓ more` (text + styled).
2. From the browser at 120x40, scrolled to the end: `↑ more` and the Shift note visible (text).
3. From the browser at 80x24 (text).
4. From the browser at 200x60: everything fits, no indicators (text).
5. From the selector at 120x40: Session selector + Application groups, with the selector visible around the overlay (text).
6. With `[keys.browser] copy = "c"` at 120x40: the copy row shows `c` (text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 021-browser-screen
