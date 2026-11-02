# Browser live updates: follow, new-content indicator and live marker

## Goal
Implement the browser's live behaviour (§7.2):
- **Follow mode.** While the viewport is at the end of the transcript, new content scrolls into view.
- **Scrolled up.** When the user has scrolled up, the position is kept and the help line shows `↓ N new`. `G`/`End` jumps to the end and resumes following.
- **In-place updates.** Updates to content already displayed keep the selection and fold states.
- **Live marker.** The header shows `● live` while the session is active, re-evaluated on `Tick`.

## Spec excerpts
From SPEC.md §7.2:

- **Follow when at bottom:** if the viewport is at the end of the transcript, new content scrolls into view automatically. If the user has scrolled up, the position is kept and a `↓ N new` indicator appears in the help line; `G` / `End` jumps to the end and resumes following.
- Updates to content already displayed (e.g. a tool result arriving for a pending tool call, or a streamed message growing) update the section in place. The current selection and fold states are preserved.
- A `● live` marker is shown in the browser header while the session is active: file/rows modified within the last 60 s, or OpenCode `time_idle` is null.

From SPEC.md §6.3:

| `g` / `Home`, `G` / `End` | Jump to start / end (`G` resumes following, §7.2) |

From SPEC.md §4.2:

- Header block: `Project <name>` (left title), session id (right title), working directory in the body. A `● live` marker shows while the session is active (§7.2), and a breadcrumb shows when viewing a subagent (§5.4).

- Help line under the transcript: context-sensitive key hints. It turns into the search/path prompt when active and shows search position (`3/17`), the `↓ N new` indicator and parse warnings.

From SPEC.md §1.4:

- **Ticks:** `Event::Tick` drives toast expiry, the `● live` marker and the redraw limit (§7.1).
- **Worker results** take one path: Port → `Event::User` → `GlobalEvents` (subscribed to all user events) turns it into a `Msg` → `App::update` applies it to domain state and pushes what changed to the components that display it (rows and stats to `ProjectList` / `SessionList`, `ModelUpdate`s to `Transcript`). Other components don't subscribe to worker events.

From SPEC.md §7.1:

- Watch events are coalesced. Workers parse new data off the UI thread and send `ModelUpdate`s; the UI thread applies them (§1.4) and redraws at most every 50 ms.

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
- **Following.**
  - The `Transcript` has a `following` flag. It is true after the initial load when the viewport shows the last row, and false as soon as the user scrolls or selects so that the last row is not visible.
  - `bottom` (`G`/`End`) and scrolling back to the very end set it true again.
  - While following, `Appended` turns and growing turns keep the view pinned to the last row. When the selection was the last section before the update, it moves to the new last section; otherwise the selection is kept.
  - While the initial load is still streaming (`Loading… N%`), following is off and the view stays at the top. The flag is evaluated after `Loaded`.
- **`↓ N new`.**
  - While not following, the `Transcript` counts sections appended since the user left the bottom, including new sections inside an updated last turn.
  - It reports the count to the `App` as a `Msg` whenever it changes. The `App` shows `↓ N new` as a help-line status item, placed before any search position item.
  - The count resets to 0 (and the item disappears) when following resumes.
- **In-place updates.** `Updated` turns keep selection, fold states, expansion states and scroll position. If the selected section's own height changes above the viewport, adjust the scroll offset so the visible content doesn't jump.
- **`● live`.**
  - The `App` stores the latest `ModelUpdate::Activity` (task 022) for the open session.
  - On each `Tick` it evaluates the task-010 rule (`idle == Some(false)`, or `last_modified` within 60 s of an injected clock's "now"), and pushes the boolean to `BrowserHeader` only when it changes.
  - `BrowserHeader` draws `● live` right-aligned on the cwd row: `●` in `primary` and `live` in `muted_foreground`, ending at column `w − 3`. The parse-warning note (task 021), when present, sits to its left with two cells between them.
- **Redraws.** Each applied update marks the screen dirty. The 50 ms limit from task 011 still applies.

## Acceptance criteria
- [ ] At the bottom, an `Appended` turn scrolls the view so the new last row is visible. The selection follows if it was on the last section (component test).
- [ ] Scrolled up, an `Appended` turn keeps the scroll offset and selection, and the reported new count increases by the number of new sections. `G` jumps to the end, resets the count and resumes following (component tests).
- [ ] The `App` shows `↓ 3 new` in the help line when the `Transcript` reports 3, and removes it at 0 (update test plus snapshot).
- [ ] `Updated` for a visible turn (a tool result arriving) keeps the selection, fold states and scroll offset (component test).
- [ ] During the initial streaming load the view stays at the top. After `Loaded`, follow mode is on only if the last row is visible (test).
- [ ] `● live` appears when `Activity.last_modified` is within 60 s of the injected now, and disappears on a `Tick` after 60 s (update test with an injected clock). `idle == Some(false)` keeps it shown regardless of the time.
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
Changes to the browser screen (task 021):
- **Header cwd row.** `● live` is right-aligned, ending at column `w − 3`: `●` in `primary`, `live` in `muted_foreground`. If the parse-warning note is present, it sits two cells to the left of `● live`.
- **Help line.** The status item `↓ N new` is right-aligned, in `help_text`.

Snapshots (screen through `App::view`, `utc = true`, injected clock):
1. Live session at the bottom, with `● live` in the header, at 120x40 (text + styled) and 80x24 (text).
2. Scrolled up after 3 new sections arrived: `↓ 3 new` in the help line, with the view unchanged, at 80x24 (text).
3. Header with both `2 entries could not be parsed` and `● live` at 120x40 (text).
4. After `G`: at the bottom with the new content visible, and no `↓ N new`, at 80x24 (text).

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
