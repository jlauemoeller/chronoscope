VERDICT: PASS

## Summary
Round 4 closes the round-3 must-fix (a file that reverts to the key whose cached stats were last sent): `Coordinator::schedule_stats` now records a fresh `seq` for a session's cached current key at every build, and still sends the event only when that key differs from the one previously recorded. The new test fails against the round-3 `worker.rs` (`left: 2, right: 3`) and passes on HEAD. The full gate is green, and the other acceptance criteria are unchanged and still pass.

## Acceptance criteria
- [x] `build_harnesses` constructs Claude when enabled, constructs nothing when disabled, and keeps an unavailable harness flagged `available: false`. Unchanged: `src/harness/mod.rs`, tests in `tests/harness_registry.rs`.
- [x] Starting the index over a temp Claude data dir delivers the expected `Snapshot`. Unchanged: `tests/index.rs::the_first_snapshot_lists_the_projects_and_sessions`.
- [x] A new session file delivers a new `Snapshot` (10 s wait), and modifying a file updates its title. Unchanged: the `tests/index.rs` watch tests. They poll every hour, so only file events can make them pass.
- [x] `Stats` events arrive for every session, with turns = User turns and duration = last − first. Unchanged computation and tests (`stats_arrive_for_every_session`, `stats_count_user_turns_and_span_first_to_last_entry`, `index_stub::every_session_of_every_harness_gets_stats`, `index_stub::a_stale_result_is_sent_so_a_busy_session_is_not_starved`). The round-4 change only affects sessions whose current key is cached, so the anti-starvation path for uncached keys is not affected.
- [x] `stats-v1.json` exists after a run. A second index on unchanged files sends the same stats with no parse, and a changed file gets fresh stats. Unchanged, still passing.
- [x] A corrupt `stats-v1.json` is ignored with a log entry, and stats are recomputed. Unchanged: `tests/index_cache_log.rs`.
- [x] `is_active` cases. Unchanged: `model::tests::*`.
- [x] Dropping the `IndexHandle` stops the worker and watchers, and the receiver disconnects. Unchanged: `dropping_the_handle_stops_the_worker_and_disconnects_the_receiver`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass. I re-ran all three: 402 tests passed, 0 failed. `tests/index.rs` and `tests/index_stub.rs` passed 5 more runs with no flakes, and the tree stayed clean.

Round-4 rework note, checked:
- 1, revert to the last-sent key: **done.**
  - At `src/index/worker.rs` ~453-469, `shown` is computed from the old `sent` entry before `self.seq += 1; self.sent.insert(slot, (key, self.seq))`. The event is sent only when `!shown`.
  - The comment at that spot and the docs on `Coordinator::sent` and `Coordinator::seq` state the rule, and the `seq` doc says that a result for a current key is always sent.
  - Trace of the scenario: K0 is cached and sent, a K1 job runs with seq n, and the file reverts to K0. The cached branch then records K0 with seq m > n. When K1's `Done` arrives, it is not current, and `n <= m`, so it is dropped. The case of a job for a key started after the cached record still works, because `seq` is monotonic and such a job carries a higher seq.
- 2, tests: **done.**
  - `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` follows the specified steps: seed `claude:s:v1` = 10, consume the cached `Stats`, bump to v2 and wait on `entered`, revert to v1 with `wait_for_title`, release, bump to v3, then assert the next `Stats` has `turns == 3`.
  - Run against the round-3 `worker.rs` in a scratchpad copy, it fails with `left: 2, right: 3`. On HEAD it passes.
  - `a_stale_result_after_newer_stats_were_sent_is_dropped` keeps its 500 ms quiet window and now ends with the v3 bump and a `turns == 3` assertion, so the drop is observed and not just inferred from silence.
- 3, `IndexEvent::Stats` doc: **done** (`src/index/mod.rs`, the sentence about an earlier version followed by a newer `Stats`).

## Findings
1. [nit] The doc comment on `Coordinator::record` (`src/index/worker.rs`, the bullet list above `fn record`) still describes the check as against "the stats last sent for the session" and "something newer was sent already". After this round, `sent` holds the last *recorded* seq, which is refreshed at every build even when nothing is sent, and the `sent` and `seq` docs now say so. Change: in the `record` doc, replace "newer than that of the stats last sent for the session" with "newer than the last one recorded for the session (see `seq`)", and replace "something newer was sent already" with "something newer was recorded already". Also, the new `sent` and `seq` docs (and the comment in `schedule_stats`) say a fresh `seq` is recorded "at every build", but deduped poll builds return before `schedule_stats`. This is harmless, because a snapshot that changes a key is never deduped. Still, "at every build that sends a snapshot" would be exact.
