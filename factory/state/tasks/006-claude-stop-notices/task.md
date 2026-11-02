# Claude stop reasons and API errors

## Goal
Produce notices from Claude transcripts: an API error message becomes one `Error` notice in place of its text block, and an unusual `stop_reason` becomes a `Warning`. The notice item and its rendering exist from the previous task.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §4.1:

Add an inline item to the model, next to `Item::ModelChange`:

```
Item::Notice { id, level: Error | Warning, text }
```

| Property | Requirement |
|---|---|
| Section | A new section kind in the turn's order; not foldable; selectable |
| Text | Word-wrapped, indented like other non-foldable lines (base spec §5.2) |
| Style | `Error` uses `error_output`; `Warning` uses the muted style |
| Search, copy, save, export | Included, like the `model → …` note |
| File name word when saved | `error` for `Error`, `notice` for `Warning` |
| Id | `notice:<message or row id>`, upserted so a row updated in place replaces its notice rather than adding another |

Only abnormal endings produce a notice. A normal ending never does.

From SPEC.md §4.2 (this task builds the Claude rows; the OpenCode rows exist):

| Harness | Input | Result |
|---|---|---|
| OpenCode | `finish: "error"`, `error.type` is `aborted` | `Warning`: `aborted`, followed by `: <error.message>` when there is one |
| OpenCode | `finish: "error"`, any other type | `Error`: `<error.type>: <error.message>`, using whichever parts exist |
| OpenCode | `finish` other than `stop`, `tool-calls`, `error`; absent is normal | `Warning`: `stopped: <finish>` |
| Claude | `isApiErrorMessage: true` | `Error` with the message's text, **replacing** its text block so it isn't shown twice |
| Claude | `stop_reason` other than `end_turn`, `tool_use`, `stop_sequence`, `pause_turn` | `Warning`: `stopped: <reason>` |

- Claude's `stop_reason` is null on many lines. Use the last non-null value per message id. Null alone produces no notice.
- Synthetic messages without `isApiErrorMessage`, such as `No response requested.`, stay as ordinary text.
- `max_tokens` and `refusal` occur in no local Claude transcript, so tests use constructed fixtures.

From SPEC.md §3 (how wire fields fail):

- **Parsing `toolUseResult`:** it is usually an object but is sometimes a string, and a known field of the wrong JSON type fails the whole line (`src/harness/claude/wire.rs`). It can also be very large (stdout, whole files), so it must not be materialised as a `Value`. Read it through an untagged object-or-`IgnoredAny` head, like `wire::Content`.

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

From spec/SPEC.md §3.1:

- Location: `~/.claude/projects/<encoded-cwd>/<session-uuid>.jsonl` (append-only JSONL).
- Directory name is a lossy encoding of the cwd (`/` and `.` → `-`); the real cwd is in each entry's `cwd` field.
- Subagent transcripts: `<session-uuid>/subagents/agent-<agentId>.jsonl` (entries have `isSidechain: true`) + `agent-<agentId>.meta.json` (`agentType`, `description`, `toolUseId`).
  - Linkage: the parent's `Agent` `tool_use.id` equals `meta.toolUseId`, and the matching `tool_result` entry's `toolUseResult.agentId` names the file.
- Relevant entry types: `user`, `assistant`, `system` (e.g. `turn_duration`), `attachment`, `ai-title`, `last-prompt`, `cost-state`.
  Noise to skip: `api-request*`, `mode`, `permission-mode`, `atis-latch`, `file-history-*`.
- Assistant entries carry `message.model`, `effort` / `perTurnEffort`, `timestamp`, `usage`.
- Content blocks: `text`, `thinking` (usually redacted — empty text + signature; ~15% of blocks observed carry readable text), `tool_use`, `server_tool_use`, `advisor_tool_result`; tool results arrive as `tool_result` blocks inside `user` entries, paired by `tool_use_id`.
- Session title available via `ai-title` (written repeatedly; use the **last** one).
- **One JSONL line per content block:** an assistant API response is split over several `assistant` lines sharing `message.id` (BRIO session: 195 lines → 73 messages, up to 16 lines per message).

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

From spec/SPEC.md §7.2:

- **Follow when at bottom:** if the viewport is at the end of the transcript, new content scrolls into view automatically. If the user has scrolled up, the position is kept and a `↓ N new` indicator appears in the help line; `G` / `End` jumps to the end and resumes following.
- Updates to content already displayed (e.g. a tool result arriving for a pending tool call, or a streamed message growing) update the section in place. The current selection and fold states are preserved.
- A `● live` marker is shown in the browser header while the session is active: file/rows modified within the last 60 s, or OpenCode `time_idle` is null.

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

- `stop_reason` is `message.stop_reason`; `isApiErrorMessage` is on the entry (top level), and if a transcript carries it on `message` that counts too. Read both so that an unexpected JSON type counts as missing rather than failing the line.
- The id is `notice:<msg_id>`, where `msg_id` is the identity `on_assistant` already computes (message id, then uuid, then line offset).
- Text of an API error: the message's text blocks joined with newlines, or its string content. A message flagged `isApiErrorMessage: true` with no text produces no notice.
- When a message is both an API error and has an unusual `stop_reason`, the `Error` wins and there is one notice.
- **Removal trap.** "Use the last non-null value per message id" means a later line can make a warning go away, for example `max_tokens` on one line and `end_turn` on a later one. `CallRef { turn, item }` in `src/harness/claude/assemble.rs` and `Builder.index` store item *positions*, so removing an item shifts them. Whatever the fix (reindexing both, or storing the last value and deciding differently), a tool result arriving afterwards must still attach to its call.
- A notice sits after the message's content blocks that have arrived so far, through `upsert`.
- Synthetic messages without the flag stay ordinary text, and the model-change logic for `<synthetic>` is unchanged.
- `max_tokens` and `refusal` occur in no local transcript, so tests use constructed lines. Add a line to `tests/fixtures/README.md` if a fixture file gains these cases.

## Acceptance criteria
- [ ] Assembler tests cover each Claude row of §4.2.
- [ ] A Claude API error shows one notice and no duplicate text block.
- [ ] An assembler test shows an API error repeated over several lines of one message gives one notice.
- [ ] Assembler tests cover `end_turn`, `tool_use`, `stop_sequence` and `pause_turn` producing no notice, and `max_tokens` and `refusal` each producing `Warning` with `stopped: <reason>`.
- [ ] An assembler test shows a message with `stop_reason: null` on every line produces no notice.
- [ ] An assembler test shows `stop_reason` null on the first lines of a message and `max_tokens` on a later line produces one notice.
- [ ] An assembler test shows `max_tokens` on one line and `end_turn` on a later line of the same message leaves no notice, and a `tool_result` arriving afterwards attaches to its call.
- [ ] An assembler test shows `No response requested.` from a synthetic message without the flag stays an `Item::Text`.
- [ ] An assembler test shows a `stop_reason` and an `isApiErrorMessage` of unexpected JSON types leave the line parsed and not counted in `unparsed_count`, with no notice.
- [ ] Feeding a transcript with notices in any chunking gives the same final turns.
- [ ] A render test shows an assembled API error drawn as one notice in `error_output`, with no text section.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] `cargo test --release -- --ignored budgets --nocapture` stays within budget.
- [ ] Any snapshot change is limited to turns that contain a notice.

## UI
UI: none

## Depends on
- 005-opencode-error-notices
