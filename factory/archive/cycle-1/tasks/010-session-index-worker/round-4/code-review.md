VERDICT: PASS

## Summary
Round 4 makes the three rework changes as the note asks. In `Coordinator::schedule_stats`, a session whose current key is cached now always takes a fresh `seq` and records it in `sent`. The shown-key comparison runs before the entry is overwritten, and the event is sent only when the key differs. `record` is unchanged, so its `done.seq <= last` check now drops a job's result for an intermediate key after the file reverts to the key already sent. The docs on `seq` and `sent`, and on `IndexEvent::Stats`, state the rule correctly. They keep the point that a result for a current key is always sent. The new test is effective: I applied the round-4 `worker.rs` change in reverse to a scratch copy of HEAD, and `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` failed there. It passes on the branch. The extended stale-drop test now checks the drop positively (a `turns == 3` event after the quiet window). I traced the revert, the revert followed by a return to the running key, and the busy-session (starvation) cases. Each behaves as intended, and the earlier round-3 behaviour is unaffected.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites green. I ran `tests/index.rs`, `tests/index_stub.rs` and `tests/index_cache_log.rs` 6 more times with no flakes. No `.snap.new` files exist, and the working tree is unchanged apart from the reviewers' files.)

## Findings
1. [nit] `src/index/worker.rs:163-167`, the field `Coordinator::sent`. It now also records sessions whose stats were not sent again at that build. The doc explains this, but the name no longer quite matches what the field holds. Change (optional): rename it to `shown` or `recorded`, or leave it as it is, since the doc is accurate.
