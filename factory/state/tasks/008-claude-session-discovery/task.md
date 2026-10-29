# Harness trait and Claude session discovery

## Goal
Introduce the `Harness` trait (discovery part) and the Claude implementation of discovery. It locates the Claude data directory (`~/.claude/projects`, or config `data_dir`) and scans the transcripts in parallel. Each file's scan result is kept in an in-memory cache keyed by (path, size, mtime), so each index build reads every file at most once and rebuilds rescan only new or changed files. From the scans it builds:
- one project per real cwd, with a display name from the basename or a configured alias, sorted by most recent activity;
- session summaries with start time, title, first prompt and last activity, sorted newest first.

The selector and the session index use these later.

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

From SPEC.md §3.1:

- Location: `~/.claude/projects/<encoded-cwd>/<session-uuid>.jsonl` (append-only JSONL).
- Directory name is a lossy encoding of the cwd (`/` and `.` → `-`); the real cwd is in each entry's `cwd` field.
- Subagent transcripts: `<session-uuid>/subagents/agent-<agentId>.jsonl` (entries have `isSidechain: true`) + `agent-<agentId>.meta.json` (`agentType`, `description`, `toolUseId`).

- Session title available via `ai-title` (written repeatedly; use the **last** one).
- **One JSONL line per content block:** an assistant API response is split over several `assistant` lines sharing `message.id` (BRIO session: 195 lines → 73 messages, up to 16 lines per message).
- **Most `user` entries are not human prompts:** in the BRIO session, 80 of 92 `user` entries only carry `tool_result` blocks. Human prompts have `origin.kind == "human"` and no `tool_result`. Entries with `isMeta: true` are injected, not typed.

From SPEC.md §4.1:

- Title: Claude `ai-title` → OpenCode `session_v2.title` → first user prompt (truncated) → `(untitled)`.

Projects:
- Claude: one project per distinct real `cwd`, read from the session entries (not the lossy directory name).
- OpenCode: from the `project` table (`name`, else the basename of `worktree`).
- Display name: basename of the cwd, unless an alias is configured (§8.5). Projects are sorted by most recent activity.
- Sessions are sorted newest first.

From SPEC.md §9.2:

- Session discovery reads each Claude transcript at most once per index build. Each file's scan result (cwd, start time, first prompt, last `ai-title`) is kept in memory, keyed by (path, size, mtime). The initial build and live rebuilds rescan only new or changed files, scanning several files in parallel. Results for files that are gone are dropped.

From SPEC.md §8.5:

```toml
[harness.claude]
enabled = true
data_dir = "~/.claude/projects"            # override auto-detection
resume_command = "claude --resume {id}"

[projects.aliases]
"/home/jacob/Development/projects/brio" = "BRIO"
"/home/jacob/Development/projects/camera-settings" = "CameraSettings"
```

From SPEC.md §9.1:

Rules that keep tiers 2–3 cheap:
- No direct OS APIs; only cross-platform crates (crossterm, notify, arboard, rusqlite bundled, etcetera).
- All filesystem paths are `PathBuf`; no hard-coded `/` separators or `~` expansion by string concatenation.
- Harness data locations are resolved from a per-platform candidate list and can be overridden in config. The Claude location is `~/.claude/projects` on all platforms. The OpenCode location on macOS and Windows is **unverified** and must be confirmed before tier-2/3 release.
- Every harness adapter is tested against recorded fixture sessions (trimmed copies of real sessions), so parsing is verified on every OS in CI.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Trait, stage 1.** In this task `Harness` has `id()`, `available()`, `projects()` and `sessions(&ProjectId)` with the §1.1 signatures. `load`, `subagent` and `resume_command` are added in task 009. Adding further methods later (e.g. paths to watch, stats) is allowed if documented.
  - `HarnessId` has the variants `Claude`, `OpenCode`, `Codex` and `Pi`, with stable numbers 1–4 and display names `Claude`, `OpenCode`, `Codex` and `Pi`. Only Claude has an adapter so far.
  - Use `Result` from `anyhow` or a crate error type, the builder's choice. Add the crate if needed.
- **Data location.** Use config `harness.claude.data_dir` when set. Otherwise use `<home>/.claude/projects` on every platform, built with `PathBuf::join`. `available()` is true when that directory exists. A harness with `enabled = false` is not constructed at all; the registry that does this comes in task 010.
- **What a session is.** Every `*.jsonl` file directly inside a project directory (`<data_dir>/<encoded-cwd>/<uuid>.jsonl`) is a session. Its id is the file stem. Files under `<uuid>/subagents/` are not sessions.
- **Scan result per file.** The scan uses the task-006/007 rules for human prompts. It records:
  - the cwd (the first entry's `cwd`);
  - the start time (the first entry's timestamp);
  - the first human prompt;
  - the last `ai-title`;
  - the last entry timestamp;
  - the file's mtime.

  Scanning a file must read it once, front to back.
- **Session titles.** The last `ai-title`, else the first human prompt's first line truncated to 80 characters with `…`, else `(untitled)`.
- **Edge cases.**
  - A file with no `cwd` anywhere (including a 0-byte file) belongs to a project keyed and named by its encoded directory name. Its start time is the file mtime and its title is `(untitled)`. Keep such files listed, because the browser shows "Session file is empty" for them.
- **Projects.**
  - `ProjectId` is the real cwd.
  - The display name is the alias from `projects.aliases` (matched on the expanded path) or the cwd's basename.
  - A project's last activity is the newest last activity of its sessions. Projects are sorted by it, newest first.
- **SessionSummary** carries at least: `id`, `harness`, `project` (`ProjectId`), `cwd`, `title`, `first_prompt: Option<String>`, `started_at`, `last_activity` (the later of the last entry timestamp and the mtime), and the transcript path. Sessions are sorted by `started_at`, newest first.
- **Scan cache and parallelism.**
  - The harness owns a cache keyed by path, holding (size, mtime, scan result).
  - A build (`refresh()`, or the first `projects()` call) stats every file, reuses cached results whose size and mtime match, and drops results for vanished files.
  - It scans the rest in parallel with `std::thread::scope` across `available_parallelism()` workers.
  - `projects()` and `sessions()` read the cached build and never rescan.
  - Expose a scan counter, or a test hook, so tests can assert how many files were read.
- Use `tempfile` directories with fixture copies for tests.

## Acceptance criteria
- [ ] `Harness` (stage 1), `HarnessId` (numbers 1–4, display names), `Project`, `ProjectId`, `SessionSummary` and `SessionId` exist, and `ClaudeHarness` implements the trait.
- [ ] `available()` is false for a missing data dir and true for an existing one. `data_dir` from config overrides the default (tests).
- [ ] Two sessions in different encoded directories but with the same real `cwd` belong to one project. Two sessions in the same encoded directory with different `cwd`s belong to two projects (fixture test).
- [ ] The project display name is the cwd basename, or the configured alias when `projects.aliases` maps that cwd. Projects are sorted by most recent activity (tests).
- [ ] Session titles follow the rule: the last of several `ai-title` entries wins, then the first prompt truncated to 80 characters with `…`, then `(untitled)`. Sessions are sorted newest first (tests).
- [ ] `subagents/*.jsonl` files are not listed as sessions, and a 0-byte `.jsonl` is listed under its directory-name project as `(untitled)` (tests).
- [ ] The cache:
  - after one build, calling `projects()` and then `sessions()` for every project reads each file exactly once;
  - a second build with no changes reads zero files;
  - appending to one file rescans only that file;
  - deleting a file removes its session.

  Tests use the scan counter.
- [ ] Scanning runs on multiple threads, using `std::thread::scope` with more than one worker when there are several files.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 003-config-file
- 007-claude-injected-content
