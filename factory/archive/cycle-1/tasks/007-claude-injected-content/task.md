# Claude injected content, noise, compaction and parse warnings

## Goal
Complete Claude transcript parsing on top of the task-006 assembler.
- Allowlisted injected/system content becomes `Item::Injected` sections placed in the adjacent turn. Consecutive entries are collapsed.
- A `queued_command` typed by the human starts a User turn.
- Compaction becomes a `Compaction` turn.
- Noise is skipped, and unknown attachment kinds are logged.
- Malformed lines and unknown entry types or blocks are skipped and counted for the browser's `N entries could not be parsed` note. Metadata entries are skipped without being counted.
- The transcript's `version` field is checked against the newest known version.

## Spec excerpts
From SPEC.md §5.6:

- System/injected content is shown by default (muted, folded). A toggle key hides or shows all of it; the default is configurable (`show_system = true`).
- It is rendered as **System sections inside the adjacent turn**: in the next turn when it precedes a prompt, in the current turn otherwise. Consecutive injected entries collapse into one section with a count (`3 injected items`). They never become turn blocks of their own, so `[` / `]` only stop at User, Assistant and Compaction blocks.
- **Allowlist** of Claude content that counts as system/injected:
  - attachments `hook_system_message`, `file`, `directory`, `opened_file_in_ide`, `selected_lines_in_ide`, `queued_command` (except human messages, see §5.1), `instructions`, `plan_mode`, `plan_mode_exit`, `compact_file_reference`;
  - `system` entries with subtype `local_command`, `informational` or `away_summary`;
  - `user` entries with `isMeta: true`.
- OpenCode: `synthetic` and `system` rows.
- **Everything else is noise and never shown:**
  - API request records, mode/permission changes, file-history snapshots, title / last-prompt / cost records;
  - all other attachment kinds (`total_tokens_reminder`, `deferred_tools_*`, `skill_listing`, `credential_org`, `batching_reminder_sent`, `environment`, `date`, `model`, `auto_mode`, …).
  - Unknown attachment kinds are treated as noise, but are counted in the log so the allowlist can be extended.

From SPEC.md §5.1:

Item    = Text(markdown)
        | Injected { entries: [{ kind, timestamp, text }] }   // §5.6; consecutive entries collapsed

1. A **User turn** starts at a `user` entry that is a human prompt: `origin.kind == "human"` (or, for older versions without `origin`, string/text content with no `tool_result` and no `isMeta`). A message typed while the agent is working is stored as an `attachment` of kind `queued_command` with `origin.kind == "human"`; it also starts a User turn, and the content after it forms a new Assistant turn.

From SPEC.md §3.1:

- Relevant entry types: `user`, `assistant`, `system` (e.g. `turn_duration`), `attachment`, `ai-title`, `last-prompt`, `cost-state`.
  Noise to skip: `api-request*`, `mode`, `permission-mode`, `atis-latch`, `file-history-*`.

From SPEC.md §5.2:

| Compaction marker | Folded | Divider `── context compacted ──` | Summary text |
| System / injected | Folded | Muted one-liner: kind + first line | Full content, muted |

From SPEC.md §9.5:

- Unknown entry types, unknown content blocks and malformed lines are skipped and counted, not fatal.
  - A muted `N entries could not be parsed` note appears in the browser header.
  - Session metadata entries (known ones such as `agent-name`, `queue-operation`, `continued-in`, `bridge-session`, `tools-used`, and unknown types without a `message`) carry no conversation content: they are skipped and logged, but not counted in the note.
  - Details are written to a log file at `$XDG_STATE_HOME/chronoscope/chronoscope.log`.
- The schema version is checked on open (OpenCode `migration` table / Claude `version` field). An unknown newer version logs a warning, and Chronoscope still tries best-effort parsing.

From SPEC.md §11:

- Claude compaction format (no compacted session available locally to sample) (`system` `compact_boundary` + summary entry) and OpenCode compaction (`session_pending` type `compaction`) (§5.2).

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Placement.** Injected content goes "in the next turn when it precedes a prompt, in the current turn otherwise". When the assembler sees an allowlisted entry, it holds it until the next entry that is neither allowlisted nor noise decides where it goes:
  - a human prompt → the entries become the first item(s) of that new User turn;
  - anything else → they go into the current turn;
  - `finish()`, or a flush at the end of a batch → the current turn. If there is no turn yet, they go into the next one.

  The same rule holds for live appends: entries flushed into the current turn stay there.
- **Collapsing.** Consecutive allowlisted entries (with only noise between them) form one `Item::Injected { entries }`. Each entry carries `kind`, `timestamp` and `text`. `kind` is:
  - the attachment `type` (e.g. `file`, `hook_system_message`);
  - the system `subtype` (e.g. `local_command`);
  - or `meta` for `user` entries with `isMeta: true`.

  `text` is the human-readable content: file path and content for `file`, the message for hooks, the command text for `local_command`, and the text content for meta entries. Inspect real entries to pick the right fields.
- **queued_command.** An attachment of type `queued_command` whose `origin.kind == "human"` (check where `origin` sits in real entries) is a human prompt. It starts a User turn whose text is the queued message, and the content after it forms a new Assistant turn. Other `queued_command`s are injected.
- **Noise vs unparsed.**
  - **Noise (skipped silently, not counted):**
    - entry types `api-request*`, `mode`, `permission-mode`, `atis-latch`, `file-history-*`, `ai-title`, `last-prompt` and `cost-state`;
    - attachment kinds not in the allowlist;
    - `system` subtypes other than `turn_duration`, `compact_boundary`, `local_command`, `informational` and `away_summary`;
    - `user` entries that are not prompts, not tool-result-only and not `isMeta` (e.g. `origin.kind` `task-notification` or `peer`).
  - **Metadata (skipped, logged at debug, not counted):** `agent-name`, `queue-operation`, `continued-in`, `bridge-session`, `tools-used`, and any unknown entry type without a `message` field.
  - **Unparsed (skipped, counted, logged with line number):** lines that are not valid JSON, unknown entry types that have a `message`, and unknown content-block types inside `user`/`assistant` messages (count one per block).
  - The assembler exposes the unparsed count, e.g. `unparsed_count()`. Unknown attachment kinds are counted per kind and logged once per kind per session at `info`, e.g. `unknown attachment kind "foo" (3×)`.
- **Compaction.** Real samples exist on this machine; §11 predates them. In `~/.claude/projects/*/` transcripts, grep for `"subtype":"compact_boundary"`. The shape is a `system` entry with `subtype: "compact_boundary"` (`content: "Conversation compacted"`, `compactMetadata`), directly followed by a `user` entry with `isCompactSummary: true` whose content is the summary.
  - Together they form one `Turn::Compaction { timestamp, summary }`, with the timestamp of the boundary entry.
  - The summary entry is not a human prompt.
  - A boundary without a following summary yields a Compaction turn with an empty summary.
  - Add a scrubbed, trimmed fixture.
- **Version check.** Define a constant for the newest known Claude Code `version` (the newest found in the fixtures, e.g. `2.1.306`). Compare versions numerically per dot-separated part. The first entry with a newer version triggers one `log::warn!` per session, and parsing continues best-effort.
- Fixtures (scrubbed, as in task 006) must cover:
  - each allowlisted attachment kind present in the real data (at least `file`, `opened_file_in_ide`, `hook_system_message`, `instructions`, `queued_command` human and non-human, `compact_file_reference`);
  - `system` `local_command`, `informational` and `away_summary`;
  - an `isMeta` user entry;
  - noise of every listed type;
  - an unknown attachment kind;
  - metadata entries;
  - a malformed line;
  - an unknown block type;
  - a compaction.

## Acceptance criteria
- [ ] Allowlisted entries before a human prompt become an `Injected` item at the start of that User turn. Allowlisted entries in the middle of assistant activity become an `Injected` item at that position in the current Assistant turn (fixture tests).
- [ ] Three consecutive allowlisted entries (with noise between them) collapse into one `Injected` item with three entries, each with the kind, timestamp and text from the planner notes (test).
- [ ] A human `queued_command` starts a User turn with the queued text, and the following assistant content forms a new Assistant turn. A non-human `queued_command` is injected (tests).
- [ ] Every noise category in the planner notes produces no item and does not change the unparsed count (table-driven test).
- [ ] Metadata entries (`agent-name`, `queue-operation`, `continued-in`, `bridge-session`, `tools-used`, unknown type without `message`) are skipped without counting (test).
- [ ] A malformed line, an unknown entry type with a `message`, and an unknown content block each increment the unparsed count by one, and parsing continues (test).
- [ ] An unknown attachment kind is skipped and logged with its count, and is not counted as unparsed (test using a test logger or a logged-kinds accessor).
- [ ] `compact_boundary` + `isCompactSummary` produce one `Turn::Compaction` with the summary text, and the summary does not start a User turn (fixture test).
- [ ] An entry with a `version` newer than the known constant logs exactly one warning per session and still parses (test).
- [ ] All task-006 tests still pass, and incremental assembly in random chunks still gives identical results with injected content present (test).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 003-config-file
- 006-claude-turn-assembly
