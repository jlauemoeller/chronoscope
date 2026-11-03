VERDICT: APPROVE

## Assessment
All three reviews are PASS with nits only, and no finding needs another round or a human decision. I read the core diff (`src/app/sidebar.rs`, `src/app/browser.rs`, `paging.rs`, `turn_sidebar/mod.rs`, the keymap, the help content, the budgets) and found nothing the reviews missed. I did not run the test gate myself: all three reviewers ran `cargo test`, and the commit after theirs touches only `factory/`.

**Code review: PASS.** Seven nits, all ignored.
1. The README chord sentence (lines 259-261) still names only `browser` and `selector`. This task made it stale, but it is one sentence about a chord binding no default uses. Task 014 exists to bring the README up to date, so fix it there.
2. `]` / `[` call `turn_stop` twice for the block they land on. This is a small repeat of work on a single block, and sidebar key presses, which render a block the same way, measured about 2.5 ms against the 16 ms budget.
3. No test pins the append while following. See the ruling below.
4. `tests/budgets.rs` prints `SIZE` while the scenario lays out `FRAME_SCREEN`. Both are 200x50, so the output is correct today.
5. The test name `the_selector_hidden_and_back_wide_...` says "selector" but opens the help overlay. Naming only.
6. The helper `comment_only` builds a link reference definition, not a comment. Naming only.
7. `selected_msg` sits in its own `impl` block. Placement only.

**Feature review: PASS.** Five nits, all ignored.
1. `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` failed once, then passed on every rerun. Neither `tests/index_stub.rs` nor `src/index` is in this branch's diff, and the same intermittent failure is recorded in task 008, so this task did not cause it.
2. No test notices if `go_to_turn` stops calling `settle_temp_unfold`. The call is correct today and is the same epilogue as `perform`. This is a test gap, not a defect.
3. The same append-while-following case as code review finding 3. See the ruling below.
4. The HTML comment example in SPEC §6.4 and in the comment at `section.rs:193` is wrong: `<!-- x -->` renders a row. That line is not in this branch's diff, and the behaviour is met either way, because the no-rows path is generic and is tested with a link reference definition. Making the renderer drop HTML comments would be a separate behaviour change outside this task. Correcting the example can go with task 014.
5. The same test name as code review finding 5.

**UI review: PASS.** Four nits, all ignored.
1. The transcript moves when a turn arrives while it is following and the sidebar has focus. This is the case the reviewer asked me to read. Ruling below.
2. A very long remapped key label is cut by the border in the sidebar title. It takes an unusually long chord, nothing breaks, and the default title is correct.
3. The hint row goes blank or loses its "switch to transcript" hint when `next_pane` (and `open`) are unbound. This only follows from a user unbinding those keys, the other keys still work, and the behaviour matches the planner's `turns_hints` wording.
4. `Start` and `End` can stay out of view when the first or last entry is selected by keys. This matches §6.1 ("only as far as needed to keep the selected row visible"), and the list cursor predates this task.

**Ruling on the append while following (code 3, feature 3, UI 1).** The behaviour stands. With the sidebar focused and the transcript following at the bottom, an appended turn scrolls the transcript and moves its selection, while the sidebar's row stays on its entry. The acceptance criterion says "without moving the selection or the transcript", but the spec settles this case:
- SPEC §6.4 says the usual follow rule applies, and base spec §7.2 says content at the bottom scrolls into view.
- SPEC §6.7 asks only that an appended turn not move the selection, and that a following transcript's selection is tracked.
- The planner note stops only the sidebar's selection from following while it has focus, not the transcript.

SPEC.md wins over the criterion's looser wording, which fits the away-from-the-bottom case that its test covers. This branch does not change the follow code, so the behaviour predates it. Freezing the transcript while the sidebar has focus would contradict §7.2 and no spec text asks for it, so this is not an A/B choice for the human. The missing at-bottom test is a nit and can be picked up later.

## Conflicts
None.

## Rework budget
Earlier REWORK verdicts: 0 of 2.
