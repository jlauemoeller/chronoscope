# Claude live watching

## Goal
Extend the Claude `load()` worker from task 009 so that, after the initial parse, it keeps the session live (§7.1). It watches the session's `.jsonl` file (and its `subagents/` directory) with `notify` and reads appended bytes from the last offset. A trailing partial line is buffered until its newline arrives. New lines go through the same turn assembler, so streamed messages and late `tool_result`s update existing turns in place (`Updated`) and new turns arrive as `Appended`. Bursts of events are coalesced. When the file shrinks or is replaced, the worker reloads it from scratch (`Reset` plus a full re-parse). It also reports the file's modification time, so the browser can show `● live`.

## Spec excerpts
From SPEC.md §7.1:

| Harness | Change detection | Incremental read |
|---|---|---|
| Claude | `notify` watch on the session `.jsonl` (and its `subagents/` dir) | Read from the last byte offset. New lines go through the same turn assembly (§5.1), so a line extending an existing `message.id` or delivering a `tool_result` updates sections in place. Keep a trailing partial line in a buffer until its newline arrives. If the file shrinks or is replaced (size < offset or inode change), reload it. |

- The database is opened read-only (`SQLITE_OPEN_READ_ONLY`). Chronoscope never writes to any harness store.
- Watch events are coalesced. Workers parse new data off the UI thread and send `ModelUpdate`s; the UI thread applies them (§1.4) and redraws at most every 50 ms.

From SPEC.md §7.2:

- Updates to content already displayed (e.g. a tool result arriving for a pending tool call, or a streamed message growing) update the section in place. The current selection and fold states are preserved.
- A `● live` marker is shown in the browser header while the session is active: file/rows modified within the last 60 s, or OpenCode `time_idle` is null.

From SPEC.md §1.1:

```rust
trait Harness {
    fn id(&self) -> HarnessId;
    fn available(&self) -> bool;                       // data store found
    fn projects(&self) -> Result<Vec<Project>>;
    fn sessions(&self, project: &ProjectId) -> Result<Vec<SessionSummary>>;
    fn load(&self, session: &SessionId, tx: Sender<ModelUpdate>) -> Result<WatchHandle>;
    // streams the initial parse, then incremental updates, as ModelUpdate events
    fn subagent(&self, parent: &SessionId, call: &ToolCallId) -> Option<SessionId>;
    fn resume_command(&self, session: &SessionSummary) -> Option<Command>;
}
```

- Parsing, watching, stats computation and search indexing run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used.

From SPEC.md §9.5:

- Never writes to harness data. The SQLite database is opened read-only.

From SPEC.md §9.1:

- No direct OS APIs; only cross-platform crates (crossterm, notify, arboard, rusqlite bundled, etcetera).

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Watching.**
  - After sending `Loaded`, the worker watches the session file's parent directory, non-recursively, filtering for the session file. It also watches `<session-uuid>/subagents/` when that exists (or once it appears).
  - Events are coalesced: after the first event, wait 100 ms for more, then process once.
  - If `notify` fails to start, fall back to polling the file's size and mtime every 1 s. Log the fallback.
- **Incremental read.**
  - Keep the byte offset of the last complete line. On change, if the size is greater than or equal to the offset, read from the offset to EOF.
  - Split on `\n`. Feed complete lines (with their byte ranges, for lazy tool outputs) to the assembler. Keep a trailing partial line in a buffer.
  - Send the assembler's updates as `Appended`/`Updated` batches, plus `ParseWarnings` when the count changes.
- **Reload.** If the size is below the offset, or the file identity changed (inode on Unix via `std::os::unix::fs::MetadataExt`; on other platforms, a creation-time change or a shrink), send `ModelUpdate::Reset`, re-parse from the start through the same streaming path as the initial load (batches + `Progress` + `Loaded`), and continue watching.
- **Activity.** Add `ModelUpdate::Activity { last_modified: SystemTime, idle: Option<bool> }`. Send it after the initial load and after every processed change, with the file's mtime and `idle: None`.
- **Subagents dir.** Changes there do not affect the parent's turns. Log them at debug level; task 027 uses subagent files through their own `load()`.
- **Never write.** Open files read-only. No locks, no writes, nothing in the harness directory is touched.
- The `WatchHandle` drop from task 009 also stops the watcher, promptly.

## Acceptance criteria
- [ ] Appending complete lines to a watched temp file produces `Appended` updates with the new turns (the test waits up to 10 s, to tolerate slow file-event delivery on macOS CI).
- [ ] Appending a line that extends an existing `message.id`, or that delivers a `tool_result` for a pending call, produces `Updated` for that turn, with the same `TurnId` and the call status no longer `Pending` (test).
- [ ] A line written in two parts (first without a newline, then the rest) is parsed exactly once, after the newline arrives (test).
- [ ] Truncating the file, or replacing it via rename, produces `Reset` followed by a full re-parse ending in `Loaded`, after which turns equal a fresh parse (tests; the inode case only on Unix, `#[cfg(unix)]`).
- [ ] 50 rapid appends within 100 ms result in at most a few batches, not 50 (coalescing test asserting fewer than 10 update batches).
- [ ] `Activity` carries the file mtime after the initial load and after each change (test).
- [ ] When watching is unavailable (simulated through a constructor flag or a test seam), polling (every 1 s) detects appends (the test waits up to 10 s).
- [ ] Dropping the `WatchHandle` stops watching: no further updates arrive and the channel disconnects (test).
- [ ] No code path opens harness files for writing (code review; files opened with `File::open`/read-only options only).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 009-claude-session-loading
