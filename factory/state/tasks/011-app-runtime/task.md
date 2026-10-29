# Application runtime, selector frame and too-small state

## Goal
Stand up the tui-realm application, so that `chronoscope` starts, draws, and quits cleanly.
- **Core types.** `App` owns the `Application` and the domain state (config, theme registry, active theme, keymap). Alongside it come the `Id`, `Msg` and `UserEvent` enums.
- **Main loop.** It ticks the `Application`, passes every `Msg` to `App::update`, and redraws only when something changed, at most every 50 ms.
- **Terminal handling.** The terminal is always restored on exit, panic or signal. Mouse capture follows the `mouse` config.
- **`GlobalEvents`.** It turns the global quit keys (and `Ctrl-c`) and terminal resizes into `Msg`s.
- **Layout function.** A pure function returns each component's area. `App::view` uses it, and so will the mouse subscriptions later.
- **Components.** `SelectorFrame` draws the selector overlay's border, sizing and background. `EmptyState` shows "Terminal too small".

At startup the app shows the empty selector frame. Later tasks fill it.

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

From ui-sketch.md (session selector overlay; the frame is the outer rounded box):

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

From SPEC.md §4.2:

- Empty states: "No sessions for this harness" / "Session file is empty".

From SPEC.md §4.3:

Overlays are centred, use rounded borders with `ring` colour, and paint `surface` behind them (unless the background is transparent, in which case `Clear` is still used so underlying text doesn't bleed through).

From SPEC.md §6.5:

| Key | Action |
|---|---|
| `q`, `Ctrl-q`, `Ctrl-w`, `Ctrl-c` | Quit |

`Ctrl-c` always quits; it is never used for copy.

From SPEC.md §6.9:

- Mouse capture can be disabled in config (`mouse = false`). Most terminals also bypass mouse capture while `Shift` is held, which allows native text selection; the help overlay mentions this.

From SPEC.md §7.1:

- Watch events are coalesced. Workers parse new data off the UI thread and send `ModelUpdate`s; the UI thread applies them (§1.4) and redraws at most every 50 ms.

From SPEC.md §9.5:

- Terminal state is always restored on exit, panic or signal (panic hook + raw-mode guard).

From SPEC.md §8.1:

Per-theme option `transparent_background` (bool): when true, `background` is not painted and the terminal's own background shows through. Users can override it per theme in config.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Startup.** `chronoscope::run()` keeps `--help`/`--version` and otherwise:
  1. loads config (task 003, default paths);
  2. initialises logging, ignoring failure;
  3. builds the theme registry and resolves the theme (task 004);
  4. builds the keymap (task 005);
  5. collects all config/theme/keymap warnings into `App` (shown as a toast in task 012);
  6. sets up the terminal and runs the loop.
- **Terminal.**
  - Use tui-realm's crossterm terminal adapter (raw mode, alternate screen, mouse capture only when `mouse = true`).
  - A guard type restores the terminal on `Drop`.
  - A panic hook restores the terminal and then calls the previous hook.
  - Signals: SIGINT, SIGTERM and SIGHUP on Unix, Ctrl-C/Break on Windows. Use the `ctrlc` crate with its `termination` feature (add it) to set a flag, which the loop checks each iteration so it quits through the normal restore path.
- **Loop.**
  - The tick interval is 250 ms, and the input poll timeout is short (≈ 20 ms).
  - A small pure `RedrawLimiter` decides whether to draw: only when dirty, and at least 50 ms after the last draw. Draw immediately after a resize.
  - `App::update` marks the state dirty when anything visible changed.
- **Msg.** Keep it application-level: `Quit`, `Resized(u16, u16)`, `Tick`, and nested enums as it grows. `Ctrl-c` and the global `quit` keys produce `Msg::Quit`. `GlobalEvents` subscribes to them, to `WindowResize` (→ `Msg::Resized`) and to `Tick` (→ `Msg::Tick`; `App::update` uses it in later tasks for toasts, the live marker and activity flags), with subscription clauses ready for later tasks to exclude prompts and overlays. `UserEvent` is defined, initially with no worker payloads in use. It must be `Eq + Clone`; task 013 adds the first variants.
- **Layout function.** `layout(&LayoutInput, Rect) -> ScreenLayout` is pure and unit-tested. `ScreenLayout` maps each visible component `Id` to its `Rect`.
  - The terminal is too small below **60 columns or 20 rows** (planner decision, not spec). Then only `EmptyState` is visible, filling the screen.
  - Otherwise the selector overlay is `x = (w − min(w,160)) / 2`, `y = 1`, width `min(w, 160)`, height `h − 2`.
- **SelectorFrame.**
  - It is given the full screen area and the overlay rect.
  - It draws a rounded border in `ring` around the overlay rect and fills the interior with `surface`, or clears it with `Clear` and paints nothing when the theme is transparent.
  - When it is the base screen (no session open behind it, which is always the case in this task), it first paints the whole screen with `background` (skipped when transparent).
  - It has no title.
- **EmptyState.** It takes a message prop and draws the message centred horizontally and vertically in its area, in `muted_foreground`, on `background` (not painted when transparent). Its text in this task is exactly `Terminal too small`.
- **Components** are `pub` and constructible from props and the theme without an `Application`.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] Running the binary in a pseudo-terminal (`#[cfg(unix)]` integration test, using `expectrl` or an equivalent pty crate as a dev-dependency), with `HOME`/`XDG_*` pointed at an empty temp dir and an 80x24 pty: after startup, sending `q` makes the process exit with code 0, and its output contains the leave-alternate-screen (`ESC[?1049l`) and show-cursor (`ESC[?25h`) sequences. Separate runs cover `Ctrl-q`, `Ctrl-w` and `Ctrl-c`.
- [ ] `GlobalEvents::on` returns `Msg::Quit` for every default global `quit` key and for `Ctrl-c`, and `Msg::Resized` for a resize event. With `[keys.global] quit = "x"`, `x` quits and `q` does not (component tests).
- [ ] `App::update(Msg::Quit)` sets the app's quit state. `App::update(Msg::Resized(..))` marks the state dirty. Both are update tests without a terminal.
- [ ] `RedrawLimiter` never allows two draws less than 50 ms apart, draws only when dirty, and draws immediately after a resize (unit tests with injected instants).
- [ ] `layout()` returns only `EmptyState` (full screen) for 59x24 and 80x19. For 80x24 it returns the overlay `Rect{0,1,80,22}`, for 200x50 `Rect{20,1,160,48}`, and for 120x40 `Rect{0,1,120,38}` (unit tests).
- [ ] The panic hook restores the terminal before delegating to the previous hook. The restore function is injectable, so a unit test installs the hook with a recording restore function and a recording previous hook, triggers a panic in `catch_unwind`, and asserts the call order (restore first).
- [ ] Mouse capture is enabled only when `mouse = true` (test the terminal-setup decision as a pure function).
- [ ] `SelectorFrame` and `EmptyState` have component snapshot tests, and `App::view` has screen snapshot tests for every state in the UI section.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
The selector overlay frame, empty inside, is the startup screen.
- **Overlay rect.** Width `min(w, 160)`, centred horizontally. One empty row above, one below (`y = 1`, height `h − 2`).
- **Border.** Rounded (`╭╮╰╯─│`) in `ring`. Interior filled with `surface`.
- **Outside the overlay.** The rows above and below (and the side columns when `w > 160`) are painted `background`.
- **Content.** None, and no title.

`EmptyState` "Terminal too small": the text `Terminal too small` in `muted_foreground`, centred on the middle row of the screen, with the rest of the screen painted `background`.

Snapshots (screen snapshots through `App::view`):
- Startup frame at 80x24, 120x40 and 200x50 (at 200x50 the frame spans columns 20–179). Text snapshots, plus one styled snapshot at 80x24 showing `ring`, `surface` and `background`.
- Startup frame with a transparent theme (`transparent_background = true`) at 80x24, styled: nothing painted with `surface` or `background`, border still `ring`.
- Too small at 59x24 and 80x19 (text snapshots).

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
- 005-keymap-bindings
