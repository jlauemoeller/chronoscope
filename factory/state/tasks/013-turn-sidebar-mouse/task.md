# Turns sidebar mouse

## Goal
Make the sidebar work with the mouse like the selector's lists: the wheel moves its selection, a click selects a row and focuses the pane, and a double-click does what `Enter` does.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §6.4:

**Mouse** (base spec §6.9): the wheel works as in the selector's lists. Clicking a selectable row focuses the sidebar and selects that row, double-clicking does what `Enter` does, and clicking the transcript gives it focus.

From SPEC.md §6.2 (which rows can be clicked):

| Row | Content | Selectable |
|---|---|---|
| `● Start` | First row of the list | No: it has no block |
| `● User` | A User turn | Yes |
| `● Assistant` | An Assistant turn, with its duration (§6.5) | Yes |
| `═══ Compaction ═══` | A Compaction turn; `═` fills to the right margin | Yes |
| `● End` | Last row of the list | No: it has no block |

- Long lists scroll by rows. `Start` and `End` scroll with the list. Connector and blank rows cannot be selected or clicked.

From SPEC.md §6.4 (what selecting an entry does):

- Selecting an entry **scrolls the transcript to its block**, with the same result as `]` / `[`: the block's top at the top of the viewport (or as near as the end of the transcript allows), and the transcript's selection on the block's first selectable section. Reuse that code rather than writing a second path. The usual follow rule applies (base spec §7.2): moving away from the bottom stops following.
- A turn can have an entry but nothing selectable, when its text renders to no rows (for example, only an HTML comment). Selecting that entry still scrolls to the block and leaves the transcript's selection unchanged.
- `Enter` does the same and also gives the transcript focus.

From SPEC.md §7:

Every task must pass:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tasks that change parsing, the model or rendering must also stay within budget on:

```
cargo test --release -- --ignored budgets --nocapture
```

Snapshot changes are limited to the rows a task intends to change. A snapshot changed by accident is a failure, not something to accept. Two changes are expected to touch many snapshots, and those are still reviewed one by one: the shared duration format (§2.4) changes every snapshot showing a footer or selector-row duration of a minute or longer, and the sidebar changes every browser screen snapshot at 90 columns or wider.

From spec/SPEC.md §1.4:

- **Focus:** exactly one component has focus at a time, set by `App::update` through `Application::active`. In the selector, the pane keys of §6.6 move focus between `ProjectList` and `SessionList`. A focused component shows it through `Attribute::Focus` (e.g. the `ring` border, §8.2).
- **Overlays and prompts:** opening one mounts its component and gives it focus. Closing it unmounts it and returns focus to the component that had it before.
- **Keys go through the keymap.** A component's `on()` resolves the key with the shared keymap (§8.5), for its own context, to an action. The action becomes a `Cmd` (handled internally with `perform`) or a `Msg`. Components never match literal `KeyEvent`s, except that `Prompt` takes printable characters as text. This keeps remapping and the generated help overlay (§4.3) correct.
- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, §6; `Esc` / `?` closes help). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (§6.6).
- **Mouse:** wheel and clicks act on the element under the pointer, whether or not it has focus (§6.9). The event goes to the topmost component drawn there, for example through mouse subscriptions whose row/column ranges are that component's area. That component turns the event into a `Cmd` or `Msg`.
  - While an overlay is open (selector, help, theme selector), the components under it get no mouse events, and a click outside the overlay closes it.
  - Component areas come from a pure layout function that `App::view` also uses. Subscriptions are refreshed from it when the screen or terminal size changes, never from inside `view`.
- **Ticks:** `Event::Tick` drives toast expiry, the `● live` marker and the redraw limit (§7.1).
- **Worker results** take one path: Port → `Event::User` → `GlobalEvents` (subscribed to all user events) turns it into a `Msg` → `App::update` applies it to domain state and pushes what changed to the components that display it (rows and stats to `ProjectList` / `SessionList`, `ModelUpdate`s to `Transcript`). Other components don't subscribe to worker events.

From spec/SPEC.md §6.9:

- Wheel scrolls the transcript or the list under the pointer.
- Click selects a section or list row. Clicking a section's summary line (or the `▶`/`▼` marker) toggles its fold. Double-clicking a session row opens it.
- Click on a harness tab switches to it. Click outside an overlay closes it.
- Mouse events go to the component under the pointer (§1.4).
- Mouse capture can be disabled in config (`mouse = false`). Most terminals also bypass mouse capture while `Shift` is held, which allows native text selection; the help overlay mentions this.

From spec/SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):

- **Routing.** `takes_mouse` in `src/app/mouse.rs` gains `Id::TurnSidebar` (not while a prompt has the input). The component handles only `UserEvent::Mouse` (`RoutedMouse`, in its own coordinates), never the raw `Event::Mouse` that goes to the focused component in screen coordinates, as the module doc of `src/app/mouse.rs` explains.
- **The wheel.** In the selector's lists the wheel moves the selection like the keys (`ProjectList::on_mouse`), so here it selects the previous or next entry and scrolls the transcript to it. It does not change the focus, since it acts on the element under the pointer whether or not that element has the focus (spec/SPEC.md §1.4). No wrap-around.
- **Wheel over an entry with nothing selectable.** The transcript's selection does not change there, and task 011's sync only pushes on a change of the transcript's selection, so the sidebar keeps its row. A test must show the wheel can move past such an entry while the transcript has the focus.
- **Clicks.** A left press on a selectable row reports its turn; the `App` focuses the sidebar, selects the row and scrolls the transcript. A press on `Start`, `End`, a connector row, a blank row, the border or the margin does nothing, including to the focus. `row_at` in `list_pane.rs` assumes one blank row above the list, while the sidebar has two: add the sidebar's own version, using its scroll offset.
- **Double-click.** Add a `ClickTarget::Turn(TurnId)` in `src/app/mouse.rs`, using `MouseState::press` like the other rows, so that a press elsewhere ends a double-click under way. A double-click does what `Enter` does: the transcript gets the focus.
- **The transcript.** Clicking it gives it the focus through the existing `focus_transcript`. Add the test; no new code should be needed. The click's selection then syncs the sidebar.

## Acceptance criteria
- [ ] Mouse tests cover clicking and double-clicking a row, a click on a connector row, and the wheel.
- [ ] A test shows a click on a selectable row focuses the sidebar, selects the row and scrolls the transcript to its block, the same result as `j` / `k` selecting it.
- [ ] A test shows a double-click leaves the transcript with the focus and its selection on the block's first selectable section.
- [ ] Tests show presses on `Start`, `End`, a connector row, a blank row, the border and the margin change neither the selection nor the focus, and that such a press between two clicks on a row ends the double-click.
- [ ] Tests show two clicks on different rows are not a double-click, and that two clicks further apart than `DOUBLE_CLICK` are not either.
- [ ] Tests show a row is found correctly when the list is scrolled.
- [ ] Tests show the wheel moves the selection and scrolls the transcript without changing the focus, with no wrap-around at either end.
- [ ] A test shows the wheel moves past an entry with nothing selectable while the transcript has the focus.
- [ ] A test shows clicking the transcript gives it the focus while the sidebar has it.
- [ ] Tests show that with a prompt open, an overlay open, or `mouse = false`, the sidebar gets no mouse events.
- [ ] A test shows a raw `Event::Mouse` sent to the focused sidebar does nothing.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] No snapshot changes.

## UI
UI: none

## Depends on
- 012-turn-sidebar-keys
