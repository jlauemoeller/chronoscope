# Session browser screen

## Goal
Assemble the session browser.
- **Opening.** `Enter` on a session in the selector unmounts the selector and mounts `BrowserHeader`, `Transcript` and the browser's `HelpLine`. The session is loaded through its harness on a worker. `ModelUpdate`s flow Port → `UserEvent` → `GlobalEvents` → `Msg` → `App::update`, which forwards turn updates to the `Transcript`.
- **Loading and errors.** The help line shows `Loading… 34%` while the initial parse streams in. The header shows the parse-warning note. An empty session shows "Session file is empty".
- **Returning to the selector.** `Ctrl-o` opens the selector over the browser with the open session selected and Sessions focused, and `Esc` closes it again.
- **Snapshots.** Screen snapshots of the browser for every built-in theme.

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

From SPEC.md §1.3:

| Component | Where | Draws / handles | Takes focus |
|---|---|---|---|
| `BrowserHeader` | Browser | Project and session titles, cwd, `● live`, breadcrumb, parse-warning note | No |
| `Transcript` | Browser | The turn blocks: selection, folding, navigation, search highlights (§5, §6.1–§6.3) | Yes |
| `HelpLine` | Both | Key hints and status items (`3/17`, `↓ N new`, `Loading… 34%`) | No |
| `Prompt` | Both | Search and path prompts and `y/n` confirmations, drawn in place of the help line while active | Yes (input mode, §6) |
| `EmptyState` | Both | "Terminal too small", "No sessions for this harness", "Session file is empty" | No |

From SPEC.md §4.2:

- Header block: `Project <name>` (left title), session id (right title), working directory in the body. A `● live` marker shows while the session is active (§7.2), and a breadcrumb shows when viewing a subagent (§5.4).
- Transcript: one rounded block per turn (§5.2), scrolling vertically, with the selected section marked by `▒` over the block's left border.
- Help line under the transcript: context-sensitive key hints. It turns into the search/path prompt when active and shows search position (`3/17`), the `↓ N new` indicator and parse warnings.
- Toasts (copy/save/export confirmations, config warnings) appear in the bottom-right corner for ~2 s.
- Loading state: `Loading… 34%` in the help line while the initial parse streams in.
- Empty states: "No sessions for this harness" / "Session file is empty".

From SPEC.md §4.1:

- Opening the selector over an open session (`Ctrl-o`) selects that session, with focus on Sessions.

From SPEC.md §4.3:

Overlays are centred, use rounded borders with `ring` colour, and paint `surface` behind them (unless the background is transparent, in which case `Clear` is still used so underlying text doesn't bleed through).

From SPEC.md §6.5:

| Key | Action |
|---|---|
| `Ctrl-o` | Open the session selector (plain `o` is not bound) |

From SPEC.md §6.6:

| Key | Action |
|---|---|
| `Esc` | Clear search; otherwise close the selector (if a session is already open) or quit (at startup) |

From SPEC.md §9.2:

- Parsing is streaming (line by line for JSONL, paged queries for SQLite) and runs off the UI thread. The UI shows a loading state and becomes interactive as soon as the first screen of content is parsed.

From SPEC.md §9.5:

  - A muted `N entries could not be parsed` note appears in the browser header.

From ui-sketch.md (browser header, first blocks and help line):

```text
                                                                 Session Browser                                    ┃
                                                                                                                    ▼
                                                                ╭─ Project BRIO ───────────────────────────────────────────────────── 85a86657─11a7─4318─a21a─0f4476babd5c ─╮  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  Session ID
                                                                │                                                                                                           │
     Working directory of session  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶  │ /home/jacob/Development/projects/BRIO                                                                     │
                                                                │                                                                                                           │
                                                                ╰───────────────────────────────────────────────────────────────────────────────────────────────────────────╯
                        User turn  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶  ╭─ User ───────────────────────────────────────────────────────────────────────── 2026─10─21 08:35:15.800Z ─╮  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  Timestamp for entry
                                                                │                                                                                                           │
                                                                │ - I have a Logitech BRIO web camera attached to this machine via USB. It works fine - both                │
                                                                │   camera and microphone - but I don't have a way to access settings that would have been                  │
                                                                │   available on say a mac or windows, since there are no proprietary drivers: we just get                  │  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  User input is always one block. Can be folded
                                                                │   the vanilla USB profiles. Can you help me investigate what additional profiles the                      │                               (default) in which case only the first two lines
                                                                │   camera has and how to access them? The ideal solution would be to be able to write an                   │                               are displayed with an ellipsis at the end
                                                                │   app that allowed me to change the settings, if possible. However, before we get to that                 │
                                                                │                                                                                                           │
                                                                ╰───────────────────────────────────────────────────────────────────────────────────────────────────────────╯
Agent turn shows model and effort  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶  ╭─ Assistant ── claude─opus─5.5 @ high  ───────────────────────────────────────── 2026─10─21 08:35:18.271Z ─╮
                                                                │                                                                                                           │
                                                                │ I'll start by looking at what the camera exposes over USB and what Linux's UVC driver makes of it.        │  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  Thinking. Not foldable
                                                                │                                                                                                           │
```

```text
╰───────────────────────────────────────────────────────────────────────────────────────────────────────────╯

 / to search | ENTER to fold or unfold sections | y or Ctrl-C to copy current | ? for keybindings               ◀━━━━━━━━━━━━━━━━━━━━━━━━━━   Help line / Search bar
```

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
- **SPEC.md overrides the sketch.** The browser help line reads `y to copy current`, not `y or Ctrl-C to copy current`: `Ctrl-c` always quits. Fold markers, the selection marker and timestamps follow task 017.
- **Screen layout** (`w`×`h`, extend the task-011 layout function):

  | Rows | Content |
  |---|---|
  | 0–4 | `BrowserHeader`, full width, 5 rows |
  | 5 … `h − 3` | `Transcript` |
  | `h − 2` … `h − 1` | `HelpLine` area (2 rows, full width, `fill = background`). The text sits on the last row, from column 1 to `w − 2`, so row `h − 2` is the blank separator. |

  Below the minimum size (60×20), "Terminal too small" (task 011) replaces everything.
- **Painting.** `BrowserHeader`, `Transcript` and the browser `HelpLine` each paint `background` over their whole area, unless the theme is transparent, so no row of the browser is left unpainted.
- **Parse warnings.** §4.2 also lists parse warnings among the help-line items, but §1.3 and §9.5 put the note in the browser header. It is shown in the header only (planner decision).
- **BrowserHeader.**
  - A rounded block in `border`: `╭─ Project <name> ─…─ <session id> ─╮`. `Project` is in `muted_foreground`, the project display name bold `foreground`, and the session id (right title) in `timestamp`.
  - Body rows: blank, then ` <cwd>` starting at column 2 in `foreground`, then blank.
  - When the parse-warning count is greater than 0, `<N> entries could not be parsed` is right-aligned on the cwd row, ending at column `w − 3`, in `muted_foreground` (`1 entry could not be parsed` for one).
  - The cwd is truncated from the left with `…` to keep at least 2 cells before the note.
  - Breadcrumb and `● live` are later tasks.
- **Opening a session.** `Msg::OpenSession(harness, id)`:
  - looks up the `SessionSummary` in the App's index;
  - drops any current `WatchHandle`, unmounts selector/prompt/old browser components, and mounts fresh `BrowserHeader`, `Transcript` and `HelpLine`;
  - focuses `Transcript`;
  - calls `harness.load(id, tx)`.

  A load generation number tags every update: an `UserEvent::Session { generation, update }` emitted by a `SessionPort`. Updates from an older generation are dropped. A small forwarding thread, or a tagging sender, is acceptable.
- **Update handling in `App::update`.**
  - `Progress(p)` sets the help-line status item `Loading… <round(p×100)>%`. `Loaded` removes it.
  - `ParseWarnings(n)` sets the header note.
  - `Appended`/`Updated`/`Reset` are forwarded to the `Transcript` via `attr` and never stored in the App.
  - After `Loaded` with zero turns, the transcript area shows `EmptyState` with `Session file is empty`.
  - The App keeps the open session's `SessionSummary` and `WatchHandle` (§1.2).
- **Browser hints**, built from the keymap: `<browser.search> to search | <browser.fold_toggle> to fold or unfold sections | <browser.copy> to copy current | <global.help> for keybindings`. With defaults: `/ to search | ENTER to fold or unfold sections | y to copy current | ? for keybindings`.
- **Ctrl-o.**
  - `GlobalEvents` handles the `global.open_selector` key and returns `Msg::OpenSelector`. Like the other global keys, it is excluded while a prompt or overlay has focus.
  - The App mounts the selector (task 011–015 components) as an overlay. The browser stays mounted and is drawn underneath.
  - `SelectorFrame` paints only the overlay rect (no full-screen `background`).
  - The open session's harness and project are selected, the open session is selected in Sessions, and Sessions has focus. The selector's `session_open` prop is true.
  - `Esc` (`Msg::CloseSelector`) unmounts it and returns focus to the `Transcript` via the focus stack.
  - Opening a different session replaces the current one. Opening the same session just closes the selector.
- **Startup** remains the selector (task 014). The command line is task 033.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] `Msg::OpenSession` mounts `BrowserHeader`, `Transcript` and `HelpLine`, unmounts the selector, focuses the `Transcript`, and starts a load (update test with a fake harness whose `load` sends scripted updates).
- [ ] Updates travel Port → `UserEvent::Session` → `GlobalEvents` → `Msg` → `App::update` → `Transcript` `attr`. The App holds no copy of the turns (test plus code inspection).
- [ ] Updates tagged with a previous generation are ignored after opening another session (update test).
- [ ] `Progress(0.34)` shows `Loading… 34%` in the help line, and `Loaded` removes it (update test plus snapshot).
- [ ] `ParseWarnings(3)` shows `3 entries could not be parsed` in the header, and `ParseWarnings(1)` shows `1 entry could not be parsed` (component tests).
- [ ] A session with zero turns shows `Session file is empty` after `Loaded` (update test plus snapshot).
- [ ] `Ctrl-o` opens the selector over the browser with the open session selected and Sessions focused. `Esc` closes it and returns focus to the `Transcript`. `Enter` on another session opens it. `q` typed in the selector's search prompt does not quit (update tests).
- [ ] The browser help line shows exactly `/ to search | ENTER to fold or unfold sections | y to copy current | ? for keybindings` with default keys (snapshot).
- [ ] Screen snapshots exist for every state in the UI section, including all seven themes.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
The browser screen:
- **Header** (rows 0–4): `╭─ Project BRIO ──…── 85a86657-11a7-4318-a21a-0f4476babd5c ─╮`, a blank row, ` /home/user/projects/brio` at column 2 (plus a right-aligned parse-warning note when present), a blank row, and the bottom border.
- **Transcript** (rows 5 … `h − 3`): turn blocks as in tasks 017–020.
- **Blank row** at `h − 2`.
- **Help line** at `h − 1`, starting at column 1: `/ to search | ENTER to fold or unfold sections | y to copy current | ? for keybindings`, keys in `help_key` and text in `help_text`, with status items right-aligned.

Snapshots (screen through `App::view`, fixture session, `utc = true`):
1. Loaded browser at 120x40 and 80x24 (text), and at 120x40 (styled).
2. Loading: `Loading… 34%` right-aligned in the help line, with a partial transcript, at 80x24 (text).
3. Empty session: header plus `Session file is empty` centred in the transcript area, at 80x24 (text).
4. Parse warnings: `3 entries could not be parsed` right-aligned on the header's cwd row, at 120x40 (text).
5. Selector opened with `Ctrl-o` over the browser: the overlay with the open session selected and Sessions focused, and the browser visible in row 0 and row `h − 1` (and in the side columns at 200x50). At 120x40 and 200x50 (text).
6. Per-theme: state 1 at 120x40 for each of the seven built-in themes (styled).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 014-selector-selection-rules
- 015-selector-search
- 020-transcript-navigation
