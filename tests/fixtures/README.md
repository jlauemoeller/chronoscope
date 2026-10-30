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
