## Rework note
Close the last gap in the stats seq rule, in `src/index/worker.rs`. Also make three small doc and test fixes.

**1. Revert to the last-sent key (required).** Today `Coordinator::schedule_stats` takes a fresh `seq` for a session's cached current key only when it actually sends the event, that is, when the key differs from the key in `sent`. Here is how that goes wrong:
- A session's stats for key K0 are cached and sent, so `sent = (K0, 1)`.
- The file changes to K1, and a job for K1 starts with seq 2.
- The file reverts to K0. K0 is still cached and equals the sent key, so nothing is sent and `sent` keeps seq 1.
- The K1 job finishes. Its key is not live, the session is listed, and 2 > 1, so `record` sends K1's stats while the file is at K0. Deduped poll builds never correct this.

Change the cached branch of `schedule_stats` like this:
- Whenever the session's current key is cached, **always** take a fresh seq and record it: `self.seq += 1; self.sent.insert(slot, (key, self.seq));`. Do this **even when no event is sent**.
- Send the `Stats` event only when the key differs from the key previously in `sent`, as today. Compare before you overwrite the entry.
- Leave `record` unchanged. Its existing `done.seq <= last` check then drops the K1 result.

Update the comment at that spot, and the docs on `Coordinator::seq` and `Coordinator::sent`, to state the rule exactly. Suggested wording for `seq`: "A result for an outdated key is only sent when its `seq` is newer than the last one recorded for its session. A session whose current key is cached records a fresh `seq` at every build, even when its stats are not sent again, so a job for an older key never overwrites them." A result for a current key is still sent unconditionally, and the `seq` doc must not say otherwise.

**2. Tests (required)**, in `tests/index_stub.rs`.
- Add `a_stale_result_after_a_revert_to_the_sent_key_is_dropped`. Use the same setup as `a_stale_result_after_newer_stats_were_sent_is_dropped`: a `blocking_first` Claude stub for `s`, the `fail_projects` OpenCode stub so `prune` stays skipped, and `poll_50ms()`. Then:
  1. Pre-seed `stats-v1.json` with `{"claude:s:v1":{"turns":10,"duration":0}}`. Assert that the first `Stats` has `turns == 10`. Consume it with `next_stats` before calling `wait_for_title`, which panics on any `Stats`.
  2. Set `version` to 2 and wait on `entered`. The held computation is version 2's, because version 1 is cached.
  3. Set `version` to 1 and `wait_for_title(&rx, "s v1")`. No `Stats` may arrive here.
  4. Release `gate`.
  5. Set `version` to 3 and assert that the next `Stats` has `turns == 3`. Version 3's job cannot be queued before version 2's `Done` has cleared `pending`, so receiving `turns == 3` proves that version 2's result was recorded. Because a `turns == 2` event would have come first, it also proves that the result was dropped.

  The current code fails this test, because its next `Stats` has `turns == 2`. Confirm that by running the test before you make the fix.
- In the existing `a_stale_result_after_newer_stats_were_sent_is_dropped`, add the same positive check after the 500 ms quiet window: set `version` to 3 and assert that the next `Stats` has `turns == 3`. Keep the quiet window.

**3. `IndexEvent::Stats` doc (small).** In `src/index/mod.rs`, add one sentence to the doc of `IndexEvent::Stats`, for example: "For a session that changes while its stats are computed, this may describe an earlier version of it; a newer `Stats` follows." Task 013 consumes this channel.

Dropped review items:
- The comment about the §9.5 panic hook at `pool_thread`'s `catch_unwind`. It belongs to the task that installs the hook.
- Renaming `a_panicking_stats_call_does_not_block_the_session_or_the_pool`. Naming only.
- Pushing deferred jobs to the front of the queue, and rewriting `warned_build &= !ok`. Dropped in round 2 already, for the same reasons.

Done when:
- The new test fails before the fix and passes after it.
- The updated stale-drop test passes.
- The rest of `tests/index_stub.rs` and `tests/index.rs` still pass.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` are all green.
