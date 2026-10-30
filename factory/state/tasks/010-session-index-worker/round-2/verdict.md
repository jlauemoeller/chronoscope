VERDICT: REWORK

## Assessment
**Code review: PASS.** The round-1 must-fix (a rebuild of one harness threw away other harnesses' waiting stats jobs) is fixed and covered by `index_stub::every_session_of_every_harness_gets_stats`. Four nits remain:
- 1, key prefix from `display_name()`: **to be fixed** as a small item in the rework note. The feature review raised the same point (its nit 2), and the fix is a few lines. If the two prefixes drift apart, Claude's stale cache entries are never pruned. OpenCode lands in task 028, which makes that drift likely.
- 2, a panicking `stats()` leaves `pending` set forever: **to be fixed** as a small item. The feature review raised it too (its nit 3). It was harmless in round 1, but with this round's one-job-per-session rule a single panic blocks that session's stats until restart and costs a pool thread for good.
- 3, push deferred jobs to the front of the queue: **ignored**. It is an ordering preference, not a correctness problem.
- 4, `warned_build &= !ok` readability: **ignored**. It is a style nit and the code is correct.

**Feature review: FAIL.**
- Finding 1 [must-fix], stats starvation for a session that is being written to: **to be fixed**. I confirmed it by reading `src/index/worker.rs`:
  - While job K1 for a session runs, every rebuild puts the newest key in `deferred` (`schedule_stats`).
  - Continuous writes trigger a rebuild about every 500 ms (`get_or_insert_with` coalescing).
  - When K1 finishes, `record` queues the deferred job and then drops K1 because `!is_live`.
  - If every parse spans a rebuild, the session never gets a `Stats` event. A session that already has stats never gets them refreshed.

  This breaks the acceptance criterion "Stats events arrive for every session" and §7.3 "titles and stats refresh". It hits exactly the large live sessions that §9.2 targets. The defect comes from combining two round-2 changes: the drop-if-not-live rule and one job per session. The code reviewer's trace found no bug because it checked overwrite safety, not progress. That is an omission, not a conflicting demand.
- Finding 2 [nit], prefix single source: same as code nit 1, **to be fixed** as a small item.
- Finding 3 [nit], `catch_unwind`: same as code nit 2, **to be fixed** as a small item.

**UI review: PASS.** `UI: none`, and no UI files changed. No findings.

No A/B choice is involved. The possible fixes (a sequence guard, or "send a stale result only if nothing newer was shown") all give the same visible behaviour: stats lag the file by at most one version, then catch up. So this is not a reason to escalate.

## Conflicts
None. The code review's PASS does not dispute feature finding 1. It just did not consider the starvation case.

## Rework budget
Earlier REWORK verdicts: 0 of 2. Round 1 has no `verdict.md`.

## Rework note
Fix stats starvation for sessions that are being written to, in `src/index/worker.rs`. Also make two small robustness fixes.

**1. Starvation (required).** Today `Coordinator::record` drops every result whose key is no longer in the harness's latest snapshot (`if !self.is_live(..) { return true; }`), and each session has at most one job at a time, with newer keys parked in `deferred`. When a transcript keeps growing and each parse outlasts the next 500 ms rebuild, every result is stale on arrival and gets dropped. The session then never gets `Stats`, or never gets fresh ones, while it is active. Relax the drop rule, do not revert it, and keep the one-job-per-session rule:
- Give the coordinator a monotonically increasing `seq: u64` counter. Stamp each `StatsJob` with the next value when `schedule_stats` creates it, and carry it through to `Done`. A deferred job keeps the seq it got when scheduled.
- Record the seq of the last `Stats` event sent per session. For example, change `sent` to `HashMap<(usize, SessionId), (String, u64)>` or add a `sent_seq` map. Both kinds of send record it:
  - pool results sent from `record`;
  - cached values sent from `schedule_stats`. These take a fresh seq when sent, so a still-running older job cannot overwrite them later. Example: the file reverts to a key that is already cached.
- In `record`, after clearing `pending` and queueing the deferred job as now:
  - If `done.key` is live, behave as today: cache it, send it, record key and seq.
  - If `done.key` is not live, the session is still in the harness's latest snapshot (keep a set of its session ids next to `live_keys`, or look it up in `last`), and `done.seq` is newer than the last seq sent for the session: send the `Stats` event and record its key and seq. Do **not** insert it into the cache, because `prune` would remove it anyway.
  - Otherwise, drop it: the session is gone, or something newer was already sent.
  - Errors keep today's handling: log and add to `failed`. A stale error needs no `failed` entry.
- This keeps the round-1 guarantee that an older result never overwrites a newer one. Jobs for a session still run one after another, and the seq check also covers the cached-send case.
- Update the doc comment on `record` (and on `sent`/`pending` if needed) to state the new rule.

**Tests.**
- Replace `tests/index_stub.rs::a_late_result_for_an_old_key_is_dropped_and_the_new_key_is_computed_after_it`, since it asserts the old drop rule. Use the `block_first` stub and the 50 ms poll timing, and do this:
  1. Wait on `entered` while the version-1 computation is held. The stub reads `version` before blocking, so this computation reports `turns == 1`.
  2. Set `version` to 2 and wait for the snapshot whose title is `s v2`. Then set it to 3 and wait for the snapshot titled `s v3`. Assert that no `Stats` arrives before the release.
  3. Release `gate`.
  4. Assert that the first `Stats` event has `turns == 1` and the next has `turns == 3`. Also assert that `stub.calls == 2`: version 2 is never computed, because version 3 replaced it in `deferred`.

  The old code fails this test, because its first `Stats` has `turns == 3`.
- Add a test showing that a stale result arriving after a newer cached value was sent is dropped. Watch out for one trap: with a single harness this case cannot be set up, because `schedule_stats` calls `prune()` on every build and keeps only the current snapshot's keys. A pre-seeded newer key would be pruned at the first build. Pick one of these:
  - A unit test in `worker.rs` that drives `Coordinator::record` and `schedule_stats` directly.
  - A stub test that keeps `prune` skipped. `prune` is skipped while some harness has no `live_keys`, which a harness whose first build fails never gets. Steps:
    1. Add a second stub (`HarnessId::OpenCode`) whose `projects()` returns `Err`.
    2. Pre-seed `stats-v1.json` with `"claude:s:v2": {"turns":42,"duration":0}`.
    3. Hold the version-1 computation, then set `version` to 2. Expect a cached `Stats` with `turns == 42`.
    4. Release `gate`. Assert that no further `Stats` arrives for `s` within about 500 ms.

**2. One source for the key prefix (small).** `key_prefix` in `worker.rs` uses `display_name().to_lowercase()`, while `ClaudeHarness::stats_key` (`src/harness/claude/discovery.rs`) hardcodes `"claude:"`. Add one stable helper, for example `HarnessId::cache_prefix(self) -> &'static str` returning `"claude"`, `"opencode"`, and so on. Use it in both places, and in the stub harness in `tests/index_stub.rs`. Update the `Harness::stats_key` doc to name the helper.

**3. Panicking `stats()` (small).** In `pool_thread`, run `harnesses[job.harness].stats(&job.summary)` under `std::panic::catch_unwind(AssertUnwindSafe(..))`. Map a panic to an `Err`, for example `anyhow!("stats computation panicked")`, so that `Done` is still sent, `record` clears `pending` and marks the key failed, and the thread keeps serving. A unit test or stub test with a harness whose `stats` panics once, followed by a version bump that then gets stats, would show it works.

Dropped review items: pushing deferred jobs to the front of the queue (ordering preference only), and rewriting `warned_build &= !ok` (style only).

Done when the new and updated tests pass, the existing `tests/index.rs` suite still passes, and `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` are all green.
