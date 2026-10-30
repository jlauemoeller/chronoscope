STATUS: DONE

## Changes
- `src/index/worker.rs`: in `schedule_stats`, a session whose current key is cached now always records a fresh `seq` in `sent`, even when no event is sent (the event is still sent only when the key differs from the one previously in `sent`; the comparison is made before the entry is overwritten). `record` is unchanged: its `done.seq <= last` check now drops the result of a job for an older key after a revert to the already-sent key. Docs on `Coordinator::seq` and `Coordinator::sent` state the rule exactly (a result for a current key is still always sent).
- `src/index/mod.rs`: `IndexEvent::Stats` doc says it may describe an earlier version of a changing session, with a newer `Stats` following.
- `tests/index_stub.rs`: new `a_stale_result_after_a_revert_to_the_sent_key_is_dropped`; the existing `a_stale_result_after_newer_stats_were_sent_is_dropped` keeps its 500 ms quiet window and now sets version 3 afterwards, asserting the next `Stats` has `turns == 3`.

## Self-check
- Rework 1 (revert to the last-sent key): `index_stub::a_stale_result_after_a_revert_to_the_sent_key_is_dropped`. Run before the fix it failed (next Stats had `turns == 2`, expected 3); it passes after.
- Rework 2 (positive check in the existing stale-drop test): `index_stub::a_stale_result_after_newer_stats_were_sent_is_dropped` passes with the added `turns == 3` assertion.
- Rework 3 (`IndexEvent::Stats` doc): added.
- `tests/index.rs` and the rest of `tests/index_stub.rs` pass (5 repeated runs, no flakes).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (402 passed, 0 failed)
