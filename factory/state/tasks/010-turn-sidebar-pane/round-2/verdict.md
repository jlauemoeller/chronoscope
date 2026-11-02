VERDICT: REWORK

## Assessment

The behaviour is right. Every acceptance criterion is met, both round-1 must-fixes are fixed, and nobody found a wrong result. What blocks is one binding test rule on the new `BackgroundFill` component. The fix is test-only and has one obvious shape, so this is a rework and not a human choice.

**Code review: FAIL**, one must-fix and three nits.
- Finding 1 (must-fix), `BackgroundFill` has no `insta` snapshot and no check of `on()`: to be fixed. See below.
- Finding 2 (nit), short line in the `hit_test` doc comment: included, because the round is happening anyway and it is one sentence.
- Finding 3 (nit), `entry_at_row`, `Row::selectable` and `TurnSidebar::offset` are `pub` with only test callers: ignored. The reviewer asks for no change now. If task 013 does not use `entry_at_row`, that task removes it.
- Finding 4 (nit), notes for later tasks: ignored here, nothing can show them yet, and they are already handled or moot.
  - Task 012's planner notes already name the trap of focus remembered in `focus_stack` and of hiding by `EmptySession`.
  - Task 013's planner notes make the wheel move the selection, so `sync` re-revealing the selected row does not undo it.
  - The `Start` / `End` question is below, under UI finding 1.

**Feature review: PASS**, four nits.
- Finding 1 (nit), the same gap as code finding 1: overruled and fixed. See below and under Conflicts.
- Finding 2 (nit), removing the deselection in `TurnSidebar::sync` fails no test: included. I reproduced it: with those lines deleted, `cargo test --lib turn_sidebar` gives 63 passed. It is the only mutant that survived the reviewer's 26. The doc comment on `selected` promises this behaviour, and without it a turn regaining its entry could silently become selected again.
- Finding 3 (nit), the test list `BROWSER_IDS` has three ids: included. It is one constant, and it checks the new ids directly in the test whose name says the browser is mounted.
- Finding 4 (nit), the same as code finding 2: included.

**UI review: PASS**, two nits, both ignored for this task.
- Finding 1 (nit), the first and last entries leave `Start` and `End` hidden on a long list: it matches "only as far as needed" in the spec, and nothing moves the selection in the running app yet. It is a question for whoever reviews task 012, which adds the keys and does not settle it in its task file.
- Finding 2 (nit), `ListCursor::set` resets the offset on updates when nothing is selected: nothing scrolls the pane in this task, and task 013's wheel moves the selection rather than scrolling freely.

**Why the must-fix is not minor.** The impact today is nil, and I want to be plain about that. The fill's only output is already asserted cell by cell, its `on()` is one `None`, and the gap appears in the browser's styled snapshots. I still do not ignore it:
- **The rule binds.** `factory/state/SPEC.md` lists §9.6 among the base rules that "bind every task and every review". §9.6 says every component is rendered and "snapshotted with `insta`", and that its events are checked. Good cell assertions do not answer either demand.
- **It is the only exception.** I checked every `AppComponent` in `src/components/`. Each one that draws has a snapshot and a check of `on()`, in its own file or a sibling, including the trivial `WorkingDir`, `EmptyState` and `SelectorFrame`. `BackgroundFill` is the only one with neither. The code reviewer's claim holds.
- **The precedent fits.** 006, 008 and 009 round 3 reworked test gaps in correct code while budget remained.
- **It costs little.** The note below is a few lines of tests, already tried.

**Independent checks**, on a private export of `HEAD` (8fbdca1) with its own target directory and `INSTA_UPDATE=no`. The repo was not touched.
- With the rework note applied, `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.
- `cargo test --lib` gives 2595 passed and 2 failed. The two are the new snapshot tests, which fail only because their `.snap` files do not exist yet under `INSTA_UPDATE=no`. The diff insta printed for the default theme is what the note expects.
- With the deselection deleted from `sync`, the strengthened tests fail exactly the two named in the note. With the original tests, the same mutant passes all 63.
- I did not generate the two `.snap` files or rerun the integration tests or the budgets. All three reviewers report the gate as passing on this code and the code and feature reviewers report the budgets within limits, which nobody disputes, and the note changes only tests and one comment. The builder reruns them as part of the gate. The code reviewer's worst sidebar frame of 3.11 ms is higher than the other reports of about 0.1 ms, and is still far under the 16 ms budget.

## Conflicts

One, on code finding 1 and feature finding 1. The code reviewer rates the gap must-fix and wants two snapshots and an events test. The feature reviewer rates it a nit, calls the snapshot optional, and left the stricter reading to the Approver. Correct, idiomatic, best-practice code outranks feature implementation, and §9.6 binds, so the note follows the code reviewer. The "snapshot is optional" demand is dropped.

## Rework budget

Earlier REWORK verdicts: 0 of 2. Round 1 has no `verdict.md`. This verdict is the first REWORK, so one remains.

## Rework note

The sidebar works and no production behaviour changes in this round. Make these four test and comment changes, and nothing else.

1. **`BackgroundFill` tests, the required change (base spec §9.6).** In `mod tests` of `src/components/background_fill.rs`:
   - Add a test `the_fill_in_the_default_theme` with `insta::assert_snapshot!(buffer_to_styled(&draw(builtin::default_theme())));`.
   - Add a test `the_fill_in_a_transparent_theme` doing the same with `builtin::default_theme().with_transparent_background(true)`. Import `buffer_to_styled` from `crate::test_support`.
   - Keep the existing `draw` helper, so each snapshot shows painted and unpainted cells. Review both new files. In the default theme, rows 1 and 2 of the styles have the background on `1..2` only, and rows 0 and 3 are all `Reset`. In the transparent theme, every row is `0..5 Reset/Reset/NONE`.
   - Add a test `it_ignores_events_and_commands`: `fill.on(&event)` is `None` for `Event::Tick`, a key event and a raw `Event::Mouse` event; `fill.perform(Cmd::None)` is `CmdResult::Invalid(Cmd::None)`; `fill.state()` is `State::None`. `src/components/working_dir.rs` shows the pattern, and `src/components/harness_tabs.rs` shows how to build the raw mouse event. Keep `Invalid` in the production code, as `TurnSidebar` does: the assertion follows the code, even though `WorkingDir` returns `NoChange`.
   - The existing cell-by-cell tests stay. The feature review's "the snapshot is optional" is dropped, because the code review outranks it and §9.6 binds.

2. **Deselection tests.** In `src/components/turn_sidebar/tests.rs`:
   - In `a_selected_turn_that_loses_its_entry_is_deselected`, after the existing assert, send `update(updated(vec![assistant("a1", 9)]))` through `MODEL_UPDATE_ATTR` and assert `selected_entry()` is still `None`.
   - In `a_selected_turn_that_is_not_in_new_rows_is_deselected`, after the first assert, push `rows_of(&wireframe())` through `TURN_ROWS_ATTR` and assert `selected_entry()` is still `None`.

3. **Test list.** In `src/app/browser_tests.rs`, make the test constant `BROWSER_IDS` `[Id; 5]` by adding `Id::TurnSidebar` and `Id::SidebarGap`. Do not add `Id::EmptySession`.

4. **Doc comment.** In `src/app/mouse.rs`, rewrap the last sentence of the doc comment on `hit_test` so its two lines read:

   ```
   /// Components under the topmost overlay are not candidates, even where the
   /// overlay does not cover them.
   ```

Leave everything else as it is. The review notes for tasks 012 and 013 and the UI question about revealing `Start` and `End` are not for this round.

Done when:
- `git diff 8fbdca1..HEAD -- src tests` shows only the test changes above, the one doc comment, and exactly two new `.snap` files under `src/components/snapshots/`. No existing snapshot changes, and no `.snap.new` or `.pending-snap` file is committed.
- With `INSTA_UPDATE=no`, `cargo test --lib background_fill` passes with six tests.
- Deleting `if row.is_none() { self.selected = None; }` from `TurnSidebar::sync` makes `cargo test --lib turn_sidebar` fail exactly `a_selected_turn_that_loses_its_entry_is_deselected` and `a_selected_turn_that_is_not_in_new_rows_is_deselected`. Restore the lines afterwards.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `cargo test --release -- --ignored budgets --nocapture` pass.
