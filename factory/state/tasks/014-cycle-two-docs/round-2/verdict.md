VERDICT: APPROVE

## Assessment
All three reviews pass and only nits remain. Round 1's two must-fixes (the §9.5 robustness claim and the cause of the unfold figure) are fixed, and both the code and feature reviewers confirmed the fixes by their own probes and timing. None of the nits changes a conclusion, a budget or an acceptance criterion, so each is ignored for this merge. They are grouped below so they are not lost.

Checked for this verdict:
- The diff touches only `README.md`, `doc/SPEC.md`, `docs/performance.md` and this task's own reports. No file under `src/`, `tests/` or `benches/` changes, and no snapshot.
- The documents have not changed since the round-2 build commit, so the reviewers' gate and budget runs apply to this HEAD. I did not rerun them.
- §9.2's budget values are unchanged; only the sentence introducing them was reworded.
- The lines added to `doc/SPEC.md` contain no "cycle" or "round" labels.
- `docs/performance.md` has the sidebar key press and the details read time.

**Code Review: PASS, 10 nits.**
- 1 (`docs/performance.md` "The assembler reads none of these fields"): ignored, docs follow-up. It is older text from `main` that went stale when the assembler began reading `stop_reason`. §9.5 now has this right, and §9.5 is the binding statement.
- 2 (§5.1, §9.2 "detail fields are not parsed while assembling"): ignored. It is loose only for the few fields both readers use, §9.5 lists those, and the cycle spec uses the same wording.
- 3 (§9.2 points to §5.4 for the subagent scan): ignored, docs follow-up. I confirmed §5.4 says nothing about it and §3.1 has the linkage. The parenthesis itself is correct and readable without the pointer.
- 4 ("so it is outside the budget", also feature 4 and noted by the UI review): ignored, docs follow-up. A grammar slip that does not change the meaning.
- 5 ("10.3 ms (1.9 MB)", also feature 1): ignored, docs follow-up. The size label is wrong: both reviewers measured the slowest output at 1 573 956 bytes. The time and the conclusion hold. One probe reached 15.7 ms, so the margin can be thin, but it stayed under a frame, and this is a one-off key press outside the scrolled-frame budget.
- 6 (`docs/performance.md` "only the details read is known to be far under a frame"): ignored. Awkward wording, and the figures next to it are correct.
- 7 (`docs/performance.md:31` is 81 columns): ignored. Formatting only.
- 8 (`src/harness/claude/wire.rs` header names `message.usage` as the only exception): ignored here, code follow-up. This task may not change `src/`.
- 9 (every fold or unfold in a block holding an expanded output costs about 115 to 165 ms): ignored here, code follow-up. `docs/performance.md` now gives the cause, and fixing it needs a change under `src/`.
- 10 (§9.5 "fails its line" is not exact for `message.content`, also feature 3): ignored, docs follow-up. The `wire.rs` header and the feature reviewer's probe confirm the difference. In both cases the line is counted as unparsed; the difference is that the rest of the line is still used.

**Feature Review: PASS, 4 nits, all acceptance criteria met.**
- 1, 3 and 4: the same points as code findings 5, 10 and 4 above, and ignored for the same reasons.
- 2 (§9.2 "never loaded as a whole value", while the assembler's untagged head buffers it once): ignored, docs follow-up. Cycle 2 §3 asks for this head, no `serde_json::Value` is kept, the buffer is transient, and the probe's peak (7.94 MiB) is far under the heap budget.

**UI Review: PASS, no findings.** The task is `UI: none` and rendering cannot have changed. Its passing remark on the grammar slip is code finding 4.

**Follow-ups for the orchestrator** (neither blocks this merge):
- One small docs polish covering code findings 1, 3, 4, 5, 6, 7 and 10 and feature finding 2. The reviewers' reports give the exact wording for each.
- One code task for `Transcript::set_folded` (`src/components/transcript/nav.rs`) and the layout it triggers (code finding 9, feature round-1 finding 5), which can also update the `wire.rs` header (code finding 8).

## Conflicts
None.

## Rework budget
Earlier REWORK verdicts: 0 of 2.
