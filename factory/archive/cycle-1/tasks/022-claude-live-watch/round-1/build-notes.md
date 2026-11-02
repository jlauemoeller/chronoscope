STATUS: DONE

## Changes
- `src/model/mod.rs`: new `ModelUpdate::Activity { last_modified: SystemTime, idle: Option<bool> }` (no-op in `apply`). `src/components/transcript/updates.rs` and `src/app/browser.rs` ignore it for now (the `● live` marker is not shown by this task; no UI change).
- `src/harness/claude/load.rs`: the loader now keeps a trailing partial line in a buffer (`Streamer.partial`, `read_pos()`), also in the initial load (an unterminated last line is still parsed at once if it is already a complete JSON value, so files lacking a final newline load as before). New `load_file` (initial load and reload share it), `Identity` (dev+inode on Unix, creation time elsewhere), `catch_up` (incremental read from the last offset: complete lines to the assembler, `flush_held`, one `Updated`/`Appended` batch, `ParseWarnings` only when the count changed), `send_activity` (file mtime, `idle: None`, sent after `Loaded` and after every processed change). `Job` gains the watch mode and poll interval. No `Progress` after `Loaded` for incremental batches; the load hook is not called for them.
- `src/harness/claude/watch.rs` (new): the watch loop. Watches the session file's directory non-recursively with `notify` (Access events ignored, other files filtered by name) and `<uuid>/subagents/` when it exists or appears (re-checked every 1 s; its events are only logged at debug level). Events coalesce over a fixed 100 ms window. A change is handled by opening the file read-only: new identity or size below the read position gives `Reset` + full re-parse (same streaming path, `Progress`/`ParseWarnings`/`Loaded`/`Activity`); bigger size gives an incremental read. One unconditional check right after the watch starts closes the gap between the initial read and the watch. If `notify` cannot start, it falls back to polling size, mtime and identity every 1 s (logged with `warn!`). All waits are sliced at 50 ms and honour the stop flag, so dropping the `WatchHandle` stops the worker promptly and the channel disconnects.
- `src/harness/claude/discovery.rs`: `ClaudeHarness::with_polling()` (`#[doc(hidden)]` test seam), passes mode/poll interval to the job; `load` docs updated. `src/harness/mod.rs`: `Harness::load` doc updated. `src/harness/claude/subagent.rs`: `subagents_dir` is `pub(super)`.
- `tests/claude_live.rs` (new): integration tests with generated session files.
- Files are only opened with `File::open`; nothing is written.

## Self-check
- AC: appended complete lines give `Appended` — `appended_lines_arrive_as_appended_turns`
- AC: extended `message.id` / `tool_result` give `Updated`, same `TurnId`, status not `Pending` — `a_line_extending_a_message_updates_its_turn_in_place`, `a_tool_result_updates_the_turn_with_the_pending_call`
- AC: a line in two parts is parsed once after its newline — `a_line_written_in_two_parts_is_parsed_once_after_its_newline`, `a_partial_last_line_at_load_time_waits_for_its_newline`
- AC: truncation / rename give `Reset` + re-parse + `Loaded`, turns equal a fresh parse — `truncating_the_file_reloads_it`, `replacing_the_file_by_a_rename_reloads_it` (`#[cfg(unix)]`), `polling_reloads_a_truncated_file`
- AC: 50 rapid appends coalesce (< 10 batches) — `a_burst_of_appends_is_coalesced` (verified to fail with the coalescing window set to 0)
- AC: `Activity` carries the mtime after load and after each change — `activity_carries_the_file_mtime_after_the_load_and_after_each_change`, also checked in `polling_detects_appends_when_watching_is_unavailable`
- AC: polling when watching is unavailable — `polling_detects_appends_when_watching_is_unavailable`, `polling_reloads_a_truncated_file`
- AC: dropping the handle stops watching and disconnects — `dropping_the_handle_stops_watching_and_disconnects`, `dropping_the_handle_stops_a_polling_worker_too`
- AC: no write paths — only `File::open` / `fs::metadata` in `load.rs` and `watch.rs`
- Extra: subagent dir and other sessions' files do not touch the parent — `other_sessions_and_subagent_files_do_not_touch_the_turns`; event classification — unit tests in `watch.rs`
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1359 passed, 0 failed)
