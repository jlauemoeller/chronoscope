STATUS: DONE

## Changes
- `src/index/worker.rs`:
  - Starvation fix: the coordinator has a monotonically increasing `seq`. `StatsJob` and `Done` carry the `seq` of the job (a deferred job keeps the one it got when scheduled). `sent` now stores `(key, seq)` per session; a cached send in `schedule_stats` takes a fresh seq.
  - `record`: a result for a current key is cached and sent as before. A result for an outdated key is sent (not cached) when the session is still in the harness's latest snapshot (`live_sessions`) and its seq is newer than the last one sent for the session. Otherwise it is dropped. Stale errors are logged without a `failed` entry. Doc comment on `record` states the rule.
  - `pool_thread` runs `stats()` under `catch_unwind(AssertUnwindSafe(..))`; a panic becomes an `Err` so `Done` is still sent and the thread keeps serving.
  - `key_prefix` uses `HarnessId::cache_prefix()`.
- `src/model/mod.rs`: `HarnessId::cache_prefix()` (`claude`, `opencode`, `codex`, `pi`) with a unit test.
- `src/harness/claude/discovery.rs`: `stats_key` uses `cache_prefix()`. `src/harness/mod.rs`: `stats_key` doc names the helper.
- Tests: `tests/index_stub.rs` replaces the old drop-rule test; stub gained `panic_first`, `fail_projects`, `blocking_first`; stub keys use `cache_prefix()`. `tests/index.rs` gained a key-prefix test.

## Self-check
- Rework 1 (starvation): `index_stub::a_stale_result_is_sent_so_a_busy_session_is_not_starved` (held v1, bumps to v2 then v3, release: first Stats turns == 1, next turns == 3, `calls == 2`; fails with the old drop rule - checked by mutation).
- Rework 1 (stale after newer cached send dropped): `index_stub::a_stale_result_after_newer_stats_were_sent_is_dropped` (second stub whose `projects()` fails keeps prune skipped; seeded `claude:s:v2` gives turns 42; no further Stats within 500 ms; fails if the seq check is removed - checked by mutation).
- Rework 2 (one prefix source): `HarnessId::cache_prefix` used by `key_prefix`, `ClaudeHarness::stats_key` and the stub; tests `model::tests::cache_prefixes_are_distinct_lowercase_names`, `index::claude_stats_keys_start_with_the_cache_prefix_and_change_with_the_file`.
- Rework 3 (panic): `index_stub::a_panicking_stats_call_does_not_block_the_session_or_the_pool`.
- Existing `tests/index.rs` suite still passes (16 tests).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (401 passed, 0 failed)
