VERDICT: APPROVE

## Assessment
Round 2 fixes both round-1 must-fixes. The index now watches OpenCode's db and `-wal` through their parent directory, non-recursively, and filters events by name, so it survives WAL churn. The debounce test now spaces its writes 30 ms apart and fails when the debounce is removed. Both reviewers confirmed both fixes with mutant runs. I re-ran `cargo test --test opencode_live` (20 passed) and the working tree stayed clean. Every remaining finding is minor.

**Code review: PASS**
1. [nit] SQL `length(CAST(data AS BLOB))` vs Rust `row.data.len()` mismatch for NULL or non-UTF-8 `data`: ignored. OpenCode's schema declares `data TEXT NOT NULL`, so a NULL cannot happen. A non-UTF-8 row would only be re-sent as `Updated` while it is the newest row, which is harmless. Hardening is optional.
2. [nit] The `assert!(wal.exists())` message in `the_index_keeps_following_after_the_wal_is_deleted_and_created_again` overstates who recreates the log: ignored. The wording is cosmetic. Both reviewers' mutant runs (`watch_files` returning nothing) show the test still tells working code from broken code.
3. [nit, Approver ruling requested] A `SQLITE_OPEN_READ_ONLY` connection to a WAL-mode database recreates missing `-wal`/`-shm` files in OpenCode's directory (§9.5 "Never writes to harness data"). Ruling: this does not violate the task, for these reasons:
   - The connection behaviour comes from the task-028 helper and predates this task.
   - The main database file and its content are never changed.
   - The recreated files are SQLite's own auxiliary files, which OpenCode's SQLite creates and manages anyway.
   - The AC test `following_a_session_never_modifies_the_database_or_its_directory` passes.
   - The extra index rebuild it triggers settles after one pass.

   The suggested alternative, `immutable=1`, is not a real option B. With it, SQLite skips locking and change detection and assumes the file never changes, which would break live following, the core of this task. If the human wants the auxiliary-file behaviour revisited, it belongs in a follow-up task on the task-028 connection helper.
4. [nit] `index/worker.rs::concerns` and `opencode/watch.rs::forward` duplicate the name-filter rule: ignored. Sharing one helper is an optional refactor, and both filters are tested.

**Feature review: PASS** (all nine ACs checked, with tests that fail on mutants)
1. [nit] `watch.rs` `MAX_WAIT` (a 1 s cap from the first event) departs from the binding planner note's pure trailing debounce. Accepted as minor:
   - SPEC §7.1 only says "debounced 200 ms".
   - It still coalesces on 200 ms of quiet and meets the coalescing AC.
   - A pure trailing debounce would hide a message that streams without a 200 ms pause, which defeats the task's goal.
   - The deviation is disclosed and documented in the module doc.
2. [nit] The WAL test sleeps a fixed 600 ms after deleting the log, so it could false-pass on a very slow machine: ignored. The risk is a false pass, not a flake. The mutant runs show the test does catch the real bug on normal hardware.
3. [nit] No `debug_assert!` that `watch_files` ⊆ `watch_paths`: ignored, optional hardening with a single implementer today.
4. [nit] Known limits, documented in the `follow.rs` module doc, none required by an AC: ignored. These are deleted rows giving no `Reset`, a new part of a row updated in place added at the end of its turn, a row unparsable when first read never re-shown, injected entries straddling passes, and a same-length rewrite within the same millisecond being missed.

**UI review: PASS.** `UI: none`, and no view, component, theme or snapshot changed. No findings.

## Conflicts
None. Round 2 resolves both round-1 FAILs (code and feature), and the reviewers agree on every nit carried forward.

## Rework budget
Earlier REWORK verdicts: 0 of 2. `round-1/` has no `verdict.md`.
