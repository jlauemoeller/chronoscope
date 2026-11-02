STATUS: DONE

## Changes
- `src/harness/opencode/assemble.rs`: `Row` gains `updated` (`time_updated`). New `Assembler::replace_row` re-feeds a row seen before (a streaming message updated in place) without pushing it again. A per-turn builder keeps each assistant row's share of the stats (`Contribution`), so a replaced row counts once; builders of earlier turns are archived (item index rebuilt on demand) so a row of an older turn can be replaced too; `row_turn` maps row id to turn; the unparsed count is now a set of row ids. Unit tests (`replace_tests`) compare in-place replacement with a fresh assembly of the final rows.
- `src/harness/opencode/follow.rs` (new): `Cursor` (last `seq`, max `time_updated`, per-row `(time_updated, hash of data)`) and `Follower::refresh`. One pass: (1) rows with `seq <= last` and `time_updated >= max seen` are re-read and, if changed (time or data hash, so same-millisecond rewrites count), fed to `replace_row`; (2) rows with `seq > last` are paged in through `fetch_page` and `push_row`; (3) `Updated`/`Appended` go out, then `ParseWarnings` if its count changed, then `Activity` (re-read of `session_v2.time_updated`/`time_idle`) after any row change or when the session row changed. Also the poll `Signature` (max seq, max time_updated, count, session time_updated/time_idle). The connection is kept between passes (reopened after an error, and every pass when the DB was opened as immutable, which would never see changes). All through the select-only `Db::select`.
- `src/harness/opencode/watch.rs` (new): `notify` watch on the DB's parent directory, non-recursive, events filtered by file name (`opencode.db`, `opencode.db-wal`), `Access` events ignored (own reads must not wake the worker). Debounce 200 ms of quiet, with a 1 s cap since the first event (a streaming message that never pauses would otherwise never be shown; this deviates from a pure trailing debounce, on purpose). One catch-up pass right after the watcher starts. When `notify` fails (or the seam is on) it polls the signature every 1 s and logs it. The stop flag is checked every 50 ms, so dropping the `WatchHandle` ends the worker and disconnects the channel.
- `src/harness/opencode/load.rs`: `Job.mode: WatchMode`; the initial load feeds the `Cursor` and then hands over to `watch::run`; shared `read_row`/`ROW_COLUMNS`/`activity_update`.
- `src/harness/opencode/discovery.rs`: `OpenCodeHarness::with_polling()` (doc(hidden) test seam, as Claude's).
- `src/harness/opencode/mod.rs`: new modules.
- `tests/opencode_live.rs` (new): 19 integration tests with a separate read-write connection as the writer.

Not changed (index): `refresh()` already re-queries projects and sessions, and `stats_key` includes `time_updated`; `watch_paths()` is untouched. The index test with a rollback-journal fixture has no `-wal` file, so `watch()` in `index/worker.rs` errors and the index falls back to polling (`Timing.poll`): `the_index_delivers_a_new_snapshot_with_a_session_inserted_into_session_v2` exercises that path (300 ms poll). `the_index_follows_a_write_ahead_log_database_through_file_events` switches the DB to WAL (both files exist, poll = 1 h) and so exercises the file events.

Known limits: rows deleted by OpenCode (e.g. a revert) are not noticed (no `Reset`); a new part of an older row (not the latest) is added at the end of its turn; `synthetic`/`system` rows updated in place are not re-read.

## Self-check
- AC: new assistant row gives `Updated` for the current Assistant turn, new User turn gives `Appended` — `a_new_assistant_row_updates_the_current_turn_when_watching` / `_when_polling`, `a_new_user_row_appends_a_user_turn_and_then_an_assistant_turn_extends_it`, `rows_written_while_the_session_loads_are_not_missed`, `a_write_ahead_log_database_is_followed_through_its_wal`.
- AC: updated row data gives `Updated` with stable ids (and stats not double-counted) — `a_row_updated_in_place_updates_its_turn_with_stable_ids`, `a_row_of_an_earlier_turn_updates_that_turn_only`, `an_update_within_the_same_millisecond_is_seen_too`, plus `replace_tests::*` in `assemble.rs`.
- AC: ten quick writes coalesced — `ten_quick_writes_are_coalesced` (1..10 `Updated` batches).
- AC: polling with notify disabled — `polling_detects_a_new_row_and_an_updated_one`, `a_new_assistant_row_updates_the_current_turn_when_polling`, `time_idle_null_is_active_and_a_timestamp_is_idle_when_polling`.
- AC: `time_idle` NULL gives `idle: Some(false)`, a timestamp gives `Some(true)` — `time_idle_null_is_active_and_a_timestamp_is_idle_when_watching` / `_when_polling`, `the_activity_carries_the_sessions_time_updated`.
- AC: index delivers a new `Snapshot` with an inserted session — `the_index_delivers_a_new_snapshot_with_a_session_inserted_into_session_v2` (also title and last-activity refresh), `the_index_follows_a_write_ahead_log_database_through_file_events`.
- AC: only SELECT is run; the reader never modifies the DB — the task-028 helper tests (`an_insert_through_the_helper_is_rejected_and_nothing_is_written`, `the_connection_is_opened_read_only_so_sqlite_refuses_writes`) still pass; `following_a_session_never_modifies_the_database_or_its_directory` (bytes of every file in the directory unchanged, watching and polling).
- AC: dropping the `WatchHandle` stops the worker and disconnects — `dropping_the_handle_stops_watching_and_disconnects`, `dropping_the_handle_stops_a_polling_worker_too`, `a_missing_session_still_ends_in_loaded_and_stops`.
- UI: none (no UI change).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites, 0 failed; lib 1666 passed, tests/opencode_live.rs 19 passed)
