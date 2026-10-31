VERDICT: PASS

## Summary
Round 2 fixes every round-1 must-fix and the nits it took on. I verified each fix:
- **Feature must-fix 1.** `R` with the Projects pane focused now resumes the session that is still selected (`ProjectList` -> `SelectorMsg::Resume` -> `App` resolves `selector.current` + `selector.session`). It still does nothing with no session selected.
- **UI nit 1.** The missing-cwd toast is sized for the real toast box (`min(MAX_TOAST_WIDTH, width - 4)`, the same formula as `layout::toast_rect`).
- **Code nit 1.** `unmount_selector` forgets the pending question. All three callers (`open_session`, `close_selector`, the `open_selector` failure path) go through it.
- **Code nit 4.** stdout is flushed before `exec`.

The new tests would fail without the fixes. The test gate is green. Only nits remain.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites ok, 0 failed; run with `INSTA_UPDATE=no`, working tree clean afterwards, no `.snap.new` files)

## Findings
1. [nit] `src/app/selector.rs`, `selector_msg`: `SelectorMsg::Resume` is handled by an early `if msg == SelectorMsg::Resume { …; return; }`, so the `SelectorMsg::Resume => {}` arm in the `match` below can never run. Change: handle `Resume` as the first arm of a single `match` that returns before `self.selector.preselecting = false;` runs. Alternatively, move the `preselecting` reset into the arms that need it and drop the early `if` and the empty arm.
2. [nit] `src/app/selector.rs`, `selector_msg`, the `if target.is_some()` guard:
   - **Why it matters.** It is load-bearing, because `request_resume(None)` means "the open session" (round-1 code nit 2, still open). Without the guard, `R` in the Projects pane with no session selected would ask to resume the session open behind the selector.
   - **The gap.** No test pins this case. `r_with_no_session_selected_does_nothing` and `r_with_the_projects_pane_focused_and_a_new_project_selected_does_nothing` both run with no session open, so they pass without the guard.
   - **Change.** Add an update test in `src/app/browser_tests/resume.rs`:
     1. `browser_rig`, then Ctrl-o.
     2. Focus Projects and move to a project with no session selected.
     3. Press `R`, and assert that no prompt is mounted and no plan is made.
   - **Better.** Also take round-1 nit 2 (a `ResumeTarget::{Open, Session(..)}` enum), so this guard is not needed.
3. [nit] `src/app/resume.rs`, `missing_dir_toast`: it copies the toast box geometry (`MAX_TOAST_WIDTH.min(screen_width.saturating_sub(4))`) from `src/app/layout.rs::toast_rect` instead of sharing it, so the two can drift apart. Change: move the box-width formula into one helper (e.g. `pub(crate) fn toast_box_width(screen_width: u16) -> u16` next to `toast_rect`) and call it from both places.
4. [nit] No test covers cancelling the question asked from the Projects pane. `n_and_esc_cancel_and_return_the_focus_in_the_selector` covers only `SessionList`. Change: in `r_with_the_projects_pane_focused_resumes_the_session_that_is_still_selected`, or in a sibling test, press `n` (or `Esc`) and assert that the focus returns to `Id::ProjectList` and no plan is made.
5. [nit] `factory/state/tasks/032-resume-in-harness/round-2/build-notes.md`, Changes bullet for `src/components/{list_pane,session_list,project_list}.rs`: it still says "`ProjectList` ignores it", which round 2 changed. Change: say that `ProjectList` answers `SelectorMsg::Resume`, which the `App` resolves against its selected session.
6. [nit] Round-1 code nits 2 (`ResumeTarget` enum instead of `Option<(HarnessId, SessionId)>`) and 3 (`confirm_two`'s empty-string sentinel and five positional `&str`s) are still open, by choice. That is acceptable. They are listed here only so they are not lost.
