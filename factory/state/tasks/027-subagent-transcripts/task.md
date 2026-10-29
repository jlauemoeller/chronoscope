# Subagent transcripts and breadcrumb navigation

## Goal
Let the user open a subagent's own transcript from its section (§5.4).
- **Opening.** `a` on a selected subagent section resolves the subagent session through `Harness::subagent()` and mounts a new `Transcript` for it, loaded and watched like any session. The parent's `Transcript` stays mounted but is not drawn.
- **Breadcrumb.** The header shows `Project BRIO › Agent: story-writer`.
- **Going back.** `Backspace` unmounts the subagent view and returns to the parent exactly as it was: same scroll position, selection and folds.

Nesting works as a stack. The UI part is harness-agnostic. This task wires it for Claude; OpenCode plugs in through the same trait in task 028.

## Spec excerpts
From SPEC.md §5.4:

- Subagent sections show the prompt and the final result inline.
- A key (§6) opens the subagent's own transcript as a full session view: Claude `subagents/agent-<id>.jsonl`, OpenCode child `session_v2` via `parent_id`.
- The browser header shows a breadcrumb (`Project BRIO › Agent: story-writer`). A back key returns to the parent view at the same scroll position.

From SPEC.md §1.3:

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (§9.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (§5.2), one body renderer per `ToolKind` (§5.3), the markdown renderer (§5.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (§5.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

From SPEC.md §1.2:

- **`App`** owns the `Application` and the domain state: config, theme registry and active theme, keymap, session index and stats, the open session's summary, watch handle and subagent stack, worker handles and the clipboard.

- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (§5.1), so a 100 MB session is held once (§9.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

From SPEC.md §4.2:

- Header block: `Project <name>` (left title), session id (right title), working directory in the body. A `● live` marker shows while the session is active (§7.2), and a breadcrumb shows when viewing a subagent (§5.4).

From SPEC.md §6.5:

| Key | Action |
|---|---|
| `a` | Open the selected subagent's transcript (§5.4) |
| `Backspace` | Back to the parent session (after `a`) |

From SPEC.md §1.1 (Harness trait, excerpt):

```rust
    fn subagent(&self, parent: &SessionId, call: &ToolCallId) -> Option<SessionId>;
```

From SPEC.md §7.1:

| Harness | Change detection | Incremental read |
|---|---|---|
| Claude | `notify` watch on the session `.jsonl` (and its `subagents/` dir) | Read from the last byte offset. New lines go through the same turn assembly (§5.1), so a line extending an existing `message.id` or delivering a `tool_result` updates sections in place. Keep a trailing partial line in a buffer until its newline arrives. If the file shrinks or is replaced (size < offset or inode change), reload it. |

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Ids.** `Id::Transcript(depth)` (or similar), where depth 0 is the session and each opened subagent pushes depth + 1. The `App` keeps a subagent stack of entries `{ session id, summary or info, WatchHandle, breadcrumb label, load generation }`. Only the top transcript is drawn and focused. Lower ones stay mounted and untouched.
- **Opening.**
  - `browser.open_subagent` on a selected Subagent tool section returns `Msg::OpenSubagent { call_id, label }` from the `Transcript`. The label is the agent type from the call input (`subagent_type`), or the description.
  - The `App` calls `harness.subagent(&current_session, &call_id)`.
    - `None` gives a toast `No transcript for this subagent`.
    - `Some(id)` mounts a fresh `Transcript` at the next depth, starts `load(id, …)` with a new generation (live watching included), focuses it, and pushes the stack entry.
  - On any other section, `a` does nothing.
- **Breadcrumb.**
  - `BrowserHeader`'s left title becomes `Project <name> › Agent: <label>`, with one ` › Agent: <label>` segment per stack level. `›` and the `Agent:` labels are in `muted_foreground`, and agent names in `foreground`.
  - The right title shows the subagent's session id.
  - The cwd row stays the parent session's cwd.
  - The header's live marker and parse-warning note reflect the top-most transcript's updates.
- **Going back.**
  - `browser.back` (`Backspace`) at depth > 0 returns `Msg::CloseSubagent`.
  - The `App` drops that level's `WatchHandle`, unmounts its `Transcript`, pops the stack, refocuses the parent `Transcript`, and restores the header to the parent's state (title, warnings, live).
  - The parent's presentation state is untouched because it was never unmounted.
  - At depth 0, `Backspace` does nothing.
  - Updates for a popped generation are ignored.
- Opening another session (`Ctrl-o` → `Enter`) clears the whole stack.
- Fixtures: reuse the scrubbed Claude subagent fixture from task 009.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] `a` on a Subagent section returns `Msg::OpenSubagent`, and on other sections returns nothing (component test).
- [ ] `App` handling of `OpenSubagent` (update test with a fake harness whose `subagent()` resolves):
  - a new `Transcript` is mounted at depth 1 and focused;
  - the load starts;
  - the header shows `Project <name> › Agent: <label>` and the subagent id.
- [ ] `subagent()` returning `None` shows the toast `No transcript for this subagent` and mounts nothing (update test).
- [ ] `Backspace` at depth 1 restores the parent, with identical scroll offset, selection and fold states (assert the parent's `state()` before and after), and drops the subagent's watch (update test). At depth 0 it does nothing.
- [ ] Nested subagents (depth 2) show two breadcrumb segments, and pop one at a time (update test).
- [ ] Late updates from a popped subagent load are ignored (update test).
- [ ] With the real Claude harness and the fixture, opening the subagent shows its prompt as the first User turn (integration-style test using the fixture dir).
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
Changes to the browser screen:
- **Header left title.** `╭─ Project BRIO › Agent: story-writer ─…─ <subagent session id> ─╮`: `Project` and `› Agent:` in `muted_foreground`, names in `foreground` (the project name bold). The cwd row is unchanged.
- **Transcript area.** Shows the subagent's transcript (its prompt as a User block first).

Snapshots (screen through `App::view`, fixtures, `utc = true`):
1. Parent session with a subagent section unfolded and selected (`Prompt:`, `Result:`, `a to open transcript`) at 120x40 (text).
2. After `a`: the subagent transcript with the breadcrumb header at 120x40 (text + styled) and 80x24 (text).
3. Nested depth 2 breadcrumb at 120x40 (text).
4. After `Backspace`: identical to state 1 (text; assert equal to snapshot 1's content).

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
- 022-claude-live-watch
