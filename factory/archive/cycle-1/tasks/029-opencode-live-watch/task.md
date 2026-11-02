# OpenCode live watching

## Goal
Keep open OpenCode sessions live (§7.1). After the initial load, the OpenCode `load()` worker watches `opencode.db` and `opencode.db-wal` with `notify`, debounced by 200 ms, falling back to polling every 1 s if watching fails. On each change it:
- reads new rows (`seq > last`);
- re-reads rows whose `time_updated` changed, since streaming messages are updated in place;
- re-assembles the affected turns, emitting `Appended`/`Updated` updates;
- reports activity (`time_idle IS NULL` → active).

The browser's follow mode, `↓ N new` and `● live` (task 023) then work for OpenCode as they do for Claude. The selector's index (task 010) also refreshes OpenCode lists live.

## Spec excerpts
From SPEC.md §7.1:

| Harness | Change detection | Incremental read |
|---|---|---|
| OpenCode | `notify` watch on `opencode.db` and `opencode.db-wal`, debounced 200 ms; fallback poll every 1 s if watching fails | `SELECT … FROM session_message WHERE session_id = ? AND seq > ?`, plus re-reading rows whose `time_updated` changed (streaming messages are updated in place) |

- The database is opened read-only (`SQLITE_OPEN_READ_ONLY`). Chronoscope never writes to any harness store.
- Watch events are coalesced. Workers parse new data off the UI thread and send `ModelUpdate`s; the UI thread applies them (§1.4) and redraws at most every 50 ms.

From SPEC.md §7.2:

- **Follow when at bottom:** if the viewport is at the end of the transcript, new content scrolls into view automatically. If the user has scrolled up, the position is kept and a `↓ N new` indicator appears in the help line; `G` / `End` jumps to the end and resumes following.
- Updates to content already displayed (e.g. a tool result arriving for a pending tool call, or a streamed message growing) update the section in place. The current selection and fold states are preserved.
- A `● live` marker is shown in the browser header while the session is active: file/rows modified within the last 60 s, or OpenCode `time_idle` is null.

From SPEC.md §7.3:

- Project and session lists update live: new sessions appear, and titles and stats refresh. The current selection is kept (matched by id, not by index).
- Active sessions show the `●` marker in their row.

From SPEC.md §9.5:

- Never writes to harness data. The SQLite database is opened read-only.

From SPEC.md §9.2:

- Parsing is streaming (line by line for JSONL, paged queries for SQLite) and runs off the UI thread. The UI shows a loading state and becomes interactive as soon as the first screen of content is parsed.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Watching.**
  - Watch the DB file and its `-wal` file through their parent directory (non-recursive, filtered), because the WAL file may be created and removed.
  - Debounce: after an event, wait until 200 ms have passed without further events, then process once.
  - If `notify` fails, poll every 1 s by checking `MAX(time_updated)` and `MAX(seq)` for the session. Log the fallback.
- **Incremental query.** Keep `last_seq` and a map `message id → time_updated` for rows seen. On change:
  1. `SELECT … WHERE session_id = ? AND seq > ? ORDER BY seq`, for new rows.
  2. `SELECT id, time_updated … WHERE session_id = ? AND time_updated > ?`, using the max `time_updated` seen, to find updated rows. Re-read their `data`.
  3. Rows that were already seen produce `Updated` for the turns containing them. New rows go through the task-028 assembly, producing `Appended` or `Updated` (a new assistant row extends the current Assistant turn).
- **Activity.** Re-read `session_v2.time_updated` and `time_idle` for the session on each change. Send `ModelUpdate::Activity { last_modified: time_updated, idle: Some(time_idle IS NOT NULL) }`. With `idle == Some(false)` the browser shows `● live` regardless of age (task 023 rule).
- **Index refresh.** The task-010 index already watches `watch_paths()` for OpenCode. Make sure its rebuild re-queries projects and sessions, so new sessions, titles and stats appear. Stats keys change with `time_updated`.
- **Read-only.** Every query goes through the task-028 select-only helper. Tests modify the temp fixture DB through a separate read-write `rusqlite` connection, simulating OpenCode.
- The `WatchHandle` drop stops watching and polling promptly.

## Acceptance criteria
- [ ] Inserting a new `assistant` row through a separate writer connection produces an update for the session (the test waits up to 10 s): `Updated` for the current Assistant turn, or `Appended` for a new User turn (test).
- [ ] Updating an existing row's `data` and `time_updated` (a streaming message growing) produces `Updated` for its turn, with stable `TurnId`/`ItemId`s (test).
- [ ] Ten writes in quick succession are coalesced by the debounce: the test asserts fewer than ten update batches.
- [ ] With `notify` disabled through a test seam, polling (every 1 s) detects a new row (the test waits up to 10 s).
- [ ] Setting `time_idle` to `NULL` produces `Activity { idle: Some(false) }`, and setting it to a timestamp produces `idle: Some(true)` (test).
- [ ] The index delivers a new `Snapshot` including a session inserted into `session_v2` (the test waits up to 10 s).
- [ ] No statement other than SELECT/PRAGMA is ever executed on the harness connection (the helper's rejection test from task 028 still passes), and the reader never modifies the DB.
- [ ] Dropping the `WatchHandle` stops the worker and the channel disconnects (test).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 023-browser-live-updates
- 028-opencode-adapter
