VERDICT: APPROVE

## Assessment

**Code review: FAIL**
- Finding 1 (must-fix), a duplicate `toolUseResult` key fails a line that parses on `main`: confirmed, and ignored as minor. See below.
- Finding 2 (nit), a launch with an invalid byte or a lone-surrogate key in `toolUseResult` counts as not async: ignored. The line parses and keeps its result, and the only effect is a duration on a launch. A JavaScript writer does not produce invalid bytes, and a lone-surrogate key in a launch record is not a realistic input.
- Finding 3 (nit), `unchecked_str` repeats `entry_with_unchecked_tool_use_result`: ignored. It is a small repetition with no effect on behaviour.
- Finding 4 (nit), `a_row_that_leaves_no_room_for_the_description_is_cut_as_a_whole` has no `…` foreground assertion: ignored. I read the test and it checks text only, so the round-2 build notes are wrong on this point. The behaviour is correct and covered by `row::tests::a_tail_without_room_for_the_description_cuts_the_whole_row`.
- Finding 5 (nit), `agent_type_key` differs from `subagent::agent_type`: ignored. It needs a blank `subagent_type` together with a non-blank `agent`, and the only effect is that this one value is not searched.
- Finding 6 (nit), stale or awkward docs in `wire.rs` and `transcript_search.rs`: ignored. These are wording only.
- Finding 7 (nit), test name and small test cleanups: ignored. They have no effect on coverage.

**Feature review: PASS.** All acceptance criteria are ticked.
- Finding 1 (nit), the `wire.rs` module doc: ignored, the same point as code finding 6.
- Finding 2 (nit), the build-notes claim: ignored, the same point as code finding 4.
- Finding 3 (nit), no committed search test with the duration on a row of its own: ignored. This is a test gap only. The code, feature and UI reviewers each probed this search and got the right highlights.
- Finding 4 (nit), the invalid-byte launch: ignored, the same point as code finding 2.

**UI review: PASS.**
- Finding 1 (nit), the build-notes claim: ignored, the same point as code finding 4.

**Why the must-fix is ignored.** The Code Reviewer asked the Approver to weigh it, and I agree it should not pass silently, so here is the reasoning.

I reproduced it through `Assembler::from_memory` on scratch copies of `main` and this branch:

| On the result line | Tree | Status | Duration | Unparsed |
|---|---|---|---|---|
| Duplicate `toolUseResult` | `main` | `Ok` | 3 s | 0 |
| Duplicate `toolUseResult` | Branch | `Pending` | none | 1 |
| Duplicate `toolUseResult`, first value `isAsync: true` | Branch | `Pending` | none | 1 |
| Duplicate `uuid` | `main` | `Pending` | none | 1 |
| Duplicate `cwd` | `main` | `Pending` | none | 1 |
| Duplicate `durationMs` | `main` | `Pending` | none | 1 |

The regression is real but minor, for these reasons:
- **No writer produces it.** Claude Code writes these lines with `JSON.stringify`, which cannot emit a duplicate key, as the reviewer also says. The round-1 lone surrogate was different: a JavaScript writer can produce one, and that case is now fixed and tested.
- **It is the behaviour every other declared field already has.** The table shows a duplicate of another known `Entry` field already fails the line on `main`. The line takes the documented base §9.5 path for malformed lines (skipped, counted, shown in the header note), so nothing crashes and the loss is visible.
- **The task does not require it.** The bar "every line that parses on `main`" was set by the round-1 reviews. Task 002 had a binding planner note saying this, and that note is what drove its REWORK for unrealistic inputs. I searched this task and `factory/state/SPEC.md` and found no such note. They require that a string `toolUseResult` and a wrong-typed `isAsync` do not fail the line, and both are met and tested. The task 002 verdict is only a partial precedent for ignoring a duplicate key, because the line stayed parsed in that case.
- **The fix costs more than it returns.** Every shape in `lenient_entry` declares the field, so the fix needs a further hand-kept copy of the 14-field `Entry`, which must stay in sync, as another try in the fallback chain. That is a poor trade for an input no writer produces.

One consequence remains: the `read_entry` doc says every line that parses with `toolUseResult` skipped still parses, and that overstates the behaviour for this one input. This is a doc inaccuracy and does not change the verdict.

**Independent check.**
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass. `INSTA_UPDATE=no cargo test` gives 2553 passed, 0 failed, 2 ignored. The tree stayed clean and there is no `.snap.new` file.
- The branch adds 13 snapshots and modifies one, the OpenCode browser screen. The UI and feature reviews both checked that only its four tool rows moved, each from a real adapter duration, which is intended.
- I read the `toolUseResult` code in `wire.rs` and `assemble.rs`, and the only `from_slice::<Entry>` outside the fallback is a test assertion. The round-1 must-fix inputs are all in `tests/claude_assembly.rs`.
- I did not rerun the release budgets, because nothing disputes them. The builder and two reviewers each report them within budget, with a slowest full parse of 408 ms against the 2 s budget.

Optional follow-ups, not conditions of this approval: correct the `read_entry` and module docs in `wire.rs`, and rename the test whose name refers to `main`.

## Conflicts

None. The code review's FAIL and the feature review's PASS make no opposing demands: the feature reviewer did not probe duplicate keys, and the finding is ignored on its own merits, not because of that PASS. It is not an A/B choice for the human, because the two outcomes differ only on lines no writer produces.

## Rework budget

Earlier REWORK verdicts: 0 of 2. Round 1 has no `verdict.md`, so its rework does not count.
