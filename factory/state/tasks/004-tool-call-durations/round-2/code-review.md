VERDICT: FAIL

## Summary
The round-1 must-fix is fixed for every input in its table, but its required behaviour, that every line parsing on `main` still parses and attaches its result, is still not met: a line with a duplicate `toolUseResult` key parses on `main` and fails on this branch. The other round-1 findings and the two UI nits are fixed, apart from one test assertion the build notes wrongly report as added (finding 4). The test gate and the release budgets pass.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

Also re-run: `cargo test --release -- --ignored budgets --nocapture` passes. Mixed: first content 2.5 ms, full parse 142.4 ms, slowest frame 4.89 ms, peak heap 0.22x. Dense: first content 1.7 ms, full parse 408.4 ms, slowest frame 1.51 ms, peak heap 0.58x. The fixtures contain `toolUseResult`.

No `.snap.new` files are committed, and the gate left the working tree clean. The only modified existing snapshot is still the OpenCode browser screen reviewed in round 1. This round adds one snapshot (`unfolded_call_keeps_a_long_duration_in_one_piece_at_40`) and changes `unfolded_call_wrapped_at_40_keeps_the_duration_muted_styled` in one invisible style run, as the build notes say.

I probed these in scratch copies of `main` and the branch, with extra tests that were not committed:
- A line with a duplicate `toolUseResult` key, through `Assembler::from_memory` on both trees (finding 1).
- `read_entry` with a launch whose `toolUseResult` is damaged: a lone surrogate value and `1e999` stay a launch; an invalid byte and a lone-surrogate key count as not async (finding 2).
- A line with a wrong-typed `uuid` and a damaged `toolUseResult` stays unparsed, as on `main`.
- `row::wrapped` and `row::folded` with empty and blank text at widths 0 to 12: no panic, so the `expect` at `row.rs:98` holds.
- Search on a block at width 40 with the duration on its own row: `5m 2s` and `· 5m` are highlighted on that row, and `camera` and `cluster` on theirs.

I also checked that nothing bypasses the fallback: `wire::Entry` is only read through `read_entry`, in `assemble.rs` and `extract_result_text`, and `scan.rs` calls `read_entry` too, so discovery gets the same fallback.

## Findings
1. [must-fix] `src/harness/claude/wire.rs:240-265` (`read_entry`, `lenient_entry`) — a line with a duplicate `toolUseResult` key parses on `main` and fails here. The call loses its result and stays `Pending` for good, and the line is added to the `N entries could not be parsed` note.

   **Evidence.** The same two-line session through `Assembler::from_memory` on exports of `main` and this branch, with a clean `message.content` of `done`:

   | On the result line | `main` | Branch |
   |---|---|---|
   | `"toolUseResult":{"a":1},"toolUseResult":{"a":2}` | `Ok`, result, 3 s, unparsed 0 | **`Pending`, no result, no duration, unparsed 1** |
   | `"toolUseResult":{"isAsync":true},"toolUseResult":{"a":2}` | `Ok`, result, 3 s, unparsed 0 | **`Pending`, no result, no duration, unparsed 1** |
   | `"toolUseResult":{"a":1}` | `Ok`, result, 3 s, unparsed 0 | `Ok`, result, 3 s, unparsed 0 |

   **Cause.** The derived `Deserialize` rejects a repeated declared field with `duplicate field`. On `main` the field is undeclared and so skipped. Every retry in `lenient_entry` still declares it, so choosing another `T` (`Box<RawValue>`, `IgnoredAny`) cannot help.

   **Why this blocks.** Round-1 finding 1 set the bar: "Every line that parses on `main` must still parse and attach its result. Reading `toolUseResult` may fall back to 'absent', and must never fail the `Entry`." The `read_entry` doc on this branch claims the same, so the doc is also wrong.

   **Realism.** This input is unrealistic: `JSON.stringify` cannot write a duplicate key, and I did not check real transcripts. So are `1e999` and 200-deep nesting, which round 1 required regardless. Every other known `Entry` field already fails this way on `main`, which is the argument for accepting it; the difference is that those fields do not regress. The Approver may weigh this, but it should not pass silently below the bar round 1 set.

   **Change, code.** Add a last-resort parse in `lenient_entry` into an entry shape that does not declare `toolUseResult`, mapped to `tool_use_result: None`. Run it after the existing tries fail, so that a line failing for any other reason stays unparsed. The invalid-UTF-8 branch needs it too: its check `Entry<MessageWithoutUsage, IgnoredAny>` and the two parses after it all declare the field, so a duplicate key plus invalid UTF-8 in `usage` must be handled as well. A duplicate key counts as absent, so the launch in the table's second row has a 3 s duration, as on `main`.

   **Change, tests.** In `tests/claude_assembly.rs`, add both duplicate rows to `a_tool_use_result_that_main_skips_does_not_fail_the_line`, with unparsed 0, status `Ok`, the result attached and 3 s. Add a duplicate key with invalid UTF-8 in `usage` to `a_damaged_tool_use_result_and_a_damaged_usage_in_one_line_still_parse`. Add a duplicate key with invalid UTF-8 in the text block to `invalid_utf8_elsewhere_than_tool_use_result_and_usage_is_still_unparsed`, with unparsed 1.

2. [nit] `src/harness/claude/wire.rs:269-276` (`entry_with_unchecked_tool_use_result`) — a launch with an invalid byte or a lone-surrogate key in its `toolUseResult`, such as `{"isAsync":true,"prompt":"\xff"}`, counts as not async, so the launch shows a duration. The result itself is kept and the line parses, so this meets the round-1 bar, and the round-1 feature review accepted "unreadable counts as absent". The both-damaged path is inconsistent with this: it reads the lossy text, so with damaged `usage` too, the invalid-byte launch stays a launch. Change: for the invalid-byte case, try `unchecked_str(&String::from_utf8_lossy(line))` before the `IgnoredAny` arm, and add that row to the test.

3. [nit] `src/harness/claude/wire.rs:278-286` — `unchecked_str` repeats `entry_with_unchecked_tool_use_result`. Change: remove it and call `entry_with_unchecked_tool_use_result(text.as_bytes())`, because `from_slice` and `from_str` agree on valid UTF-8.

4. [nit] `src/components/turn_block/durations.rs:248` — round-1 code-review nit 4 is not done as the build notes report. `a_row_that_leaves_no_room_for_the_description_is_cut_as_a_whole` still checks text only, with no `…` foreground assertion. The behaviour is covered at unit level by `row::tests::a_tail_without_room_for_the_description_cuts_the_whole_row`. Change: add the `distinct_theme()` foreground assertion to that test, or correct the build notes.

5. [nit] `src/transcript_search.rs:317-329` (`agent_type_key`) — it repeats the logic of `subagent::agent_type` and differs on one input. With a blank `subagent_type` and a non-blank `agent`, `agent_type` gives `None` (`str_field` stops at the first string), so the row draws no agent type, but `agent_type_key` picks `agent` and filters it out, so that value is neither drawn nor searched. Change: have one helper in `subagent.rs` return the key and value, and use it for both.

6. [nit] Stale or awkward documentation. Change:
   - `src/harness/claude/wire.rs:3-8` (module doc): it names `message.usage` as the one exception to "a known field with a different JSON type fails the line". Add `toolUseResult`.
   - `src/transcript_search.rs:268-272` (`section_text` doc): the bullet has one overlong line. Re-flow it.

7. [nit] `tests/claude_assembly.rs` — small test cleanups. Change:
   - Rename `a_tool_use_result_that_main_skips_does_not_fail_the_line`, because `main` will mean something else once this merges. For example, `a_damaged_tool_use_result_does_not_fail_the_line`.
   - In that test's loop, `assert_result_and_duration(&c, duration)` does not name the failing case. Pass `what`, or assert with a message.
   - In `invalid_utf8_elsewhere_than_tool_use_result_and_usage_is_still_unparsed`, `bytes.clone()` is not needed, and `let damaged = ...; let mut bytes = damaged;` can be one statement.
