STATUS: DONE

## Changes
- `tests/claude_details.rs`: the seven tests of the rework note, added before `an_empty_string_is_no_value`: `a_line_after_the_stopped_line_leaves_the_message_group_on_the_stopped_line`, `the_footer_reads_its_last_turn_duration_line`, `a_hook_before_the_prompt_reads_the_environment_of_its_own_line`, `a_summary_without_a_boundary_reads_the_environment_of_its_own_line`, `a_call_sent_again_in_its_turn_keeps_the_rows_of_its_result`, `a_call_id_seen_in_an_earlier_turn_reads_its_own_line`, `the_other_field_locations_are_read_and_the_top_of_the_line_wins`. `an_empty_string_is_no_value` also has an empty `timestamp`. The long-turn timing test compares 3 000 and 48 000 items with a limit of 48x. The duplicate local `hook` closure is gone.
- `src/harness/claude/details.rs`: an empty member of a permission-decision object is no value (`Member::visit_str`); the unit test covers `{"decision":"","source":"config"}` and `{"decision":""}`. This is the only production change.
- `tests/budgets.rs`: comment says ~70 MB. `docs/performance.md`: the single-turn paragraph ends as the note says (the 0.66 s is `Assembler::from_memory`, the load path is left out; 3 000 and 48 000 items).
- Left as the note says: `has_result`, `unavailable_80x24_default` (its `model` value moved to offset 7; round 2's notes untouched), the load path on a long turn, all other production code. No `.snap` file changed and no `.snap.new` is left.

## Self-check
- Each temporary edit of the rework note's "Done when" was made on its own, `INSTA_UPDATE=no cargo test --no-fail-fast` run, and the code restored (`git status` shows only the intended files). Each failed exactly the named test and no other:
  - `.note_message_line(message_no, range, false)`: `a_line_after_the_stopped_line_leaves_the_message_group_on_the_stopped_line`
  - `set_footer` keeping the first line: `the_footer_reads_its_last_turn_duration_line`
  - removing `held.add_to(&mut sources, &self.source);` in `start_user_turn`: `a_hook_before_the_prompt_reads_the_environment_of_its_own_line`
  - summary without a boundary with `TurnSources::new()`: `a_summary_without_a_boundary_reads_the_environment_of_its_own_line`
  - `Some(known) if known.turn == b.idx && false`: `a_call_sent_again_in_its_turn_keeps_the_rows_of_its_result`
  - `Some(known) => known.source`: `a_call_id_seen_in_an_earlier_turn_reads_its_own_line`
  - `Rows::time` without the empty filter: `an_empty_string_is_no_value`
  - `Member::visit_str` keeping empty values: `a_permission_decision_is_text_or_its_scalar_members`
  - `cache_miss` from `message` only; removing the `toolUseResult` fallback of `permissionDecision`; `resolvedModel` from `toolUseResult` only: each fails `the_other_field_locations_are_read_and_the_top_of_the_line_wins`
  - a scan back in `add_message_item`: `assembling_a_long_turn_takes_time_in_proportion_to_its_items`
- Round-2 review findings: code 1-4, 8 and feature 1-2: the new tests; code 5, feature 4: wider timing test; code 6: field-location test; code 7: earlier-turn call test (`has_result` left as told); code 9, feature 7: empty `timestamp`; code 10, feature 5: comment; code 11: closure; feature 6: doc sentence; ui 1: empty member; ui 2: offset read as 7.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2847 passed, 0 failed, 2 ignored; `INSTA_UPDATE=no`)
- cargo test --release -- --ignored budgets --nocapture: pass (mixed full parse worst 140 ms, peak heap 0.22x; dense 430 ms, 0.63x, slowest frame 0.95 ms; endless turn 680 ms)
