STATUS: DONE

## Changes
- `src/harness/claude/assemble.rs`:
  - Error text blocks without `apiBlockIndex` are numbered like ordinary blocks: a text that equals or extends a block the message already has (not one from the same line) replaces it, any other text takes the next free number (`error_block_key`). Distinct blocks on separate lines now join as `first\nsecond`; repeated lines still collapse.
  - When the flag first appears for a message id, the text blocks already shown for it move into the error text and are removed from the turn (`take_message_text`, using `remove`, so `Builder.index` and `CallRef` positions stay right). Text of other messages is left alone. This replaces the earlier text instead of showing the error twice.
  - Comments: empty `stop_reason` is deliberately no reason; `stop` is kept per turn.
- `src/harness/claude/wire.rs`: `isApiErrorMessage` on `Entry` is now a third type parameter `F` (`ApiErrorFlag`, read leniently); the UTF-8 check parse in `lenient_entry` reads it as `IgnoredAny`, so invalid UTF-8 inside a top-level flag string no longer drops a line that parses on `main`.
- `tests/claude_notices.rs`: new tests for each of the above.

## Self-check
- Review (code 1, feature 1): distinct blocks without index — `distinct_error_blocks_on_separate_lines_without_an_index_are_joined`, `repeated_lines_of_several_error_blocks_without_an_index_collapse`, `a_grown_error_block_without_an_index_replaces_the_earlier_one`; `an_api_error_repeated_over_several_lines_gives_one_notice` still passes.
- Review (code 2): invalid UTF-8 in top-level flag — `invalid_utf8_in_a_top_level_flag_keeps_the_line_parsed` (fails with the old check type; checked).
- Review (feature 2, code 3): flag after text — `a_flag_arriving_after_the_same_text_replaces_the_text_block`, `text_shown_before_the_flag_joins_the_errors_text`, `other_messages_text_stays_when_a_message_turns_out_to_be_an_error`, `a_tool_result_attaches_to_a_call_that_follows_the_text_taken_into_the_error`.
- Review (feature 3): `an_empty_stop_reason_is_no_reason` plus comment. Review (feature 4): noted in the `stop` field comment.
- All round-1 acceptance tests still pass. UI: none, no snapshot changed.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (0 failed)
- cargo test --release -- --ignored budgets --nocapture: pass
