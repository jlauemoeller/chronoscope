# Background session index and stats cache

## Goal
Build the background session index that feeds the selector. It runs on worker threads and reports over a channel:
- **Harness registry.** It constructs the enabled harnesses from config.
- **Index builds.** It produces the project and session lists for every registered harness, and rebuilds when a harness store changes, rescanning only changed files.
- **Stats.** It computes per-session stats (turn count and duration) in the background and caches them on disk in `$XDG_CACHE_HOME/chronoscope/`.
- **Activity data.** It reports the activity data the selector needs for the `●` marker.

The UI wires this channel into a tui-realm Port in task 013.

## Spec excerpts
From SPEC.md §1.1:

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

- Parsing, watching, stats computation and search indexing run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used.

From SPEC.md §4.1:

Session row format: `<local start timestamp>  <title>` on the left, plus muted stats on the right (turn count · duration). The model is not shown: the browser shows it on every Assistant turn.
- Title: Claude `ai-title` → OpenCode `session_v2.title` → first user prompt (truncated) → `(untitled)`.
- Stats are computed in the background and cached (§9.2). Until they are ready, the row shows just timestamp + title.

From SPEC.md §7.2:

- A `● live` marker is shown in the browser header while the session is active: file/rows modified within the last 60 s, or OpenCode `time_idle` is null.

From SPEC.md §7.3:

- Project and session lists update live: new sessions appear, and titles and stats refresh. The current selection is kept (matched by id, not by index).
- Active sessions show the `●` marker in their row.

From SPEC.md §9.2:

- Session list stats are computed in the background and cached in `$XDG_CACHE_HOME/chronoscope/`, keyed by (path, size, mtime) or by (db, session id, `time_updated`).
- Session discovery reads each Claude transcript at most once per index build. Each file's scan result (cwd, start time, first prompt, last `ai-title`) is kept in memory, keyed by (path, size, mtime). The initial build and live rebuilds rescan only new or changed files, scanning several files in parallel. Results for files that are gone are dropped.

From SPEC.md §6.6:

- Codex and Pi tabs are hidden until their adapters exist; their numbers stay reserved.
- Tabs for harnesses with no data on this machine are shown muted and can't be selected.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Registry.** `build_harnesses(&Config) -> Vec<Arc<dyn Harness>>` constructs every enabled harness. Only Claude exists so far; OpenCode is added in task 028. A disabled harness (`enabled = false`) is not constructed, so its tab is hidden; its number stays reserved. A constructed harness whose `available()` is false is kept, and the selector shows it muted.
- **Index worker.** `SessionIndex::start(harnesses, cache_dir) -> IndexHandle`. `IndexHandle` exposes a `Receiver<IndexEvent>` and stops the worker when dropped.
  - `IndexEvent::Snapshot { harness, available, projects, sessions }` is sent once per harness after each build. Payloads are `Arc`-wrapped.
  - `IndexEvent::Stats { harness, session, stats }` is sent as stats become available.
- **Change detection.** Add a documented trait method, e.g. `watch_paths() -> Vec<PathBuf>` (for Claude, the data dir, recursive).
  - The worker watches those paths with `notify` and coalesces events for 500 ms before rebuilding that harness.
  - If watching fails, it polls by rebuilding every 2 s. Builds are cheap thanks to the task-008 scan cache.
- **Stats.** `SessionStats { turns: usize, duration: Duration }`.
  - `turns` is the number of User turns.
  - `duration` is the last entry timestamp minus the first entry timestamp.

  Add documented trait methods to compute them, e.g. `stats(&SessionSummary) -> Result<SessionStats>` (full parse with the task-006/007 assembler), and `stats_key(&SessionSummary) -> String` (Claude: path + size + mtime; OpenCode later: db + session id + `time_updated`).
  - Compute stats on a small pool of worker threads, newest sessions first, and only for sessions whose key has no cached value.
- **Disk cache.** One JSON file, `<cache dir>/stats-v1.json`, mapping `stats_key` → stats.
  - It is loaded at start; cached values are sent as `Stats` events right after the first `Snapshot`.
  - It is saved (debounced, at most every 5 s and on shutdown) by writing a temp file and renaming it over the old one.
  - A corrupt cache file is logged and ignored.
- **Activity.** `SessionSummary` already carries `last_activity`. Add `idle: Option<bool>` for harnesses that record it (OpenCode `time_idle`, later; `None` for Claude). The UI decides "active" against its clock: `idle == Some(false)`, or `last_activity` within 60 s. Provide that rule as a pure function `is_active(&SessionSummary, now) -> bool`, so selector and browser share it.
- Tests use temp dirs and a real `ClaudeHarness` over fixture copies. Use timeouts of a few seconds when waiting for events.

## Acceptance criteria
- [ ] `build_harnesses` constructs Claude when enabled, constructs nothing when `harness.claude.enabled = false`, and keeps an unavailable harness flagged `available: false` (tests).
- [ ] Starting the index over a temp Claude data dir delivers a `Snapshot` with the expected projects and sessions (test).
- [ ] Creating a new session file in the watched dir delivers a new `Snapshot` containing it (the test waits up to 10 s, to tolerate slow file-event delivery on macOS CI), and modifying a file updates its title in the next `Snapshot` (tests).
- [ ] `Stats` events arrive for every session, with turns = the number of User turns and duration = last − first timestamp (fixture test).
- [ ] After a run, `stats-v1.json` exists. A second index started on unchanged files sends the same stats without parsing any session (asserted through a parse counter or test hook). A changed file gets fresh stats (tests).
- [ ] A corrupt `stats-v1.json` is ignored with a log entry, and stats are recomputed (test).
- [ ] `is_active` returns true for `idle == Some(false)`, true for `last_activity` 30 s before `now`, and false for 90 s before `now` with `idle` `None` (tests).
- [ ] Dropping the `IndexHandle` stops the worker and watchers, and the receiver disconnects (test).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 009-claude-session-loading
