STATUS: DONE

## Changes
- `tests/claude_notices.rs`: the five new tests from the rework note (later-turn removal leaves an earlier turn's call attachable; merged injected item's id is free again; flagged message with only whitespace has no notice; invalid UTF-8 in `message.isApiErrorMessage`; invalid UTF-8 in `stop_reason`) and the entry-order assertion at the end of `injected_items_a_removal_leaves_side_by_side_collapse`.
- `src/harness/claude/assemble.rs`: the doc comment on `error_block_key` re-wrapped. No other change in `src/`; no behaviour change.

## Self-check
- Each temporary edit below was made, the named test failed, and the code was restored from a backup (the final `src/` diff is the comment re-wrap only):
  - delete `c.turn == b.idx &&` in `shift_after`: `a_removal_in_a_later_turn_leaves_a_call_of_an_earlier_turn_attachable` fails.
  - swap the merge order in `remove`: `injected_items_a_removal_leaves_side_by_side_collapse` fails.
  - drop `b.index.remove(&second);`: `a_merged_injected_items_id_is_free_again` fails.
  - `text.trim().is_empty()` -> `text.is_empty()`: `a_flagged_message_with_only_whitespace_has_no_notice` fails.
  - add `stop_reason` to `MessageWithoutUsage`: `invalid_utf8_in_the_stop_reason_keeps_the_line_parsed` fails.
  - add the `isApiErrorMessage` field to `MessageWithoutUsage`: `invalid_utf8_in_the_flag_inside_the_message_keeps_the_line_parsed` fails.
- Dropped items of the note (`saturating_add`, the feature reviewer's alternative sequence) not done.
- UI: none. No `.snap` changed, no `.snap.new` left.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (2664 passed, 0 failed)
- INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture: pass (full parse worst 297 ms vs 2000 ms, first content 2.2 ms, memory 0.58x, worst frame 3.3 ms)
