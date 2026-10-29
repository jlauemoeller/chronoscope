# Mouse support in the transcript and overlays

## Goal
Complete mouse support (§6.9) using the routing from task 034.
- **Transcript.**
  - The wheel scrolls it, with the §6.3 rule that the selection moves to the topmost fully visible section when it leaves the viewport.
  - A click selects the section under the pointer.
  - A click on a section's summary line or its `▶`/`▼` marker toggles its fold.
  - A click on a `… N more lines` row expands it.
- **Help overlay.** The wheel scrolls it.
- **Overlay rules** for the selector over the browser, the help overlay and the theme selector:
  - components under an open overlay get no mouse events;
  - a click outside the overlay closes it, exactly as `Esc` would.

## Spec excerpts
From SPEC.md §6.9:

- Wheel scrolls the transcript or the list under the pointer.
- Click selects a section or list row. Clicking a section's summary line (or the `▶`/`▼` marker) toggles its fold. Double-clicking a session row opens it.
- Click on a harness tab switches to it. Click outside an overlay closes it.
- Mouse events go to the component under the pointer (§1.4).
- Mouse capture can be disabled in config (`mouse = false`). Most terminals also bypass mouse capture while `Shift` is held, which allows native text selection; the help overlay mentions this.

From SPEC.md §1.4:

- **Mouse:** wheel and clicks act on the element under the pointer, whether or not it has focus (§6.9). The event goes to the topmost component drawn there, for example through mouse subscriptions whose row/column ranges are that component's area. That component turns the event into a `Cmd` or `Msg`.
  - While an overlay is open (selector, help, theme selector), the components under it get no mouse events, and a click outside the overlay closes it.
  - Component areas come from a pure layout function that `App::view` also uses. Subscriptions are refreshed from it when the screen or terminal size changes, never from inside `view`.

From SPEC.md §6.3:

- Selecting a section scrolls it into view. If the selected section is taller than the viewport, `j`/`k` first scroll through it a page at a time and only then move to the next section.
- Scrolling with page keys or the mouse wheel moves the selection to the topmost fully visible section when the current one leaves the viewport.

From SPEC.md §6.2:

- Jumping to a match inside a folded section unfolds it temporarily. It refolds when the selection moves to a different section, unless the user unfolded it explicitly in the meantime.

From SPEC.md §5.3:

Results longer than N lines (default 200, configurable) are truncated, with a `… 1 234 more lines` row; pressing `Enter` on that row expands the rest.

From SPEC.md §8.4:

- `ESC` closes the overlay and restores the theme that was active when it opened.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Transcript wheel.** Each wheel event scrolls 3 rows (planner decision) through the same path as the page keys, so following/`↓ N new` (task 023) and the selection-follows rule apply.
- **Transcript click** (left press, positions relative to the Transcript area):
  1. The `Transcript` maps the row to (block, row within block) through its layout cache, then to a section or selection stop.
  2. A click on a section's first row (its summary line, or the `▶`/`▼` marker cells) of a foldable section selects it and toggles its fold. This is an explicit fold change for task 024's temporary-unfold rule.
  3. A click elsewhere in a section selects it without toggling.
  4. A click on a truncation row expands it.
  5. A click on borders, padding or the blank rows between sections selects nothing.
  6. The click also focuses the `Transcript` if, for example, nothing else had focus.
- **BrowserHeader / help line.** Clicks do nothing.
- **Overlays.**
  - While the selector (over a session), the help overlay or the theme selector is open, hit-testing considers only the topmost overlay.
  - A left press inside it goes to it; a press outside closes it with the same `Msg` as `Esc`. For the theme selector that is `CancelTheme`, which restores the theme. For the selector over a session it is `CloseSelector`.
  - At startup (selector with no session behind it), clicks outside the overlay do nothing, because there is nothing to return to and closing would mean quitting.
  - Wheel events outside the overlay are ignored.
- **Inside the theme selector.** A click on a valid theme row moves the cursor there (preview). A double-click (400 ms, injected clock, as in task 034) selects it, like `Enter`. Wheel moves the cursor.
- **Inside the help overlay.** The wheel scrolls; clicks do nothing.
- With `mouse = false`, none of this is active.

## Acceptance criteria
- [ ] Wheel over the transcript scrolls 3 rows per event and moves the selection to the topmost fully visible section when needed. While following, a wheel up stops following (component and update tests).
- [ ] A click on a section's summary/marker row toggles its fold and selects it. A click on its body selects it without toggling. A click on a truncation row expands the output. A click on a border or gap selects nothing (component tests with positions computed from the layout cache).
- [ ] A click while the selector is open over the browser: inside, it acts on the selector; outside, it closes the selector and returns focus to the `Transcript`. The transcript underneath receives no event (update tests).
- [ ] A click outside the help overlay closes it. A click outside the theme selector cancels and restores the previous theme. Wheel outside overlays is ignored (update tests).
- [ ] At startup, a click outside the selector does nothing (update test).
- [ ] Theme selector: a click previews that theme, a double-click selects it, and the wheel moves the cursor (update tests with an injected clock).
- [ ] A help overlay wheel event scrolls it (component test).
- [ ] With `mouse = false`, mouse events have no effect (test).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 030-help-overlay
- 031-theme-selector
- 034-selector-mouse
