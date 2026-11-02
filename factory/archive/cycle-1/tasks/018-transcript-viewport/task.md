# Transcript component: layout cache and virtualised viewport

## Goal
Build the `Transcript` component: one mounted tui-realm component that holds the open session's turns and draws them as a vertically scrolling stack of `TurnBlock`s (task 017). It owns:
- fold states and the selection, keyed by stable section ids;
- a scroll offset;
- a per-block layout cache for the current width.

It renders only the blocks that overlap the viewport, clipping partially visible blocks. `ModelUpdate`s arrive through `attr` (appended turns, in-place updates of existing turns). Selection and fold states survive them. Basic `Cmd`s scroll by rows and move the selection by section; task 020 binds all the keys. Property tests verify that cached heights always match the rendered output.

## Spec excerpts
From SPEC.md §1.3:

- A component implements tui-realm's `Component` trait (`view`, `query`/`attr`, `state`, `perform(Cmd) -> CmdResult`), plus `AppComponent` (`on(&Event) -> Option<Msg>`) if it handles events. An element that appears on several screens (e.g. the help line) is one component type, mounted with different props.
- A component draws only itself. Its `view` draws its own border and content, and leaves sub-elements to child types: a child component it owns, or a small `Widget`/`StatefulWidget` type.
- **No long render functions.** A render function (`view`, or a helper it calls) that draws more than one distinct element, or grows past about 50 lines, is split into smaller components or widgets. Rendering has no side effects.
- Components are `pub` types that can be built from plain data (props and the theme) and rendered into a `TestBackend` frame on their own, without an `Application` or a terminal (§9.6).

| Component | Where | Draws / handles | Takes focus |
|---|---|---|---|
| `Transcript` | Browser | The turn blocks: selection, folding, navigation, search highlights (§5, §6.1–§6.3) | Yes |

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (§9.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (§5.2), one body renderer per `ToolKind` (§5.3), the markdown renderer (§5.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (§5.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

From SPEC.md §1.2:

Rules:
- Data flows **down** into components through `attr` (typed data via `AttrValue::Payload(PropPayload::Any(..))`, `Arc`-wrapped when large). Intent flows **up** as `Msg`. Components never see the `App`, and the `App` never reaches into a component's internals.
- No shared mutable state between components, or between a component and the `App`: no `Rc<RefCell<…>>`, `Arc<Mutex<…>>` or globals for UI state.
- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (§5.1), so a 100 MB session is held once (§9.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

From SPEC.md §4.2:

- Transcript: one rounded block per turn (§5.2), scrolling vertically, with the selected section marked by `▒` over the block's left border.

From SPEC.md §5.5:

- Wrapping is recomputed when the terminal is resized.

From SPEC.md §6.3:

- Selecting a section scrolls it into view. If the selected section is taller than the viewport, `j`/`k` first scroll through it a page at a time and only then move to the next section.

From SPEC.md §7.2:

- Updates to content already displayed (e.g. a tool result arriving for a pending tool call, or a streamed message growing) update the section in place. The current selection and fold states are preserved.

From SPEC.md §9.2:

Target: sessions up to **~100 MB** stay smooth.
- Parsing is streaming (line by line for JSONL, paged queries for SQLite) and runs off the UI thread. The UI shows a loading state and becomes interactive as soon as the first screen of content is parsed.
- Large tool outputs: the model stores line counts and byte ranges up front. The full text is materialised only when the section is unfolded, searched, copied or exported.
- Layout is computed lazily per block for the current width. Only blocks overlapping the viewport are rendered.

- Budgets (to be measured with benchmarks on a synthetic 100 MB fixture):
  - first content within 300 ms;
  - full parse under 2 s;
  - each frame rendered in under 16 ms;
  - memory use within roughly 3× the file size.

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
- **Props and data.** The `Transcript` gets its props through `attr`: theme, `dim_unselected`, `show_system` (initial value), `truncate_output_lines`, timestamp formatter settings, and `ModelUpdate`s. Updates are a `PropPayload::Any` holding `Arc`-wrapped data.
  - `Appended` adds turns at the end.
  - `Updated` replaces turns with the same `TurnId`, in place.
  - A `Reset` variant (add it now if task 009 did not) clears all turns, folds and selection; task 022 uses it for reloads.
  - Other variants (`Progress`, `Loaded`, `ParseWarnings`) are not for the Transcript; the App consumes them.
  - The Transcript holds the turns. It never keeps a second copy, and the App keeps none.
- **Presentation state, owned by the Transcript.**
  - Fold overrides are stored by `SectionId`, so an updated turn keeps its sections' fold states.
  - The selected `SectionId` starts at the first section once turns arrive.
  - The scroll offset is the top row of the viewport, in transcript rows.
  - The layout cache holds per-block heights for the current width, plus prefix sums for row → block lookup.
- **Layout cache.**
  - Heights are computed lazily, per block, for the current width.
  - A width change (resize) invalidates all heights.
  - A fold change, an `Updated` turn, or a `show_system` change invalidates only the affected blocks.
  - Locating the block at a given row must not scan all blocks linearly per frame; use prefix sums or an equivalent.
  - Blocks are stacked with no blank rows between them, as in the sketch.
- **Viewport rendering.**
  - `view` renders only blocks overlapping `[offset, offset + area.height)`. It uses `TurnBlock`'s clipped rendering for the first and last block.
  - It paints `background` behind the transcript area (not when transparent).
  - It draws nothing outside its area, and rendering has no side effects. The cache is filled before `view`: lazily in `perform`/`attr`, or in a pre-render step the App triggers on resize. It is never mutated during `view`; use interior cache filling only if it cannot be avoided, and document why.
- **Basic Cmds** (keys come in task 020):
  - `Cmd::Scroll(Direction::Down/Up)` scrolls one row, clamped so the last row cannot scroll above the viewport bottom.
  - `Cmd::Move(Direction::Down/Up)` selects the next/previous section, across blocks, and scrolls it into view minimally.
  - `Cmd::Submit` toggles the selected section's fold.
- **Selection after updates.** The selected `SectionId` is kept when its turn is updated. If the section disappears, the selection moves to the nearest following section, or else the last one.
- **Property tests.** Use `proptest` (add it as a dev-dependency). Generate random widths (20–200), random fold states and random fixture turns, and assert that the cached height of every block equals the number of rows `TurnBlock` actually draws, and that the total height equals the sum.

## Acceptance criteria
- [ ] `Transcript` implements tui-realm's `Component` (`view`, `query`/`attr`, `state`, `perform`). It can be built from props alone and rendered into a `TestBackend` without an `Application` (component test).
- [ ] `Appended` updates add blocks. An `Updated` update replaces a turn in place and keeps the selection and every fold state of its sections. `Reset` clears everything (component tests).
- [ ] The layout cache:
  - after a width change, heights are recomputed and match rendering;
  - a fold toggle invalidates only that block (asserted through a recompute counter);
  - row → block lookup is not a linear scan per frame (code review plus a test of the lookup function).
- [ ] `view` renders only visible blocks. With 2 000 turns, rendering a frame calls the block renderer only for blocks overlapping the viewport (counter test).
- [ ] A block partly above or below the viewport is clipped correctly: the screen rows equal the corresponding slice of the full rendering (test).
- [ ] `Cmd::Scroll` clamps at the top and bottom. `Cmd::Move` crosses block boundaries and scrolls the new selection into view. `Cmd::Submit` toggles the fold (tests).
- [ ] Proptest: cached heights equal rendered heights for random widths and fold states.
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
A new component, `Transcript`. It draws a stack of `TurnBlock`s (geometry from task 017) filling its area, with no gaps between blocks. Only the viewport slice is drawn, partially visible blocks are clipped at the top and bottom edges, and the background is `background`.

Snapshots (component, fixture session of about 8 turns: user, assistant with tools, compaction, …):
1. Top of the transcript, first section selected, at 80x24 and 120x40 (text) and 120x40 (styled: `▒` and dimming).
2. Scrolled to the middle so that one block is clipped at the top edge and another at the bottom edge, at 80x24 (text).
3. A selection in the 5th block after `Cmd::Move` steps, scrolled into view, at 80x24 (text).
4. The same session after a resize from 120 to 80 columns, with text re-wrapped (text, 80x24).
5. `dim_unselected = false` at 120x40 (styled).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 017-turn-block-sections
