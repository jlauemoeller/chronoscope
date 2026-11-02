VERDICT: PASS

## Summary
Round 3 does all three required items from the approver's rework note, and leaves the placement logic alone. The new test `entries_before_and_after_a_later_prompt_stay_in_separate_items` and its live variant both pass at HEAD. In a scratch copy with `self.user_flushed = false;` deleted from `start_user_turn`, exactly these two tests fail (45 others pass), so the reset now has a real regression guard. The `into_turns` doc comment is correct, the debug message now agrees in number with its count, and fmt, clippy and the tests all pass.

## Acceptance criteria
- [x] Allowlisted entries before a human prompt become an `Injected` item at the start of that User turn; mid-assistant entries stay at that position. Implemented in `start_user_turn` and `flush_current` (`src/harness/claude/assemble.rs`). Covered by `entries_before_a_human_prompt_open_that_user_turn`, `entries_in_the_middle_of_assistant_activity_stay_at_that_position` and `entries_before_and_after_a_prompt_stay_in_separate_items`. The new `entries_before_and_after_a_later_prompt_stay_in_separate_items` and `entries_before_and_after_a_later_prompt_stay_apart_across_a_live_flush` cover the later-turn case. I mutation-checked them: they fail with the reset removed.
- [x] Three consecutive allowlisted entries (with noise between them) collapse into one item with kind, timestamp and text. Covered by `three_entries_with_noise_between_collapse_into_one_item_with_kind_time_and_text`, `noise_does_not_break_a_run_of_injected_entries` and `a_batch_flush_between_consecutive_entries_in_a_user_turn_still_collapses_them`. Unchanged since round 2.
- [x] A human `queued_command` starts a User turn and a new Assistant turn; a non-human one is injected. Covered by `human_queued_command_starts_a_user_turn_and_a_new_assistant_turn` and `queued_command_without_a_human_origin_is_injected`.
- [x] Every noise category produces no item and does not change the unparsed count. Covered by `noise_never_makes_an_item_and_is_not_counted`.
- [x] Metadata entries are skipped without counting. Covered by `metadata_entries_are_skipped_without_counting`.
- [x] A malformed line, an unknown type with a `message` and an unknown block each count once, and parsing continues. Covered by `malformed_line_unknown_type_with_message_and_unknown_block_each_count_once`.
- [x] An unknown attachment kind is skipped, logged with its count, and not counted as unparsed. Covered by `unknown_attachment_kinds_are_counted_and_logged_once_each`.
- [x] `compact_boundary` + `isCompactSummary` give one `Turn::Compaction`, and the summary does not start a User turn. Covered by `compact_boundary_and_summary_make_one_compaction_turn`.
- [x] A newer `version` logs exactly one warning per session and still parses. Covered by `a_newer_version_warns_exactly_once_per_session_and_still_parses`.
- [x] The task-006 tests pass, and random chunking gives identical results with injected content present. Covered by `tests/claude_assembly.rs` and `random_chunks_give_identical_turns_with_injected_content_and_compaction`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran all three at HEAD; all pass, with 47 tests in `claude_injected`.

## Findings
1. [nit] `entries_left_without_a_turn_are_logged_at_debug_level` checks only the singular form ("1 injected entry has no turn"). The plural branch of the `end_of_input` message ("N injected entries have") is not tested. It is a trivial debug-only string. Change (optional): add a second held entry in a variant of that test and assert `starts_with("2 injected entries have no turn")`.
2. [nit] The `end_of_input` debug message still repeats on every `flush_held()` while the same entries stay held. The approver marked this optional and the builder left it. It has low impact at debug level, so no change is needed.
