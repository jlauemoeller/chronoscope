# OpenCode section details from the source

## Goal
Fill the details overlay with what OpenCode records, through the interface added for Claude. Turns keep row ids only, and the row is read from the database when the overlay opens. This completes the §5 work, including the cost, provider and agent rows that only OpenCode has.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §5.2:

Most of these fields vary across Claude Code versions and are rarely looked at, so they must not be parsed into the model.

- Turns store **only cheap source references**: a byte range for a Claude line, a row id for OpenCode. They never store the field values.
- A tool call has two references, for its call and its result. On Claude, permission, sandbox and `toolUseResult` data live on the result's line.
- The source is re-read when the overlay opens, following `extract_result_text` and `ByteSource` (`src/harness/claude/wire.rs`, `src/model/output.rs`).
- The harness adapter interface provides the read, so the UI stays harness-agnostic (base spec §1.1). Codex and Pi adapters can provide none.
- Reading `toolUseResult` for this view uses a typed head too (§3), and never loads the whole value.
- The §9.2 budgets must still be met.

From SPEC.md §5.3:

Rows are `label  value`, in the groups below, in this order. Groups that do not apply to the selected section are left out.

| Group | Applies to | Claude | OpenCode |
|---|---|---|---|
| Turn | Every section of an Assistant turn | Model, effort, start, duration, messages, tokens by input / output / thinking / cache read / cache write | The same, plus cost, provider (`model.providerID`) and agent |
| Message | Text, thinking, notice | `message.id`, `requestId`, `timestamp`, `requestedModel`, `stop_reason`, `service_tier`, `speed`, `inference_geo`, 5m and 1h cache writes, web search and fetch counts, `diagnostics.cache_miss_reason` | Row id, `time.created`, `time.streamed`, `time.completed`, `finish`, `rawFinish`, `error` |
| Tool call | Tool call | Start, duration, status, `permissionDecision`, `sandboxed`, `resolvedModel`, `isAsync` | Start, duration, status |
| Prompt | User prompt | `timestamp`, `permissionMode`, `promptSource` | `time.created` |
| Environment | Every section with a source | `gitBranch`, `cwd`, `version`, `entrypoint` | `snapshot.start`, `snapshot.end` |

- Timestamps use the configured format (base spec §8.6).
- Durations use the shared format (§2.4), and token counts use the existing format.
- Missing fields are left out. A group with no rows is left out.
- Claude records no cost, so none is shown. **Do not build a price table.**
- An unreadable source (file gone, row deleted, line unparseable) shows one muted `unavailable` row in the affected group. It never panics or closes the overlay.
- Compaction and system sections show what their source provides under Environment, plus their timestamp.

From SPEC.md §4.2 (the text of an error):

| Harness | Input | Result |
|---|---|---|
| OpenCode | `finish: "error"`, `error.type` is `aborted` | `Warning`: `aborted`, followed by `: <error.message>` when there is one |
| OpenCode | `finish: "error"`, any other type | `Error`: `<error.type>: <error.message>`, using whichever parts exist |
| OpenCode | `finish` other than `stop`, `tool-calls`, `error`; absent is normal | `Warning`: `stopped: <finish>` |

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

From spec/SPEC.md §1.1:

```
            ┌───────────── adapters (one per harness) ─────────────┐
            │ claude: JSONL files     opencode: SQLite (read-only) │
            └──────────────┬──────────────────────┬────────────────┘
                 discover projects/sessions   load + watch session
                           ▼                      ▼
                    Session index         Normalised model (§5.1)
                           └──── worker threads ──┘
                                      │ channels, polled by Ports → Event::User
                                      ▼
  crossterm keys/mouse ──► tui-realm Application ──► focused / subscribed components (§1.3)
          ticks ─────────►   (routes Event)                      │ Msg
                                                                 ▼
                              App::update ◄──────────────────────┘
                                │  changes domain state, mounts/unmounts components,
                                │  moves focus, pushes data into components (attr)
                                ▼
                              App::view: screen layout only; Application::view(&Id, frame, area)
```

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

- The UI is a tui-realm `Application` (§2) driven by a single-threaded loop: the loop calls `Application::tick`, passes every returned `Msg` to `App::update`, and redraws only when something changed (at most every 50 ms, §7.1).
- Parsing, watching, stats computation and search indexing run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used.

From spec/SPEC.md §3.2:

- Location: `~/.local/share/opencode/opencode.db` (SQLite, WAL mode).
- Tables: `project` (`worktree`, `name`), `session_v2` (`project_id`, `parent_id`, `directory`, `title`, `model`, tokens/cost, timestamps), `session_message` (`session_id`, `type`, `seq`, `data` JSON).
- Message types: `user`, `assistant`, `synthetic`, `system`, `idle`. Assistant `data` has `agent`, `model {id, providerID, variant}`, and a `content` array of `text` / `tool` parts.
- Subagent sessions are linked via `session_v2.parent_id`.

From spec/SPEC.md §7.1 (the database is read-only):

- The database is opened read-only (`SQLITE_OPEN_READ_ONLY`). Chronoscope never writes to any harness store.

From spec/SPEC.md §8.6:

- Displayed in local time using `timestamp_format`. With `utc = true`, UTC is shown with a `Z` suffix.
- The selector's session rows use the same format without seconds.

From spec/SPEC.md §9.2:

Target: sessions up to **~100 MB** stay smooth.
- Parsing is streaming (line by line for JSONL, paged queries for SQLite) and runs off the UI thread. The UI shows a loading state and becomes interactive as soon as the first screen of content is parsed.
- Large tool outputs: the model stores line counts and byte ranges up front. The full text is materialised only when the section is unfolded, searched, copied or exported.
- Layout is computed lazily per block for the current width. Only blocks overlapping the viewport are rendered.
- Session list stats are computed in the background and cached in `$XDG_CACHE_HOME/chronoscope/`, keyed by (path, size, mtime) or by (db, session id, `time_updated`).
- Session discovery reads each Claude transcript at most once per index build. Each file's scan result (cwd, start time, first prompt, last `ai-title`) is kept in memory, keyed by (path, size, mtime). The initial build and live rebuilds rescan only new or changed files, scanning several files in parallel. Results for files that are gone are dropped.
- Budgets (to be measured with benchmarks on a synthetic 100 MB fixture):
  - first content within 300 ms;
  - full parse under 2 s;
  - each frame rendered in under 16 ms;
  - memory use within roughly 3× the file size.

From spec/SPEC.md §9.5:

- Never writes to harness data. The SQLite database is opened read-only.
- Unknown entry types, unknown content blocks and malformed lines are skipped and counted, not fatal.
  - A muted `N entries could not be parsed` note appears in the browser header.
  - Session metadata entries (known ones such as `agent-name`, `queue-operation`, `continued-in`, `bridge-session`, `tools-used`, and unknown types without a `message`) carry no conversation content: they are skipped and logged, but not counted in the note.
  - Details are written to a log file at `$XDG_STATE_HOME/chronoscope/chronoscope.log`.
- The schema version is checked on open (OpenCode `migration` table / Claude `version` field). An unknown newer version logs a warning, and Chronoscope still tries best-effort parsing.
- Terminal state is always restored on exit, panic or signal (panic hook + raw-mode guard).

From spec/SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):

- **References.** The model type from task 008 gets an OpenCode row form: `session_message.id`, plus the part's id for a tool call, since the call and its result share one `tool` part. The assembler records them; it gains no detail fields. `AssistantData` and `UserData` stay as they are for this task.
- **Reading.** A new `src/harness/opencode/details.rs` opens the database with the existing read-only `Db` (`src/harness/opencode/db.rs`) and reads one row with `select_one`. It parses `data` only then, with its own tolerant heads.
- **Fields.** `data.time.created`, `time.streamed`, `time.completed`, `finish`, `rawFinish`, `error`, `agent`, `model.providerID`, `snapshot.start`, `snapshot.end`. A tool part's start is `time.created` of the part (the same source as the duration in `tool_call`, `src/harness/opencode/assemble.rs`).
- **Row labels.** The raw field names as the spec writes them, with `row id` for the row's id, `start` for the tool call's start, `provider` and `agent` in the Turn group. `cost` already comes from the model (task 007).
- **Turn group.** `provider` and `agent` come from the turn's first assistant row, so the Assistant turn needs a reference to it. They follow `cost` in the Turn group.
- **`error` row.** Uses the same text as the notice of §4.2 (`<error.type>: <error.message>`, whichever parts exist). Reuse that function rather than formatting it a second time.
- **Times.** Stored in Unix milliseconds. They use the configured `TimestampFormatter`. A value of an unexpected JSON type is left out.
- **`unavailable`.** Database gone, row deleted, or `data` not parseable: one muted `unavailable` row in each affected group, as in task 008. It never panics or closes the overlay.
- **Prompt.** `time.created` from the source replaces the model's `timestamp`, so it is not shown twice.
- OpenCode has no compaction turns (`src/harness/opencode/assemble.rs` explains why). A system or synthetic section references its first row.

## Acceptance criteria
- [ ] Adapter tests cover each OpenCode group of §5.3 (Turn, Message, Tool call, Prompt, Environment), a row missing the fields, and an unreadable source (database gone, row deleted, `data` not parseable).
- [ ] A test shows Turn rows include `cost`, `provider` and `agent` for an OpenCode assistant turn, and that a turn without them leaves them out.
- [ ] A test shows the `error` row matches the notice's text for the same row.
- [ ] A test shows a tool call's Tool call group has `start`, `duration` and `status` in that order, and that a call and its result share one reference.
- [ ] A test shows an unexpected JSON type in any detail field is left out, without `unavailable`.
- [ ] A test shows reading details leaves the database untouched: the connection is the read-only `Db`, and the database file's bytes are unchanged afterwards.
- [ ] A test through `App::update` opens the overlay on the OpenCode fixture (`tests/fixtures/opencode/fixture.sql`, built with the existing test helper) and shows Message and Environment rows.
- [ ] The assembler's parse is unchanged: `AssistantData` and `UserData` gain no detail fields beyond those earlier tasks added for stats and notices.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] `cargo test --release -- --ignored budgets --nocapture` stays within budget.
- [ ] Snapshot changes are limited to details overlay snapshots at App level, if any.

## UI
The overlay's frame, layout and keys are unchanged from task 007, and `unavailable` is drawn as in task 008. This task adds rows, in the order of §5.3.

Snapshot: an OpenCode assistant text section at 80x24 and 120x40 in the `default` theme, through the App.

## Depends on
- 005-opencode-error-notices
- 008-claude-detail-sources
