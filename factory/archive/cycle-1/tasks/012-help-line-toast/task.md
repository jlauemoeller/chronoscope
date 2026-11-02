# Help line and toasts

## Goal
Add two shared components:
- **`HelpLine`** shows context-sensitive key hints, generated from the active keymap, and right-aligned status items. It appears inside the selector overlay first; the browser reuses it later.
- **`Toast`** is a small notification in the bottom-right corner that expires on `Tick` after about 2 s.

Use the toast to show the config, theme and keymap warnings collected at startup, exactly once, as §8.5 requires. Later tasks reuse both components for search positions, `↓ N new`, loading progress, and copy/save/export confirmations.

## Spec excerpts
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

From SPEC.md §1.4:

- **Keys go through the keymap.** A component's `on()` resolves the key with the shared keymap (§8.5), for its own context, to an action. The action becomes a `Cmd` (handled internally with `perform`) or a `Msg`. Components never match literal `KeyEvent`s, except that `Prompt` takes printable characters as text. This keeps remapping and the generated help overlay (§4.3) correct.

- **Ticks:** `Event::Tick` drives toast expiry, the `● live` marker and the redraw limit (§7.1).

From SPEC.md §4.2:

- Help line under the transcript: context-sensitive key hints. It turns into the search/path prompt when active and shows search position (`3/17`), the `↓ N new` indicator and parse warnings.
- Toasts (copy/save/export confirmations, config warnings) appear in the bottom-right corner for ~2 s.

From SPEC.md §8.5:

- Unknown keys produce a warning (shown once as a toast), not an error.
- Invalid values fall back to the default with a warning.

From SPEC.md §8.1 (tokens used here):

| Token | Default | Used for |
|---|---|---|
| `help_key` / `help_text` | `primary` / `muted_foreground` | Help line and help overlay |
| `toast` | fg `foreground` on `surface` | Toast notifications |

From ui-sketch.md (selector help line, second-to-last content row of the overlay):

```text
│                                                                                                          │
│    / to search sessions | ENTER to open | TAB to switch between projects and sessions | ESC to Cancel    │  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  Help line / Search bar
│                                                                                                          │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────╯
```

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **HelpLine props.** A list of hints `(action label, text)`, a list of status items, and the theme. The `App` builds the hints from the keymap, using `Keymap::label(context, action)` from task 005, so remapped keys show up.
  - Selector hints, in order: `<search> to search sessions`, `<open> to open`, `<next_pane> to switch between projects and sessions`, `<cancel> to Cancel`. With default keys this renders exactly as the sketch: `/ to search sessions | ENTER to open | TAB to switch between projects and sessions | ESC to Cancel`.
- **HelpLine rendering.**
  - Key labels use `help_key`; hint text and the ` | ` separators use `help_text`.
  - Status items are right-aligned in `help_text`, separated by two spaces.
  - When everything does not fit, whole hints are dropped from the end, never cut mid-hint. Status items are never dropped for hints.
  - A `fill` prop names the role painted behind its whole area before drawing: `surface` inside the selector overlay, and `background` in the browser (task 021). Nothing is painted when the theme is transparent. Text is drawn on the last row of its area.
- **HelpLine position in the selector.** The overlay's content rows, from the top border, are:

  | Row | Content |
  |---|---|
  | 1 | blank |
  | 2 | harness tabs |
  | 3 | tab underline |
  | 4 | blank |
  | … | panes |
  | 5 rows | Working Directory box |
  | 1 | blank |
  | 1 | help line |
  | 1 | blank |
  | — | bottom border |

  The help line is therefore on row `overlay.y + overlay.height − 3`, two rows above the bottom border (help, blank, border): row 20 at 80x24 and row 36 at 120x40. It runs from overlay column 5 to overlay width − 6. Extend the task-011 layout function with this area.
- **Toast.**
  - Props: text, theme, created-at `Instant` and duration (2 s). It subscribes to `Tick` and returns `Msg::ToastExpired` (or equivalent) on the first `Tick` at or after expiry. The `App` then unmounts it.
  - A new toast replaces the current one.
  - It is drawn last (on top) as a 3-row rounded box: the text padded by one space on each side, border and text in the `toast` style (fg `foreground` on bg `surface`).
  - The box's right edge is one column left of the screen's right edge, and its bottom edge is on row `h − 2`.
  - Its width is `text width + 4`, at most `min(60, w − 4)`. Longer text is truncated with `…`.
- **Startup warnings.**
  - With exactly one warning, the toast text is `Config: <warning>`.
  - With more, it is `Config: <first warning> (+N more, see log)`.
  - All warnings are already logged by tasks 003/004/005.
  - The toast is shown once, at startup.

## Acceptance criteria
- [ ] `HelpLine` renders the selector hints exactly as `/ to search sessions | ENTER to open | TAB to switch between projects and sessions | ESC to Cancel` with default keys, keys in `help_key` and text in `help_text` (component snapshot, text and styled).
- [ ] With `[keys.selector] search = "f"`, the first hint reads `f to search sessions` (test through the App's hint builder).
- [ ] When the width is too small, trailing hints are dropped whole and status items stay right-aligned (component snapshots at widths 40 and 60 with a status item `3/17`).
- [ ] `Toast::on(Tick)` returns no message before the duration elapses and returns the expiry message at or after it. Test with an injected clock, or a zero duration plus a pre-expiry check.
- [ ] `App` with one startup warning mounts a toast with `Config: <warning>`. With three warnings the text ends with `(+2 more, see log)`. After the expiry message the toast is unmounted and does not reappear on later ticks (update tests).
- [ ] The layout function places the help line at `y = overlay.y + overlay.height − 3` (row 20 at 80x24, row 36 at 120x40), from overlay column 5 to width − 6, and places the toast's rect per the planner notes (unit tests at 80x24 and 120x40).
- [ ] Screen snapshots exist for every state in the UI section.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
Changes to the selector overlay (task 011) and a new toast.
- **Help line.** It sits on row `overlay.y + overlay.height − 3` (one blank row between it and the bottom border), starting at overlay column 5: `/ to search sessions | ENTER to open | TAB to switch between projects and sessions | ESC to Cancel`. Key labels (`/`, `ENTER`, `TAB`, `ESC`) are in `help_key` and the rest in `help_text`. At narrow widths, hints are dropped from the right, whole.
- **Toast.**

  ```text
  ╭──────────────────────────╮
  │ Config: unknown key "x"  │
  ╰──────────────────────────╯
  ```

  A 3-row rounded box in the `toast` style. Its right edge is one column from the screen's right edge, its bottom border is on row `h − 2`, and it is drawn on top of everything.

Snapshots:
- Selector with help line at 120x40 and 80x24 (screen, text). At 80x24 the trailing hint(s) are dropped. One styled snapshot at 120x40 shows `help_key`/`help_text`.
- HelpLine component at widths 40 and 60 with a status item `3/17` (component, text).
- Selector with a startup warning toast at 80x24 and 120x40 (screen, text plus one styled).
- Toast with text longer than the maximum width, truncated with `…`, at 80x24 (component, text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 011-app-runtime
