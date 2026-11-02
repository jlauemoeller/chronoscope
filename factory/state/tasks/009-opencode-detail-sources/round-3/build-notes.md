STATUS: DONE

## Changes
- `src/harness/opencode/details.rs`: `find_part` reads the part the assembler uses: only `type: "tool"` parts (exact comparison), the last part with an id wins (`.rev()` keeps the original index), doc comment replaced. `PartHead` gains `kind`. `Rows::text` trims, and a blank string is no value.
- `src/harness/opencode/details_tests.rs`: the extra `error` shape `{"type":5,"message":true}` in `the_error_row_says_what_the_notice_of_the_same_row_says`; new tests `a_tool_part_sent_twice_in_one_row_is_read_from_the_last`, `a_part_of_another_type_with_the_id_of_a_tool_part_is_not_read`, `a_position_that_holds_another_type_now_makes_the_tool_group_unavailable`, `a_text_value_is_trimmed_and_a_blank_one_is_left_out`, `reading_details_in_wal_mode_leaves_the_database_file_untouched`.
- Nothing else: not `assemble.rs`, `AssistantData`, `UserData`, and no `.snap` file.

## Self-check
- Rework 1 (`find_part`) — the three tests above; deleting `.rev()` fails only `a_tool_part_sent_twice_in_one_row_is_read_from_the_last`; deleting the `Some("tool")` filter fails only `a_part_of_another_type_with_the_id_of_a_tool_part_is_not_read` and `a_position_that_holds_another_type_now_makes_the_tool_group_unavailable`.
- Rework 2 (trim) — `a_text_value_is_trimmed_and_a_blank_one_is_left_out`; deleting `.map(str::trim)` fails only it.
- Rework 3 (error shape) — added to the `errors` list.
- Rework 4 (tests incl. WAL) — added as given.
- Every mutation was made alone and undone (file identical to the pre-mutation copy).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2902 passed, 0 failed; `INSTA_UPDATE=no`, no `.snap` changed)
- cargo test --release -- --ignored budgets --nocapture: pass (exit 0)
