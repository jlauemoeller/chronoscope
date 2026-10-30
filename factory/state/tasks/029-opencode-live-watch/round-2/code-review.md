VERDICT: PASS

## Summary
Round 2 fixes the round-1 must-fix. The index now watches harness files through their parent directory, non-recursively, and filters events by file name (`Harness::watch_files`, `index/worker.rs::watch`/`concerns`). Its watch therefore survives the `-wal` being deleted and created again, and Claude's recursive watch is unchanged. A new integration test covers this. I confirmed it fails without the fix: with `OpenCodeHarness::watch_files` returning nothing in a scratch copy, it times out. The debounce test now spaces the writes 30 ms apart and asserts `1..=3` batches. The updated-rows query lists `id, time_updated, length(data)` and reads the full `data` only of rows that changed. The other round-1 nits are fixed or listed as known limits in the `follow.rs` module doc. The gate passes, and `tests/opencode_live.rs` passed 5 of 5 repeated runs.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (lib 1668 passed, `tests/opencode_live.rs` 20 passed, all other suites green; no `.snap.new` files)

## Findings
1. [nit] `src/harness/opencode/follow.rs:203-213` (step 1, the listing query): the SQL length and the Rust length are not computed the same way in every case.
   - `length(CAST(data AS BLOB))` is compared with `row.data.len()`. But `row.data` comes from `load.rs::text_at`, which reads a blob lossily (`from_utf8_lossy`), writes numbers out as text, and maps NULL to `""`.
   - A row whose `data` is not valid UTF-8 never matches. It is re-read and sent as `Updated` on every pass while its `time_updated` is the newest.
   - A NULL `data` makes `row.get::<_, i64>(2)` fail, so every pass errors out before step 2. The session then stops following altogether. That goes against `text_at`'s own intent ("one odd row does not fail a whole query") and §9.5 ("malformed lines are skipped … not fatal").
   - The same mismatch would hit every row of a UTF-16-encoded database.
   - OpenCode's schema declares `data TEXT NOT NULL`, so these cases are unlikely, which is why this is a nit.
   - Change: read the column as `Option<i64>` and map `None` to 0, or select `ifnull(length(CAST(data AS BLOB)), 0)`. In `Cursor::note`, store the length the database reports instead of `row.data.len()`. For example, add `length(CAST(data AS BLOB))` to the full re-read and the page query, or make `note` take the length as an argument.
2. [nit] `tests/opencode_live.rs`, `the_index_keeps_following_after_the_wal_is_deleted_and_created_again` (the `assert!(wal.exists(), "the write went into a new log")` and the comment "OpenCode starts again and writes into a new log"): the new `-wal` is most likely created by the index's own read-only connection, not by the writer.
   - The index rebuild that the deletion triggers opens the database. I checked separately: a read-only SQLite connection to a WAL database whose `-wal` is gone creates `-wal` and `-shm` and leaves them behind.
   - So the assertion is trivially true. The test is still valid: it is a new inode, and the test fails without the directory watch (my mutant run, see Summary).
   - Change: reword the comment and the assertion message, for example "the log exists again (recreated by a reader or the writer)", or drop the assertion.
3. [nit] For the Approver (§9.5 "Never writes to harness data"): when OpenCode's `-wal`/`-shm` files are missing, a `SQLITE_OPEN_READ_ONLY` connection to a WAL-mode database creates them in OpenCode's directory and leaves them there. I reproduced this with a standalone SQLite script.
   - This comes from SQLite and predates this task: the connection helper is from task 028. The main database file and its content are never changed.
   - It interacts with this task: the index now reacts to the WAL's `Remove` event. So each time OpenCode exits and SQLite deletes the log, the index rebuilds, and its read-only connection recreates `-wal`/`-shm` straight away. This converges after one extra rebuild, because the files then exist and no further create event follows.
   - Change: none required in this task. The Approver may want to decide whether this counts as "writing to harness data". The alternative is opening with `immutable=1` when the log is absent, which has its own trade-offs.
4. [nit] `src/index/worker.rs` `concerns` and `src/harness/opencode/watch.rs` (the name filter in `forward`) each implement the same "directory watch filtered by file name" rule. Consider sharing one helper so that the two filters cannot drift apart. No change is required.
