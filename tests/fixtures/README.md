# Test fixtures

Fixture transcripts used by the adapter tests (SPEC §9.6). All fixtures are
synthetic: no real session content, secrets, e-mail addresses, personal names,
hostnames or home-directory paths. Paths use the neutral `/home/user/project`.

## claude/session.jsonl

- **Origin:** hand-written (generated once with a throw-away script that is
  not part of the repository), following the transcript format observed in
  real Claude Code 2.1.x sessions under `~/.claude/projects/`. Nothing was
  copied from a real session, so no scrubbing was necessary; ids, uuids and
  timestamps are made up.
- **Used by:** `tests/claude_assembly.rs`.
- **Covers:**
  - noise entries that must be skipped (`mode`, `permission-mode`, `ai-title`,
    `attachment`, `last-prompt`) and a malformed JSON line;
  - three human prompts: with `origin.kind == "human"` and string content, an
    older-format prompt without `origin`, and an `origin: human` prompt whose
    content is a list of text blocks;
  - a message split over several `assistant` lines sharing `message.id`
    (`msg_A`), with `apiBlockIndex`, a text block repeated with grown text
    and a repeated `tool_use`;
  - readable thinking, redacted (empty, signature-only) thinking, and two
    adjacent redacted blocks that must stay separate (`msg_B`);
  - tool calls with results: a `Bash` call with a `description`, a `Read`
    whose result is over 4 KiB (120 lines, non-ASCII and escaped characters),
    a `Grep` with `is_error: true` and list-form result content;
  - an `api-request-blob` entry carrying a `tool_result` that must not attach;
  - a pending call (`mcp__docs__search`, no result);
  - a model change within a turn (`claude-opus-5-5` to `claude-sonnet-5-5`);
  - `effort` and `perTurnEffort`;
  - `turn_duration` footers, with large cache-token counts in `usage` that
    must not be summed;
  - a `task-notification` user entry that must not start a turn.

## claude/injected.jsonl

- **Origin:** hand-written (generated once with a throw-away script that is
  not part of the repository), following the entry shapes observed in real
  Claude Code 2.1.x sessions. Nothing was copied from a real session; ids,
  uuids, paths (`/home/user/project`) and texts are made up.
- **Used by:** `tests/claude_injected.rs`.
- **Covers:**
  - injected content: attachments `opened_file_in_ide`, `instructions`,
    `file` (three consecutive entries with noise between them, before the
    first prompt), `hook_system_message` and a non-human `queued_command`
    (`task-notification`) in the middle of an Assistant turn,
    `selected_lines_in_ide`, a `peer` `queued_command` and `directory` at the
    end of the last turn; `user` `isMeta`, `system` `local_command`,
    `informational` and `away_summary` before the second prompt;
  - a human `queued_command` (`origin` inside the attachment) that starts a
    User turn in the middle of assistant work;
  - noise: `mode`, `permission-mode`, `api-request`, `api-request-blob`,
    `api-request-shape`, `file-history-snapshot`, `file-history-delta`,
    `atis-latch`, `ai-title`, `last-prompt`, `cost-state`, the attachments
    `total_tokens_reminder`, `deferred_tools_record` and `date`, a `system`
    `agents_killed` entry and a `task-notification` user entry;
  - an unknown attachment kind (`brand_new_reminder`, twice);
  - metadata entries: `agent-name`, `queue-operation`, `bridge-session`,
    `tools-used`, `continued-in` and an unknown type without a `message`;
  - unparsed content: a malformed line, an unknown entry type with a
    `message` and an unknown content block (`hologram`);
  - the newest `version` in the fixtures (`2.1.314`, the adapter's
    `NEWEST_KNOWN_VERSION`).

## claude/compaction.jsonl

- **Origin:** hand-written, modelled on a real compaction (`system`
  `compact_boundary` with `compactMetadata`, followed by a `user` entry with
  `isCompactSummary: true`). Nothing was copied.
- **Used by:** `tests/claude_injected.rs`.
- **Covers:** a boundary and its summary between two exchanges, and a
  `compact_file_reference` attachment and an `informational` system entry that
  follow the summary (and land in the next User turn).
