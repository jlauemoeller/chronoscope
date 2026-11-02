# Mouse support in the session selector

## Goal
Add mouse support to the session selector (§6.9, §1.4). The infrastructure comes first: mouse events are routed to the component under the pointer, using areas from the pure layout function. Areas are refreshed when the screen or terminal size changes, never from inside `view`. On top of that:
- the wheel moves through the list under the pointer;
- clicking a row selects it (and focuses its pane);
- double-clicking a session row opens it;
- clicking a harness tab switches to it.

Mouse capture stays off when `mouse = false`. The transcript and overlay mouse rules are task 035.

## Spec excerpts
From SPEC.md §1.4:

- **Mouse:** wheel and clicks act on the element under the pointer, whether or not it has focus (§6.9). The event goes to the topmost component drawn there, for example through mouse subscriptions whose row/column ranges are that component's area. That component turns the event into a `Cmd` or `Msg`.
  - While an overlay is open (selector, help, theme selector), the components under it get no mouse events, and a click outside the overlay closes it.
  - Component areas come from a pure layout function that `App::view` also uses. Subscriptions are refreshed from it when the screen or terminal size changes, never from inside `view`.

- **Focus:** exactly one component has focus at a time, set by `App::update` through `Application::active`. In the selector, the pane keys of §6.6 move focus between `ProjectList` and `SessionList`. A focused component shows it through `Attribute::Focus` (e.g. the `ring` border, §8.2).

From SPEC.md §6.9:

- Wheel scrolls the transcript or the list under the pointer.
- Click selects a section or list row. Clicking a section's summary line (or the `▶`/`▼` marker) toggles its fold. Double-clicking a session row opens it.
- Click on a harness tab switches to it. Click outside an overlay closes it.
- Mouse events go to the component under the pointer (§1.4).
- Mouse capture can be disabled in config (`mouse = false`). Most terminals also bypass mouse capture while `Shift` is held, which allows native text selection; the help overlay mentions this.

From SPEC.md §4.1:

- When the Sessions pane gets the focus (`Tab`, `Enter` on a project, `/`, clicking a row) and no session is selected, the newest session is selected.
- Moving to another project from the Projects pane (keys, click, wheel) clears the session selection again. Switching harness tab selects the first project, and its newest session only if Sessions has focus.
- While nothing is selected, live index updates keep the cursor on the newest session. `Esc` on a search that started from this state returns to it: Projects focused, no session selected.

From SPEC.md §6.6:

- Harness tabs are switched only by number or by clicking a tab (§6.9). `←` / `→` move between the panes, never between tabs; there is no previous/next-harness action.
- Codex and Pi tabs are hidden until their adapters exist; their numbers stay reserved.
- Tabs for harnesses with no data on this machine are shown muted and can't be selected.

From SPEC.md §1.2:

- **`App::update(Msg)`** is the only place that changes domain state, mounts and unmounts components, moves focus and pushes new data into components. It is a dispatcher: each `Msg` arm calls a small handler, grouped in modules by concern (selector, browser, copy/save/export, resume, theme, live updates). It never interprets keys.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Routing.**
  - Mouse events are routed from the task-011 `ScreenLayout`, the same pure function `App::view` uses. Hit-test the event position against the visible components' rects, topmost first (overlays before what's under them).
  - The implementation may use tui-realm mouse subscriptions with area clauses, refreshed by `App::update` on `Resized` and on every screen/overlay change. Alternatively, `GlobalEvents` can receive all mouse events and the `App` dispatches to the hit component through `attr`/a `Msg`.
  - Either way, the target component turns the event into a `Cmd` or `Msg`, and areas are never computed inside `view`.
  - Positions passed to components are relative to their own area.
- **Wheel** (one row per wheel event):
  - Over Projects: move the project selection up/down. This is a project change, so the session selection is cleared, using the same `Msg` as the keys (task 014).
  - Over Sessions: move the session selection if one exists; otherwise scroll the list viewport without selecting.
  - Focus does not change on wheel.
- **Click** (left button, on press):
  - A project row: focus Projects and select that project (clearing the session selection when it changed).
  - A session row (normal or search/filtered results): focus Sessions and select that row.
  - Empty space inside a pane: focus that pane only.
  - A harness tab (its `[N] Name` label cells): switch harness if it is registered and available (task 013 rules).
  - Elsewhere (Working Directory box, help line, frame): nothing.
- **Double-click.** Two left presses on the same session row within 400 ms (planner decision) open it (`Msg::OpenSession`). Time comes from an injected clock, so tests are deterministic. A double-click on a project row behaves like `Enter` on it (focus Sessions, select newest).
- **mouse = false.** No capture (task 011) and no routing. Tests assert the router ignores events in that mode.
- Coordinates outside every rect are ignored.

## Acceptance criteria
- [ ] The router's hit-testing is a pure function over `ScreenLayout`. It maps points to the right component at 80x24 and 200x50 (the centred overlay), including the gap between panes (unit tests).
- [ ] After a resize, the routing areas change accordingly. Routing areas are never computed in `view` (update test plus code inspection).
- [ ] Wheel over Projects changes the project and clears the session selection. Wheel over Sessions moves the session selection, or only scrolls when none is selected. Focus is unchanged (update tests).
- [ ] A click on a project row focuses Projects and selects it. A click on a session row focuses Sessions and selects that row. A click in the empty pane area only focuses (update tests).
- [ ] A double-click on a session row within 400 ms opens it. Two clicks 500 ms apart don't (update tests with an injected clock).
- [ ] A click on an available harness tab switches to it. A click on a muted tab does nothing (update test).
- [ ] With `mouse = false`, mouse events have no effect (test).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 015-selector-search
- 028-opencode-adapter
