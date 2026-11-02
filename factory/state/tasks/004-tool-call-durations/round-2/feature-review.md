VERDICT: PASS

## Summary
Round 1's blocking regression is fixed: a `toolUseResult` that `main` skips no longer fails the line, and every damaged input from the round-1 table now keeps its result. All acceptance criteria are implemented and covered by tests, and the test gate and release budgets pass in my own runs. In a scratch copy, I broke the fallback, the asynchronous check, search, muting, the pending check and the export summary one at a time, and each change failed at least one test. Only nits remain.

## Acceptance criteria
- [x] Fixtures set `duration: Some(..)`, with snapshots folded and unfolded, including a subagent suffix and a too-narrow row — `fixtures::with_duration`; 13 new snapshots from `src/components/turn_block/durations.rs`, at widths 80 and 120, and 40 for the narrow and wrapped cases.
- [x] Format tests cover the boundaries at 1 second and 60 seconds — `renderers/tool/timing.rs` tests: `999ms`, `1.0s`, `59.9s`, `1m 0s`, including truncation below a millisecond.
- [x] Assembler tests cover `isAsync: true` and a string `toolUseResult` — `an_asynchronous_launch_has_no_duration`, `a_string_tool_use_result_still_parses_the_line_and_its_result` in `tests/claude_assembly.rs`. With the `is_async` check in `attach_result` disabled, the two asynchronous tests fail.
- [x] A search match in a tool description highlights the correct cells with a duration shown — `a_hit_in_a_description_with_a_duration_lands_on_its_row_with_the_right_cells`. With `tool_text` giving the bare description, it fails.
- [x] A pending call and a call without a known duration show no duration and no separator — `a_pending_call_shows_no_duration_and_no_separator` (its fixture carries a duration; it fails without the status check), `a_call_without_a_known_duration_shows_no_duration_and_no_separator`.
- [x] A finished `Error` call shows its duration — `a_failed_call_with_a_duration_shows_it`, folded and unfolded.
- [x] A wrong-typed `isAsync` still parses the line and its result, and the call gets a duration — `an_is_async_of_the_wrong_type_still_parses_the_line_and_its_result`; `wire::tests::only_a_boolean_true_is_an_asynchronous_launch`.
- [x] `toolUseResult` is not read as a `serde_json::Value` by `Entry` — `Entry<M = Message, T = ToolUseResult>` in `src/harness/claude/wire.rs`. The fallback keeps it as `Box<RawValue>` and only on a line that failed its first parse, so it is not a `Value` there either.
- [x] The duration appears in the as-displayed copy (folded and unfolded), full copy, saved file, as-folded export and full export summary — tests in `durations.rs` and `browser_tests::save::w_saves_the_duration_with_the_description_and_capital_w_with_the_full_content`. With `details()` using `description_text()`, `the_full_export_summary_has_the_duration` fails.
- [x] A search for the duration's text finds it and highlights its cells — `a_search_for_the_duration_finds_it_and_highlights_its_cells` checks the cell backgrounds, and the cell before the match.
- [x] The narrow-row snapshot shows the description cut with `…` and the duration intact — `folded_call_too_narrow_for_its_description_at_40`: `▶ Investigate the descrip… · 340ms`.
- [x] Generated fixtures contain `toolUseResult`, `GENERATOR_VERSION` is bumped, and budgets hold — `generate.rs`, version 2; `every_generated_tool_result_carries_a_tool_use_result_that_repeats_its_output`. My run, worst of five: mixed first content 1.9 ms, parse 96.1 ms, slowest frame 3.24 ms, heap 0.22x; dense 1.9 ms, 283.3 ms, 0.97 ms, 0.58x.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — rerun with `INSTA_UPDATE=no`: 2553 passed, 0 failed.
- [x] Snapshot changes are limited to tool rows of calls with a duration — one snapshot is modified, the OpenCode browser screen: four tool rows, each with a real adapter duration. The others are new.
- [x] Round 1's open item, that reading `toolUseResult` must not fail a line that parses on `main` — `read_entry` and `lenient_entry` in `wire.rs`; `a_tool_use_result_that_main_skips_does_not_fail_the_line`, `a_damaged_tool_use_result_and_a_damaged_usage_in_one_line_still_parse`, `invalid_utf8_elsewhere_than_tool_use_result_and_usage_is_still_unparsed`. Each of these fallback mutations fails at least one of the first two tests: no fallback, the raw `isAsync` read dropped, and no unchecked read after the lossy retry.

## Round 1 follow-up
- Finding 1: fixed as above. I also probed cases the committed tests leave out, through the real `Assembler`. All keep the result with 0 unparsed:
  - `isAsync: true` with damaged `usage`, with and without a lone surrogate in `toolUseResult`, gives no duration.
  - `isAsync: true` with 200-deep nesting gives no duration.
  - A wrong-typed `isAsync` with `1e999`, and a top-level `-1e999` or 30-digit number, give a duration.
- Finding 2: `generate.rs` now checks `2 * large_size + 1024`, and the version stays 2.
- Finding 3: the key type is `String`.
- Finding 4: the wrapped case is its own test, `a_description_wrapped_over_two_rows_is_found_by_its_end`.
- Duration in one piece (ui-review 1): I searched an unfolded call at width 40 whose duration is on its own row. `5m 2s`, `cluster` and `sensor` each report `1/1` and highlight only their own cells. Copy is not affected, because it does not depend on the block width.

## Findings
1. [nit] `src/harness/claude/wire.rs:3-8`: the module doc still names `message.usage` as the one exception to "a known field with a different JSON type fails the line", and `toolUseResult` is now one too. Change: add that a `toolUseResult` of another shape, or a damaged one, counts as absent and never fails the line.
2. [nit] `round-2/build-notes.md`, "code-review 4": it says `durations::a_row_that_leaves_no_room_for_the_description_is_cut_as_a_whole` asserts the `…` cell's foreground with a distinct theme, but that test (`src/components/turn_block/durations.rs:248`) checks only text. The behaviour is covered at unit level by `row::tests::a_tail_without_room_for_the_description_cuts_the_whole_row`, so no coverage is missing. Change: add the assertion to that test using the `fg` helper and `distinct_theme()`, or correct the notes.
3. [nit] `src/components/transcript/tests/search.rs`: no committed search test has the duration on a row of its own, the path added in this round; `a_search_for_a_duration_of_minutes_finds_it` uses a row where it fits. The behaviour is correct in my probe. Change: add a test with the description from `unfolded_call_keeps_a_long_duration_in_one_piece_at_40`, drawn at width 40, that searches `5m 2s` and asserts `highlighted` is `["5m", "2s"]`.
4. [nit] `src/harness/claude/wire.rs:269-276`: an asynchronous launch with an invalid UTF-8 byte in `toolUseResult` counts as not asynchronous and shows a 3 s duration, while the same line with `usage` also damaged goes through the lossy path, reads `isAsync: true` and shows none. The first outcome is the one round 1 allowed ("an unreadable `toolUseResult` counts as absent"), and a JavaScript writer does not produce invalid bytes, so the inconsistency has no realistic effect. Change: none needed unless this code is touched again; if it is, make both paths give the same answer.
