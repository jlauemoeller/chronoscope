# Claude turn message count and wall-clock duration

## Goal
Fix two wrong numbers in the Claude footer. `messageCount` is a running total for the session, so the count becomes the turn's own distinct messages. `durationMs` can restart inside a turn, so the duration becomes wall-clock time from the prompt to the turn's last `turn_duration` entry.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §2.2:

`turn_duration.messageCount` is a running total for the session, not a count for the turn. It never decreased in any of 14 sampled sessions, and a 3-second turn reported 4124.

Requirements:

- The count is the number of distinct message ids in the turn, excluding messages whose model is `<synthetic>`.
- It stays current when messages arrive after a `turn_duration` entry.
- `messageCount` is no longer read.
- The `TurnStats.message_count` documentation no longer says "as reported by the harness".
- OpenCode's count is unchanged.

From SPEC.md §2.3:

One Assistant turn can contain many `turn_duration` entries, and the footer keeps only the last. Deliveries such as `queued_command` attachments with a non-human origin (task notifications, peer messages) can restart `durationMs` without starting a new User turn (base spec §5.1). One such turn shows 16s but ran from 07:18:45 to 07:25:36. Summing does not work either: in another turn the entries add up to more than the elapsed time.

Requirements:

- The duration is the timestamp of the turn's last `turn_duration` entry minus the timestamp of the prompt that started the turn.
- Without a prompt (for example a turn following a compaction), use the turn's first assistant entry.
- Without a usable timestamp, use that entry's `durationMs`.
- Entries are never summed.
- This includes any time spent waiting on background agents, which is the documented wall-clock meaning of `TurnStats.duration`.
- A footer still appears only once a `turn_duration` entry exists, as today.
- OpenCode's duration is unchanged.

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

From spec/SPEC.md §5.1:

Harness adapters translate each native format into one shared model:

```
Session { id, harness, project, cwd, title, started_at, turns: [Turn], children: [SessionRef] }
Turn    = User      { timestamp, text, items: [Item] }     // items: injected content preceding/attached to the prompt
        | Assistant { timestamp, model, effort?, items: [Item], stats? }
        | Compaction{ timestamp, summary }
Item    = Text(markdown)
        | Injected { entries: [{ kind, timestamp, text }] }   // §5.6; consecutive entries collapsed
        | Thinking(text | Redacted)
        | ToolCall { id, tool: ToolKind, description, input, result?: ToolResult, status, duration? }
ToolKind = Shell | Read | Write | Edit | Grep | Glob | WebFetch | WebSearch | Subagent | Other(name)
```

Turn assembly (Claude):
1. A **User turn** starts at a `user` entry that is a human prompt: `origin.kind == "human"` (or, for older versions without `origin`, string/text content with no `tool_result` and no `isMeta`). A message typed while the agent is working is stored as an `attachment` of kind `queued_command` with `origin.kind == "human"`; it also starts a User turn, and the content after it forms a new Assistant turn.
2. Everything up to the next human prompt forms **one Assistant turn**: `assistant` lines, `tool_result`-only `user` lines, and allowlisted injected content (§5.6).
3. `assistant` lines with the same `message.id` are merged into one message, keeping their content blocks in order. Repeated blocks (same `tool_use.id`, or same index for text/thinking) replace the earlier version instead of being duplicated.
4. `tool_result` blocks are attached to the `ToolCall` with the matching `tool_use_id`. A tool call without a result yet has status `pending`.
5. The model shown in the Assistant title is that of the turn's first message. If the model changes within a turn, the new model is shown as a muted inline note.
6. `system` `turn_duration` entries become the stats footer of the preceding Assistant turn.

Turn assembly (OpenCode): each `user` row (with `synthetic` rows treated as injected content) starts a turn. `assistant` rows up to the next `user` row form the Assistant turn. Tool calls and their results are already combined in each `tool` part (`state.input`, `state.content`, `state.status`).

- Tool names are mapped to `ToolKind` per harness (e.g. Claude `Bash` and OpenCode `shell` → `Shell`; Claude `Agent`/`Task` and OpenCode `subagent` → `Subagent`).
- Tool description: the tool's own `description` input if present (Claude Bash). Otherwise a derived label (`Read src/main.rs`, `Grep "pattern" in path`, `WebFetch example.com/…`, `<tool name>` for unknown tools).
- `effort` is shown only when the harness records it (Claude `effort`; OpenCode `model.variant` when it isn't `default`).

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

- All changes are in the Claude adapter (`src/harness/claude/assemble.rs`, `wire.rs`) and the `TurnStats` doc comments. Remove `message_count` from `wire::Entry`.
- "Usable timestamp" means the entry's own `timestamp` parsed successfully. `Assembler::timestamp()` falls back to the last seen timestamp and then to the epoch for display, so the turn's `timestamp` field cannot be used for this.
- The start is the first of these with a usable timestamp: the prompt that started the turn (a `user` prompt, or a human `queued_command` attachment, both through `start_user_turn`), then the turn's first assistant entry. A compaction ends the association with the prompt.
- The end is the turn's latest `turn_duration` entry. Each new entry replaces the duration, and nothing is summed.
- Use that entry's `durationMs` when the start or the end has no usable timestamp, and also when the end is before the start (clock skew), because a negative span is not usable.
- A `turn_duration` entry with no computable duration and no `durationMs` sets no stats, as today.
- Messages without `message.id` keep the existing fallback identity (uuid, then line offset). A message whose model is `<synthetic>` is not counted. A turn containing only such messages has a count of 0, so `format_stats` leaves the part out, as it already does for 0.
- `on_assistant` already updates existing stats for tokens. The count is updated there too, and the turn is marked dirty so that an `Updated` is sent.
- Tests that assert the old values are expected to change, for example `tests/claude_assembly.rs` (`message_count` 9 and 3, durations 30 s and 10 s), along with any snapshot built from a fixture file whose footer changes because of them.

## Acceptance criteria
- [ ] Assembler tests cover a turn with several `turn_duration` entries and a `messageCount` larger than the turn: the duration is wall-clock and the count is per turn.
- [ ] Tests cover each duration fallback and a `<synthetic>` message being left out of the count. The fallbacks are: no prompt (a turn following a compaction, and a transcript starting with an assistant entry) uses the first assistant entry; no usable timestamp uses `durationMs`.
- [ ] A test reproduces the spec's example: a turn from 07:18:45 to 07:25:36 whose last entry has `durationMs` of 16 000 has a duration of 6 m 51 s.
- [ ] A test shows that a `queued_command` attachment with a non-human origin neither starts a new turn nor resets the start of the duration.
- [ ] A test shows that a human `queued_command` prompt is the start of the Assistant turn that follows it.
- [ ] A test shows that a message arriving after a `turn_duration` entry updates the count and produces `ModelUpdate::Updated`, fed both by `push_chunk` in one piece and line by line, with the same final turns.
- [ ] A test shows that an end timestamp before the start falls back to `durationMs`.
- [ ] A footer appears only once a `turn_duration` entry exists: a test shows `stats` is `None` before it.
- [ ] `messageCount` is no longer read: `wire::Entry` has no such field, and a line with `messageCount` of a wrong JSON type still parses.
- [ ] The `TurnStats.message_count` documentation no longer says "as reported by the harness".
- [ ] OpenCode's count and duration are unchanged: the OpenCode tests for them pass without edits.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] `cargo test --release -- --ignored budgets --nocapture` stays within budget.
- [ ] Any snapshot change is limited to footer rows of turns assembled from Claude fixtures.

## UI
UI: none

## Depends on
- 002-turn-token-usage
