STATUS: DONE

## Changes
- `src/model/mod.rs`: `SessionSummary.idle: Option<bool>`; `is_active(&SessionSummary, now)` and `ACTIVE_WINDOW` (60 s, inclusive); `SessionStats { turns, duration }` (serde, duration stored as whole milliseconds).
- `src/harness/mod.rs`: `Harness: Send + Sync`; new documented trait methods `refresh()`, `watch_paths()`, `stats()`, `stats_key()`; `build_harnesses(&Config)` plus `build_harnesses_with_home(&Config, &Path)` (the former detects home through `Paths::detect()`; the latter exists for tests). Disabled harness is not constructed; unavailable one is kept.
- `src/harness/claude/`: `ClaudeHarness` implements the new methods. `stats` fully parses with `Assembler::from_file` (turns = User turns, duration = last - first entry timestamp via new `Assembler::first_timestamp()/last_timestamp()`). `stats_key` = `claude:<path>:<size>:<mtime ns>`. `stats_parse_count()` is the parse counter test hook. `idle: None` in summaries.
- `src/index/` (new): `mod.rs` (`SessionIndex::start`, `start_with_timing`, `IndexHandle` with `events()`/`take_events()`/`stop()`/`shutdown()`/Drop, `IndexEvent::{Snapshot, Stats}`, `Timing`), `worker.rs` (coordinator thread + stats pool of 1-3 threads; `notify` watchers that ignore `Access` events; 500 ms coalescing; 2 s polling fallback with snapshot dedupe and watcher retry; newest-first job queue replaced on each snapshot; debounced cache saving; prune of entries whose session is gone), `cache.rs` (`stats-v1.json`, temp file + rename, corrupt file logged and ignored).
- Existing tests updated for the new `idle` field.

Decisions: `IndexHandle` holds the receiver as an `Option` so a Port can take it (`take_events`) while the handle (which stops the worker on drop) stays with the App. Drop signals only; `shutdown()` joins and guarantees the cache is saved. The worker saves the cache, then drops the events sender, so a receiver disconnect implies the final save is done. Cache entries are pruned to the keys of the current snapshots once every harness has reported.

## Self-check
- AC: build_harnesses (enabled / disabled / unavailable kept) — `tests/harness_registry.rs`: `claude_is_constructed_when_enabled`, `a_disabled_harness_is_not_constructed`, `an_unavailable_harness_is_kept_and_flagged`, `the_default_data_dir_is_under_the_given_home`; snapshot `available:false` — `tests/index.rs::a_missing_data_dir_gives_an_unavailable_snapshot_and_is_polled`
- AC: initial Snapshot — `the_first_snapshot_lists_the_projects_and_sessions`
- AC: new session file delivers Snapshot (10 s wait), modified title — `a_new_session_file_delivers_a_new_snapshot`, `a_new_project_directory_delivers_a_new_snapshot`, `modifying_a_file_updates_its_title_in_the_next_snapshot` (also `the_index_does_not_rebuild_in_a_loop_on_its_own_reads`, polling fallback in the missing-data-dir test)
- AC: Stats for every session — `stats_arrive_for_every_session` (hand-derived expectations from the fixtures), `stats_count_user_turns_and_span_first_to_last_entry`, `a_growing_file_gets_fresh_stats_while_the_index_runs`
- AC: cache file, reuse with zero parses, changed file fresh — `stats_are_cached_on_disk_and_reused_by_the_next_index`, `a_changed_file_gets_fresh_stats`, `the_cache_is_saved_while_running_and_pruned_of_gone_sessions`; cache unit tests in `src/index/cache.rs`
- AC: corrupt cache — `tests/index_cache_log.rs::a_corrupt_cache_is_ignored_with_a_log_entry_and_stats_are_recomputed`
- AC: `is_active` — `model::tests::{not_idle_is_active_however_old_the_activity, activity_30_seconds_ago_is_active, activity_90_seconds_ago_without_idle_is_not_active, ...}`
- AC: Drop stops worker, receiver disconnects — `dropping_the_handle_stops_the_worker_and_disconnects_the_receiver`, `the_worker_ends_when_the_receiver_is_dropped`, `an_index_without_harnesses_ends_quietly`
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (393 passed, 0 failed)
