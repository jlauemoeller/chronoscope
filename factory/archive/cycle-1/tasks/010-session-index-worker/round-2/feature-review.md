VERDICT: FAIL

## Summary
Round 2 fixes the round-1 must-fixes from both reviews. It does this by dropping late results for outdated keys, keeping one stats job per session (later keys are deferred), and keeping other harnesses' waiting jobs on a rebuild. New stub-harness tests in `tests/index_stub.rs` cover each fix. I re-ran the gate (fmt, clippy `-D warnings`, `INSTA_UPDATE=no cargo test`, 397 passed) and it is green. However, the combination of "drop any result whose key is no longer live" and "one job per session" brings in a new bug. A session that keeps being written while its parse runs never gets stats, or never gets fresh ones, until writing pauses. That goes against "Stats events arrive for every session" and §7.3 ("titles and stats refresh"). Every round-1 follow-up item is done as asked.

## Acceptance criteria
- [x] `build_harnesses` constructs Claude when enabled, constructs nothing when `harness.claude.enabled = false`, and keeps an unavailable harness flagged `available: false`. Implemented in `src/harness/mod.rs` `build_harnesses` / `build_harnesses_with_home`. Tests: `tests/harness_registry.rs` (3 tests), and `tests/index.rs::a_missing_data_dir_gives_an_unavailable_snapshot_and_is_polled`.
- [x] Starting the index over a temp Claude data dir delivers a `Snapshot` with the expected projects and sessions. Implemented in `src/index/worker.rs` `build` / `collect`. Test: `tests/index.rs::the_first_snapshot_lists_the_projects_and_sessions`.
- [x] A new session file delivers a new `Snapshot` (10 s wait), and modifying a file updates its title. Implemented in `watch` and the `Msg::Fs` 500 ms coalescing. Tests: `a_new_session_file_delivers_a_new_snapshot`, `a_new_project_directory_delivers_a_new_snapshot` and `modifying_a_file_updates_its_title_in_the_next_snapshot`. These now run with a 1 h poll (`watching_only()`), so only file events can make them pass.
- [ ] `Stats` arrive for every session, with turns = User turns and duration = last − first timestamp. The computation is correct and tested: `ClaudeHarness::stats`, `tests/index.rs::stats_arrive_for_every_session`, `stats_count_user_turns_and_span_first_to_last_entry`, and `index_stub::every_session_of_every_harness_gets_stats` for several harnesses. But a session that is written to while its stats are computed never gets a `Stats` event while it stays active (finding 1).
- [x] `stats-v1.json` exists after a run; a second index on unchanged files sends the same stats with no parse; a changed file gets fresh stats. Implemented in `src/index/cache.rs` (unique temp name, removed on failure) and `schedule_stats`. Tests: `stats_are_cached_on_disk_and_reused_by_the_next_index` (parse count 0, no leftover temp file), `a_changed_file_gets_fresh_stats` and `a_growing_file_gets_fresh_stats_while_the_index_runs`. These cover a single change. Sustained changes are covered by finding 1.
- [x] A corrupt `stats-v1.json` is ignored with a log entry, and stats are recomputed. Implemented in `StatsCache::load`. Test: `tests/index_cache_log.rs`.
- [x] `is_active` is true for `idle == Some(false)` and for 30 s ago, and false for 90 s ago with `idle` `None`. Implemented in `src/model/mod.rs`. Tests: `model::tests::*`.
- [x] Dropping the `IndexHandle` stops the worker and watchers, and the receiver disconnects. Implemented in `IndexHandle::drop`; the docs now require `shutdown()` on exit. Test: `dropping_the_handle_stops_the_worker_and_disconnects_the_receiver`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran all three: they pass, with 397 tests and 0 failures.

Round-1 follow-up, checked:
- Feature 1 and code 2 (a stale result overwrites a newer one): done, but see finding 1.
- Code 1 (other harnesses' jobs): fixed and tested.
- Feature 5: fixed through `pending` + `deferred`.
- Feature 2 and code 3: `HashSet`.
- Feature 3 and code 5: unique temp name.
- Feature 4: once per streak.
- Code 4: prefix-scoped pruning, tested.
- Feature 6, 7, 8 and code 6, 7, 8, 9: done.

## Findings
1. [must-fix] A session that is being written to is starved of stats. In `Coordinator::record` (`src/index/worker.rs`, about lines 490-524), `if !self.is_live(done.harness, &done.key) { return true; }` drops any result whose key is no longer in the harness's *latest* snapshot. Since this round, a session also has only one job at a time, and a newer key waits in `deferred` until the running job finishes. With 500 ms coalescing, Claude rebuilds about every 500 ms while a transcript is being written.
   - Once a parse takes longer than the time to the next rebuild, the cycle repeats: K1 runs. The file grows and K2 is deferred. K1 finishes stale and is dropped. K2 starts, the file grows, and K2 is dropped too.
   - It is deterministic: all it needs is a parse that spans one rebuild. That is the normal case for the large live sessions §9.2 targets (100 MB), during an agentic run.
   - A session with no stats yet shows none for as long as it stays active. One that has stats keeps its old ones and never refreshes, against §7.3 "titles and stats refresh".
   - Stale results are not cached either, so quitting during activity loses that work.
   - Jobs for one session now run one after another, so results arrive in scheduling order. A stale result is therefore never older than one already sent from the pool, which means the drop isn't needed to prevent overwrites.

   Change:
   - Add a per-coordinator, monotonically increasing `seq: u64` to `StatsJob` and `Done`.
   - Record the `seq` of the last stats sent per session, for example by changing `sent` to `HashMap<(usize, SessionId), (String, u64)>`, or by adding a `sent_seq` map.
   - In `record`, when `done.key` is not live but the session is still in the harness's snapshot (track ids, or keep a `live_ids` set next to `live_keys`), still send the `Stats` event if `done.seq` is newer than the session's last sent seq. Don't insert the result into the cache, because prune would drop it anyway.
   - Drop the result only when a newer seq has already been sent, or when the session is gone.
   - Sends of cached values in `schedule_stats` should also record the current seq, so they order correctly.
   - Update `index_stub::a_late_result_for_an_old_key_is_dropped_and_the_new_key_is_computed_after_it` to the new rule. Add a test with the `block_first` stub:
     - Hold the first computation.
     - Bump `version` twice, waiting for each new snapshot.
     - Release the computation.
     - Assert that a `Stats` event arrives for the session promptly (without waiting for the final key's computation), that every later `Stats` for it has `turns` ≥ the previous one, and that the last one has `turns` == the final version.
2. [nit] The prune prefix comes from two places that must agree. `worker.rs` `key_prefix` uses `id.display_name().to_lowercase()`, while each `Harness::stats_key` writes its prefix by hand (`"claude:"` in `discovery.rs`). A future harness (OpenCode, task 028) that writes a different prefix would never have its stale entries pruned, and no test would notice. Change: use one source of truth, for example a `HarnessId::cache_prefix()` used by both `ClaudeHarness::stats_key` and `prune`. Or add a test asserting that `ClaudeHarness::stats_key(..)` starts with the prefix `prune` uses.
3. [nit] If a harness's `stats()` panics, the pool thread dies without sending `Done`. That session's `pending` entry then stays forever, so it (and any newer key in `deferred`) never gets stats until restart, and the pool shrinks by one thread. Change: wrap the `stats` call in `pool_thread` (`src/index/worker.rs`) in `std::panic::catch_unwind(AssertUnwindSafe(..))`, and map a panic to an `Err` result so `record` clears `pending` and marks the key as failed.
