VERDICT: FAIL

## Summary
Round 3 fixes the round-2 must-fix (stats starvation for a session that is being written to) largely as the rework note asked. It adds a coordinator `seq` stamped on jobs and on cached sends, records `(key, seq)` per session in `sent`, and sends an outdated result only when the session is still listed and nothing newer was sent. It also makes the two small fixes: `HarnessId::cache_prefix` is now the single source of the key prefix, and `stats()` runs under `catch_unwind`. The gate is green, and the busy-session, cached-then-stale (newer key), removed-and-re-added and error cases behave correctly. However, the seq rule misses the case the rework note names as its example: the file reverts to the key whose cached stats were last sent. The cached send is then skipped, so no fresh seq is recorded, and a still-running job for the intermediate key overwrites the correct stats. I reproduced this in a scratchpad copy (finding 1).

## Acceptance criteria
- [x] `build_harnesses` constructs Claude when enabled, constructs nothing when disabled, and keeps an unavailable harness flagged `available: false`. Unchanged since round 2: `src/harness/mod.rs`, tests in `tests/harness_registry.rs`.
- [x] Starting the index over a temp Claude data dir delivers the expected `Snapshot`. Unchanged: `tests/index.rs::the_first_snapshot_lists_the_projects_and_sessions`.
- [x] A new session file delivers a new `Snapshot` (10 s wait), and modifying a file updates its title. Unchanged: `tests/index.rs` watch tests (1 h poll, so only file events can pass them).
- [x] `Stats` events arrive for every session, with turns = User turns and duration = last − first. The computation and tests are unchanged (`stats_arrive_for_every_session`, `stats_count_user_turns_and_span_first_to_last_entry`, `index_stub::every_session_of_every_harness_gets_stats`). The busy-session gap is closed by `Coordinator::record` (`src/index/worker.rs` ~525-570): an outdated result is sent when `live_sessions` contains the session and `done.seq` > the last sent seq. It is covered by `index_stub::a_stale_result_is_sent_so_a_busy_session_is_not_starved`. That test holds v1, bumps to v2 and then v3, and asserts first `turns == 1`, then `turns == 3`, with `calls == 2`. It would fail under the old drop rule, because the first `Stats` would then have `turns == 3`.
- [x] `stats-v1.json` exists after a run; a second index on unchanged files sends the same stats with no parse; a changed file gets fresh stats. Unchanged, still passing. Outdated results are deliberately not cached (as the rework note specifies).
- [x] A corrupt `stats-v1.json` is ignored with a log entry and stats are recomputed. Unchanged: `tests/index_cache_log.rs`.
- [x] `is_active` cases. Unchanged: `model::tests::*`.
- [x] Dropping the `IndexHandle` stops the worker and watchers, and the receiver disconnects. Unchanged: `dropping_the_handle_stops_the_worker_and_disconnects_the_receiver`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass. I re-ran all three and they are green. I also ran `tests/index.rs` and `tests/index_stub.rs` 5 more times with no flakes.

Round-3 rework note, checked:
- 1, starvation: **incomplete** (finding 1). `seq` is stamped in `schedule_stats` and carried through `StatsJob` to `Done`; a deferred job keeps its seq. Cached sends take a fresh seq, but only when the event is actually sent (see finding 1). `record` follows the specified three-way rule. A stale error is logged with no `failed` entry. The `record`/`sent`/`seq` docs state the rule. Both requested tests are present:
  - The replaced test is described above.
  - `a_stale_result_after_newer_stats_were_sent_is_dropped` uses the failing-`projects()` OpenCode stub to keep `prune` skipped. It seeds `claude:s:v2` = 42, then asserts that no `Stats` arrives within 500 ms after release. Without the seq check, v1's result (stale but listed) would be sent with `turns == 1`, so the test would fail.
- 2, prefix. `HarnessId::cache_prefix` (`src/model/mod.rs`) is used by `key_prefix`, `ClaudeHarness::stats_key` and the stub. The `Harness::stats_key` doc names it. Tests: `cache_prefixes_are_distinct_lowercase_names` and `claude_stats_keys_start_with_the_cache_prefix_and_change_with_the_file`.
- 3, panic. `pool_thread` wraps `stats()` in `catch_unwind(AssertUnwindSafe(..))` and maps a panic to an `Err`. `Cargo.toml` does not set `panic = "abort"`, so this is effective. Test: `a_panicking_stats_call_does_not_block_the_session_or_the_pool`. Without the fix, `pending` would stay set and v2 would stay deferred, so the test would time out.

## Findings
1. [must-fix] An older result still overwrites newer stats when a file reverts to the key that was last sent. In `Coordinator::schedule_stats` (`src/index/worker.rs`, ~444-461), a fresh seq is taken only inside `if self.sent.get(&slot).map(|(k, _)| k) != Some(&key)`. Here is the sequence:
   - The session is at K0, whose stats are cached and sent, so `sent = (K0, 1)`.
   - The file changes to K1, and a job for K1 (seq 2) starts.
   - The file reverts to K0. K0 is still cached and equals the sent key, so nothing is sent and `sent` keeps seq 1.
   - The K1 job finishes. Its key is not live, the session is listed, and 2 > 1, so `record` sends K1's stats.
   - The UI now shows K1's stats while the file is at K0. Nothing corrects this: later poll builds are deduped, and a watched harness gets no further events.

   This breaks the rework note's explicit requirement ("cached values … take a fresh seq … so a still-running older job cannot overwrite them later. Example: the file reverts to a key that is already cached") and the round-1 guarantee that an older result never overwrites a newer one. It needs K0 to survive `prune`, which happens whenever another harness has not built successfully yet (e.g. an unreadable OpenCode store once task 028 lands). That is the same setup as `a_stale_result_after_newer_stats_were_sent_is_dropped`.

   I confirmed it in a scratchpad copy with this stub test:
   - Seed `claude:s:v1 = {turns:10}` and use the `fail_projects` OpenCode stub.
   - Expect a cached `Stats` with `turns == 10`.
   - Set `version` to 2, then wait on `entered` (the v2 computation is held).
   - Set `version` to 1 and `wait_for_title(&rx, "s v1")`.
   - Release `gate`. A `Stats` with `turns == 2` arrives.

   Change: pick one of these.
   - In `schedule_stats`, when a session's current key is cached, always record a fresh seq in `sent` (`self.seq += 1; sent.insert(slot, (key, self.seq))`), and send the event only when the key differs from the sent one.
   - Or, in `record`, drop an outdated result when the session's `sent` key is live (`self.is_live(done.harness, &sent_key)`), because the stats shown already match the current state.

   Add the test above to `tests/index_stub.rs`: after the release, assert that no `Stats` arrives for `s` within about 500 ms. Update the `seq`/`record` doc comments if the rule changes.
2. [nit] `src/index/mod.rs`, doc on `IndexEvent::Stats` ("The stats of a session, from the cache or freshly computed."): it doesn't mention that a session being written to can now receive stats for an earlier version of its transcript, with a newer `Stats` following. Task 013 consumes this channel and should know this. Change: add one sentence, e.g. "For a session that changes while its stats are computed, this may describe an earlier version; a newer `Stats` follows."
3. [nit] `tests/index_stub.rs::a_panicking_stats_call_does_not_block_the_session_or_the_pool`: the name claims the pool survives. On a multi-core machine, though, another pool thread could serve the v2 job even if the panicking thread had died, so the test only proves the session's `pending` slot is freed. Change (optional): rename it to `..._does_not_block_the_session`, or make the pool size configurable through `Timing` and run this test with one thread.
