VERDICT: APPROVE

## Assessment
All three round-2 reviews are PASS. The round-1 code-review must-fix (the startup preselection was lost after an empty first snapshot) is fixed in `src/app/selector.rs` `preselect_again` / `settle_preselection`. It is covered by update tests that fail without the fix. I re-ran `INSTA_UPDATE=no cargo test` on this branch: 0 failures, no `.snap.new` files, and the working tree is unchanged. The only remaining findings are nits.

**Code review: PASS**
- Nit 1, stale doc comment on `SelectorState::preselecting`: ignored. It is documentation only. The suggested wording is accurate: `settle_preselection` runs at the end of every snapshot (`selector.rs:334`), so outside the empty-harness case the window still closes once every harness has reported. Worth fixing the next time this code is touched.
- Nit 2, `preselecting` stays true while the shown harness is empty, so a later matching snapshot can switch the harness: ignored. This follows "before the user has acted" (SPEC §4.1, and round-1 must-fix part (a) as asked). The missing test is optional.
- Nit 3, `EmptyState` has no narrow-width test of its own: ignored. The ellipsis path is already exercised through `SessionList` (`the_empty_message_keeps_the_rows_margin_and_is_cut_in_a_narrow_pane`). The extra coverage is optional.
- Nit 4, `harness_empty` narrowed to "no projects": ignored, with a planner follow-up (see Feature nit 2).
- Nit 5, wording of the `harness_empty` doc comment about `all_reported`: ignored. Documentation only.

**Feature review: PASS.** Every acceptance criterion is checked off and tested.
- Nit 1, the preselection window stays open while the shown harness is empty, and the help or theme overlays do not end it: ignored. It does not breach the spec, because the user has not sent a selector action. A broader "any input ends preselection" rule would be a new design choice and is not needed for this task.
- Nit 2, `harness_empty` is `projects.is_empty()`, not "zero sessions": ignored for this task. I did not escalate it as an A/B choice, for three reasons:
  - The state cannot be reached with the only adapter that exists. Claude's `build_index` creates projects only from sessions, so "no sessions" implies "no projects", and the acceptance criterion holds for every reachable state.
  - The planner note scopes the empty state to "an empty data dir, or no available harness at all". Both of those have no projects.
  - The round-1 feature and UI reviews both offered this narrowing as an acceptable fix.

  **Planner follow-up:** the first adapter task that can produce projects without sessions (likely OpenCode, whose projects come from its `project` table) must either draw the full empty state for such a harness or confirm the narrower rule. `a_harness_with_projects_but_no_sessions_shows_an_ordinary_empty_list` currently pins the narrower rule.

**UI review: PASS.** All required snapshots are present and match the `UI` section.
- Nit 1, the no-match fallback lands on an empty Claude while OpenCode has projects: ignored for this task. The implementation follows the binding planner note ("first available harness (by number) and its first project"), so changing it is a planner or spec decision, not a builder fix. **Planner follow-up, if wanted:** in the fallback, skip available harnesses that have no projects.

## Conflicts
None. The round-2 reviewers agree that everything left is a nit. The `harness_empty` narrowing follows option (b), which the round-1 feature and UI reviews both proposed, and the round-2 code and feature reviews both accept it as a nit.

## Rework budget
Earlier REWORK verdicts: 0 of 2 (round-1 has no `verdict.md`).
