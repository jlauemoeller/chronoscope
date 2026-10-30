STATUS: DONE

## Changes
- `src/harness/claude/inject.rs` (new): allowlist / noise / metadata classification, `NEWEST_KNOWN_VERSION` (`2.1.314`, the newest version in the fixtures), numeric per-part version comparison, and text extraction per attachment kind / system subtype / meta entry (fields chosen from real entries: `file` = path + content, `instructions` = path + content per file, `hook_system_message` = `content`, `local_command` = `/command args` + stdout, `selected_lines_in_ide` = `path:start-end` + content, ...).
- `src/harness/claude/wire.rs`: `Head` now carries `subtype`, `version`, `message` presence and the attachment kind; `Entry` gained `content`, `commandRun`, `attachment`, `isCompactSummary`.
- `src/harness/claude/assemble.rs`:
  - `process` dispatches all entry classes. Invalid JSON, unreadable entries, unknown types with a `message`, unknown content blocks (one per block) and unreadable content are skipped, counted (`unparsed_count()`), and logged with the line number at `warn`. Noise is silent; metadata is logged at `debug` and not counted. Blank lines are ignored.
  - Allowlisted entries are held and placed by the next deciding entry: a human prompt (or human `queued_command`) makes them the first item of the new User turn; anything else flushes them into the current turn (open Assistant turn, or a User turn with no reply yet). If the last turn is a Compaction turn or there is none, they stay held for the next turn. Flush at `finish()` / `into_turns()`. Entries flushed into an Assistant turn directly after an `Injected` item are merged into it (consecutive entries collapse).
  - Human `queued_command` (`attachment.origin.kind == "human"`, falling back to an entry-level `origin`) starts a User turn; other `queued_command`s are injected.
  - Compaction: `compact_boundary` creates a `Turn::Compaction` (empty summary, boundary timestamp and uuid); the next `isCompactSummary` user entry fills the summary (an `Updated` if it arrives in a later batch); an orphan summary becomes its own Compaction turn; the summary is never a prompt.
  - Unknown attachment kinds (neither allowlisted nor known noise) are counted per kind (`unknown_attachment_kinds()`), not unparsed, and logged once per kind at `info` (`unknown attachment kind "x" (3×)`) at end of input.
  - `flush_held()` (new, public): the end-of-batch flush for live appends. It places held entries in the current turn, leaves a trailing partial line buffered and does not count it as unparsed. `finish()` does the same after flushing the trailing line.
  - Version check: the first entry with a newer `version` logs one `warn` per session; parsing continues.
- `src/harness/claude/mod.rs`: exports `NEWEST_KNOWN_VERSION`.
- `src/model/mod.rs`: doc comments only.
- Fixtures: `tests/fixtures/claude/injected.jsonl`, `compaction.jsonl` (hand-written, documented in `tests/fixtures/README.md`).
- `tests/claude_injected.rs`: 40 tests with a thread-local capturing logger.

Decisions beyond the planner notes: the held entries are chunk-invariant (only `finish()`/`into_turns()` flush at end of input, `push_chunk` does not), so random chunking gives identical turns. Unparsed lines and metadata do not decide placement. The fixtures cover every allowlisted kind found in the real data (including `plan_mode`, `plan_mode_exit`) and every spec-named noise attachment. Real local sessions (53 files) assemble with 0 unparsed; `edited_text_file`, `invoked_skills`, `plan_file_reference`, `skill_mention` and `task_status` show up as unknown attachment kinds (candidates for the allowlist).

## Self-check
- AC: allowlisted entries before a human prompt open the User turn; mid-assistant entries stay in place — `entries_before_a_human_prompt_open_that_user_turn`, `entries_in_the_middle_of_assistant_activity_stay_at_that_position`, `entries_between_a_prompt_and_the_first_reply_belong_to_the_prompt_turn`, `entries_before_the_very_first_turn_go_into_it`
- AC: three consecutive entries with noise collapse into one item with kind/timestamp/text — `three_entries_with_noise_between_collapse_into_one_item_with_kind_time_and_text`, `noise_does_not_break_a_run_of_injected_entries`, `system_and_meta_entries_use_their_kind_and_readable_text`
- AC: human queued_command starts a User turn and a new Assistant turn; non-human is injected — `human_queued_command_starts_a_user_turn_and_a_new_assistant_turn`, `human_queued_command_takes_held_entries_as_its_first_item`, `queued_command_without_a_human_origin_is_injected`
- AC: noise categories produce nothing and do not change the unparsed count — `noise_never_makes_an_item_and_is_not_counted` (table-driven)
- AC: metadata entries skipped without counting — `metadata_entries_are_skipped_without_counting`
- AC: malformed line / unknown type with message / unknown block count once, parsing continues — `malformed_line_unknown_type_with_message_and_unknown_block_each_count_once`, `every_unknown_block_counts_once`, `the_fixture_has_three_unparsed_entries`
- AC: unknown attachment kind logged with count, not unparsed — `unknown_attachment_kinds_are_counted_and_logged_once_each`, `unknown_attachment_kinds_are_logged_once_per_session_even_when_finished_twice`
- AC: compaction — `compact_boundary_and_summary_make_one_compaction_turn`, `entries_after_the_summary_go_into_the_next_turn`, `a_boundary_without_a_summary_has_an_empty_summary`, `a_summary_arriving_in_a_later_batch_updates_the_compaction_turn`
- AC: newer version warns exactly once and still parses — `a_newer_version_warns_exactly_once_per_session_and_still_parses`, `versions_up_to_the_newest_known_do_not_warn`, `the_newest_known_version_is_the_newest_in_the_fixtures`, unit test `versions_compare_numerically_per_part`
- AC: task-006 tests pass and random chunks give identical results with injected content — all of `tests/claude_assembly.rs` unchanged and passing; `random_chunks_give_identical_turns_with_injected_content_and_compaction`, `line_by_line_equals_the_whole_transcript`, `every_prefix_replays_to_the_matching_turns`, `a_flush_at_the_end_of_input_puts_trailing_entries_in_the_current_turn`, `flushing_held_entries_mid_stream_keeps_a_partial_line_for_later`
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (276 passed, 0 failed)
