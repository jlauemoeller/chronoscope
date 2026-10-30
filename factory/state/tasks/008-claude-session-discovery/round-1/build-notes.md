STATUS: DONE

## Changes
- `src/model/mod.rs`: added `ProjectId`, `SessionId`, `Project`, `SessionSummary` and `HarnessId` (Claude=1, OpenCode=2, Codex=3, Pi=4; `number()`, `from_number()`, `display_name()`, `Display`). The old two-variant `model::Harness` enum (only used by `Session.harness`) is replaced by `HarnessId`, which avoids a clash with the new `Harness` trait.
- `src/harness/mod.rs`: the stage-1 `Harness` trait (`id`, `available`, `projects`, `sessions`). `Result` is `anyhow::Result` (crate `anyhow` added).
- `src/harness/claude/scan.rs`: the per-file scan (`FileScan`: cwd, start time, first human prompt, last `ai-title`, last entry timestamp). One front-to-back pass with `read_until`; malformed lines, invalid UTF-8 and a partial last line are skipped. Only `user` and `queued_command` lines are fully parsed, and only until the first prompt is found.
- `src/harness/claude/discovery.rs`: `ClaudeHarness` (`new(&Config, home)`, `with_data_dir`, `refresh()`, `scan_count()`, `last_build()` -> `BuildStats`, `with_max_workers`, doc-hidden `with_scan_hook`). The cache is keyed by path and holds (size, mtime, scan). A build stats every file, reuses matches, drops vanished files and scans the rest with `std::thread::scope` (workers = `available_parallelism()` capped by the file count, taking files from a shared atomic index). `projects()`/`sessions()` read the cached index and only build when none exists. Also `truncate_title` and `default_data_dir`.
- `src/harness/claude/wire.rs`, `inject.rs`, `assemble.rs`: the "human prompt" rule now has one definition, `Entry::human_prompt_text()` (plus `queued_prompt_text()` for human `queued_command` attachments), used by both the assembler and the scanner. `content_text` moved to `wire.rs`. Assembler behaviour is unchanged (all existing tests pass).
- `tests/fixtures/claude/discovery/` (+ README entry) and `tests/claude_discovery.rs`; unit tests in `scan.rs`.

Decisions where the task was open:
- Title truncation: first non-blank line of the prompt; if it is over 80 chars, the first 79 chars plus `…` (80 in total).
- A file with no `cwd` (the 0-byte case included) always gets `(untitled)` and the file mtime as its start, as the task says, even if it has an `ai-title`. `cwd` is `None` on its `SessionSummary` and `Project`.
- `first_prompt` is kept up to 4096 chars; prompts that are blank do not count as the first prompt.
- A no-cwd file still carries the scanned `first_prompt` on its summary while its title is forced to `(untitled)`; in practice such files have no prompts.
- A file that fails to open or read is logged and left out of the build (not cached, so it is retried).
- Missing data dir: `projects()` is `Ok(empty)`; unknown project: `sessions()` is `Ok(empty)`.
- `refresh()` is an inherent method of `ClaudeHarness`, not on the trait.

## Self-check
- AC: types exist, `ClaudeHarness` implements the trait — `claude_harness_implements_the_trait`, `harness_ids_have_stable_numbers_and_names`, `sessions_carry_their_summary_fields`
- AC: `available()`, `data_dir` override — `available_follows_the_data_dir`, `default_data_dir_is_under_home_and_config_overrides_it`, `a_missing_data_dir_has_no_projects_and_an_unknown_project_no_sessions`
- AC: same cwd in two dirs is one project; two cwds in one dir are two — `sessions_in_different_encoded_dirs_with_one_cwd_are_one_project`, `sessions_in_one_encoded_dir_with_different_cwds_are_two_projects`
- AC: display name / alias / project order — `project_names_are_the_basename_or_the_configured_alias`, `aliases_from_the_config_are_matched_on_the_expanded_path`, `projects_are_sorted_by_most_recent_activity`, `a_newer_mtime_moves_the_project_up_and_is_the_last_activity`
- AC: titles and session order — `the_last_ai_title_wins`, `without_a_title_the_first_prompt_gives_a_truncated_first_line`, `truncation_keeps_80_characters_and_cuts_81`, `without_title_or_prompt_a_session_is_untitled`, `sessions_are_sorted_newest_first_by_start_time`
- AC: subagents not listed, 0-byte file — `subagent_transcripts_are_not_sessions`, `a_zero_byte_transcript_is_listed_under_its_directory_name_as_untitled`
- AC: cache — `one_build_reads_each_file_exactly_once`, `sessions_alone_also_builds_once`, `a_second_build_without_changes_reads_nothing`, `appending_to_one_file_rescans_only_that_file`, `a_changed_mtime_alone_rescans_the_file`, `a_new_file_is_scanned_and_listed`, `a_deleted_file_loses_its_session`, `a_file_that_comes_back_unchanged_is_scanned_again`
- AC: parallel scanning — `scanning_uses_several_threads` (scans must overlap, via a hook; independent of core count), `the_worker_count_never_exceeds_the_files_to_scan`, `the_default_worker_count_follows_available_parallelism`
- Scan rules (shared with the assembler) — unit tests in `src/harness/claude/scan.rs` (`first_prompt_skips_tool_results_meta_and_origins_other_than_human`, `human_queued_command_counts_as_a_prompt`, malformed/partial/invalid UTF-8 lines, last non-empty `ai-title`)
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (321 passed, 0 failed)
