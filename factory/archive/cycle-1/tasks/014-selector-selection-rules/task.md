# Selector selection rules, startup preselection, Enter/Esc and empty state

## Goal
Implement the selector's behaviour rules from §4.1 and §6.6 on top of the task-013 panes:
- **Startup preselection.** Preselect the harness and project matching `$PWD` or its nearest ancestor, falling back to the most recently active project.
- **Session selection.** Changing project clears the session selection. While nothing is selected, live updates keep the implicit cursor on the newest session.
- **Enter.** On a project it moves focus to Sessions. On a session it requests opening that session.
- **Esc.** It quits at startup, or closes the selector when a session is open behind it.
- **Empty state.** "No sessions for this harness" is shown when the current harness has nothing to list.

## Spec excerpts
From SPEC.md §4.1:

Startup (no arguments):
- Opens the selector.
- Preselects the harness and project whose cwd matches `$PWD` (or the nearest ancestor), falling back to the most recently active project (the first in the list).
- Focus is on the Projects pane and **no session is selected**: the Sessions pane lists the project's sessions without a `>` marker.

Session selection:
- Opening the selector over an open session (`Ctrl-o`) selects that session, with focus on Sessions.
- When the Sessions pane gets the focus (`Tab`, `Enter` on a project, `/`, clicking a row) and no session is selected, the newest session is selected.
- Moving to another project from the Projects pane (keys, click, wheel) clears the session selection again. Switching harness tab selects the first project, and its newest session only if Sessions has focus.
- While nothing is selected, live index updates keep the cursor on the newest session. `Esc` on a search that started from this state returns to it: Projects focused, no session selected.

From SPEC.md §4.2:

- Empty states: "No sessions for this harness" / "Session file is empty".

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

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Preselection.**
  - The `App` is constructed with the working directory as a parameter: `std::env::current_dir()` in production, injected in tests.
  - A project matches when its cwd equals the working directory or is an ancestor of it. Compare `Path` components, never strings.
  - The deepest match wins, ties go to the lowest harness number, and only available harnesses take part.
  - With no match, use the first available harness (by number) and its first project, which is the most recently active.
- **Clearing.**
  - Any project change made from the Projects pane clears the session selection. The keys are wired here; click and wheel come in the mouse task, which must use the same `Msg`.
  - When the user later focuses Sessions with nothing selected, the newest session is selected.
  - "Keep the cursor on the newest session" means: while no session is selected, a live `Snapshot` does not create a selection, and the next focus on Sessions selects whatever is newest at that moment.
- **Enter.** In Projects, Enter moves focus to Sessions, selecting the newest session if none is selected. In Sessions, Enter on a selected session returns `Msg::OpenSession(harness, session id)`. The browser task (021) handles it; until then `App::update` does nothing for it.
- **Esc.** The selector components get a prop saying whether a session is open behind the selector.
  - With no session open, `cancel` returns `Msg::Quit`.
  - With a session open, it returns `Msg::CloseSelector`. Task 021 handles that and is where it is tested end to end.
  - Clearing a search comes first; that is task 015.
- **Empty state.**
  - When the current harness has no sessions (an empty data dir, or no available harness at all), the Projects pane is drawn empty (border and title only).
  - The Sessions pane keeps its border, titled ` Sessions `, and its interior shows `EmptyState` with `No sessions for this harness`, centred, in `muted_foreground`.
  - The Working Directory box is empty.
  - With no available harness, the first registered harness's tab is shown and muted.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] With the working directory `/w/brio/src` and projects `/w/brio` and `/w`, startup preselects `/w/brio` in the harness that owns it, with Projects focused and no session selected (update test).
- [ ] With no matching project, startup selects the first available harness and its first project (update test).
- [ ] Moving to another project with `j`/`k` clears the session selection. Focusing Sessions afterwards selects that project's newest session (update test).
- [ ] With no session selected, a live `Snapshot` adding a newer session leaves nothing selected. Focusing Sessions then selects the new newest session (update test).
- [ ] Enter in Projects focuses Sessions and selects the newest session. Enter in Sessions returns `Msg::OpenSession` with the selected session's harness and id (component and update tests).
- [ ] Esc returns `Msg::Quit` when no session is open and `Msg::CloseSelector` when the prop says one is open (component tests).
- [ ] A harness with zero sessions shows `No sessions for this harness` in the Sessions pane and an empty Projects pane (screen snapshot). A machine with no available harness shows the same, with the tab muted (update test plus snapshot).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
Changes to the selector screen of task 013:
- **Empty state.** The Projects pane shows border and title only. The Sessions pane has its border, the title ` Sessions `, and `No sessions for this harness` centred in its interior in `muted_foreground`. The Working Directory box is empty. The help line is unchanged.
- **Preselected project.** At startup the project matching the working directory is selected and highlighted with the focused band (§8.2), and the list is scrolled if needed to show it. No session has a `>`.

Snapshots (screen, through `App::view`, fixture data with 12 projects):
1. Empty state at 80x24 and 120x40 (text) and 120x40 (styled).
2. Startup with the working directory matching the 9th project, at 80x24, where the list must scroll to show the selection (text).
3. After moving from the preselected project to the next one: the new project selected and no session marker, at 120x40 (text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 013-selector-panes
