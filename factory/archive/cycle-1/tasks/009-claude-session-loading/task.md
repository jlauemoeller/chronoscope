# Claude session loading, subagent linkage and resume command

## Goal
Complete the Claude `Harness`:
- `load()` parses a session on a worker thread and streams the result as `ModelUpdate`s: early batches first, so the UI can show the first screen quickly, plus progress and the parse-warning count. It returns a `WatchHandle` that stops the worker when dropped. Live watching of the file is added to this same worker in task 022.
- `subagent()` links an `Agent`/`Task` tool call to its `subagents/agent-<id>.jsonl` transcript, which `load()` can then open.
- `resume_command()` builds the configured resume command for a session.

## Spec excerpts
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

From SPEC.md §3.1:

- Subagent transcripts: `<session-uuid>/subagents/agent-<agentId>.jsonl` (entries have `isSidechain: true`) + `agent-<agentId>.meta.json` (`agentType`, `description`, `toolUseId`).
  - Linkage: the parent's `Agent` `tool_use.id` equals `meta.toolUseId`, and the matching `tool_result` entry's `toolUseResult.agentId` names the file.

From SPEC.md §4.2:

- Loading state: `Loading… 34%` in the help line while the initial parse streams in.

From SPEC.md §5.4:

- Subagent sections show the prompt and the final result inline.
- A key (§6) opens the subagent's own transcript as a full session view: Claude `subagents/agent-<id>.jsonl`, OpenCode child `session_v2` via `parent_id`.
- The browser header shows a breadcrumb (`Project BRIO › Agent: story-writer`). A back key returns to the parent view at the same scroll position.

From SPEC.md §6.7:

Commands are configurable templates (§8.5). Defaults:
- Claude: `claude --resume {id}`
- OpenCode: `opencode --session {id}`. **Verify this flag** before release.

From SPEC.md §9.2:

Target: sessions up to **~100 MB** stay smooth.
- Parsing is streaming (line by line for JSONL, paged queries for SQLite) and runs off the UI thread. The UI shows a loading state and becomes interactive as soon as the first screen of content is parsed.

  - first content within 300 ms;
  - full parse under 2 s;

From SPEC.md §9.5:

  - A muted `N entries could not be parsed` note appears in the browser header.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Trait, stage 2.** Add `load`, `subagent` and `resume_command` to `Harness` with the §1.1 signatures. `Sender` is `std::sync::mpsc::Sender`. `Command` is `std::process::Command`.
- **New ModelUpdate variants.**
  - `Progress(f32)`: the fraction of bytes parsed, 0.0–1.0.
  - `Loaded`: the initial parse is complete.
  - `ParseWarnings(usize)`: the unparsed-entry count from task 007. Send it with `Loaded`, and again whenever it changes.
- **Streaming.**
  - The worker reads the file with a buffered reader, line by line, tracking byte offsets for the lazy tool-output ranges, and feeds the task-006/007 assembler.
  - The first `Appended` batch is sent as soon as about 50 turns, or 256 KiB of input, have been assembled. Later batches go every 2 MiB or so.
  - Each batch is followed by a `Progress` update.
  - Batches coalesce the assembler's updates: a turn that is both appended and updated within one batch is sent once, as `Appended`.
  - On I/O error the worker logs the error, sends `Loaded`, and stops.
- **WatchHandle.** Dropping it signals the worker to stop (e.g. an `AtomicBool` or a channel) and does not block the UI thread. A worker whose receiver has hung up stops by itself.
- **Session ids for subagents.** `subagent(parent, call)` resolves as follows:
  1. Read `<parent-uuid>/subagents/agent-*.meta.json` and find the one whose `toolUseId` equals the call id.
  2. If none matches, use the `agentId` from the matching `tool_result` entry's `toolUseResult` (the parent must be parsed, or scanned, for that).

  It returns a `SessionId` that `load()` understands and that addresses that file. Expose the meta's `agentType` and `description`, e.g. `subagent_info(&SessionId)`, for the breadcrumb in task 027. A subagent's first entry is a `user` entry without `origin` (string content). The task-006 rules make it the User turn.
- **resume_command.** Take the template from config `harness.claude.resume_command` (default `claude --resume {id}`). Split it into program + arguments with the `shell-words` crate (add it), and replace `{id}` in every argument with the session id. Set the command's `current_dir` to the session cwd. An empty or unparsable template gives `None`, plus a logged warning.

## Acceptance criteria
- [ ] `load()` returns immediately and delivers, over the channel, `Appended` batches whose turns concatenated equal the full parse of the fixture, followed by `ParseWarnings(n)` and `Loaded` (test).
- [ ] For a synthetic file of at least 20 MB (generated in the test into a temp dir), the first `Appended` arrives before any `Progress` ≥ 0.5, and `Progress` values are non-decreasing and end at 1.0 (test).
- [ ] Dropping the `WatchHandle` stops the worker: the sender is dropped and the receiver sees disconnection (the test waits up to 5 s).
- [ ] A missing file or a read error results in `Loaded` without a panic, and the error is logged (test).
- [ ] `subagent()` finds the subagent file through `meta.json`'s `toolUseId`, and through `toolUseResult.agentId` when no meta matches. It returns `None` for an unknown call id. `load()` on the returned id streams the subagent's turns, with the prompt as the first User turn (fixture tests with a scrubbed subagent directory).
- [ ] `subagent_info` (or the equivalent) returns `agentType` and `description` from the meta file (test).
- [ ] `resume_command()` with the default template yields program `claude`, args `["--resume", "<id>"]` and `current_dir` = the session cwd. A custom template `my-claude --resume={id} --flag` substitutes inside the argument. An empty template yields `None` (tests).
- [ ] Parsing happens off the calling thread: `load()` itself does no file reading beyond opening the file.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 008-claude-session-discovery
