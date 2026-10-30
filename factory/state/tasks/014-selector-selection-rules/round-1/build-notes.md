STATUS: DONE

## Changes
- `src/app/mod.rs`: `App::new(init, listener, working_dir: PathBuf)`; new `Msg::OpenSession(HarnessId, SessionId)` and `Msg::CloseSelector` (ignored by `App::update` until the browser task 021).
- `src/app/runtime.rs`: passes `std::env::current_dir()` to `App::new`.
- `src/app/selector.rs`: startup preselection (`SelectorState::preselection`): project cwd equal to or an ancestor of the working directory (`Path::starts_with`, component-wise), deepest wins, ties to the lowest harness number (then list order), only available harnesses; fallback is the first available harness and its first project. While the user has not acted and not every registered harness has reported, a snapshot of another harness can change the preselected harness (`preselect_again`); any `SelectorMsg` ends the preselection. `App::set_session_open` pushes the "session open behind the selector" prop to both lists. `SessionRows` now carries the harness and `harness_empty` (set by the app: the shown harness has no sessions in any project, or no harness is available once every registered harness has reported; before that the lists are loading and no message flashes, so the 013 screen snapshots are unchanged). A project without sessions in a harness that has some does not show the message.
- `src/components/list_pane.rs`: `SESSION_OPEN_ATTR`, `PaneKey::Cancel` (keymap action `cancel`), `PaneState::cancel` (`Msg::Quit` or `Msg::CloseSelector`).
- `src/components/project_list.rs`: Esc -> cancel. Enter still focuses Sessions.
- `src/components/session_list.rs`: Enter on the selected session -> `Msg::OpenSession(harness, id)`; Esc -> cancel; owns an `EmptyState` child drawn in the pane interior when `SessionRows.harness_empty` is set.
- `src/components/empty_state.rs`: `NO_SESSIONS` message and `without_background()` so the child leaves the overlay surface visible.
- Existing 013 session/screen behaviours that already satisfied the rules (project change clears session, focus Sessions selects newest, live snapshot does not create a selection) are kept and now covered by explicit tests.
- Tests: new `src/app/selection_tests.rs` (update tests + screen snapshots), helpers in `selector_tests.rs` made `pub(super)`, fixtures (`twelve_projects`, `snapshot_of`, `project_in`), component tests.

## Self-check
- AC: `/w/brio/src` preselects `/w/brio` in its harness, Projects focused, no session — `startup_preselects_the_project_of_the_nearest_ancestor_of_the_working_directory`, `startup_preselects_the_harness_that_owns_the_deepest_match_whatever_arrives_first`, `the_working_directory_itself_matches_its_project`, `equally_deep_matches_go_to_the_lowest_harness_number`, `an_unavailable_harness_takes_no_part_in_the_preselection`, `a_later_snapshot_does_not_move_a_selection_the_user_made`, `a_late_snapshot_after_every_harness_reported_does_not_move_the_preselection`
- AC: no match -> first available harness + first project — `without_a_match_startup_selects_the_first_available_harness_and_its_first_project`
- AC: j/k clears session, Sessions then selects newest — `j_and_k_in_projects_clear_the_session_selection_and_sessions_selects_the_newest_again`
- AC: live Snapshot leaves nothing selected; focusing selects the new newest — `a_live_snapshot_leaves_nothing_selected_and_focusing_sessions_selects_the_new_newest`
- AC: Enter in Projects focuses Sessions + newest; Enter in Sessions -> OpenSession — `enter_in_projects_focuses_sessions_and_selects_the_newest_session`, `enter_in_sessions_asks_to_open_the_selected_session_and_changes_nothing_else`, `enter_asks_for_the_session_of_the_harness_shown`, component tests `enter_asks_to_open_the_selected_session_of_the_harness`, `enter_without_a_selected_session_or_a_harness_does_nothing`, `enter_focuses_sessions_and_does_not_open_anything`
- AC: Esc Quit / CloseSelector — component tests `esc_quits_without_an_open_session_and_closes_the_selector_with_one` (project_list and session_list), app tests `esc_quits_at_startup`, `esc_in_sessions_quits_at_startup_too`, `esc_closes_the_selector_when_a_session_is_open_behind_it`
- AC: empty state — `a_harness_without_sessions_has_empty_panes_and_an_empty_working_directory`, `a_machine_without_an_available_harness_selects_nothing_and_mutes_the_tab`, `the_empty_message_is_in_the_sessions_pane_in_muted_foreground`, `a_harness_that_empties_shows_the_empty_state_again`, `a_list_of_an_empty_harness_shows_the_empty_state_in_muted_foreground`, `a_project_without_sessions_in_a_harness_that_has_some_shows_no_message`, `an_empty_project_in_a_harness_with_sessions_does_not_show_the_message`, `the_message_waits_until_every_harness_has_reported`, `an_empty_list_draws_only_its_border_and_title`
- UI: Empty state @ 80x24 text — `empty_state_80x24_text`
- UI: Empty state @ 120x40 text — `empty_state_120x40_text`
- UI: Empty state @ 120x40 styled — `empty_state_120x40_styled` (also no-available-harness: `no_available_harness_120x40_text`, `no_available_harness_120x40_styled`)
- UI: Startup preselect 9th project @ 80x24 text, list scrolled — `startup_with_the_9th_project_preselected_80x24_text` (plus `the_preselected_project_is_scrolled_into_view`, `preselection_before_the_first_resize_is_scrolled_too`, `the_preselected_project_is_the_focused_band_and_no_session_has_a_marker`)
- UI: Startup preselect @ 80x24 styled (focused band) — `startup_with_the_9th_project_preselected_80x24_styled`
- UI: After moving to next project @ 120x40 text — `after_moving_to_the_next_project_no_session_has_a_marker_120x40_text`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (743 passed, 0 failed)
