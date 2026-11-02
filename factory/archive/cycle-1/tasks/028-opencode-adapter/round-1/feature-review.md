VERDICT: PASS

## Summary
The OpenCode adapter meets every acceptance criterion. It reads the database read-only through a single SELECT-only door (`Db::select` / `guarded_select`, which also covers the open probe), assembles turns as the planner notes describe, streams 500-row pages, resolves subagents through `state.metadata.sessionID`, and is registered as the `[2] OpenCode` tab. The fixture-DB, app-level and snapshot tests behind each criterion would fail if the behaviour were wrong. I re-ran the gate: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean, and `INSTA_UPDATE=no cargo test` passes everything.

## Acceptance criteria
- [x] Read-only flags; every query goes through the select-only helper; `INSERT` rejected; bytes unchanged after a full load.
  - `db.rs`: `Db::open` uses `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`, with an `immutable=1` URI fallback that is logged. `conn` is private, and `guarded_select` is the only code that prepares statements. It checks for a `SELECT` prefix and `stmt.readonly()`.
  - Tests: `an_insert_through_the_helper_is_rejected_and_nothing_is_written` (INSERT, UPDATE, DELETE, DROP, PRAGMA and CTE-INSERT are all refused), `a_full_load_leaves_the_database_file_and_its_directory_unchanged`, `db::tests::the_connection_is_opened_read_only_so_sqlite_refuses_writes`, `the_immutable_fallback_connection_refuses_writes_too`, and `a_database_that_cannot_be_opened_read_only_is_read_as_immutable`.
- [x] Projects map per the planner notes; only top-level sessions are listed, newest first, with the title fallback.
  - `discovery.rs::build_index`, `display_name` and `session_title`.
  - Tests: `projects_have_a_name_else_the_basename_and_are_sorted_by_activity` (beta-tool has no `name`), `a_configured_alias_beats_the_name_and_the_basename`, `only_top_level_sessions_are_listed_newest_first` (children excluded), `the_title_falls_back_to_the_first_prompt_then_untitled` (fixture-level and unit-level), and `session_summaries_carry_times_directory_and_idleness`.
- [x] Turn assembly.
  - Implemented in `assemble.rs`.
  - Grouping is covered by `users_and_assistants_alternate_with_injected_content_placed_like_claudes`.
  - Text and tool parts are covered by `text_reasoning_and_tool_parts_become_items_in_order` and `tool_parts_carry_kind_description_input_result_status_and_duration`.
  - Failed and pending tools are covered by `a_failed_tool_is_an_error_with_its_content_else_its_error_message` and `a_tool_that_has_not_finished_is_pending_without_a_result`.
  - Injected `synthetic`/`system` rows are covered by the grouping test above and by `injected_rows_after_the_last_row_go_into_the_current_turn`.
  - `idle_rows_make_no_turn_and_no_item` covers `idle` rows.
  - `effort_is_the_variant_unless_it_is_default` covers effort.
  - `a_model_change_inside_a_turn_adds_a_note` covers the model change note.
  - `the_stats_footer_sums_duration_messages_tokens_and_cost` covers the stats footer (17 s / 2 / 3300 / 0.03 and 39 s / 2 / 880 / 0.008, both checked against the fixture by hand).
  - `the_page_size_does_not_change_the_turns` shows that paging does not change the turns.
- [x] Tool names map to `ToolKind` per the table. `tools.rs::tool_kind`, tested by `every_listed_name_maps_to_its_kind`, `other_names_are_kept_as_they_are` and `names_are_case_sensitive_and_claude_names_are_not_known`.
- [x] `load()` streams pages of at most 500 rows with `Appended`, `Progress`, `ParseWarnings` and `Loaded`.
  - Implemented in `load.rs::stream` and `fetch_page`: `seq > ? ORDER BY seq LIMIT ?`.
  - Tests: `a_session_of_1200_rows_streams_in_pages_of_at_most_500` (batches of 500/500/200, progress 5/12, 10/12, 1.0), `load_streams_batches_progress_warnings_loaded_and_activity`, `rows_whose_data_does_not_parse_are_skipped_and_counted` (`ParseWarnings(2)`), `a_page_never_has_more_rows_than_asked_for` and `the_default_page_size_is_500`.
- [x] `subagent()` resolves the fixture's subagent call to the child session through the metadata, and loading the child streams its turns.
  - Implemented in `subagent.rs::find`.
  - Tests: `a_subagent_call_resolves_to_its_child_session_through_the_metadata` (which also loads the child), the title fallback and untrusted-metadata tests, and the app-level `a_subagent_section_of_an_opencode_session_opens_the_child_transcript`.
- [x] `resume_command()` gives `opencode --session <id>` with `current_dir` set to the session directory, and the `opencode --help` check is recorded in the build notes.
  - The shared `harness/resume.rs` builds the command, tested by `the_resume_command_runs_in_the_session_directory`.
  - The build notes quote the `--session, -s` line from `opencode --help`.
- [x] A newer-than-known migration logs one warning and parsing still succeeds.
  - `discovery.rs::connect` / `migration_newer` read `migration.id` (the real column; documented deviation). The warning is logged once per harness through a shared `AtomicBool`.
  - Tests: `tests/opencode_log.rs::a_newer_migration_logs_one_warning_and_parsing_still_succeeds` (exactly one WARN line after refresh, load, stats, subagent lookup and a second refresh), `a_database_with_a_newer_migration_still_parses` and `migrations_are_compared_by_their_timestamp_prefix`.
- [x] `stats_key` changes when `time_updated` changes, and the stats are correct. Tested by `the_stats_key_changes_with_time_updated` and `stats_are_the_user_rows_and_the_time_span` (2 turns / 30 min, 0 / 0, 1).
- [x] Both harnesses registered: the selector shows `[1] Claude  [2] OpenCode`, `2` switches to OpenCode, and an OpenCode session renders in the browser.
  - `harness/mod.rs::enabled_harnesses` and `build_harnesses_with_home`.
  - App-level tests: `both_tabs_show_and_two_switches_to_opencode_with_its_projects_and_sessions`, `an_opencode_session_opens_in_the_browser_and_renders_its_turns`, `a_non_default_variant_is_shown_in_the_title_and_default_is_not` and `the_stats_and_the_active_marker_come_from_the_database`.
  - Registry tests: `tests/harness_registry.rs` and `startup::tests`.
- [x] Every UI state has a snapshot.
  - (1) `selector_with_opencode_selected_120x40_text` and `_styled`. In the styled snapshot, row 3 has `[1] Claude` in Rgb(161,161,161) (muted) and `[2] OpenCode` plus its `━` underline in Rgb(229,229,229) (primary).
  - (2) `an_unavailable_opencode_is_muted_and_cannot_be_selected_80x24_text`, plus a styled snapshot that checks the colour of each cell.
  - (3) `an_opencode_session_in_the_browser_with_a_shell_tool_unfolded_120x40_text`, which shows `acme/coder-1 @ high`, the unfolded `List the sources` shell body and the stats footer.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass (re-run during this review).

I also checked tool-body parity against the real database (`sqlite3 -readonly`). The real input keys are `path`, `oldString`/`newString`, `command` and `content`, and the shared renderers already accept them (`edit.rs` reads both spellings, and `mod.rs` path lookup includes `path`/`filePath`). The index-worker polling fallback applies per harness, so OpenCode's polling does not affect Claude's watching.

## Findings
1. [nit] The comment in `src/harness/opencode/tests.rs:583` says "Three rows", but the second Assistant turn has two assistant rows (`msg_main_08`, `msg_main_09`), and the assertion checks `message_count == 2`. Change: reword the comment to "Two rows: 09:05:01 .. 09:05:40 (the last row's completion)."
2. [nit] When OpenCode is not running, its `-wal` file usually does not exist. The `notify` watch over `watch_paths()` (db and `-wal`) then fails, and the index worker rebuilds the OpenCode index every 2 s for as long as OpenCode stays closed. Each rebuild is a full project and session query with a first-prompt subquery per session. The build notes record this, and it follows the planner's `watch_paths` instruction, so it is not a defect of this task. Change (for task 029 or a follow-up): have `watch_paths` return only the paths that exist, or the database's parent directory, so that a missing `-wal` does not force polling.
3. [nit] `a_full_load_leaves_the_database_file_and_its_directory_unchanged` runs against a rollback-journal fixture, while the real `opencode.db` uses WAL mode. So nothing in CI shows that a read-only open of a WAL database with existing `-wal`/`-shm` leaves the side files alone. SQLite-level read-only behaviour is tested separately, and the builder checked the real database by hand. Change (optional): add a variant that switches the temp database to `PRAGMA journal_mode = WAL` (with OpenCode's own writer connection still open, so the side files exist) and asserts that the main file's bytes are unchanged after a full load.
4. [nit] Projects without any top-level session (fixture `prj_empty`) are not listed. This matches the Claude adapter, and the build notes mention it, but the planner notes do not state it. Change: none required. Optionally, add a sentence to the `build_index` doc comment saying this mirrors Claude's behaviour.
