VERDICT: APPROVE

## Assessment
**Code review: PASS.** The reviewer confirmed all three round-4 rework items. They also showed that the new test fails when the round-4 `worker.rs` change is reversed. One finding remains:
- 1 [nit], the name `Coordinator::sent` no longer quite matches what it holds, since it now also records cached sessions whose stats were not sent again: **ignored**. It is naming only, and the doc on the field, which the reviewer calls accurate, says exactly what it holds.

**Feature review: PASS.** Every acceptance criterion is met, and the three round-4 rework items are done:
- 1, revert to the last-sent key: the cached branch of `schedule_stats` now computes `shown` from the old entry, then always takes a fresh `seq` and records it, and sends only when `!shown`. `record` is unchanged.
- 2, tests: `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` fails against the round-3 `worker.rs` (`left: 2, right: 3`) and passes on HEAD. `a_stale_result_after_newer_stats_were_sent_is_dropped` keeps its quiet window and now ends with the positive `turns == 3` check.
- 3, the `IndexEvent::Stats` doc sentence is added.

Both reviewers confirmed independently that the new test fails before the fix and passes after it. That is the "done when" condition of the rework note. One finding remains, in two parts:
- 1a [nit], the doc on `Coordinator::record` still says "the stats last sent for the session" and "something newer was sent already", where it now means the last *recorded* seq: **ignored**. It is a stale-wording nit. The docs on `seq` and `sent`, which define the rule, are correct, and the code matches them.
- 1b [nit], the docs say a fresh `seq` is recorded "at every build", but deduped poll builds return before `schedule_stats`: **ignored**. A snapshot that changes a key is never deduped, so behaviour is the same. Whoever next works on the coordinator (task 028, OpenCode) can fold both doc fixes into that work: replace "sent" with "recorded" in the `record` doc, and write "at every build that sends a snapshot".

**UI review: PASS.** The task is `UI: none`, and no UI, component, theme or `*.snap` files changed. No findings.

My own check: `cargo fmt --check` passes. `INSTA_UPDATE=no cargo test --test index --test index_stub --test index_cache_log` passes (23 tests). The working tree stays clean, and no `.snap.new` files exist. The reviewers also ran the full gate (402 tests, clippy with `-D warnings`) green, with repeated runs and no flakes.

Carry-forward for the task that installs the §9.5 terminal-restoring panic hook: panics on the `index-stats-*` pool threads are caught with `catch_unwind` in `pool_thread`, but the global panic hook still runs for them.

## Conflicts
None.

## Rework budget
Earlier REWORK verdicts: 2 of 2 (round 2 and round 3). Round 1 has no `verdict.md`.
