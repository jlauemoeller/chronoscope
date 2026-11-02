STATUS: DONE

## Changes
- `src/index/worker.rs`:
  - A rebuild of harness `i` now replaces only harness `i`'s waiting stats jobs; other harnesses' waiting jobs stay queued.
  - `pending` is keyed by (harness, session): a session has at most one job queued or running. A newer key for a session whose job is still running is deferred and queued when that job finishes, so a growing transcript is not parsed several times at once.
  - `record` drops (neither caches nor sends) a result whose key is no longer in the harness's latest snapshot, so a late old result can never overwrite a newer one.
  - `sent.retain` uses a `HashSet` of the snapshot's ids instead of an O(n^2) scan.
  - Pruning only removes keys with the prefix of a harness that is running (`claude:` etc.), so entries of disabled harnesses survive and a zero-harness index prunes nothing. The `Harness::stats_key` doc states the prefix convention.
  - A failing build is logged at error level once per streak (debug afterwards).
- `src/index/cache.rs`: unique temp file name (`stats-v1.json.<pid>.<n>.tmp`), removed on failure; `retain_live` takes the owned prefixes.
- `src/harness/mod.rs`: `build_harnesses` logs a warning when the home directory cannot be detected; `stats_key` prefix convention documented.
- Docs: `Assembler::first_timestamp/last_timestamp` and `ClaudeHarness::stats` say that only conversation entries count; `IndexHandle` says the app must call `shutdown()` on exit.
- Tests: new `tests/index_stub.rs` (stub harnesses); `tests/index.rs`: watching tests run with a 1 h poll interval so only file events can deliver them; misleading comments reworded; temp-file assertion updated.

## Self-check
- Review (code 1, must-fix): other harnesses' jobs discarded — `index_stub::every_session_of_every_harness_gets_stats` (two harnesses, 12 slow + 3 sessions; verified it fails with the old behaviour).
- Review (feature 1, must-fix; code 2): older result overwrites newer — `index_stub::a_late_result_for_an_old_key_is_dropped_and_the_new_key_is_computed_after_it` (also asserts no concurrent parse of the same session; feature 5 and code 2's optional part).
- Review (code 3): O(n^2) retain — replaced with a HashSet.
- Review (code 4): prune with no/disabled harness — `index_stub::cache_entries_of_harnesses_that_are_not_running_are_kept`, `cache::tests::retain_live_keeps_the_entries_of_harnesses_it_does_not_own`.
- Review (code 5, feature 3): fixed temp name — unique name; `cache::tests::round_trips_through_the_file` and `index.rs` assert no leftovers.
- Review (code 6): home detection warning — added.
- Review (code 7, feature 8): docs state which entries count.
- Review (code 8, feature 7): comment reworded.
- Review (code 9): polling/watching — the new-session, new-project and title tests now run with a 1 h poll; polling-test comment reworded.
- Review (feature 4): build failure logged once per streak.
- Review (feature 6): `IndexHandle` docs require `shutdown()` on exit.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (397 passed, 0 failed)
