VERDICT: APPROVE

## Assessment

Round 4 did everything the round-3 note asked, and every acceptance criterion is met. One must-fix remains, in the feature review. I reproduced it and judge it minor, for the reasons below.

**Code review: PASS**, four nits.
- Finding 1 (nit), a string `toolUseResult` with a lone surrogate shows `unavailable`: ignored, as feature finding 1.
- Finding 2 (nit), the module comment in `details.rs` is not quite accurate: ignored. This is wording only, and it is listed as a follow-up below.
- Finding 3 (nit), the name `compose`: ignored. This is naming only.
- Finding 4 (nit), one unwrapped doc comment: ignored. This is cosmetic.

**Feature review: FAIL**, one must-fix and two nits.
- Finding 1 (must-fix), a non-object `toolUseResult` the reader cannot decode fails the line: ignored as minor. See below.
- Finding 2 (nit), a lone surrogate in `diagnostics.cache_miss_reason` shows `unavailable`: ignored. The reader does show that field, so it has to decode it, and the feature reviewer saw no such value in real transcripts. The result is the §5.3 fallback.
- Finding 3 (nit), one failure of `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` in 19 runs: ignored for this task. That file is not in the branch's diff. It is listed as a follow-up below.

**UI review: PASS**, no findings.

**Why the must-fix is minor.** I ran these lines through `Assembler::from_file` and the adapter on a private export of `HEAD` (509c6fb):

| On the result line | Assembler | Result reference | Tool call group from the adapter |
|---|---|---|---|
| String `toolUseResult` with a lone surrogate, other text in `tool_result` | parsed | yes | `start`, `unavailable` |
| The same string, same text in `tool_result` | unparsed | none | `start`, no `unavailable` |
| String `toolUseResult` with invalid UTF-8 | parsed | yes | `start`, `unavailable` |
| Bare `1e999` | parsed | yes | `start`, `unavailable` |
| Object with a lone surrogate, invalid UTF-8, `1e999` or deep nesting in a skipped member | parsed | yes | `start`, `permissionDecision`, `sandboxed` |
| Array holding a lone surrogate, or a deeply nested array | parsed | yes | `start`, `permissionDecision`, `sandboxed` |
| Lone surrogate in `cwd` | unparsed | none | `start`, no `unavailable` |
| Lone surrogate in `diagnostics.cache_miss_reason` | parsed | yes | `start`, `unavailable` |

The finding is real, and the reasons it is minor are these:
- **Two of the three inputs have no writer.** Claude Code writes these lines with `JSON.stringify`, which cannot produce `1e999` or invalid UTF-8. The 004 round-2 verdict ignored a must-fix on the same ground. `1e999` is the one input that literally breaks the planner rule on unexpected types, and that rule exists because field types vary across Claude Code versions, not to cover a value no version can write.
- **The third input is narrower than it looks.** A JavaScript writer can produce a lone surrogate, but when the same text is also in `tool_result`, the assembler already skips the line and no result reference exists. The reader is never asked, and neither proposed fix changes that case. The 004 feature review checked this skip on `main`, and this branch does not change `wire.rs`.
- **Real string results mostly repeat their text.** Of 254 string `toolUseResult` values in the 913 local transcripts (143 545 lines, read-only, counts only), 211 equal the `tool_result` text once a leading `Error: ` is ignored, 38 overlap it in one direction or the other, and 5 differ. I did not check which way the 38 overlap. The code and feature reviewers each scanned these transcripts (143 218 and 143 114 lines) and found no lone surrogate at all. I did not repeat that scan.
- **The planner's type rule does not clearly cover it.** §3 names a string as a shape this field takes, and ordinary strings work and are tested. The failure comes from decoding this value, not from its type. `serde_json` rejects the line, and the planner notes list "the line does not parse as JSON" as a cause of `unavailable`. The untagged head §3 prescribes fails on the same values, as the doc comment on `wire::ToolUseResult` says.
- **The input task 004 fixed already works here.** That was damaged text in a `toolUseResult` object, such as `originalFile`, which `message.content` does not repeat. The table shows the reader keeps its rows for it. Task 004 was also a regression in assembly, where a call lost its result. Here the transcript is unaffected.
- **The outcome is the fallback §5.3 specifies.** The overlay stays open and shows one muted `unavailable` row. What is lost is the `permissionDecision`, `sandboxed`, `resolvedModel` and `isAsync` rows of that one call.
- **The fix would not close the class.** A lone surrogate in a field the reader shows still gives `unavailable`, as the feature reviewer's own finding 2 says.

What remains is a string `toolUseResult` with a lone surrogate in text that `tool_result` does not repeat. I accept that. The exhausted budget is not the reason: rule 1 comes before the budget, and I would not open a round for this alone. This differs from the test gaps in rounds 2 and 3, where I refused to treat rarity as a reason, because a regression there would have been silent. Here the result is visible, and the spec names it.

**Why this is not an A/B choice.** The two proposed fixes differ only on invalid UTF-8 and on a duplicate key inside `toolUseResult`. Neither difference changes what a user would meaningfully see.

**The snapshot criterion.** `git diff main...HEAD` adds four App-level snapshots and changes one component snapshot, `unavailable_80x24_default`. I read that change: only the `model` row and its style line differ. Round 2 accepted it because the task's UI section asks for one row with no label column, and §7 allows changes to the rows a task intends to change. No `.snap` file changed in this round.

**Independent checks**, on the same export with its own target directory and `INSTA_UPDATE=no`:
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.
- `cargo test` gives 2852 passed, 0 failed, 2 ignored, and writes no `.snap.new` file.
- `cargo test --release -- --ignored budgets --nocapture` passes.
  - Mixed: first content worst 2.4 ms, full parse worst 140.7 ms, peak heap 0.22x, worst frame 4.99 ms, details of the largest tool result (2 MB) 1.6 ms.
  - Dense: first content worst 1.9 ms, full parse worst 440.1 ms, peak heap 0.63x, worst frame 1.57 ms.
  - One endless turn: 695.7 ms.
- The merge base equals `main`.

**Optional follow-ups, not conditions of this approval.** A search of `factory/state/backlog.md` and the `task.md` of tasks 009 to 014 finds none of these, so they still need a home:
- **The `toolUseResult` decoding limit.** The feature reviewer's fix and test are in `round-4/feature-review.md`, finding 1. Of the two fixes it is the one that also forgives invalid UTF-8, matching what the assembler forgives for this field. The reviewer tried it on a private copy and I did not. If it is ever wanted, correct the `details.rs` module comment with it (code finding 2).
- **The real load path on one long turn.** Round 2's feature review measured `ClaudeHarness::load` on the 70 MB single turn at 3.4 s on this branch and 4.0 s on `main`, over the 2 s budget on both. Those were single runs, the cause was not profiled, and I have not measured it myself.
- **The control-character filter in `display_lines`** (`src/components/details_overlay/content.rs`), recommended by the 007 round-4 verdict.
- **The possible flake in `a_stale_result_after_a_revert_to_the_sent_key_is_dropped`** (`tests/index_stub.rs`). Reviewers keep seeing rare failures, its message has not been captured, and I did not reproduce it.

## Conflicts

The code and feature reviews agree on the behaviour and differ only on severity: nit against must-fix. They make no opposing demands, so the priority order does not decide this. I judged it on the merits above, and the result happens to match the code reviewer's rating.

## Rework budget

Earlier REWORK verdicts: 2 of 2 (rounds 2 and 3). Round 1 has no `verdict.md`, and `round-3/approver-review.md` and `round-4/approver-review.md` are the copied notes, not verdicts. No rework rounds remain.
