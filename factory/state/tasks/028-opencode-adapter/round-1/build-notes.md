STATUS: DONE

## Changes
- `src/harness/opencode/` (new adapter):
  - `db.rs`: `Db`, the only door to the database. It opens with `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` and probes with a read; when that fails (e.g. no `-wal`/`-shm` and an unwritable directory) it retries with a `file:<percent-encoded path>?immutable=1` URI and logs it. `Db::select`/`select_one` are the only query entry points: the statement must start with `SELECT` and SQLite must report `stmt.readonly()`, otherwise it is refused before running. No `PRAGMA` is ever issued. `NEWEST_KNOWN_MIGRATION`.
  - `location.rs`: per-platform candidate list (`$XDG_DATA_HOME/opencode/opencode.db`, `%LOCALAPPDATA%\opencode\opencode.db` on Windows, `~/.local/share/opencode/opencode.db`; macOS and Windows marked unverified in comments); first existing wins, else the first.
  - `discovery.rs`: `OpenCodeHarness` and the `Harness` impl. Projects come from `project` (alias by worktree, else `name`, else basename of worktree; projects without top-level sessions are not listed). Sessions are the rows with `parent_id IS NULL`, newest first, with title fallback chain, `started_at = time_created`, `last_activity = time_updated`, `idle = Some(time_idle IS NOT NULL)`. `stats` (user-row count and `time_updated - time_created`), `stats_key` (`opencode:<db>:<id>:<time_updated ms>`), `watch_paths` (db and `-wal`), `resume_command`, `subagent_info`, and the schema check.
  - `assemble.rs`: row-to-turn assembly. `user` starts a User turn; `assistant` rows up to the next `user` form one Assistant turn (`text`, `reasoning` and `tool` parts); `synthetic`/`system` become `Injected` with Claude's placement rules; `idle` is skipped; effort is `model.variant` unless `default`; a model change adds `ModelChange`; the stats footer sums duration/messages/tokens/cost. Paging-independent (nothing is decided at a page boundary). Compaction is not supported (documented in the module comment, no format invented).
  - `load.rs`: paged loader (`WHERE session_id = ? AND seq > ? ORDER BY seq LIMIT <page>`, 500 rows). It sends `Appended`/`Updated` batches, `Progress` after each page, then `Progress(1.0)` if not yet sent, `ParseWarnings`, `Loaded`, `Activity`. Returns a `WatchHandle`; the worker ends after the initial load (live watching is task 029).
  - `subagent.rs`: `subagent()` finds the tool part with that id in the parent, takes `state.metadata.sessionID` if that session has `parent_id = parent`, else falls back to the child whose title equals `state.input.description` (ordered by `time_created`); `subagent_info`.
  - `tools.rs`: tool name to `ToolKind` table; descriptions reuse the shared task-006 rules (`claude::tools::describe`, which now also accepts `filePath`).
  - `testing.rs` (test-only): temp DB built from the fixture SQL.
- `src/harness/resume.rs`: the resume-command builder moved out of `claude/resume.rs` so both adapters share it.
- `src/harness/mod.rs`: `build_harnesses` constructs OpenCode when `harness.opencode.enabled`; `registered_harnesses` and its test cover both.
- `tests/fixtures/opencode/fixture.sql` and `newer_migration.sql`, documented in `tests/fixtures/README.md`.
- Tests: `src/harness/opencode/tests.rs`, `tests/opencode_log.rs` (own binary for the global logger), `tests/harness_registry.rs` (rewritten for two harnesses, hermetic paths), `src/app/startup.rs` (two default tabs), `src/app/browser_tests/opencode.rs` (+ 4 accepted snapshots).

Deviations and decisions to note:
- The real `migration` table has columns `(id, time_completed)`, not `name`; the adapter reads `id`. The known constant is `20261029165540_schedule_checks` (newest migration on this machine), compared by timestamp prefix.
- "One warning per open" is implemented as one warning per harness instance (an `AtomicBool` shared by the index and loader connections), so a load or a stats run does not repeat it.
- The real database has no `session_pending` table and no compaction row among the messages, so `Turn::Compaction` is never produced.
- The real data has `reasoning` content parts and error tools carry `state.error.message` and no `state.content`. Reasoning parts become `Thinking` items; an error tool's result is its content text, else `error.message`. Text parts have no id, so their ids are `<message id>:<part index>`. Whitespace-only text parts (the real data has `"\n\n"`) are dropped.
- Tool results are always stored inline: `LineFormat` only knows Claude JSONL lines, so there is no lazy re-read source for a database row. Known limitation for very large tool outputs.
- `watch_paths` returns the db and `-wal` as the task says. If the `-wal` file does not exist (OpenCode not running), the index worker's `notify` watch fails and it falls back to polling every 2 s.
- `available()` opens the database (file exists and a read succeeds).
- `Harness::subagent_info` is an inherent method of `OpenCodeHarness`, like Claude's.
- A row whose `data` is not a JSON object, or whose fields have the wrong types, is counted as unparsed. Unknown row types are skipped and logged once, not counted.

## `opencode --help` check (SPEC §11, resume flag)
`opencode` is installed on this machine. `opencode --help` lists, verbatim:
`--session, -s string    Session ID to continue, or to create if it does not exist`
So `opencode --session {id}` exists on Linux. The default template is unchanged (`opencode --session {id}`); macOS and Windows remain unchecked.

## Real-database check
Ran the adapter (read-only) against `~/.local/share/opencode/opencode.db` once with a throw-away test (not committed): all sessions of all projects loaded with 0 parse warnings, and the database directory had no new files and unchanged mtimes.

## Self-check
- AC: DB opened read-only, every query through the select-only helper, INSERT rejected, bytes unchanged after a full load — `an_insert_through_the_helper_is_rejected_and_nothing_is_written`, `the_connection_itself_is_read_only`, `a_full_load_leaves_the_database_file_and_its_directory_unchanged`, `a_database_that_cannot_be_opened_read_only_is_read_as_immutable`, `only_selects_pass_the_prefix_check`
- AC: projects (no-name project, alias), top-level sessions newest first, title fallback — `projects_have_a_name_else_the_basename_and_are_sorted_by_activity`, `a_configured_alias_beats_the_name_and_the_basename`, `only_top_level_sessions_are_listed_newest_first`, `the_title_falls_back_to_the_first_prompt_then_untitled`, `session_summaries_carry_times_directory_and_idleness`, `the_config_chooses_the_database`
- AC: turn assembly — `users_and_assistants_alternate_with_injected_content_placed_like_claudes`, `idle_rows_make_no_turn_and_no_item`, `text_reasoning_and_tool_parts_become_items_in_order`, `tool_parts_carry_kind_description_input_result_status_and_duration`, `a_failed_tool_is_an_error_with_its_content_else_its_error_message`, `a_tool_that_has_not_finished_is_pending_without_a_result`, `effort_is_the_variant_unless_it_is_default`, `a_model_change_inside_a_turn_adds_a_note`, `injected_rows_after_the_last_row_go_into_the_current_turn`, `the_stats_footer_sums_duration_messages_tokens_and_cost`, `the_page_size_does_not_change_the_turns`
- AC: tool names to `ToolKind` — `opencode::tools::tests::*` (`every_listed_name_maps_to_its_kind`, `other_names_are_kept_as_they_are`, ...)
- AC: `load()` pages of at most 500, `Appended`, `Progress`, `ParseWarnings`, `Loaded` — `load_streams_batches_progress_warnings_loaded_and_activity`, `a_session_of_1200_rows_streams_in_pages_of_at_most_500`, `a_page_never_has_more_rows_than_asked_for`, `the_default_page_size_is_500`, `rows_whose_data_does_not_parse_are_skipped_and_counted`, `a_session_without_rows_loads_to_nothing`
- AC: `subagent()` through `state.metadata.sessionID`, child streams — `a_subagent_call_resolves_to_its_child_session_through_the_metadata`, `without_metadata_the_child_is_found_by_its_title`, `metadata_naming_a_session_that_is_not_a_child_is_not_trusted`, `an_unknown_call_or_parent_resolves_to_nothing`, `subagent_info_names_the_agent_and_the_description`, and in the app `a_subagent_section_of_an_opencode_session_opens_the_child_transcript`
- AC: `resume_command()` — `the_resume_command_runs_in_the_session_directory`, `an_unusable_resume_template_gives_no_command`; `opencode --help` result above
- AC: newer migration logs one warning, parsing succeeds — `tests/opencode_log.rs::a_newer_migration_logs_one_warning_and_parsing_still_succeeds`, `a_database_with_a_newer_migration_still_parses`, `migrations_are_compared_by_their_timestamp_prefix`
- AC: `stats_key` changes with `time_updated`, stats correct — `the_stats_key_changes_with_time_updated`, `stats_are_the_user_rows_and_the_time_span`
- AC: both harnesses registered, `[1] Claude  [2] OpenCode`, `2` switches, session opens — `both_tabs_show_and_two_switches_to_opencode_with_its_projects_and_sessions`, `the_stats_and_the_active_marker_come_from_the_database`, `an_opencode_session_opens_in_the_browser_and_renders_its_turns`, `a_non_default_variant_is_shown_in_the_title_and_default_is_not`, `tests/harness_registry.rs::*`, `startup::tests::*`, `registered_tests::registered_harnesses_match_the_constructed_ones`
- UI: selector, both tabs, OpenCode selected, 120x40 (text + styled) — `selector_with_opencode_selected_120x40_text`, `selector_with_opencode_selected_120x40_styled`
- UI: selector, Claude selected, OpenCode unavailable (muted), 80x24 text — `an_unavailable_opencode_is_muted_and_cannot_be_selected_80x24_text`
- UI: browser, OpenCode session, shell tool unfolded, non-default variant in the title, 120x40 text — `an_opencode_session_in_the_browser_with_a_shell_tool_unfolded_120x40_text`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1887 passed, 0 failed)
