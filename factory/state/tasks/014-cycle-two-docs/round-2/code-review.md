VERDICT: PASS

## Summary
All nine round-1 code review findings are fixed, including both must-fixes: `doc/SPEC.md` §9.5 and §3.1 now match the parsers, and `docs/performance.md` gives the right cause for the 116 to 173 ms unfold figure, which I confirmed by measuring it. The diff still touches only `README.md`, `doc/SPEC.md`, `docs/performance.md` and the task's own reports, and the gate and budgets pass. What remains is small wording and precision nits, plus two follow-ups outside this docs-only task.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

Also run: `cargo test --release -- --ignored budgets --nocapture` passes. An instrumented scratch copy of `tests/budgets.rs` gave these figures for the mixed session:
- Worst plain unfold before any expansion, three runs: 10.4 to 11.0 ms. A fourth probe, which timed every fold in that block, measured the same output at 15.7 ms, still under a frame but close to the 16 ms line.
- Reference unfold line after the expansion: 115 to 121 ms.
- Expansion: 37 to 56 ms.
- Details read: 1.2 ms.

The endless turn took 670 to 686 ms in the four instrumented runs and 1336 ms in one earlier run, all within the 2 s budget. I did not rerun `cargo bench`: no code changed since round 1, whose review agreed with the page's table. No file under `src/` or `tests/`, and no snapshot, differs from `main`, and no `.snap.new` file is tracked.

## Findings
1. [nit] `docs/performance.md:150-152`. "The assembler reads none of these fields, so a line with detail fields of any type or size assembles as before" is no longer true. The sentence's list at lines 146-147 names `stop_reason`, which the assembler now reads (`Message.stop_reason`, `src/harness/claude/wire.rs:351`), as `doc/SPEC.md` §9.5 says. Other detail fields are read too: `timestamp`, `message.id` and `cwd`, and a wrong type in those fails the line. Change: "The assembler reads almost none of these fields and keeps none of them for the overlay (the exceptions are in SPEC §9.5), so a line with the other detail fields of any type or size assembles as before."

2. [nit] `doc/SPEC.md:338` (§5.1, "assembling a turn never parses the detail fields") and `doc/SPEC.md:810` (§9.2, "Detail fields are not parsed while assembling"). Both are loose in the same way. `message.id`, `timestamp`, `stop_reason`, `cwd` and `isAsync` are listed as detail fields in §3.1 and are also parsed by the assembler, as §9.5 now shows. The cycle spec uses the same wording (`factory/state/SPEC.md:239`), so this is not a must-fix. Change: in both places say "the fields only the details overlay shows are not parsed while assembling".

3. [nit] `doc/SPEC.md:810` (§9.2). The new parenthesis points to §5.4 for the subagent scan, but §5.4 (`doc/SPEC.md:434-438`) says nothing about it. The linkage is described in §3.1 (`doc/SPEC.md:140`). Change: point to §3.1 instead of §5.4, or add the fallback to §5.4.

4. [nit] `docs/performance.md:103-105`. The rewrite left a grammar mismatch and a short line: "They are not scrolled frames, so it is outside the budget, / and the frames after it are as fast as before." Change: "They are not scrolled frames, so they are outside the budget, and the frames after them are as fast as before.", then rewrap the bullet.

5. [nit] `docs/performance.md:89-90`. "the slowest unfold ... took 10.3 ms (1.9 MB)": in three runs of `worst_unfold` before the expansion, the slowest was the 1 573 956 byte output each time (10.4 to 11.0 ms). The time is about right, but the size is not reproducible. That output is in the same turn as the scenario's unfolded large output, and a fourth probe measured it at 15.7 ms, so "up to about 10 ms" is typical rather than an upper bound, and the margin to a frame can be thin. Change: "(1.5 MB)", or drop the size, and say "usually about 10 ms, and 15.7 ms in the slowest run measured".

6. [nit] `docs/performance.md:185-186`. "and only the details read is known to be far under a frame (the unfold is up to 10 ms)" reads as if the unfold's time were in doubt. Change: "It runs on the UI thread, like unfolding a large output; the details read is far under a frame, and the unfold (up to about 10 ms) is under one."

7. [nit] `docs/performance.md:31` is 81 columns, one past the wrap of the surrounding prose. Change: rewrap.

8. [nit] Follow-up outside this task, because it may not change `src/`: `src/harness/claude/wire.rs:3-8` says `message.usage` is "the exception" to a wrong type failing the line. `message.stop_reason`, both `isApiErrorMessage` fields and `toolUseResult` are lenient too, as §9.5 now states correctly. Change: in a later task, update the module comment to match §9.5.

9. [nit] Follow-up outside this task, adding to round-1 feature review finding 5, which took the cost for a slow re-read of the result line. It is not specific to large outputs. Once a block holds an expanded output, every fold or unfold in that block re-lays it out: in one instrumented run, the six slowest unfolds were of small outputs and took 144 to 164 ms (163 ms for a 104-byte one), and folding them again took 118 to 165 ms. That is about three times the expansion itself (56 ms in that run). The page does not explain the difference, and I did not trace it. Change: the orchestrator may want a task to look at `Transcript::set_folded` (`src/components/transcript/nav.rs:255-266`) and the layout it triggers.

10. [nit] `doc/SPEC.md` §9.5 (the "While assembling" sentence, in the second bullet of the section). "A known field of another JSON type fails its line" overstates what happens for `message.content` and the fields inside its blocks. These fall to `Content::Other` (`src/harness/claude/wire.rs`), so `check_blocks` counts the line as unparsed (`src/harness/claude/assemble.rs:496`), but `on_assistant` goes on and the line's `message.id`, `model`, `usage` and `stop_reason` still feed the turn. I read this from the code and did not probe it. Change: add "(inside `message.content`, only the content is lost: the line is counted, and the rest of it is still used)".
