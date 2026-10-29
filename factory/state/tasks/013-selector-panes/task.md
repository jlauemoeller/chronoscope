# Selector panes: harness tabs, projects, sessions, working directory

## Goal
Fill the selector overlay with `HarnessTabs`, `ProjectList`, `SessionList` and `WorkingDir`, and wire them to the background session index from task 010. The path is the one §1.4 prescribes: a Port polls the index channel → `Event::User` → `GlobalEvents` → `Msg` → `App::update`, which updates domain state and pushes rows and stats into the components through `attr`.

This task delivers:
- movement within the focused list;
- moving focus between the panes;
- switching harness by number;
- the rendering and emphasis rules of §8.2;
- live list updates that keep the selection by id;
- the `●` marker for active sessions;
- screen snapshots of the selector for every built-in theme.

## Spec excerpts
From SPEC.md §1.1:

- The UI is a tui-realm `Application` (§2) driven by a single-threaded loop: the loop calls `Application::tick`, passes every returned `Msg` to `App::update`, and redraws only when something changed (at most every 50 ms, §7.1).
- Parsing, watching, stats computation and search indexing run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used.

From SPEC.md §1.2:

The UI follows tui-realm's model: components turn events into messages, and one update function acts on them.

- **`App`** owns the `Application` and the domain state: config, theme registry and active theme, keymap, session index and stats, the open session's summary, watch handle and subagent stack, worker handles and the clipboard.
- **`Id`**: one enum variant per mountable component (§1.3).
- **`Msg`**: what happened, in application terms (`OpenSession(id)`, `CopyText(text)`, `PreviewTheme(id)`, `Quit`, …), never raw keys. Split it into nested enums per screen or concern once it grows.
- **`UserEvent`**: what the Ports deliver: index updates, stats results, session `ModelUpdate`s, search results. tui-realm requires `UserEvent: Eq + Clone`, so large payloads are `Arc`-wrapped and compared by a sequence number.
- **`App::update(Msg)`** is the only place that changes domain state, mounts and unmounts components, moves focus and pushes new data into components. It is a dispatcher: each `Msg` arm calls a small handler, grouped in modules by concern (selector, browser, copy/save/export, resume, theme, live updates). It never interprets keys.
- **`App::view`** only computes the screen layout (§4) and calls `Application::view(&Id, frame, area)` for each visible component, overlays last. It draws nothing itself.

Rules:
- Data flows **down** into components through `attr` (typed data via `AttrValue::Payload(PropPayload::Any(..))`, `Arc`-wrapped when large). Intent flows **up** as `Msg`. Components never see the `App`, and the `App` never reaches into a component's internals.
- No shared mutable state between components, or between a component and the `App`: no `Rc<RefCell<…>>`, `Arc<Mutex<…>>` or globals for UI state.
- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (§5.1), so a 100 MB session is held once (§9.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

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

- **Focus:** exactly one component has focus at a time, set by `App::update` through `Application::active`. In the selector, the pane keys of §6.6 move focus between `ProjectList` and `SessionList`. A focused component shows it through `Attribute::Focus` (e.g. the `ring` border, §8.2).
- **Overlays and prompts:** opening one mounts its component and gives it focus. Closing it unmounts it and returns focus to the component that had it before.
- **Keys go through the keymap.** A component's `on()` resolves the key with the shared keymap (§8.5), for its own context, to an action. The action becomes a `Cmd` (handled internally with `perform`) or a `Msg`. Components never match literal `KeyEvent`s, except that `Prompt` takes printable characters as text. This keeps remapping and the generated help overlay (§4.3) correct.
- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, §6; `Esc` / `?` closes help). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (§6.6).
- **Mouse:** wheel and clicks act on the element under the pointer, whether or not it has focus (§6.9). The event goes to the topmost component drawn there, for example through mouse subscriptions whose row/column ranges are that component's area. That component turns the event into a `Cmd` or `Msg`.
  - While an overlay is open (selector, help, theme selector), the components under it get no mouse events, and a click outside the overlay closes it.
  - Component areas come from a pure layout function that `App::view` also uses. Subscriptions are refreshed from it when the screen or terminal size changes, never from inside `view`.
- **Ticks:** `Event::Tick` drives toast expiry, the `● live` marker and the redraw limit (§7.1).
- **Worker results** take one path: Port → `Event::User` → `GlobalEvents` (subscribed to all user events) turns it into a `Msg` → `App::update` applies it to domain state and pushes what changed to the components that display it (rows and stats to `ProjectList` / `SessionList`, `ModelUpdate`s to `Transcript`). Other components don't subscribe to worker events.

From SPEC.md §4.1:

Layout as in ui-sketch.md: harness tabs, Projects list, Sessions list, Working Directory box, and help/search line.

Sizing:
- Width: the full terminal width, up to 160 columns; on wider terminals it is 160 columns wide and centred horizontally. Height: the terminal height minus one row above and one below.
- The Projects pane takes a third of the content width, but at least 20 and at most 36 columns. The Sessions pane gets the rest, so extra width mainly goes to the session list.

Session row format: `<local start timestamp>  <title>` on the left, plus muted stats on the right (turn count · duration). The model is not shown: the browser shows it on every Assistant turn.
- Title: Claude `ai-title` → OpenCode `session_v2.title` → first user prompt (truncated) → `(untitled)`.
- Stats are computed in the background and cached (§9.2). Until they are ready, the row shows just timestamp + title.

Projects:
- Claude: one project per distinct real `cwd`, read from the session entries (not the lossy directory name).
- OpenCode: from the `project` table (`name`, else the basename of `worktree`).
- Display name: basename of the cwd, unless an alias is configured (§8.5). Projects are sorted by most recent activity.
- Sessions are sorted newest first.

- Focus is on the Projects pane and **no session is selected**: the Sessions pane lists the project's sessions without a `>` marker.

- When the Sessions pane gets the focus (`Tab`, `Enter` on a project, `/`, clicking a row) and no session is selected, the newest session is selected.
- Moving to another project from the Projects pane (keys, click, wheel) clears the session selection again. Switching harness tab selects the first project, and its newest session only if Sessions has focus.

From ui-sketch.md:

```text
                                                                              Session Selector Overlay

                                                                             ╭──────────────────────────────────────────────────────────────────────────────────────────────────────────╮
                                                                             │                                                                                                          │
                              Harness selector  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶  │    [1] Claude  [2] OpenCode  [3] Codex  [4] Pi                                                           │
                                                                             │    ━━━━━━━━━━                                                                                            │
Selected harness is highlighted and underlined                               │                                                                                                          │
                                                                             │   ╭─ Projects ─────────────────────╮ ╭─ Sessions in BRIO ────────────────────────────────────────────╮   │
                                                                             │   │                                │ │                                                               │   │
                 Projects for selected harness  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶  │   │    Agentmon                    │ │    2025-06-12 08:35:44.010Z                                   │   │
                                                                             │   │  > BRIO                        │ │    2025-09-22 14:12:27.021Z                                   │   │  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  Sessions in selected project
       Name of selected project is highlighted                               │   │    Calira                      │ │    2026-01-12 17:10:05.561Z                                   │   │
                                                                             │   │    CameraSettings              │ │  > 2026-10-01 12:45:23.800Z                                   │   │                               Name of selected session is highlighted
                                                                             │   │    FactoryFight                │ │                                                               │   │
                                                                             │   │    Nayra                       │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   │                                │ │                                                               │   │
                                                                             │   ╰────────────────────────────────╯ ╰───────────────────────────────────────────────────────────────╯   │
                                                                             │   ╭─ Working Directory ──────────────────────────────────────────────────────────────────────────────╮   │
                                                                             │   │                                                                                                  │   │
         Working directory of selected project  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶  │   │  /home/jacob/Development/projects/Calira                                                         │   │
                                                                             │   │                                                                                                  │   │
                                                                             │   ╰──────────────────────────────────────────────────────────────────────────────────────────────────╯   │
                                                                             │                                                                                                          │
                                                                             │    / to search sessions | ENTER to open | TAB to switch between projects and sessions | ESC to Cancel    │  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  Help line / Search bar
                                                                             │                                                                                                          │
                                                                             ╰──────────────────────────────────────────────────────────────────────────────────────────────────────────╯
```

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

From SPEC.md §6.6:

| Key | Action |
|---|---|
| `1`–`4` | Switch harness tab (the number shown next to its name) |
| `Tab` / `Shift-Tab` | Move focus between Projects and Sessions |
| `←` / `h`, `→` / `l` | Focus Projects / Sessions (no wrap-around: `←` in Projects and `→` in Sessions do nothing) |
| `↑↓` / `j` `k` | Move in the focused list |
| `/` | Fuzzy search sessions (§4.1) |
| `Enter` | Open the selected session; in the Projects pane, move focus to Sessions |
| `Esc` | Clear search; otherwise close the selector (if a session is already open) or quit (at startup) |
| `R` | Resume the selected session in its harness (§6.7) |
| `t`, `?`, `q` / `Ctrl-q` / `Ctrl-c` | Theme selector, help overlay, quit |

- Harness numbers are stable: `1` Claude, `2` OpenCode, `3` Codex, `4` Pi.
- Harness tabs are switched only by number or by clicking a tab (§6.9). `←` / `→` move between the panes, never between tabs; there is no previous/next-harness action.
- Codex and Pi tabs are hidden until their adapters exist; their numbers stay reserved.
- Tabs for harnesses with no data on this machine are shown muted and can't be selected.

From SPEC.md §7.3:

- Project and session lists update live: new sessions appear, and titles and stats refresh. The current selection is kept (matched by id, not by index).
- Active sessions show the `●` marker in their row.

From SPEC.md §8.6:

- The selector's session rows use the same format without seconds.

From SPEC.md §8.1:

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

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **SPEC.md overrides the sketch.** The sketch's session rows (`2025-06-12 08:35:44.010Z`, without titles) are illustrative only. Rows use `format_short` from task 003 (no seconds or milliseconds, `Z` only with `utc = true`), followed by the title, as specified under "Session row" below.
- **Data flow.**
  - An `IndexPort` (sync `Poll`) drains the task-010 `IndexHandle` receiver and emits `UserEvent::Index(..)`. Payloads are `Arc`-wrapped, and equality compares a sequence number.
  - `GlobalEvents` subscribes to all user events and returns `Msg`s.
  - `App` owns the index data (per harness: availability, projects, sessions, stats).
  - `ProjectList` reports selection changes as a `Msg` (e.g. `ProjectSelected(ProjectId)`). The `App` then pushes that project's sessions to `SessionList` and its cwd to `WorkingDir`.
  - No component reads `App` state, and the `App` never reads component internals.
- **Initial state.**
  - The harness is the first available registered harness by number. (`$PWD` preselection is task 014.)
  - Its first project is selected.
  - Projects is focused.
  - No session is selected: Sessions lists the project's sessions with no `>` marker.
  - When Sessions gets focus with nothing selected, the newest (first) session is selected.
- **Keys** (selector context):
  - `up`/`down` move within the focused list. The list scrolls to keep the selection visible, and there is no wrap-around.
  - `next_pane`/`prev_pane` toggle focus between the panes.
  - `focus_projects`/`focus_sessions` focus Projects/Sessions; pressing one for the pane that already has focus does nothing.
  - `harness_N` switches to harness N if it is registered and available, otherwise does nothing. A switch selects the harness's first project, and selects its newest session only if Sessions has focus.
  - The `App` moves focus with `Application::active`.
- **Live updates.** A new `Snapshot` for the current harness replaces the rows. Selected project and session are kept by id. If a selected item vanished, the selection moves to the same index, clamped. A `Stats` event updates that row's stats.
- **Activity.** The `App` recomputes the `●` flags with the task-010 `is_active(summary, now)` on `Tick`, and pushes them only when they change. Tests inject `now`.

### Overlay layout
All offsets are relative to the overlay rect (`OW`×`OH`) from task 011.

| Element | Placement |
|---|---|
| Tabs | row 2, from column 5 |
| Tab underline | row 3 |
| Content area | columns 4 … `OW − 5`, so content width `CW = OW − 8` |
| Panes | from row 5, height `OH − 14` |
| Projects pane | width `PW = clamp(CW / 3, 20, 36)` (integer division) |
| Gap | 1 column |
| Sessions pane | the rest (`CW − PW − 1`) |
| Working Directory box | directly below the panes, 5 rows, full content width |

The help line (task 012) follows the Working Directory box after a blank row. The sketch is approximate: its Projects pane is 34 wide where the formula gives 33.

### Harness tabs
- Tabs show `[N] Name` for each registered harness, separated by two spaces. Codex and Pi are not registered, so they are hidden.
- The selected tab is in `primary`. The row below has `━` in `primary` under exactly the selected tab's label width.
- Other tabs, including unavailable ones, are `muted_foreground`. Unavailable tabs can't be selected.

### Panes
- Rounded border: `ring` when focused, `border` otherwise.
- Left title after one border cell: `╭─ Projects ─…╮` and `╭─ Sessions in <project display name> ─…╮`, title text in `foreground`.
- Pane row 1 is blank, and list rows start at pane row 2.
- Within a pane of width `W`: the marker `>` is at column 3, text starts at column 5, and the inverted band covers columns 4 … `W − 4`. Two cells of margin are left before the right border.

### Row styles (§8.2)
| Row | Style |
|---|---|
| Unselected | `muted_foreground` |
| Selected, focused pane | `>` in `primary`, never inverted. Columns 4 … `W − 4` form one band: fg `foreground` + `REVERSED`, not bold. Everything inside takes the band style. |
| Selected, unfocused pane | text bold in `secondary_foreground`; `>` in `secondary_foreground`, not bold; no band |

### Session row
- Format: `<started_at via format_short>  <dot slot><title>`, plus stats right-aligned so they end at column `W − 4`.
- The dot slot is 2 cells: `● ` when active, two spaces otherwise. Outside a band the `●` is `primary`.
- Stats are `muted_foreground` outside a band.
  - Format: `<n> turns · <d>`, with `1 turn` for one.
  - `<d>`: `45s` under a minute, `12m` under an hour, `1h 05m` otherwise.
  - Rows without stats yet show no stats.
- The title is truncated with `…`. Stats are omitted when fewer than 10 cells would remain for the title.

### Projects and working directory
- A project row shows the display name, truncated with `…`.
- The Working Directory box: rounded border in `border`, title `Working Directory`, a blank row, then the selected project's cwd starting at box column 3 in `foreground`, then a blank row.
  - A cwd that is too long is truncated from the left with a leading `…`.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] Index events reach the `App` only via Port → `UserEvent` → `GlobalEvents` → `Msg`. An update test feeds a `Snapshot` message and asserts the domain state, plus the rows pushed to `ProjectList`, `SessionList` and `WorkingDir`.
- [ ] Initial state: first available harness, first project selected, Projects focused, no session selected. Focusing Sessions selects the newest session (update tests).
- [ ] `ProjectList`/`SessionList` `on()`:
  - `j`/`k`/`↓`/`↑` move the selection without wrap-around, and the list scrolls to keep the selection visible;
  - `Tab`/`Shift-Tab` toggle focus; `←`/`h` focus Projects; `→`/`l` focus Sessions;
  - `←` in Projects and `→` in Sessions do nothing.

  Component tests check the returned `Msg`/`CmdResult`.
- [ ] Moving the project selection emits a selection `Msg`, and the `App` pushes that project's sessions and cwd (update test).
- [ ] `1` switches to Claude. A key for an unregistered or unavailable harness does nothing. A switch selects the first project, and selects its newest session only when Sessions has focus (update tests with a fake second harness registered in the test).
- [ ] A live `Snapshot` keeps the selected project and session by id even when their indices change, and a vanished selection clamps to the same index (update tests).
- [ ] `Stats` events add `<n> turns · <d>` to the matching row. The duration formatter gives `45s`, `12m`, `1h 05m` and `1 turn` (unit tests).
- [ ] An active session (per `is_active` with an injected `now`) shows `●`. The flag clears on a later `Tick` once 60 s have passed (update test).
- [ ] The layout matches the planner-notes formulas: at 120x40 the content width is 112, Projects is 36, Sessions is 75, and panes are 24 rows high. At 80x24 Projects is 24 and Sessions is 47 (layout unit tests).
- [ ] Every state in the UI section has a snapshot. The per-theme screen snapshots render `App::view` for all seven built-in themes.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
The selector overlay as in the ui-sketch, with the exact geometry and styles from the planner notes:
- harness tabs plus the `━` underline under the selected tab;
- the Projects and Sessions panes side by side, with `ring` on the focused pane;
- the Working Directory box below them;
- the help line (task 012).

States to snapshot. Screen snapshots go through `App::view` with fixture index data: 6 projects, the selected project with 5 sessions, two sessions with stats, one active.
1. Startup: Projects focused, the first project selected (band + `>`), Sessions listing rows without any `>`. At 120x40 (text + styled), 80x24 (text) and 200x50 (text).
2. Sessions focused with the newest session selected. Sessions has the band; Projects shows its selected row bold in `secondary_foreground` with `>`, no band. The `ring` border has moved to Sessions. At 120x40 (text + styled) and 80x24 (text).
3. Long titles at 80x24: titles truncated with `…`, and stats omitted where fewer than 10 cells remain.
4. An active session's `●` in an unselected row and inside the band (styled, 120x40).
5. Scrolled list: 40 sessions, selection near the bottom, so the list is scrolled to keep it visible (80x24).
6. Working directory longer than the box, truncated from the left with `…` (80x24).
7. Per-theme: state 2 at 120x40 for each of `default`, `terminal`, `catppuccin`, `gruvbox`, `nord`, `tokyo-night` and `solarized` (styled).

Component snapshots: `HarnessTabs` with one selected tab and one muted tab (a fake harness, width 60); `ProjectList` focused and unfocused; `SessionList` with and without stats (width 60, height 10).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 010-session-index-worker
- 012-help-line-toast
