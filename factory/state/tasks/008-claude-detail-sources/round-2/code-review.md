VERDICT: FAIL

## Summary
Both round-1 must-fix findings are fixed: assembly is linear again, and the budget test catches a re-added scan (36.9 s against the 2 s budget, checked in a scratch copy). The reference links from round 1 now each have a test that fails when the link is removed. The code behaves correctly everywhere I probed, but four more required reference behaviours have no test that fails if they break, so the review fails on test coverage only.

Note for the Approver: `unavailable_80x24_default` is a task-007 component snapshot, so changing it goes beyond the acceptance criterion that limits snapshot changes to App-level ones. Round 1's UI review asked for this change, based on the task's UI section ("no label column") and SPEC §7. Only the `model` row and its styles changed, as intended, so I do not raise it as a finding.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass
- cargo test --release -- --ignored budgets --nocapture (also required by §7): pass. Mixed: full parse 122.6 ms, peak heap 0.22x, worst frame 3.19 ms. Dense: full parse 314.2 ms, peak heap 0.63x, worst frame 0.96 ms. One endless turn (70 MB): 671.3 ms. Details of the largest tool result: 1.3 ms.

## Findings
For findings 1 to 4, I broke the line named below in a scratch copy of HEAD and ran the full `cargo test`. Nothing in the existing suite failed. The proposed test passes on HEAD and is the only test that fails with the line broken.

1. [must-fix] `src/harness/claude/assemble.rs:862` — the `stopped` flag passed to `note_message_line` has no test through the assembler. The planner notes bind the Message group to "the message's latest line with a non-null `stop_reason`, or else its latest line". Replacing `stop_changed` with `false` makes a section read the latest line even when it has no `stop_reason`, and the suite still passes. The existing test ends each message on its stopped line, so stopped and latest are the same line there, and the unit test in `src/model/sources.rs` does not exercise the assembler's call. None of the 53 local transcripts I sampled (47 787 lines) has a null line after a stopped one, so this protects a binding rule and does not fix a bug seen in practice. Change: add a test to `tests/claude_details.rs` with `prompt_line()`, `first_line()`, `call_line()`, then a third line of `m1` with `stop_reason: null`, a later `timestamp` and a new text block. Assert that the Message group of `m1:0` has `stop_reason=tool_use` and `timestamp=2026-01-01T00:00:02+00:00`.

2. [must-fix] `src/harness/claude/assemble.rs:1040` — nothing tests that the footer reads its **last** `turn_duration` line, as the planner notes require. SPEC §2.3 says a turn can hold many of these entries. Making `set_footer` keep the first line leaves the suite passing, because `the_footer_reads_its_turn_duration_line_for_the_environment_only` has only one such line. Change: add a test to `tests/claude_details.rs` with two `turn_duration` lines in one turn and different `gitBranch` values. Assert that `SourceKey::Footer` reads the Environment of the second.

3. [must-fix] `src/harness/claude/assemble.rs:567` — nothing tests the reference of injected entries held before a prompt, which become the first item of a User turn (`start_user_turn`). Removing `held.add_to` leaves such a section with no source, so it loses its Environment group, and the suite still passes. The planner notes say a system section references its first entry's line, and §5.3 gives Environment to every section with a source. The other three paths for held entries (lines 609, 635 and 813) have tests, but this one, which a hook written before the first prompt reaches, does not. Change: add a test to `tests/claude_details.rs` with `hook("h0", "before-prompt")`, `prompt_line()`, `first_line()`. Read the first item of `turns[0]` through `section_source` and assert `gitBranch=before-prompt`.

4. [must-fix] `src/harness/claude/assemble.rs:963` — nothing tests that a tool call sent again in the same turn keeps its entry in the table. Removing the `Some(known) if known.turn == b.idx => known.source` arm adds a second entry for the call. The result still goes to the first entry, while `TurnSources::get` finds the last, so the overlay loses the rows read from the result's line: `permissionDecision`, `sandboxed`, `resolvedModel` and `isAsync`. The suite still passes. The task requires these rows to come from the result's line, and SPEC §2.5 expects repeated lines. Change: add a test to `tests/claude_details.rs` with `prompt_line()`, `first_line()`, `call_line()`, `call_line()` again, then `result_line(json!({"isAsync": true}))`. Assert that the Tool call group of `c1` is not unavailable and has `sandboxed=false` and `isAsync=true`.

5. [nit] `tests/claude_details.rs:588-591` — `assembling_a_long_turn_takes_time_in_proportion_to_its_items` catches a re-added scan only some of the time. With the scan back in `add_message_item`, it failed 3 of 5 runs, because 24x sits too close to what the debug build measures. It did not fail falsely on HEAD in 14 runs, 6 of them under full CPU load. `budgets` remains the reliable check. Change: widen the gap, for example compare 3 000 and 48 000 items with a limit of about 48x, or drop this test and rely on `budgets`. Round 2's build notes should also not present it as a dependable catch.

6. [nit] `src/harness/claude/details.rs:86-95`, `:142-146`, `:150-153` — three alternative field locations in the planner notes have no test: `diagnostics` at the top of the line, `permissionDecision` in `toolUseResult`, and `resolvedModel` at the top of the result's line. Removing each one leaves the suite passing. In the local transcripts these locations do not occur (`diagnostics` is always under `message`, `permissionDecision` at the top, `resolvedModel` in `toolUseResult`), and those real locations are all tested, so this is not a blocker. Change: add one adapter test that sets all three and asserts the rows, including that top-level `diagnostics` wins over `message.diagnostics`.

7. [nit] `src/harness/claude/assemble.rs:963` and `:1101` — two behaviours around repeated calls and results have no test: a call id seen in an earlier turn gets its own entry (round 1's nit 4), and `has_result` moves the reference on a repeated identical result. Removing either leaves the suite passing. Neither is named in the spec or the planner notes. Change: add a test for the same id in a later turn. If no behaviour depends on `has_result`, remove it.

8. [nit] `src/harness/claude/assemble.rs:700` — a compaction summary without a boundary has no test for its own line. Change: add a small test asserting its `SourceKey::Own` reads that line's Environment.

9. [nit] `src/harness/claude/details.rs:226` — the empty-string filter in `Rows::time` is untested, because `an_empty_string_is_no_value` covers `Rows::text` only. Change: add `line["timestamp"] = json!("")` to a line read by that test and assert there is no `timestamp` row. The assembler must still accept that line.

10. [nit] `tests/budgets.rs:259` — the comment says "~85 MB", but the fixture is 70 MB (70 022 359 bytes). Change: say "~70 MB".

11. [nit] `tests/claude_details.rs:392` — the local `hook` closure duplicates the module-level `hook` function added at line 422. Change: delete the closure and use the function.
