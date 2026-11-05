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
  - `turn_duration` footers, with large cache-token counts in `usage` (50k
    cache writes and 90k cache reads per line), which the footer counts once
    per message: cache writes go into `tokens`, cache reads into `cached`;
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
    `selected_lines_in_ide`, a `peer` `queued_command`, `directory`, `plan_mode`
    and `plan_mode_exit` at the end of the last turn; `user` `isMeta`, `system` `local_command`,
    `informational` and `away_summary` before the second prompt;
  - a human `queued_command` (`origin` inside the attachment) that starts a
    User turn in the middle of assistant work;
  - noise: `mode`, `permission-mode`, `api-request`, `api-request-blob`,
    `api-request-shape`, `file-history-snapshot`, `file-history-delta`,
    `atis-latch`, `ai-title`, `last-prompt`, `cost-state`, the attachments
    `total_tokens_reminder`, `deferred_tools_record`, `deferred_tools_delta`,
    `skill_listing`, `credential_org`, `batching_reminder_sent`, `environment`,
    `model`, `auto_mode` and `date`, a `system`
    `agents_killed` entry and a `task-notification` user entry;
  - an unknown attachment kind (`brand_new_reminder`, twice);
  - metadata entries: `agent-name`, `queue-operation`, `bridge-session`,
    `tools-used`, `continued-in` and an unknown type without a `message`;
  - unparsed content: a malformed line, an unknown entry type with a
    `message` and an unknown content block (`hologram`);
  - the newest `version` in the fixtures (`2.1.314`, the adapter's
    `NEWEST_KNOWN_VERSION`).

## claude/local_commands.jsonl

- **Origin:** hand-written (generated once with a throw-away script that is
  not part of the repository), following the local-command entries of Claude
  Code 2.1.x transcripts (string content, the `<command-name>` echo with its
  `<command-message>` and `<command-args>` tags on the following lines, and
  the `<local-command-stdout>` output). Nothing was copied.
- **Used by:** `tests/claude_local_commands.rs` and the browser snapshot
  `browser_on_the_local_commands_fixture_120x40_text`.
- **Covers:** the sequence of SPEC cycle 3 §2.5: the `<local-command-caveat>`
  meta entry, a `/effort` echo, its `Cancelled` output, a `/factory` prompt
  (`origin.kind` `human`, starting with `<command-message>`) and an Assistant
  reply with a `turn_duration` footer. One User turn whose injected item holds
  `meta`, `local_command` (`/effort`) and `local_command` (`Cancelled`), and
  one Assistant turn.

## claude/local_commands_mixed.jsonl

- **Origin:** as `local_commands.jsonl`.
- **Used by:** `tests/claude_local_commands.rs`.
- **Covers:**
  - a local command (`/model opus` and its output) in the middle of an
    Assistant turn, which continues after it (one turn with a footer that
    runs from the first prompt);
  - an echo of `/clear` followed by a `system` `local_command` entry for the
    same command (`/clear` with `All clear`): both entries are kept, on
    purpose, in one injected item;
  - a local command (`/effort`, `Cancelled`) between two prompts, which lands
    in the second prompt's User turn;
  - entries that stay prompts: ` /clear` with a leading space and
    `origin.kind` `human`, `<command-name>` text with `origin.kind` `human`,
    and an origin-less `<command-message>` entry (older-format rule);
  - `!` shell entries (`<bash-input>`, `<bash-stdout>` with `<bash-stderr>`)
    without `origin`, which keep their User rows;
  - a local command (`/exit`, `Bye!`) at the end of the input, which lands in
    the last Assistant turn.

## claude/local_commands_only.jsonl

- **Origin:** as `local_commands.jsonl`.
- **Used by:** `tests/claude_local_commands_only.rs`,
  `src/app/browser_tests/local_commands.rs` (the `local_commands_only_*` and
  `the_first_frame_after_a_turn_arrives_120x40_text` snapshots),
  `tests/claude_stats.rs` (its row is `0 turns · 0s`, its run time the span of
  the commands) and `src/app/selector_totals_tests.rs` (the
  `a_local_commands_only_row_*` snapshots).
- **Covers:** SPEC cycle 3 §2.3 and §2.5: a session that holds nothing but
  local commands, so it has no turn: the `<local-command-caveat>` meta entry,
  an `/effort` echo and its `Cancelled` output, a `/clear` echo and a `system`
  `local_command` entry for that `/clear` (with a `commandRun`). The commands
  listed are `/effort` and `/clear`, once each: the output and the caveat are
  not commands, and the `system` entry repeats the second echo.

## claude/compaction.jsonl

- **Origin:** hand-written, modelled on a real compaction (`system`
  `compact_boundary` with `compactMetadata`, followed by a `user` entry with
  `isCompactSummary: true`). Nothing was copied.
- **Used by:** `tests/claude_injected.rs`.
- **Covers:** a boundary and its summary between two exchanges, and a
  `compact_file_reference` attachment and an `informational` system entry that
  follow the summary (and land in the next User turn).

The real compaction samples that `compaction.jsonl` was modelled on came from
Claude Code 2.1.304 to 2.1.311 sessions.

## claude/discovery/

- **Origin:** hand-written (generated once with a throw-away script that is
  not part of the repository). A miniature `~/.claude/projects` tree, so the
  directory layout is part of the fixture. Nothing was copied from a real
  session; ids and texts are made up and paths use `/home/user/...`.
- **Used by:** `tests/claude_discovery.rs` (copies the tree into a temporary
  directory and sets file mtimes there).
- **Covers:**
  - `-home-user-alpha/aaaa0001.jsonl`: three `ai-title` entries (the last
    wins) and a prompt; leading `mode` entry without `cwd` or timestamp;
  - `-home-user-alpha/aaaa0001/subagents/agent-a1.jsonl` and
    `agent-a1.meta.json`: a subagent transcript that must not be listed (its
    timestamps are the newest in the tree). It carries no `usage`, so the
    session's tokens in `tests/index.rs` stay zero until its test of the
    subagent refresh appends a line with `usage` to its copy; the tests of the
    totals and of the usage memo build their own subagent trees in temporary
    directories;
  - `-home-user-my-app/bbbb0001.jsonl` and `-home-user-my.app/bbbb0002.jsonl`:
    two encoded directories (the lossy encoding of `/home/user/my.app`) with
    the same real `cwd`; the first has a prompt but no title;
  - `-home-user-multi/cccc0001.jsonl` and `cccc0002.jsonl`: one encoded
    directory, two different `cwd`s (`/home/user/multi/one` and `.../two`);
  - `-home-user-prompt/dddd0001.jsonl`: no `ai-title`; a `tool_result`-only
    user entry and an `isMeta` entry precede the human prompt, whose first
    line is over 80 characters and which has a second line;
  - `-home-user-prompt/dddd0002.jsonl`: no prompt and no title (only an
    assistant entry and a `tool_result`-only user entry);
  - `-home-user-empty/ffff0001.jsonl`: a 0-byte transcript.

## claude/subagents/

- **Origin:** hand-written (generated once with a throw-away script that is
  not part of the repository), following the subagent layout of real Claude
  Code 2.1.x sessions. Nothing was copied from a real session; ids and texts
  are made up and paths use `/home/user/loading`.
- **Used by:** `tests/claude_loading.rs` (and `tests/claude_loading_log.rs`).
  A miniature `~/.claude/projects` tree with one project, `-home-user-loading`.
- **Covers:**
  - `pppp0001.jsonl`, a parent session with four subagent calls:
    - `toolu_meta01` (`Agent`): linked through `agent-m1.meta.json`'s
      `toolUseId`; the result entry also names `agentId` `m1`;
    - `toolu_res02` (`Task`): `agent-r2.meta.json` carries a different
      `toolUseId`, so it is linked through the result entry's
      `toolUseResult.agentId` (`r2`);
    - `toolu_gone03` (`Agent`): the result names `agentId` `zz9`, which has no
      transcript;
    - `toolu_plain04` (`Agent`): the result has no `toolUseResult` at all;
  - `pppp0001/subagents/agent-m1.jsonl` and `agent-r2.jsonl`: subagent
    transcripts (`isSidechain: true`). The first entry is a `user` entry with
    string content and no `origin`, the second an `assistant` entry;
  - `agent-m1.meta.json` and `agent-r2.meta.json`: `agentType`, `description`
    and `toolUseId`.

## opencode/fixture.sql

- **Origin:** hand-written (generated once with a throw-away script that is
  not part of the repository), following the schema and the `session_message`
  data shapes observed in a real OpenCode `opencode.db`. Only the tables and
  columns the adapter reads are created, plus the NOT NULL ones. Nothing was
  copied from a real database; ids and texts are made up and paths use
  `/home/user/...`. The tests build a temporary database from the script with
  `rusqlite` (rollback-journal mode, so read-only opens create no side
  files); no `.db` file is committed.
- **Used by:** `src/harness/opencode/tests.rs`,
  `src/harness/opencode/stats_tests.rs` (which adds a grandchild session and
  other rows with `TempDb::exec`, not here), `src/app/browser_tests/opencode.rs`,
  `tests/opencode_log.rs`, `tests/harness_registry.rs` and
  `tests/index_opencode.rs` (which adds a grandchild session and writes rows
  to the copy, as OpenCode would; the stats-key tests of
  `src/harness/opencode/tests.rs` do the same with `TempDb::exec`).
- **Covers:**
  - the `migration` table, whose newest row is the one the adapter knows;
  - three projects: `prj_alpha` (named `Alpha App`), `prj_beta` (no `name`, so
    the basename of its worktree names it) and `prj_empty` (no session, not
    listed);
  - `ses_main`, a parent session of 10 messages: a `synthetic` row before the
    first prompt, a `system` row (with `description`) after it, an assistant
    turn of two rows (variant `high`, a `reasoning` part, whitespace-only
    `text` parts, a completed `read` and `shell` tool, a failed `shell` tool
    with content and a failed `write` tool with only `error.message`), an
    `idle` row, a second prompt, and a second assistant turn of two rows
    (variant `default`, two `subagent` tools - one linked through
    `state.metadata.sessionID`, one without metadata - then a row of another
    model with a `pending` tool) followed by a trailing `synthetic` row;
  - `ses_child1` and `ses_child2`, child sessions of `ses_main` (`parent_id`),
    which are not listed as sessions;
  - `ses_untitled` (no title: the first non-empty line of the first prompt),
    `ses_long` (no title, a prompt over 80 characters, `time_idle` NULL),
    `ses_blank` (no title, no messages) and `ses_bad` (two rows whose `data`
    does not parse: invalid JSON, and valid JSON that is not an object).

## opencode/newer_migration.sql

Applied on top of `fixture.sql` by the version tests: a `migration` row newer
than the newest one the adapter knows.
