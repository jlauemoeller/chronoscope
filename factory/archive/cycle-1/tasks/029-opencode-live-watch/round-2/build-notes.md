STATUS: DONE

## Changes
- `src/harness/mod.rs`: new `Harness::watch_files()` (default: none), the subset of `watch_paths()` that are individual files.
- `src/harness/opencode/discovery.rs`: `watch_files()` returns the db and its `-wal`; `watch_paths()` is unchanged (task-028 and registry tests still hold). Doc link to the private `watch` module removed and the comment rewrapped.
- `src/index/worker.rs`: `watch()` watches files through their parent directory, non-recursively, and `concerns()` filters events by file name (`-shm`, `-journal` and other files are ignored; `Access` still ignored). Directories listed in `watch_paths()` but not in `watch_files()` are watched recursively as before, so Claude is unchanged. The watch now survives the WAL being deleted and created again. Unit tests for `concerns`.
- `src/harness/opencode/follow.rs`: the updated-rows query now lists `id, time_updated, length(data)` first and reads the `data` only of rows whose time or length differs from what was read (replaces the hash); the polling `Signature` doc mentions its same-millisecond blind spot; module doc lists the known limits.
- `src/harness/opencode/load.rs`: doc links to private modules removed.
- `tests/opencode_live.rs`: `ten_quick_writes_are_coalesced` now spaces the writes 30 ms apart and asserts 1..=3 batches; new `the_index_keeps_following_after_the_wal_is_deleted_and_created_again`.

## Self-check
- Review (code 1, must-fix, index loses the WAL watch): fixed as option (a); test `the_index_keeps_following_after_the_wal_is_deleted_and_created_again` (long `Timing.poll`, WAL mode, writer closed and `-wal` asserted gone, new writer inserts into `session_v2`, new `Snapshot` expected). Mutant check: with `watch_files()` returning nothing, the test fails; with it, it passes.
- Review (feature 1, must-fix, debounce test): writes 30 ms apart, `1..=3` batches. Mutant check: `DEBOUNCE` and `MAX_WAIT` set to zero gives 10 batches and the test fails.
- Review (code 2 / feature 2, MAX_WAIT cap): kept on purpose, explained in the `watch.rs` module doc.
- Review (code 3 / feature 3, `finish()` per pass): documented as a known limit in the `follow.rs` module doc, not changed.
- Review (code 4): documented as a known limit (part of a row updated in place is added at the end of its turn), not changed.
- Review (code 5 / feature 6, row unparsable when first read): documented as a known limit, not changed.
- Review (code 6 / feature 7, doc links, long line): fixed.
- Review (feature 4, full `data` re-read each pass): fixed with the light query (time and length). A rewrite to text of the same length within the same millisecond is no longer seen.
- Review (feature 5, `Signature` doc): added.
- All round-1 ACs: unchanged tests, still passing (see the round-1 notes).
- UI: none.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (0 failed; lib 1668 passed, tests/opencode_live.rs 20 passed)
