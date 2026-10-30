VERDICT: PASS

## Summary
Round 2 fixes the round-1 UI must-fix. An unfolded OpenCode Read now moves OpenCode's `N: ` numbers into the gutter and leaves out the `Read file …` / `Read directory …` line, so it matches Claude's Read body. Round 2 also takes up the round-1 nits: the `immutable=1` fallback is narrower, the immutable URI handles relative paths, the shared title, subagent and describe code now lives in harness-neutral modules, and there is a WAL-mode full-load test. Every acceptance criterion is still met, and I re-ran the gate. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean, `INSTA_UPDATE=no cargo test` reports 0 failures, and the worktree is unchanged afterwards.

## Acceptance criteria
- [x] Read-only flags; every query goes through the select-only helper; `INSERT` rejected; bytes unchanged after a full load.
  - `db.rs::Db::open` opens with `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`. The `immutable=1` retry now runs only for `CannotOpen`/`ReadOnly` (`needs_immutable`). `guarded_select` is still the only code that prepares statements.
  - Tests: `an_insert_through_the_helper_is_rejected_and_nothing_is_written`, `a_full_load_leaves_the_database_file_and_its_directory_unchanged`, `only_open_failures_of_the_right_kind_fall_back_to_immutable`, and `a_relative_path_is_made_absolute`.
  - New test: `a_write_ahead_log_database_is_read_through_its_wal_and_the_main_file_is_unchanged`. It keeps a writer connection open and puts a change only in the `-wal`. It then checks that the change is visible, that the main file's bytes are unchanged and that the set of side files is the same.
- [x] Projects map per the planner notes, including a project without `name` and a configured alias. Only top-level sessions are listed, newest first, with the title fallback. `discovery.rs` is unchanged in behaviour (the title helpers moved to `harness/shared.rs` with the same code), and the round-1 fixture tests still pass.
- [x] Turn assembly: grouping, text and tool parts, pending and failed tools, injected `synthetic`/`system` rows, skipped `idle` rows, effort only for a non-`default` variant, the model change note, and the stats footer. `assemble.rs` was not changed this round, and the same `tests.rs` fixture tests still pass.
- [x] Tool names map to `ToolKind` per the table (`opencode/tools.rs::tool_kind` and its unit tests). `describe` moved to `harness/describe.rs`, and I confirmed its body is the same as the old `claude::tools::describe`.
- [x] `load()` streams pages of at most 500 rows with `Appended`, `Progress`, `ParseWarnings` and `Loaded`. `fetch_page` now reads `data` through the shared `text_at`, with the same handling as before. The 1200-row paging test and the malformed-row warning test still pass.
- [x] `subagent()` resolves the call through `state.metadata.sessionID`, and loading the child streams its turns. `subagent.rs` now reads `data` through `text_at`, so a BLOB row no longer makes the lookup fail. The round-1 tests still pass.
- [x] `resume_command()` gives `opencode --session <id>` with `current_dir` set to the session directory (unchanged; `the_resume_command_runs_in_the_session_directory`). The `opencode --help` result is in the round-1 build notes.
- [x] A newer-than-known migration logs one warning and parsing still succeeds (`tests/opencode_log.rs`; unchanged).
- [x] `stats_key` changes when `time_updated` changes, and the stats are correct (unchanged tests).
- [x] Both harnesses registered: the selector shows `[1] Claude  [2] OpenCode`, `2` switches, and an OpenCode session renders in the browser. The app-level tests and snapshots are unchanged.
  - The new app test `an_unfolded_opencode_read_shows_the_file_in_a_gutter_without_the_framing` unfolds `call-read-1` at 120x60. It checks that the content line is ` 1  fn main() {}`, and that neither `Read file /home` nor `1: fn main` appears on screen.
  - The renderer tests `opencode_numbering_and_header_become_a_plain_gutter`, `opencode_numbering_keeps_the_numbers_of_a_later_range` and `an_opencode_directory_listing_loses_its_header_line` cover the edge cases: an empty `N:` line, offset ranges and directory listings.
  - I checked the real `opencode.db` read-only (`sqlite3 -readonly`). All 2312 `read` parts have at most one text part in `state.content`. In those texts, the only lines that are not `N: text` are:
    - the `Read file …, lines a-b` / `Read directory …, entries a-b` header;
    - directory entries;
    - a trailing `[Output truncated. Continue reading with offset: N]` line, in 368 results;
    - an empty result, for the 14 `error` parts.

    The trailing note follows numbered lines, so `numbered()` gives it an empty gutter, as documented for Claude's `<system-reminder>` trailer. It gets no bogus number and does not shift the numbering (see nit 3).
- [x] Every UI state has a snapshot. The three required snapshots (and the styled variants) are unchanged and pass.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran all three during this review.

## Findings
1. [nit] Carried over from round 1 (feature-review nit 2), and deferred in the build notes. `watch_paths` returns the db and `-wal` paths even when the `-wal` does not exist, which happens when OpenCode is not running. The `notify` watch then fails, and the index worker falls back to polling and rebuilds the OpenCode index every 2 s. This follows the planner's instruction, so it is not a defect of this task. Change (task 029 or a follow-up): return only the paths that exist, or watch the database's parent directory.
2. [nit] `src/components/turn_block/renderers/tool/read.rs` (`numbered` / `is_opencode_header`): the `Read file ` / `Read directory ` header is stripped from the first line of any Read result, whichever harness produced it. This is harmless for Claude, whose results start with a numbered line. Change (optional): document in the doc comment that the check is harness-agnostic on purpose. Alternatively, strip the header in the OpenCode adapter so that the shared renderer stays format-neutral.
3. [nit] In real data, `src/components/turn_block/renderers/tool/read.rs` (`numbered`) draws OpenCode's trailing `[Output truncated. Continue reading with offset: N]` line in the `output` style with an empty gutter, as if it were part of the file. This matches how Claude's trailer lines are drawn, so the two harnesses stay the same, and the line numbers stay correct. The fixture has no truncated read, so no test covers this case. Change (optional): add a fixture or renderer test for a truncated OpenCode read so that the empty-gutter placement is pinned. Optionally, also draw such a trailing note in `muted_foreground`.
