VERDICT: FAIL

## Summary
The durations themselves are correct: the format, folded and unfolded rows, pending, unknown and `Error` calls, asynchronous launches, copy, save, export and search all work and are tested, and the test gate and release budgets pass. One regression blocks the work: adding `toolUseResult` to `Entry` makes lines unparsed that parse on `main`, so a finished call loses its result and stays `Pending`.

## Acceptance criteria
- [x] Fixtures set `duration: Some(..)`, with snapshots folded and unfolded, including a subagent suffix and a too-narrow row — `fixtures::with_duration`, 12 snapshots in `src/components/turn_block/durations.rs`.
- [x] Format tests cover the boundaries at 1 second and 60 seconds — `renderers/tool/timing.rs` tests: `999ms`, `1.0s`, `59.9s`, `1m 0s`, including sub-millisecond truncation.
- [x] Assembler tests cover `isAsync: true` and a string `toolUseResult` — `an_asynchronous_launch_has_no_duration`, `a_string_tool_use_result_still_parses_the_line_and_its_result` in `tests/claude_assembly.rs`. An ordinary string parses; a string with a lone surrogate does not (finding 1).
- [x] A search match in a tool description highlights the correct cells with a duration shown — `a_hit_in_a_description_with_a_duration_lands_on_its_row_with_the_right_cells` exercises the truncation path. It asserts the output was not expanded and that only the query's cells are marked.
- [x] A pending call and a call without a known duration show no duration and no separator — `a_pending_call_shows_no_duration_and_no_separator` (the fixture carries a duration, so the status check is what is tested), `a_call_without_a_known_duration_shows_no_duration_and_no_separator`.
- [x] A finished `Error` call shows its duration — `a_failed_call_with_a_duration_shows_it`, folded and unfolded.
- [x] A wrong-typed `isAsync` still parses the line and its result, and the call gets a duration — `an_is_async_of_the_wrong_type_still_parses_the_line_and_its_result`; `wire::tests::only_a_boolean_true_is_an_asynchronous_launch`.
- [x] `toolUseResult` is not read as a `serde_json::Value` by `Entry` — `Entry.tool_use_result: Option<ToolUseResult>` in `src/harness/claude/wire.rs`. The derived `untagged` still buffers the value; see finding 1.
- [x] The duration appears in the as-displayed copy (folded and unfolded), full copy, saved file, as-folded export and full export summary — tests in `durations.rs` and `browser_tests::save::w_saves_the_duration_with_the_description_and_capital_w_with_the_full_content`. `y`/`w` go through `plain::displayed_text`, which those tests call or drive.
- [x] A search for the duration's text finds it and highlights its cells — `a_search_for_the_duration_finds_it_and_highlights_its_cells` checks the cell backgrounds, and the cell before the match.
- [x] The narrow-row snapshot shows the description cut with `…` and the duration intact — `folded_call_too_narrow_for_its_description_at_40`: `▶ Investigate the descrip… · 340ms`.
- [x] Generated fixtures contain `toolUseResult`, `GENERATOR_VERSION` is bumped, and budgets hold — `generate.rs`, version 2. My run on the regenerated fixtures: mixed first content 2.1 ms, parse 139.5 ms, worst frame 4.76 ms, heap 0.22x; dense 1.4 ms, 303.5 ms, 1.03 ms, 0.58x.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — rerun: 2542 passed, 0 failed.
- [x] Snapshot changes are limited to tool rows of calls with a duration — the one modified snapshot is the OpenCode browser screen, where only four tool rows moved, each from a real adapter duration. The other 12 are new.
- [ ] Goal and spec requirement that a `toolUseResult` is read safely and must not fail the line — not met for the inputs in finding 1.

## Findings
1. [must-fix] A `toolUseResult` that `main` skips can now fail the whole Claude line. The call's result is lost, the call stays `Pending` for good, and the line is added to the `N entries could not be parsed` note.

   I fed these lines to the real `Assembler` from scratch copies of `main` and this branch, each built in its own target directory:

   | `toolUseResult` on the result line | `main` | This branch |
   |---|---|---|
   | `{"stdout":"cut \ud83d here"}` (lone surrogate) | `Ok`, result, 3 s, unparsed 0 | `Pending`, no result, no duration, unparsed 1 |
   | `"cut \ud83d here"` (a string, lone surrogate) | same | same as above |
   | object with an invalid UTF-8 byte in `stdout` | same | same as above |
   | `{"n":1e999}` | same | same as above |
   | an object with arrays nested 200 deep | same | `Pending`, no result, no duration (unparsed count not measured) |

   The lone surrogate is the realistic case. Claude Code is a JavaScript program, so a string cut in the middle of a surrogate pair is written as a `\udXXX` escape. The scope is narrower than every such line: on `main`, a lone surrogate in `message.content` already fails the line (I checked), so this regression bites when the text is in `toolUseResult` but not in the message, such as `originalFile` of an Edit. The other inputs are unrealistic, but task 002 round 2 treated `1e400` and invalid UTF-8 as must-fix for this same kind of regression.

   Cause: `#[serde(untagged)]` on `ToolUseResult` (`src/harness/claude/wire.rs`) first buffers the whole value into serde's internal content type through `deserialize_any`. That decodes every string, range-checks every number and applies serde_json's recursion limit, and an error there fails `Entry` before any arm is tried, including the `IgnoredAny` arm. `main` skipped the field as unknown, which checks none of this. The same buffering also copies the very large values the spec wants left unmaterialised, although the budgets hold. The `read_entry` retry does not help, because `Entry<MessageWithoutUsage>` has the same field.

   The spec prescribes the head because a known field of the wrong type "fails the whole line", so a head that still fails lines defeats its purpose. The object-or-`IgnoredAny` head can stay; what is required is the outcome.

   Change: make every line that parses on `main` parse on this branch with the same result; `toolUseResult` may only add `isAsync`. Every other line's outcome stays as on `main`: for example, invalid UTF-8 in a text block or `uuid` is still counted as unparsed. There are two directions:
   - A fallback in `read_entry` (recommended): when the parse fails, parse again with `toolUseResult` ignored, in the style of `entry_with_invalid_utf8_in_usage`, for example by making the field's type a parameter of `Entry` and using `IgnoredAny`. This gives `main` parity for all the inputs above. An unreadable `toolUseResult` then counts as absent, so `is_async_launch` is `false`. Also make the existing usage retry work when `usage` and `toolUseResult` are both damaged; on `main` that line parses too.
   - A hand-written, non-buffering `Deserialize`, which also avoids the copy. I expect `deserialize_any` still decodes a top-level string, so check the lone-surrogate string case; this alone may not cover it.

   Tests: add a test in `tests/claude_assembly.rs` that feeds each input in the table as raw bytes through `Assembler` (`push_line` or `from_memory`; a `&str` cannot carry the invalid byte), and asserts the result is attached, the status is `Ok`, the duration is 3 s and `unparsed_count()` is 0. Add a case with both `usage` and `toolUseResult` damaged, and one showing that invalid UTF-8 in a text block is still counted as unparsed. Keep `an_asynchronous_launch_has_no_duration` as it is.

2. [nit] `src/bench_support/generate.rs:461`: the check `large_size + 1024 <= room` still assumes a large result is written once, but its line now holds the output twice, so the last large output can overshoot the target by up to `LARGE_MAX`. The effect was negligible here (104,861,111 bytes against a 104,857,600 target). Change: use `2 * large_size + 1024`, and keep `GENERATOR_VERSION` at 2 if this lands in the same task.

3. [nit] `src/harness/claude/wire.rs:109`: `next_key::<Cow<'de, str>>()` suggests a borrowed key, but serde's plain `Cow` impl always deserialises to an owned `String`, so each key allocates. Change: use a `String`, which says what happens, or a small key visitor if the type stays after finding 1.

4. [nit] `src/components/transcript/search.rs:958`: `a_description_row_ending_in_the_duration_is_found_as_the_source_line` asserts the wrapped case through a helper, `view_wrapped`, defined after it. Change: split the wrapped case into its own test, so a failure names the case.
