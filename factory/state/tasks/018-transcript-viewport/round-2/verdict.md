VERDICT: APPROVE

## Assessment
All three round-2 reviews PASS. Round 1 had two must-fixes and no security findings. The first was the feature must-fix: a tall off-screen selection was paged blindly by `Cmd::Move`. The second was the UI must-fix: a selection that moved by itself was left off screen after `show_system = false` or an `Updated` turn. Both are fixed, and each has a test that fails without the fix. The code reviewer, the feature reviewer and the UI reviewer all re-ran the test gate (fmt, clippy, `INSTA_UPDATE=no cargo test`), and it is green. All acceptance criteria are met. Every remaining finding is a nit, and I ignore each one for the reason given below.

- **Code review: PASS.**
  1. [nit] `Window::build_clamped` builds twice when the offset needs clamping. The first build uses up the reused layouts, so a `Cmd::Scroll(Down)` at the bottom of the transcript lays out the visible window again. I confirmed this in `src/components/transcript/window.rs`: `build` runs before `clamp_offset`. **Ignored.**
     - It costs performance only and does not affect correctness.
     - The cost is at most one viewport's worth of block layouts per key press, and only when the offset is clamped at the end of the transcript.
     - It is not a regression. Round 1 rebuilt the whole window on every scroll.
     - No acceptance criterion covers it. The counter criterion is about `view`, and `drawing_a_frame_runs_no_layout_when_prepared` still holds.
     - The 16 ms frame budget is measured by the later benchmark task.
     - Suggested follow-up for task 020 (key bindings) or the benchmark task: clamp first, then build once, and add the counter assertion for a scroll at the bottom.
  2. [nit] `measured_reusing` writes types inline and duplicates `measured`. **Ignored.** Style only.
  3. [nit] `let mut exposed` in a test. **Ignored.** Style only.
  4. [nit] The early-return branches of `restore_selection` do not call `reveal_selection`. **Ignored.**
     - The feature reviewer notes that the second branch cannot be reached in practice.
     - The first branch only applies when nothing was selected, which means the transcript had no selectable section at all.
     - In that case, revealing the new selection would also pull the view away from where the user had scrolled, so leaving it out is defensible.
- **Feature review: PASS.** Every acceptance criterion is checked off.
  1. [nit] The same `build_clamped` double build as code nit 1. **Ignored**, as above.
  2. [nit] When the partly visible top block is itself replaced and grows, the screen shifts. **Ignored.** The spec does not specify this behaviour, and the reviewer suggests task 022.
  3. [nit] The same as code nit 3. **Ignored.**
  4. [nit] The same as code nit 4. **Ignored.**
- **UI review: PASS.** All required snapshots are present and unchanged, and the new `before_a_resize_at_120x24_text` snapshot was added. The UI reviewer compared 400 interleaved steps with a forced full rebuild and found no mismatches.
  1. [nit] A user block whose only content is injected items is drawn as an empty 3-row shell when `show_system = false`. **Ignored.** The shell comes from `TurnBlock` (task 017), which is outside this task's scope. It is deferred on purpose and noted for a later task.

## Conflicts
None. The code reviewer and the feature reviewer describe how reachable nit 4 is in different ways, but both rate it a nit.

## Rework budget
Earlier REWORK verdicts: 0 of 2. Round 1 has no `verdict.md`.
