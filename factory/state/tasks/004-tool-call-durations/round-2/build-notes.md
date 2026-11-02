STATUS: DONE

## Changes
- `src/harness/claude/wire.rs` (code-review 1, feature-review 1, must-fix): a `toolUseResult` that `main` skips can no longer fail the line. `Entry` is now `Entry<M, T>` (the `toolUseResult` type is a parameter). `read_entry` falls back in `lenient_entry`: the line is read again with `toolUseResult` as raw text (`Box<RawValue>`, which is not checked, so lone surrogates, `1e999` and deep nesting pass) and `isAsync` is read from that text with the map-only head, so `{"isAsync":true,"prompt":"\ud83d"}` is still a launch with no duration; if even that fails (invalid UTF-8) the field counts as absent (`IgnoredAny`). The existing damaged-`usage` retry also works when `toolUseResult` is damaged too. Invalid UTF-8 anywhere else is still unparsed, as on `main`. The `ToolUseResult` doc no longer claims it is never buffered; the key type is `String` (feature-review 3).
- `renderers/tool/row.rs` (new): the folded and unfolded description rows, split out of `mod.rs`. `folded` is a free function (code-review 3), uses `style.patch(muted)` for the duration as the unfolded rows do (code-review 2), and `cut` keeps the `…` in the style of the text it replaces. `wrapped` keeps ` · <duration>` together (ui-review 1): it goes on the last row when it fits there, else on a row of its own (`· 5m 2s`), and is broken only when wider than a row. `mute_tail` is gone.
- `src/transcript_search.rs` (ui-review 2): a subagent's `subagent_type`/`agent` input value is not searched on top of the agent type in the description row, so `story-writer` is one hit. Module and `section_text` docs describe the drawn row (code-review 5).
- Docs (code-review 5): `ToolCallSection`, `ToolCall.duration` (empty for an asynchronous launch).
- `assemble.rs` (code-review 9): plain `if is_async`.
- `bench_support/generate.rs` (code-review 6): the room check is `2 * large_size + 1024`, as the line holds the output twice. `GENERATOR_VERSION` stays 2; cached v2 fixtures were deleted and regenerated. Fixture figures are unchanged (104,861,111 bytes, 48 large outputs).

## Self-check
- Review: code-review 1 / feature-review 1 — `claude_assembly::{a_tool_use_result_that_main_skips_does_not_fail_the_line (lone surrogate in object and string, invalid byte in object and string, 1e999, 200-deep nesting, two isAsync:true rows: result attached, Ok, unparsed 0, 3 s or no duration), a_damaged_tool_use_result_and_a_damaged_usage_in_one_line_still_parse, invalid_utf8_elsewhere_than_tool_use_result_and_usage_is_still_unparsed}`; raw byte inputs through `Assembler::from_memory`. The first test fails for the surrogate rows against the previous branch state (verified by the review's table; the fallback is what makes them pass).
- Review: code-review 2 — `row::tests::the_folded_row_cuts_the_description_and_keeps_the_tail` (spans) and `durations::the_duration_is_muted_and_the_description_keeps_its_token`.
- Review: code-review 3 — `row::folded` is a free function.
- Review: code-review 4 — `durations::a_row_that_leaves_no_room_for_the_description_is_cut_as_a_whole` asserts the `…` cell's foreground (distinct theme); `row::tests::a_tail_without_room_for_the_description_cuts_the_whole_row`.
- Review: code-review 5, 9 — docs and `if is_async` as described above.
- Review: code-review 6, 7 — generator; `bench_support::tests::every_generated_tool_result_carries_a_tool_use_result_that_repeats_its_output` (object for success, string for failure).
- Review: code-review 8 / feature-review 4 — `transcript::search::tests::{a_description_row_ending_in_the_duration_is_found_as_the_source_line, a_description_wrapped_over_two_rows_is_found_by_its_end}`.
- Review: feature-review 2 — see above (generator).
- Review: ui-review 1 — `row::tests::{the_duration_goes_to_a_row_of_its_own_rather_than_be_split, a_duration_wider_than_a_row_is_broken, ...}` and snapshot `unfolded_call_keeps_a_long_duration_in_one_piece_at_40` (new: `· 5m 2s` on its own row). The wrapped-at-40 styled snapshot changed in one style run only: the space before `·` is now part of the muted run (invisible).
- Review: ui-review 2 — `transcript::tests::search::a_search_for_the_duration_of_a_subagent_finds_it_after_the_agent_type` now searches `Explore` and expects `1/1`.
- Round 1 AC/UI mapping is unchanged (see round-1 notes).

## Budgets
`cargo test --release -- --ignored budgets --nocapture` (v2 fixtures regenerated, worst of five): mixed first content 2.6 ms, parse 136 ms, slowest frame 4.99 ms, peak heap 0.22x; dense first content 1.7 ms, parse 289 ms, slowest frame 0.97 ms, peak heap 0.58x. All within budget. Good lines take the unchanged first parse; the fallback runs only on a line that failed.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2553 passed, 0 failed)
