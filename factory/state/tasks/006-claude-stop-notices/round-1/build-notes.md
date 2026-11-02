STATUS: DONE

## Changes
- `src/harness/claude/wire.rs`: `Message.stop_reason` and `isApiErrorMessage` (on `Entry` and on `Message`), all read with `lenient`, so a value of an unexpected JSON type counts as missing and never fails the line. `Entry::is_api_error()` combines the two flags. `toolUseResult` handling is untouched (still the untagged head, never a `Value`).
- `src/harness/claude/assemble.rs`:
  - `Builder` keeps, per message id, the last non-null `stop_reason`, a sticky set of API-error message ids, and the text blocks of flagged messages (keyed by block number, so a repeated line replaces rather than adds).
  - A text block of a flagged message goes to that map instead of becoming `Item::Text`; after the blocks, `message_notice` decides: flagged with text -> `Error` (wins over any stop reason); flagged without text -> nothing; else last stop reason not in `end_turn`/`tool_use`/`stop_sequence`/`pause_turn` -> `Warning` `stopped: <reason>`. The notice (`notice:<msg_id>`) is upserted; no notice removes an earlier one.
  - Removal trap: new `remove` helper drops the item, decrements the positions in `Builder.index` and in every `CallRef` of the current turn that sit after it, so a later `tool_result` still attaches. It runs only on an actual removal, and the notice is only recomputed for lines that carry a stop reason or belong to a flagged message (no allocation on the common null-stop line).
  - `<synthetic>` model-change logic is unchanged; unflagged synthetic text stays `Item::Text`.
- `tests/claude_notices.rs` (new, constructed lines; no fixture file changed, so no README or snapshot change).
- `src/components/turn_block/tests.rs`: render test for an assembled API error.

## Self-check
- AC: each Claude row of 4.2 — `an_api_error_is_one_error_notice_and_no_text_block`, `max_tokens_and_refusal_each_produce_a_warning`, `any_other_stop_reason_is_a_warning_with_that_reason`, `normal_stop_reasons_produce_no_notice`
- AC: API error shows one notice, no duplicate text — `an_api_error_is_one_error_notice_and_no_text_block`, `the_flag_inside_the_message_counts_like_the_one_on_the_entry`, `an_api_error_given_as_string_content_is_a_notice_too`
- AC: API error over several lines gives one notice — `an_api_error_repeated_over_several_lines_gives_one_notice`, `an_api_error_repeated_with_api_block_indices_gives_one_notice`
- AC: normal reasons none, max_tokens/refusal Warning — `normal_stop_reasons_produce_no_notice`, `max_tokens_and_refusal_each_produce_a_warning`
- AC: null on every line, no notice — `a_null_stop_reason_on_every_line_produces_no_notice`, `a_missing_stop_reason_produces_no_notice`
- AC: null then max_tokens gives one notice — `null_on_the_first_lines_and_max_tokens_on_a_later_one_gives_one_notice`
- AC: max_tokens then end_turn leaves no notice and a later tool_result attaches — `the_last_stop_reason_decides_and_a_normal_one_removes_the_notice`, `a_tool_result_after_the_notice_was_removed_still_attaches_to_its_call`, `a_removed_notice_keeps_every_call_after_it_attachable` (the two attach tests fail when the CallRef fix-up is disabled; checked)
- AC: `No response requested.` stays text — `a_synthetic_message_without_the_flag_stays_text`
- AC: unexpected JSON types — `stop_reason_and_flag_of_unexpected_types_leave_the_line_parsed_without_notice`, `the_flag_of_an_unexpected_type_inside_the_message_leaves_the_line_parsed`
- AC: any chunking, same turns — `any_chunking_gives_the_same_final_turns_with_notices`, `line_by_line_and_byte_by_byte_give_the_same_final_turns_with_notices`, `removing_a_notice_is_reported_as_an_update_of_its_turn`
- AC: render test, one notice in `error_output`, no text section — `an_assembled_claude_api_error_is_drawn_as_one_notice_in_error_output`
- AC: snapshots limited to turns with a notice — no snapshot changed (no `.snap` in the diff)
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites; 2287 lib + 30 new in claude_notices, 0 failed)
- cargo test --release -- --ignored budgets --nocapture: pass (full parse 148 ms / 319 ms vs 2000 ms, first content < 3 ms, memory 0.58x, worst frame 5 ms)

## Decisions
- A flagged message with no text produces no notice at all, even with an unusual stop reason (literal reading of the planner note).
- The API-error flag is sticky per message id; its text is the message's text blocks joined by newlines.
