STATUS: DONE

## Changes
- `tests/claude_details.rs`: the five new tests of the rework note (`a_queued_prompt_reads_the_prompt_group_of_its_own_line`, `the_latest_of_two_stopped_lines_gives_the_message_group`, `a_timestamp_that_does_not_parse_is_shown_as_written`, `a_call_line_that_fails_marks_the_tool_call_unavailable`, `a_later_footer_line_updates_the_turn_even_when_the_stats_do_not_change`), and `"promptSource": 2.5` in `a_field_of_an_unexpected_type_counts_as_missing_and_does_not_fail_the_line`.
- `src/harness/claude/details.rs`: `read_line` calls the new `parse_line(&[u8])`, which the unit-test helper `parse` now calls too, so `only_a_json_object_is_a_line` runs production code. No behaviour change. The unit tests gain the values `-7`, `2.5` (other-type `toolUseResult`), `below=-2, ratio=2.5` and `-5`, `2.5` (permission decision).
- Left as told: empty top-level strings, `visit_none`/`visit_some`, `has_result`, `unavailable_80x24_default`, the load path, all other production code. No `.snap` file changed, no `.snap.new` left.

## Self-check
Each temporary edit of the note's "Done when" was made on its own, `INSTA_UPDATE=no cargo test --no-fail-fast` run with no other build running, and the code restored (`git status` shows only the two intended files). Each failed exactly the named test and no other; `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` did not flake in any run:
- `tool_group` `unavailable: matches!(result, Some(None)),`: `a_call_line_that_fails_marks_the_tool_call_unavailable`
- `on_attachment` passing `&(0..0)`: `a_queued_prompt_reads_the_prompt_group_of_its_own_line`
- `note_message_line` `if stopped && lines.stopped.is_none() {`: `the_latest_of_two_stopped_lines_gives_the_message_group`
- `parse_line` without `json.end().ok()?;`: `only_a_json_object_is_a_line`
- `on_turn_duration` ignoring `set_footer`: `a_later_footer_line_updates_the_turn_even_when_the_stats_do_not_change`
- `Rows::time` `Err(_) => SourceValue::Text(String::new())`: `a_timestamp_that_does_not_parse_is_shown_as_written`
- `PickVisitor` without `visit_f64`: `a_field_of_an_unexpected_type_counts_as_missing_and_does_not_fail_the_line`
- `ObjectVisitor` without `visit_i64`, and separately without `visit_f64`: `tool_use_result_of_another_type_than_an_object_counts_as_missing`
- `TextVisitor` without `visit_i64`, and separately without `visit_f64`: `a_permission_decision_is_text_or_its_scalar_members`
- `Member`'s visitor without `visit_i64`, without `visit_f64`, and without `visit_unit`, each: `a_permission_decision_is_text_or_its_scalar_members`
- Round-3 review findings: code 1-2, 5-6 and feature 1-2, 4-6: the new tests; code 3 and feature 3: `parse_line`; the rest as the verdict ruled.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2852 passed, 0 failed, 2 ignored; `INSTA_UPDATE=no`)
- cargo test --release -- --ignored budgets --nocapture: pass (mixed full parse worst 109 ms, peak heap 0.22x; dense 305 ms, 0.63x, slowest frame 1.0 ms; endless turn 674 ms)
