VERDICT: PASS

## Summary
Every acceptance criterion is implemented and covered, all six rework items are done, and all test gates pass on my own runs. Round 4 stayed in scope: outside `factory/`, it touches only `tests/claude_notices.rs` and the `error_block_key` doc comment in `src/harness/claude/assemble.rs`, so behaviour is unchanged. Each of the six required mutations fails its named test, which also closes the round-3 nit on `b.index.remove(&second)`.

## Acceptance criteria
- [x] Assembler tests cover each Claude row of §4.2 — `message_notice`, `is_normal_stop` and the `Some("text") if api_error` arm in `src/harness/claude/assemble.rs`; `an_api_error_is_one_error_notice_and_no_text_block`, `max_tokens_and_refusal_each_produce_a_warning`, `any_other_stop_reason_is_a_warning_with_that_reason` in `tests/claude_notices.rs`.
- [x] A Claude API error shows one notice and no duplicate text block — `take_message_text` and the error arm; `an_api_error_is_one_error_notice_and_no_text_block`, `a_flag_arriving_after_the_same_text_replaces_the_text_block`, `text_shown_before_the_flag_joins_the_errors_text`.
- [x] An assembler test shows an API error repeated over several lines of one message gives one notice — `an_api_error_repeated_over_several_lines_gives_one_notice`, `an_api_error_repeated_with_api_block_indices_gives_one_notice`, `repeated_lines_of_several_error_blocks_without_an_index_collapse`.
- [x] Assembler tests cover `end_turn`, `tool_use`, `stop_sequence` and `pause_turn` producing no notice, and `max_tokens` and `refusal` each producing `Warning` with `stopped: <reason>` — `normal_stop_reasons_produce_no_notice`, `max_tokens_and_refusal_each_produce_a_warning`.
- [x] An assembler test shows a message with `stop_reason: null` on every line produces no notice — `a_null_stop_reason_on_every_line_produces_no_notice`.
- [x] An assembler test shows `stop_reason` null on the first lines of a message and `max_tokens` on a later line produces one notice — `null_on_the_first_lines_and_max_tokens_on_a_later_one_gives_one_notice`.
- [x] An assembler test shows `max_tokens` on one line and `end_turn` on a later line of the same message leaves no notice, and a `tool_result` arriving afterwards attaches to its call — `remove` and `shift_after`; `the_last_stop_reason_decides_and_a_normal_one_removes_the_notice`, `a_tool_result_after_the_notice_was_removed_still_attaches_to_its_call`, `a_removed_notice_keeps_every_call_after_it_attachable`, and now `a_removal_in_a_later_turn_leaves_a_call_of_an_earlier_turn_attachable`.
- [x] An assembler test shows `No response requested.` from a synthetic message without the flag stays an `Item::Text` — `a_synthetic_message_without_the_flag_stays_text`.
- [x] An assembler test shows a `stop_reason` and an `isApiErrorMessage` of unexpected JSON types leave the line parsed and not counted in `unparsed_count`, with no notice — lenient fields in `src/harness/claude/wire.rs`; `stop_reason_and_flag_of_unexpected_types_leave_the_line_parsed_without_notice`, `the_flag_of_an_unexpected_type_inside_the_message_leaves_the_line_parsed`, and the three `invalid_utf8_*` tests.
- [x] Feeding a transcript with notices in any chunking gives the same final turns — `any_chunking_gives_the_same_final_turns_with_notices`, `line_by_line_and_byte_by_byte_give_the_same_final_turns_with_notices`, `removing_a_notice_is_reported_as_an_update_of_its_turn`.
- [x] A render test shows an assembled API error drawn as one notice in `error_output`, with no text section — `an_assembled_claude_api_error_is_drawn_as_one_notice_in_error_output` in `src/components/turn_block/tests.rs`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — rerun with `INSTA_UPDATE=no`; all pass, 2664 passed, 0 failed, 2 ignored.
- [x] `cargo test --release -- --ignored budgets --nocapture` stays within budget — rerun: first content worst 2.5 ms, full parse worst 331.9 ms, memory peak 0.58x, worst frame 3.44 ms.
- [x] Any snapshot change is limited to turns that contain a notice — no `.snap` file in `git diff main...HEAD`, and no `.snap.new` file left behind.

## Rework items
Mutations were applied to a `git archive` copy of `HEAD` in the scratchpad, and the repo was not touched. Each was run against `tests/claude_notices.rs` only, with `cargo clean -p chronoscope` before every build. A first pass without the clean gave stale binaries (results shifted by one mutant), so I discarded it. In the rerun each mutant was confirmed compiled, and the unmutated copy passed 50 of 50 tests before and after the series. Each mutant failed exactly the one test named below.

- Item 1, later-turn removal: done, as specified. Deleting `c.turn == b.idx &&` in `shift_after` fails `a_removal_in_a_later_turn_leaves_a_call_of_an_earlier_turn_attachable`.
- Item 2, merge order: done, as specified. Swapping the merge in `remove` fails `injected_items_a_removal_leaves_side_by_side_collapse`.
- Item 3, merged id: done, as specified. Deleting `b.index.remove(&second);` fails `a_merged_injected_items_id_is_free_again`.
- Item 4, whitespace error: done, as specified. Replacing `text.trim().is_empty()` with `text.is_empty()` fails `a_flagged_message_with_only_whitespace_has_no_notice`.
- Item 5, invalid UTF-8 in the `message` fields: done, as specified. Adding `stop_reason` to `MessageWithoutUsage` fails `invalid_utf8_in_the_stop_reason_keeps_the_line_parsed`, and adding the `isApiErrorMessage` field fails `invalid_utf8_in_the_flag_inside_the_message_keeps_the_line_parsed`.
- Item 6, doc comment: done. The round-4 `src/` diff is this re-wrap only, with the wording from the note.
- Dropped items and "Leave as is": respected. There is no `saturating_add`, the alternative sequence was not used, and `remove`, `shift_after`, `take_message_text`, `wire.rs`, the views and all snapshots are unchanged in this round.

## Other checks
Scratch probes gave the expected results:
- Two error blocks joined with a newline draw as two rows of one notice.
- A warning followed by a flagged line with no text leaves no notice.
- A flagged line followed by an unflagged line of the same message joins both texts in the notice.
- A flagged message keeps its thinking block and its tool call, and the call still gets its result.
- `isApiErrorMessage: false` with `refusal` gives the `Warning`.
- A warning, then a flagged line with the same text and `end_turn`, gives one `Error` notice.
- The notice id falls back to the uuid, then the line offset (`notice:a9`, `notice:L130`).
- A flag on a `tool_result`-only `user` line has no effect.

## Findings
None.
