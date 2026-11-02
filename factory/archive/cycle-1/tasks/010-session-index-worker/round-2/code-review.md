VERDICT: PASS

## Summary
Round 2 fixes the round-1 must-fix. A rebuild of harness `i` now replaces only harness `i`'s waiting stats jobs, and `tests/index_stub.rs::every_session_of_every_harness_gets_stats` would fail under the old behaviour. The other findings are fixed too: stale results are dropped in `record`, each session has at most one job queued or running (newer keys are deferred), `sent.retain` uses a `HashSet`, pruning is scoped by prefix, the temp name is unique, build failures are logged once per streak, and the docs and test comments are corrected. I traced the new pending/deferred state machine through running, waiting, reverted, removed and multi-harness cases and found no bug. The test gate passes; what remains are nits.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites green; `tests/index.rs` and `tests/index_stub.rs` re-run 3 times with no flakes; no `.snap.new` files)

## Findings
1. [nit] `src/index/worker.rs` `key_prefix` (around line 557): The pruning prefix comes from `HarnessId::display_name().to_lowercase()`, a user-facing string. `ClaudeHarness::stats_key` (`src/harness/claude/discovery.rs:406`) hardcodes `"claude:"` separately. If the display name changes (e.g. to "Claude Code"), the two drift apart silently. Claude's entries would then never be pruned, and the cache would grow without bound. Change: add a stable identifier, e.g. `HarnessId::key(self) -> &'static str` returning `"claude"`, `"opencode"`, … Use it in both `key_prefix` and `ClaudeHarness::stats_key`, or have `stats_key` call a shared helper so the convention is enforced in one place.
2. [nit] `src/index/worker.rs` `pool_thread` / `Coordinator::pending`: If `Harness::stats` panics on a malformed transcript, that pool thread dies and its `Done` is never sent. The session's `pending` entry then stays forever. Every newer key for that session goes to `deferred` and is never queued, so the session gets no stats until restart, and the pool shrinks by one thread. Change: run `harnesses[job.harness].stats(&job.summary)` under `std::panic::catch_unwind(AssertUnwindSafe(..))` and turn a panic into an `Err` result, so `record` clears `pending` and marks the key failed.
3. [nit] `src/index/worker.rs` `Coordinator::record`: The deferred job is `push`ed onto the back of the queue, behind other sessions' waiting jobs. This breaks the "newest sessions first" ordering for the live session the user is most likely looking at. Change (optional): push deferred jobs to the front, e.g. add a `Queue::push_front`.
4. [nit] `src/index/worker.rs` `build`, `state.warned_build &= !ok;`: This is correct but terse. Change: write `if ok { state.warned_build = false; }` for readability.
