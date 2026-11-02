# OpenCode harness adapter

## Goal
Add the OpenCode `Harness`, so that `[2] OpenCode` appears in the selector and OpenCode sessions open in the browser with full parity:
- **Store access.** It reads `opencode.db` strictly read-only. Projects come from the `project` table and top-level sessions from `session_v2`, with title fallback, start time, first prompt and activity.
- **Turn assembly.** `session_message` rows become the shared model: user/assistant turns, `synthetic`/`system` rows as injected content, `tool` parts mapped to `ToolKind`, and effort from `model.variant`.
- **Loading.** `load()` streams with paged queries.
- **Other trait methods.** Stats and their cache key; subagent linking through child sessions (`parent_id`); the resume command; and a schema check against the `migration` table.

Live watching is task 029.

## Spec excerpts
From SPEC.md §3.2:

- Location: `~/.local/share/opencode/opencode.db` (SQLite, WAL mode).
- Tables: `project` (`worktree`, `name`), `session_v2` (`project_id`, `parent_id`, `directory`, `title`, `model`, tokens/cost, timestamps), `session_message` (`session_id`, `type`, `seq`, `data` JSON).
- Message types: `user`, `assistant`, `synthetic`, `system`, `idle`. Assistant `data` has `agent`, `model {id, providerID, variant}`, and a `content` array of `text` / `tool` parts.
- Subagent sessions are linked via `session_v2.parent_id`.

From SPEC.md §5.1:

Turn assembly (OpenCode): each `user` row (with `synthetic` rows treated as injected content) starts a turn. `assistant` rows up to the next `user` row form the Assistant turn. Tool calls and their results are already combined in each `tool` part (`state.input`, `state.content`, `state.status`).

- Tool names are mapped to `ToolKind` per harness (e.g. Claude `Bash` and OpenCode `shell` → `Shell`; Claude `Agent`/`Task` and OpenCode `subagent` → `Subagent`).
- Tool description: the tool's own `description` input if present (Claude Bash). Otherwise a derived label (`Read src/main.rs`, `Grep "pattern" in path`, `WebFetch example.com/…`, `<tool name>` for unknown tools).
- `effort` is shown only when the harness records it (Claude `effort`; OpenCode `model.variant` when it isn't `default`).

From SPEC.md §4.1:

- Title: Claude `ai-title` → OpenCode `session_v2.title` → first user prompt (truncated) → `(untitled)`.

- OpenCode: from the `project` table (`name`, else the basename of `worktree`).

From SPEC.md §5.4:

- A key (§6) opens the subagent's own transcript as a full session view: Claude `subagents/agent-<id>.jsonl`, OpenCode child `session_v2` via `parent_id`.

From SPEC.md §5.6:

- OpenCode: `synthetic` and `system` rows.

From SPEC.md §6.7:

Commands are configurable templates (§8.5). Defaults:
- Claude: `claude --resume {id}`
- OpenCode: `opencode --session {id}`. **Verify this flag** before release.

From SPEC.md §7.1:

- The database is opened read-only (`SQLITE_OPEN_READ_ONLY`). Chronoscope never writes to any harness store.

From SPEC.md §7.2:

- A `● live` marker is shown in the browser header while the session is active: file/rows modified within the last 60 s, or OpenCode `time_idle` is null.

From SPEC.md §8.5:

```toml
[harness.opencode]
enabled = true
db_path = "~/.local/share/opencode/opencode.db"
resume_command = "opencode --session {id}"
```

From SPEC.md §9.2:

- Parsing is streaming (line by line for JSONL, paged queries for SQLite) and runs off the UI thread. The UI shows a loading state and becomes interactive as soon as the first screen of content is parsed.

- Session list stats are computed in the background and cached in `$XDG_CACHE_HOME/chronoscope/`, keyed by (path, size, mtime) or by (db, session id, `time_updated`).

From SPEC.md §9.5:

- Never writes to harness data. The SQLite database is opened read-only.

- The schema version is checked on open (OpenCode `migration` table / Claude `version` field). An unknown newer version logs a warning, and Chronoscope still tries best-effort parsing.

From SPEC.md §9.1:

Rules that keep tiers 2–3 cheap:
- No direct OS APIs; only cross-platform crates (crossterm, notify, arboard, rusqlite bundled, etcetera).
- All filesystem paths are `PathBuf`; no hard-coded `/` separators or `~` expansion by string concatenation.
- Harness data locations are resolved from a per-platform candidate list and can be overridden in config. The Claude location is `~/.claude/projects` on all platforms. The OpenCode location on macOS and Windows is **unverified** and must be confirmed before tier-2/3 release.
- Every harness adapter is tested against recorded fixture sessions (trimmed copies of real sessions), so parsing is verified on every OS in CI.

From SPEC.md §11:

- OpenCode data directory on macOS and Windows (§9.1).
- OpenCode resume flag (`opencode --session <id>`) (§6.7).

- Claude compaction format (no compacted session available locally to sample) (`system` `compact_boundary` + summary entry) and OpenCode compaction (`session_pending` type `compaction`) (§5.2).
- Reasoning-effort field in OpenCode (`model.variant`?) (§5.1).

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (observed on this machine with `sqlite3 -readonly`, plus decisions not in the spec; binding for this task):
- **Observed schema** (OpenCode's real `~/.local/share/opencode/opencode.db`):
  - `project(id, worktree, name, time_created, time_updated, …)`
  - `session_v2(id, project_id, parent_id, directory, title, model, cost, tokens_input, tokens_output, …, time_created, time_updated, time_idle, …)`. `model` is JSON `{"id","providerID"}`, and times are Unix milliseconds.
  - `session_message(id, session_id, type, seq, time_created, time_updated, data)`, with a unique index on `(session_id, seq)`.
  - `migration(…)`. Its rows are named like `20261010180456_project_time_archived`.
  - `session_pending(id, session_id, type, data, …)`, where `type = 'compaction'` exists.
- **Observed `data` shapes per message `type`:**

  | `type` | `data` |
  |---|---|
  | `user` | `{time, text, files, agents}` |
  | `assistant` | `{time:{created,streamed,completed}, agent, model:{id,providerID,variant}, content:[parts], finish, cost, tokens:{input,output,reasoning,cache}}` |
  | `synthetic` | `{time, text}` |
  | `system` | `{time, text, description, metadata}` |
  | `idle` | `{time, outcome}` |

  Content parts are `{type:"text", text}` and `{type:"tool", id, name, state:{status, input, content:[{type:"text",text}], metadata}, time}`.
  - A `subagent` tool's `state.metadata.sessionID` is the child session id, and its `state.input` has `agent`, `description` and `prompt`.
  - Tool names seen: `shell`, `read`, `write`, `edit`, `grep`, `glob`, `webfetch`, `subagent`, `execute`, `skill`.

  Inspect the real DB, read-only, for exact field names in `state.input`.
- **Fixtures.** Do not copy the real `.db`. Put a SQL script in the repo (`tests/fixtures/opencode/fixture.sql`) that creates the observed tables (only the columns used, plus required NOT NULL ones) and inserts scrubbed rows. Tests build a temp database from it with `rusqlite`. Cover:
  - two projects, one without `name`;
  - a parent session with a `subagent` tool pointing to a child session;
  - `synthetic` and `system` rows;
  - an `idle` row;
  - a pending tool;
  - a failed tool;
  - a variant other than `default`;
  - a newer-than-known migration row for the version test.
- **Location.** Use config `harness.opencode.db_path` if set. Otherwise the per-platform candidate list, with the first existing path winning:
  - Linux: `$XDG_DATA_HOME/opencode/opencode.db`, defaulting to `~/.local/share/opencode/opencode.db`.
  - macOS: the same XDG-style path. **Unverified**; mark it as such in a code comment.
  - Windows: `%LOCALAPPDATA%\opencode\opencode.db`. **Unverified**; mark it as such in a code comment.

  `available()` means the file exists and opens.
- **Read-only.** Open with `OpenFlags::SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` (plus URI if needed). Never execute any statement other than `SELECT`/`PRAGMA` reads, and wrap queries in a helper that guarantees this. Never modify the database. Rely on the `-wal`/`-shm` files OpenCode already maintains. If a read-only open fails (e.g. they are missing and the directory isn't writable), retry with the `immutable=1` URI parameter and log it.
- **Mapping.**
  - **Projects:** `ProjectId` is `project.id`. The display name is the alias from config (matched on `worktree`), else `name`, else the basename of `worktree`. The cwd is `worktree`.
  - **Sessions:** rows with `parent_id IS NULL`.
    - Title: `session_v2.title`, else the first user prompt (truncated to 80 characters with `…`), else `(untitled)`.
    - `started_at` is `time_created`, and `last_activity` is `time_updated`.
    - `idle` is `Some(time_idle IS NOT NULL)`.
    - The cwd is `directory`.
  - **Turns:**
    - each `user` row starts a User turn;
    - `assistant` rows up to the next `user` row form one Assistant turn: text parts become `Text`, and tool parts become `ToolCall`s with input `state.input`, result text from `state.content` text parts, and status from `state.status` (`completed` → Ok, `error` → Error, otherwise Pending);
    - `synthetic` and `system` rows become `Injected` entries with the same placement rules as Claude (kind `synthetic`/`system`; for `system`, the text plus its `description`);
    - `idle` rows are skipped.
  - **Model and effort:** the model is `model.id`. Effort is `model.variant` when it is not `default`. A model change within a turn adds `ModelChange`.
  - **Stats footer:** duration from the first `time.created` to the last `time.completed` of the turn; messages = the number of assistant rows; tokens = the sum of `tokens.input + tokens.output`; cost = the sum of `cost`.
  - **Tool kinds:**

    | OpenCode name | `ToolKind` |
    |---|---|
    | `shell` | `Shell` |
    | `read` | `Read` |
    | `write` | `Write` |
    | `edit` | `Edit` |
    | `grep` | `Grep` |
    | `glob` | `Glob` |
    | `webfetch` | `WebFetch` |
    | `websearch` | `WebSearch` |
    | `subagent` | `Subagent` |
    | anything else | `Other(name)` |

    Descriptions follow the task-006 rules, using `state.input.description` when present.
  - Item ids are the part `id`, and turn ids the message `id`.
- **Compaction.** §11 lists OpenCode compaction (`session_pending` type `compaction`) as unverified. If real compaction rows exist in `session_message`, map them to `Turn::Compaction`. Otherwise, document in a code comment that it is not yet supported. Do not invent a format.
- **load().**
  - Stream with paged queries: `SELECT … FROM session_message WHERE session_id = ? AND seq > ? ORDER BY seq LIMIT 500`.
  - Send batches with `Progress` (the fraction of the session's row count), `ParseWarnings` (rows whose `data` fails to parse are skipped and counted), `Loaded`, and an initial `Activity { last_modified: time_updated, idle }`.
  - Return a `WatchHandle`.
- **stats / stats_key.** Stats are turns (the user-row count) and duration (`time_updated − time_created`). The key is `opencode:<db path>:<session id>:<time_updated>`.
- **watch_paths** returns the db and `-wal` paths (used by the task-010 index; incremental session watching is task 029).
- **subagent(parent, call)** finds the tool part with that id in the parent and returns `state.metadata.sessionID` if that session exists with `parent_id = parent`. Otherwise it falls back to the child sessions of the parent ordered by `time_created`, matched by `state.input.description == title`. Provide `subagent_info` (agent and description) like task 009.
- **resume_command:** template `harness.opencode.resume_command`, default `opencode --session {id}`, split with `shell-words`, with `current_dir` set to the session directory. §11 says the flag is unverified. If `opencode` is installed on this machine, check `opencode --help` and record the result in the build notes. Do not change the default without a spec change.
- **Version check.** The newest `migration` row name at build time is the known constant (from the fixtures/observed DB). A newer name, compared lexicographically by its timestamp prefix, logs one warning per open, and parsing continues best-effort.
- **Registration.** `build_harnesses` (task 010) constructs it when `harness.opencode.enabled`. Its tab shows as `[2] OpenCode`, muted when unavailable.

## Acceptance criteria
- [ ] The DB is opened with read-only flags, and every query goes through the select-only helper. A test asserts that an `INSERT` via the helper is rejected, and that the fixture DB file's bytes are unchanged after a full load.
- [ ] Projects map per the planner notes, including a project without `name` (basename of `worktree`) and a configured alias. Sessions list only top-level sessions, newest first, with the title fallback chain (fixture tests).
- [ ] Turn assembly:
  - user/assistant grouping;
  - text and tool parts;
  - tool status mapping, including a pending and a failed tool;
  - `synthetic`/`system` rows as injected content;
  - `idle` skipped;
  - effort only for a non-`default` variant;
  - a model change note;
  - stats footer duration, messages, tokens and cost.

  Fixture tests cover each.
- [ ] Tool names map to `ToolKind` per the table (unit tests).
- [ ] `load()` streams in pages of at most 500 rows, sending `Appended` batches, `Progress`, `ParseWarnings` (a malformed `data` row is counted) and `Loaded` (test).
- [ ] `subagent()` resolves the fixture's subagent call to the child session through `state.metadata.sessionID`, and `load()` of the child streams its turns (test).
- [ ] `resume_command()` yields `opencode --session <id>` with `current_dir` = the session directory (test). The `opencode --help` check result is recorded in the build notes.
- [ ] A newer-than-known migration logs one warning and parsing still succeeds (test).
- [ ] `stats_key` changes when `time_updated` changes, and the stats are correct (test).
- [ ] With both harnesses registered, the selector shows `[1] Claude  [2] OpenCode`. `2` switches to OpenCode and lists its projects and sessions. Opening an OpenCode session renders it in the browser (update tests with fixture DB plus snapshots).
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
- **Selector.** The tab row now shows `[1] Claude  [2] OpenCode`, two spaces apart. The selected tab is in `primary` with the `━` underline, and the other tab is `muted_foreground` (task 013 rules). An unavailable OpenCode stays muted and cannot be selected.
- **Browser.** OpenCode sessions render with the same blocks, sections and tool bodies as Claude. The Assistant title shows `<model id> @ <variant>` only when the variant is not `default`.

Snapshots (screen through `App::view`, Claude fixtures + OpenCode fixture DB, `utc = true`):
1. Selector with both tabs, OpenCode selected, its projects and sessions listed, at 120x40 (text + styled).
2. Selector with Claude selected and OpenCode unavailable (muted tab) at 80x24 (text).
3. Browser showing an OpenCode session with a shell tool unfolded and a non-default variant in the title, at 120x40 (text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 010-session-index-worker
- 027-subagent-transcripts
