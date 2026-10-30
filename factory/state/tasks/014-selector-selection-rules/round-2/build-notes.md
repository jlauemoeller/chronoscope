STATUS: DONE

## Changes
- `src/app/selector.rs`: `preselection()` now returns a `Preselection { harness, project, matched }` and skips projects whose cwd is not absolute. `preselect_again` re-applies the preselection while it is open when the harness differs, or when the harness is the same and a cwd match names another project; a fallback (no match) never moves a selection on a live update. `settle_preselection` no longer ends the preselection while the shown harness has no projects (a failed first build sends an empty `available = true` snapshot). `harness_empty` is now "the harness has no projects" (or no harness is available once all have reported), so a harness whose projects have no sessions renders as an ordinary list.
- `src/components/empty_state.rs`, `session_list.rs`: the empty message is centred inside the interior minus the 2-cell row margin and is cut with an ellipsis when narrower.
- `src/app/fixtures.rs`: `unavailable_snapshot_of`; the one-off `IndexEventParts` helper in `selection_tests.rs` is gone.
- Snapshots: only the style-run bounds of the message row in `empty_state_120x40_styled`, `no_available_harness_120x40_styled` and the `SessionList` component styled snapshot changed (text snapshots are unchanged).

## Self-check
- Review (code 1, must-fix): preselection lost after an empty first snapshot — `an_empty_first_snapshot_does_not_lose_the_preselection` (one harness, and two harnesses with the other already reported), `a_better_match_in_the_shown_harness_wins_before_the_user_acts`, `a_new_most_recent_project_does_not_move_a_fallback_preselection`
- Review (code 2, nit): non-absolute cwd — `a_project_without_an_absolute_cwd_never_matches`
- Review (code 3, nit): `IndexEventParts` removed in favour of `fixtures::unavailable_snapshot_of`
- Review (code 4, nit): `$PWD` not added; the planner note binds `std::env::current_dir()`
- Review (feature 1, nit): same fix as code 1 (the empty first snapshot case)
- Review (feature 2 / ui 1, nit): `harness_empty` only when there are no projects — `a_harness_with_projects_but_no_sessions_shows_an_ordinary_empty_list`, `an_empty_project_in_a_harness_with_sessions_does_not_show_the_message`
- Review (feature 3, nit): left as is (`PathBuf::new()` fallback matches nothing)
- Review (ui 2, nit): margin and ellipsis — `the_empty_message_keeps_the_rows_margin_and_is_cut_in_a_narrow_pane`
- Review (ui 3, nit): planner note, left as is
- All round-1 acceptance criteria and UI snapshots still pass unchanged.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass
