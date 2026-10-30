STATUS: DONE

## Changes
- `src/model/mod.rs`: `ModelUpdate` gains `Progress(f32)`, `Loaded`, `ParseWarnings(usize)` (`apply` ignores them); `ToolCallId` alias for `ItemId`; `SessionId` doc names the subagent id form `<parent-uuid>/subagents/agent-<agentId>`.
- `src/harness/mod.rs`: `Harness` gains `load`, `subagent`, `resume_command` (§1.1 signatures, still object-safe). New `WatchHandle` (stop flag + worker `JoinHandle`; `Drop` sets the flag and never joins; `stop()`, `is_finished()`).
- `src/harness/claude/load.rs` (new): the worker. Resolves the id to a file, opens it, reads line by line (`read_until`), feeds `Assembler::push_line` with absolute byte ranges, and coalesces: at each flush it sends `Updated` for already-sent turns changed since the last flush and `Appended` for the rest (so a turn appended and updated in one batch goes once, as `Appended`), then `Progress`. First batch at 50 new turns or 256 KiB; later ones every 2 MiB. End: last batch, `Progress(1.0)`, `ParseWarnings(n)`, `Loaded`. Stop flag checked per line; a failed send stops the worker. Not found or I/O error: logged at error level, partial turns flushed if any, `Loaded`, stop.
- `src/harness/claude/subagent.rs` (new): session id to path (`<project dir>/<id>.jsonl`, found by listing project dirs, no file reads, ids validated so they cannot leave the data dir), subagent linkage (meta `toolUseId`, else parent scan for the call's `tool_result` and `toolUseResult.agentId`; only returns ids whose transcript exists), `SubagentInfo` and `info()`.
- `src/harness/claude/resume.rs` (new): `shell-words` split, `{id}` replaced in each argument, `current_dir` = session cwd when known; empty/unparsable gives `None` + `log::warn!`.
- `src/harness/claude/discovery.rs`: `ClaudeHarness` implements the three methods; `with_resume_command`, `subagent_info`, doc-hidden `with_batch_limits` / `with_load_hook` (test hooks); `new()` reads `harness.claude.resume_command`.
- `Cargo.toml`/`Cargo.lock`: `shell-words` 1.1.
- Tests: `tests/claude_loading.rs` (32), `tests/claude_loading_log.rs` (3, own binary because the logger is global); `tests/claude_assembly.rs` match made exhaustive-safe. New fixture tree `tests/fixtures/claude/subagents/`, documented in `tests/fixtures/README.md`.

Decisions: `load()` does id resolution, open and parse on the worker (the caller only spawns the thread), so a bad id is `Loaded` plus a log line rather than `Err`. `ParseWarnings` is sent once, with `Loaded` (no live changes until task 022). The hook test seams make the stop/hang-up tests deterministic.

## Self-check
- AC: load streams Appended batches equal to the full parse, then ParseWarnings and Loaded — `load_streams_the_fixture_then_warnings_and_loaded` (session, injected, compaction), `updates_to_already_sent_turns_arrive_as_updated`, `the_first_batch_goes_out_after_50_turns`, `..._256_kib_of_input`, `later_batches_go_out_every_2_mib`
- AC: 20 MB file, first Appended before Progress >= 0.5, progress non-decreasing, ends 1.0 — `a_20_mib_file_shows_its_first_turns_early_and_progress_is_monotonic`
- AC: dropping the handle stops the worker (receiver sees disconnection within 5 s, no Loaded) — `dropping_the_handle_stops_the_worker`, `dropping_the_handle_does_not_block`, `a_worker_whose_receiver_hung_up_stops_by_itself`
- AC: missing file / read error gives Loaded, no panic, logged — `a_missing_session_ends_in_loaded`, `a_missing_data_directory_ends_in_loaded`, `a_read_error_ends_in_loaded`, `a_missing_file_is_logged_as_an_error_and_ends_in_loaded`, `a_read_error_is_logged_as_an_error_and_ends_in_loaded` (log test binary)
- AC: subagent via meta, via toolUseResult.agentId, None for unknown, load of the subagent id with the prompt as first User turn — `subagent_is_found_through_the_meta_file`, `subagent_meta_linkage_does_not_need_the_parent_transcript`, `subagent_falls_back_to_the_tool_use_result_agent_id`, `subagent_is_none_without_a_link`, `a_meta_file_without_its_transcript_does_not_link`, `load_streams_the_subagent_with_its_prompt_as_the_first_user_turn`, `every_subagent_call_of_a_loaded_parent_resolves`
- AC: subagent_info — `subagent_info_reads_the_meta_file`, `subagent_info_is_none_without_a_subagent_or_a_meta_file`
- AC: resume_command default, custom template, empty gives None — `the_default_resume_command_resumes_in_the_session_cwd`, `a_custom_template_substitutes_inside_arguments`, `an_empty_or_unparsable_template_gives_no_command`, `quoting_is_honoured_and_every_placeholder_is_replaced`, `the_config_template_is_used_by_new`, `an_unusable_resume_template_is_logged_as_a_warning`
- AC: parsing off the calling thread — `load_returns_before_the_parse_and_parses_on_another_thread` (worker thread id differs), `load_does_not_wait_for_a_slow_worker` (load returns while the worker is held in its hook)
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (362 passed, 0 failed)
