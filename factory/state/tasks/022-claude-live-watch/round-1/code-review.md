VERDICT: FAIL

## Summary
The live watcher is well built. It watches the directory non-recursively and filters events by name, coalesces events over a fixed 100 ms window, buffers a trailing partial line, reloads on shrink or identity change through the same streaming path, falls back to polling, and stops promptly via the stop flag. Files are only opened with `File::open`, and the test gate is green (the live suite passed 5 runs in a row with no flakes). One binding requirement has no test: `ParseWarnings` must be sent on a live change when the warning count changes. That gap is the only must-fix. The rest are nits.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [must-fix] `tests/claude_live.rs`: nothing tests the binding planner rule "Send the assembler's updates as `Appended`/`Updated` batches, plus `ParseWarnings` when the count changes" (implemented in `Streamer::catch_up` / `send_warnings` in `src/harness/claude/load.rs`).
   - All three `ParseWarnings` assertions in the file (lines ~341, ~354, ~370) are negative: they only check that half a line produces no warning.
   - If `catch_up` never sent `ParseWarnings`, or sent it on every batch, the suite would still pass.

   Change: add a test such as `a_malformed_appended_line_updates_the_warning_count`:
   - Start a live worker on `initial()`, then `clear()`.
   - Append `"not json\n"` and wait for `ModelUpdate::ParseWarnings(1)`.
   - `clear()`, append a valid `prompt(3, "ok")`, and wait for the turns with `wait_for_turns`.
   - Drain stragglers with a short sleep and `try_recv`, then assert no `ParseWarnings(_)` is in `live.seen`. The count did not change, so none may be sent.
2. [nit] `src/harness/claude/watch.rs`, `Watch::reload`: on `Err` from `load_file` it sends `Loaded` and returns `false`. The worker then exits, after the transcript has already been cleared by `Reset`. The planner says to "continue watching" after a reload. This path only runs on an I/O error after a successful open, so it is rare. Change: log the error, send `Loaded`, and return `true` so the watch goes on. The next event (or poll) would retry. That needs the identity/position state to force another reload: for example, keep a `needs_reload` flag that `on_change` checks first.
3. [nit] `src/harness/claude/load.rs`, `Streamer::read_all`: a final line with no newline that is not valid JSON is now buffered silently instead of counted as an unparsed entry. A session that crashed mid-write therefore shows no parse warning for its cut-off last line (§9.5 "malformed lines are skipped and counted"). This trade-off follows from §7.1's partial-line buffering, so it is acceptable. Consider documenting it in the module docs, or counting the buffered line as a warning if the session is still unchanged after some time.
4. [nit] `tests/claude_live.rs`: the live tests use synthetic prompt/assistant/tool_result lines only. §9.6 asks for adapter tests on fixture sessions "including live append scenarios". The assembler-level tests (`tests/claude_assembly.rs`, `tests/claude_injected.rs`) already cover fixture chunking and live `flush_held` boundaries. Still, one watcher-level test would add coverage of the file-backed byte ranges and the per-`catch_up` `flush_held` through `load()`. It would append `tests/fixtures/claude/session.jsonl` (or `injected.jsonl`) to a watched temp file in a few parts and compare the replayed updates with `Assembler::from_file`.
5. [nit] `src/harness/claude/load.rs`, `WatchMode`: the `Default` derive and the `#[default]` attribute are unused, because `ClaudeHarness::with_data_dir` sets `WatchMode::Notify` explicitly. Change: drop `Default` and `#[default]`.
6. [nit] `src/harness/claude/discovery.rs`: `ClaudeHarness::poll_every` is plumbed into `Job`, but nothing can set it, so it is always 1 s. Change: either make it a `const POLL_EVERY: Duration` in `watch.rs` and drop the field from `ClaudeHarness` and `Job`, or add a `#[doc(hidden)]` setter if the tests should poll faster.
7. [nit] `src/harness/claude/watch.rs`, `forward`: an event with no paths is dropped, for example `notify`'s `Flag::Rescan` after an inotify queue overflow. Changes in that window are only picked up by the next real event. Change: treat an event whose `need_rescan()` is true (or that has empty `paths`) as `Wake::Session`.
8. [nit] `src/harness/claude/load.rs`, `load_file`: the parameter is spelled `path: &std::path::Path` while the module imports `PathBuf` from `std::path`. Change: import `Path` and use `&Path`.
9. [nit] `tests/claude_live.rs`, `a_missing_session_still_ends_in_loaded_and_stops`: the line `let _: &Path = &data.file;` exists only to use the `Path` import. Change: remove the line and import only `PathBuf`.
10. [nit] `src/harness/claude/load.rs`, `Streamer::catch_up`: it allocates a fresh 256 KiB buffer on every change. Change: keep the buffer on the `Streamer`, or use a smaller one sized to `meta.len() - read_pos`. This is minor and off the hot path.
