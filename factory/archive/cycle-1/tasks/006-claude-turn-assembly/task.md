# Normalised model and Claude turn assembly

## Goal
Define the harness-agnostic conversation model (§5.1), and build an incremental Claude JSONL turn assembler. The assembler turns transcript lines into User and Assistant turns:
- human prompts start User turns;
- `assistant` lines that share `message.id` merge into one message;
- `tool_result` blocks pair with their tool calls;
- a model change within a turn becomes an inline note;
- `turn_duration` becomes the stats footer;
- tool names map to `ToolKind`, and each tool call gets a description.

The model is designed for the later live-update and scale requirements:
- turns and items have stable ids, so fold and selection state survives updates;
- large tool outputs are stored as a line count plus a byte range and materialised on demand;
- the assembler emits `ModelUpdate`s that distinguish new turns from in-place updates of existing ones.

Injected/system content, noise classification, compaction and parse-warning counting are task 007.

## Spec excerpts
From SPEC.md §3.1:

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
- **Most `user` entries are not human prompts:** in the BRIO session, 80 of 92 `user` entries only carry `tool_result` blocks. Human prompts have `origin.kind == "human"` and no `tool_result`. Entries with `isMeta: true` are injected, not typed.

From SPEC.md §5.1:

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


- Tool names are mapped to `ToolKind` per harness (e.g. Claude `Bash` and OpenCode `shell` → `Shell`; Claude `Agent`/`Task` and OpenCode `subagent` → `Subagent`).
- Tool description: the tool's own `description` input if present (Claude Bash). Otherwise a derived label (`Read src/main.rs`, `Grep "pattern" in path`, `WebFetch example.com/…`, `<tool name>` for unknown tools).
- `effort` is shown only when the harness records it (Claude `effort`; OpenCode `model.variant` when it isn't `default`).

From SPEC.md §7.1:

| Harness | Change detection | Incremental read |
|---|---|---|
| Claude | `notify` watch on the session `.jsonl` (and its `subagents/` dir) | Read from the last byte offset. New lines go through the same turn assembly (§5.1), so a line extending an existing `message.id` or delivering a `tool_result` updates sections in place. Keep a trailing partial line in a buffer until its newline arrives. If the file shrinks or is replaced (size < offset or inode change), reload it. |

From SPEC.md §7.2:

- Updates to content already displayed (e.g. a tool result arriving for a pending tool call, or a streamed message growing) update the section in place. The current selection and fold states are preserved.

From SPEC.md §9.2:

Target: sessions up to **~100 MB** stay smooth.
- Parsing is streaming (line by line for JSONL, paged queries for SQLite) and runs off the UI thread. The UI shows a loading state and becomes interactive as soon as the first screen of content is parsed.
- Large tool outputs: the model stores line counts and byte ranges up front. The full text is materialised only when the section is unfolded, searched, copied or exported.

- Budgets (to be measured with benchmarks on a synthetic 100 MB fixture):
  - first content within 300 ms;
  - full parse under 2 s;
  - each frame rendered in under 16 ms;
  - memory use within roughly 3× the file size.

From SPEC.md §1.2:

- **`UserEvent`**: what the Ports deliver: index updates, stats results, session `ModelUpdate`s, search results. tui-realm requires `UserEvent: Eq + Clone`, so large payloads are `Arc`-wrapped and compared by a sequence number.

- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (§5.1), so a 100 MB session is held once (§9.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

From SPEC.md §9.1:

Rules that keep tiers 2–3 cheap:
- No direct OS APIs; only cross-platform crates (crossterm, notify, arboard, rusqlite bundled, etcetera).
- All filesystem paths are `PathBuf`; no hard-coded `/` separators or `~` expansion by string concatenation.
- Harness data locations are resolved from a per-platform candidate list and can be overridden in config. The Claude location is `~/.claude/projects` on all platforms. The OpenCode location on macOS and Windows is **unverified** and must be confirmed before tier-2/3 release.
- Every harness adapter is tested against recorded fixture sessions (trimmed copies of real sessions), so parsing is verified on every OS in CI.

Planner notes (decisions not in the spec; binding for this task):
- **Placement.** Harness-agnostic types go in a `model` module. The Claude assembler goes in a Claude harness module (e.g. `harness::claude::assemble`).
- **Model types.** Implement §5.1 with these refinements:
  - Every `Turn` has a stable `TurnId`, and every `Item` a stable `ItemId`. Suggested ids:
    - User turn: the `uuid` of its prompt entry.
    - Assistant turn: the `uuid` of its first entry.
    - Tool call: `tool_use.id`.
    - Text or thinking block: `<message.id>:<block index>`.
  - Add `Item::ModelChange { model }` for rule 5. Later tasks render it as `model → <name>`.
  - `ToolCall.status` is `Pending`, `Ok` or `Error` (from `tool_result.is_error`). `duration` is the result entry's timestamp minus the `tool_use` entry's timestamp.
  - Assistant `stats` is `TurnStats { duration, message_count, tokens: Option<u64>, cost: Option<f64> }`:
    - `duration` and `message_count` come from `turn_duration`'s `durationMs` and `messageCount`.
    - `tokens` is the sum of `usage.input_tokens + usage.output_tokens` over the turn's distinct `message.id`s. Count each message once, even though its usage repeats on every line; cache tokens are excluded.
    - `cost` is `None` for Claude.
  - Timestamps are `chrono::DateTime<Utc>`.
  - Turns are shared as `Arc<Turn>`, so updates can be cloned cheaply into tui-realm user events.
- **Effort.** Assistant entries carry `effort` (and `perTurnEffort`) as a string such as `"high"`. Use `effort`, falling back to `perTurnEffort`.
- **Lazy tool output.** `ToolOutput` exposes `line_count()`, `byte_len()` and `text()`. `text()` returns the full text, materialising it if needed, and returns a `Result`.
  - A result whose text is longer than 4 KiB is stored as its line count, byte length and a source reference (the session file path plus the byte range of the JSONL line it came from). `text()` re-reads that line and extracts the text again.
  - Smaller results are stored inline.
  - The caller passes each line's byte range to the assembler.
  - In-memory sources (`Arc<[u8]>`) are supported for tests.
- **ModelUpdate.**
  - `ModelUpdate::Appended(Vec<Arc<Turn>>)` adds new turns at the end.
  - `ModelUpdate::Updated(Vec<Arc<Turn>>)` replaces existing turns that have the same `TurnId` (e.g. a message grew, or a `tool_result` arrived for an earlier tool call).
  - Later tasks add more variants (progress, reset, warnings, activity) when they first need them.
  - The assembler API is incremental, e.g. `push_line(line, byte_range) -> Vec<ModelUpdate>` plus `finish()` for a final flush. Feeding a file line by line in any chunking yields the same final turns as feeding it at once.
- **Entry handling in this task.**
  - Handle `user` (human prompts and `tool_result`-only entries), `assistant`, and `system` with subtype `turn_duration`.
  - Map `tool_use` and `server_tool_use` blocks to `ToolCall`s. A later `*_tool_result` block with a matching `tool_use_id` (e.g. `advisor_tool_result`) becomes the result.
  - Thinking with empty text (signature only) is `Thinking(Redacted)`.
  - Ignore every other entry type for now, without failing. Task 007 classifies them.
  - `user` entries with an `origin.kind` other than `human` (e.g. `task-notification`, `peer`) are not prompts.
  - `api-request-blob` entries contain a `message` with tool results. They are noise and must not attach results.
- **Tool kinds (Claude names).**

  | Claude name | `ToolKind` |
  |---|---|
  | `Bash` | `Shell` |
  | `Read` | `Read` |
  | `Write` | `Write` |
  | `Edit`, `MultiEdit` | `Edit` |
  | `Grep` | `Grep` |
  | `Glob` | `Glob` |
  | `WebFetch` | `WebFetch` |
  | `WebSearch`, server tool `web_search` | `WebSearch` |
  | `Agent`, `Task` | `Subagent` |
  | anything else, including `mcp__*` | `Other(name)` |
- **Descriptions.**
  - A tool with a `description` input uses it.
  - Otherwise the description is derived:
    - `Read <path>`, `Write <path>` and `Edit <path>`, with the path shown relative to the session cwd when it is inside it;
    - `Grep "<pattern>" in <path>` (without ` in …` when there is no path);
    - `Glob <pattern>` (plus ` in <path>`);
    - `WebFetch <host>/…` (just `<host>` when the URL has no path);
    - `WebSearch "<query>"`;
    - `<tool name>` for anything else.
- **Fixtures.** Put Claude fixtures under `tests/fixtures/claude/`, and record each fixture's provenance in `tests/fixtures/README.md`.
  - Real transcripts on this machine (`~/.claude/projects/*/<uuid>.jsonl`) may be used as the source, trimmed to the lines needed.
  - Scrub them first: no secrets, tokens, e-mail addresses, personal names, hostnames or home-directory paths. Replace with neutral values like `/home/user/project`.
  - Hand-written fixtures that follow the same format are fine too.
  - The fixtures must cover:
    - a multi-line split message (several `assistant` lines sharing `message.id`, including a repeated block);
    - tool calls with results, one with `is_error: true`;
    - a pending tool call;
    - a model change within a turn;
    - a `turn_duration` footer;
    - an older-format prompt without `origin`;
    - readable and redacted thinking;
    - a tool result over 4 KiB.

## Acceptance criteria
- [ ] The `model` module defines `Session`, `Turn` (User / Assistant / Compaction), `Item` (Text, Injected, Thinking(text | Redacted), ToolCall, ModelChange), `ToolCall`, `ToolKind`, `ToolResult`/`ToolOutput`, `TurnStats`, stable `TurnId`/`ItemId`, and `ModelUpdate::{Appended, Updated}`. Compaction and Injected are defined here and produced by task 007.
- [ ] A human prompt (`origin.kind == "human"`, or no `origin` with string/text content, no `tool_result` and no `isMeta`) starts a User turn. Everything up to the next human prompt forms one Assistant turn. A `user` entry with `origin.kind == "task-notification"` does not start a turn. Fixture tests cover both.
- [ ] Several `assistant` lines with the same `message.id` merge into one message with blocks in order. A repeated block (same `tool_use.id`, or the same index for text/thinking) replaces the earlier one instead of duplicating it (fixture test).
- [ ] `tool_result` blocks attach to the `ToolCall` with the matching `tool_use_id`. They set status `Ok`/`Error` and the duration. A call without a result is `Pending`. Results inside `api-request-blob` entries are ignored (tests).
- [ ] The Assistant turn's model is the model of its first message. A later message with a different model inserts an `Item::ModelChange` at that point. `effort` is taken from `effort`/`perTurnEffort` (tests).
- [ ] `turn_duration` sets the preceding Assistant turn's `stats` (duration, message count). `tokens` sums input+output tokens once per distinct `message.id` (test).
- [ ] Tool kinds and descriptions follow the planner-notes tables, with a unit test per row, including the Bash `description` input and an `mcp__` tool.
- [ ] A tool result over 4 KiB is stored lazily: `line_count()` and `byte_len()` are correct without materialising, and `text()` returns exactly the original text. A result under 4 KiB is inline (tests for both file and in-memory sources).
- [ ] Incremental assembly:
  - feeding a fixture in one call, line by line, and in random chunk splits gives identical turns (test);
  - a line that extends an existing `message.id` produces `Updated` for that turn with an unchanged `TurnId`, and a later `tool_result` produces `Updated` for the turn owning the call (tests).
- [ ] Malformed JSON lines do not panic or abort assembly; counting them is task 007 (test).
- [ ] `tests/fixtures/README.md` documents each Claude fixture's origin and the scrubbing applied.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 001-scaffold-crate
