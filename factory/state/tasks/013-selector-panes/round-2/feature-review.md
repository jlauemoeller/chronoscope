VERDICT: PASS

## Summary
The round-1 must-fix is resolved. `App::session_selected` now pushes the selection through `set_attr`, which requests the redraw, and the new test `moving_in_either_list_asks_for_a_redraw` would have caught the original bug: it also asserts `RedrawRequest::None` for a move that goes nowhere, which shows that a key press alone does not mark the frame dirty. The round-1 nits were addressed too: the state-4 snapshots no longer duplicate others, a shown harness that becomes unavailable gives way to an available one, and the code uses `GAP.width()`. I checked the new stats pruning against the worker. `schedule_stats` in `src/index/worker.rs` also forgets the `sent` entries of sessions that are gone, so a session that comes back gets its cached stats sent again, and the UI pruning matches the worker. The gate is green (fmt, clippy `-D warnings`, `INSTA_UPDATE=no cargo test`), and the working tree stays clean afterwards.

## Acceptance criteria
- [x] Index events go only through Port → `UserEvent` → `GlobalEvents` → `Msg`.
  - Implemented in `src/app/index_port.rs` (`IndexPort`, `IndexUpdate` with sequence-number equality). The `GlobalEvents` discriminant subscription now has its own `worker_events_clause()` (always). `Msg::Index` goes to `App::index_update`.
  - Tests: `a_snapshot_travels_from_the_port_through_global_events_to_update` and `a_snapshot_message_sets_the_domain_state_and_fills_the_components`. The second checks the domain state and the rows queried back from `ProjectList`, `SessionList` and `WorkingDir`.
- [x] Initial state: first available harness, first project, Projects focused, no session selected; focusing Sessions selects the newest.
  - Implemented in `snapshot`, `enter_harness` and `focus_pane` in `src/app/selector.rs`.
  - Tests: `the_first_available_harness_by_number_is_shown`, `an_unavailable_harness_is_not_selected_at_startup` and `focusing_sessions_selects_the_newest_session`.
- [x] `ProjectList`/`SessionList` `on()` handles j/k/↓/↑ without wrap-around and scrolls to keep the selection visible. It also handles Tab/Shift-Tab, ←/h and →/l, and the two no-op keys, all resolved through the keymap.
  - Tests: `project_list::tests` and `session_list::tests` (movement, no wrap, scrolling, focus keys, no-ops, `keys_follow_the_keymap`) and `list_pane::tests`.
  - Sessions moves now reach the screen (`moving_in_either_list_asks_for_a_redraw`).
- [x] A project move emits `ProjectSelected`, and the App pushes that project's sessions and cwd. The Projects selection is now pushed back as well.
  - Tests: `moving_the_project_selection_pushes_its_sessions_and_cwd`, `keys_drive_the_selection_through_the_focused_component` and `moving_to_another_project_clears_the_session_selection`.
- [x] `1` switches to Claude. Unregistered or unavailable harnesses do nothing. A switch selects the first project, and its newest session only when Sessions has focus.
  - Tests use a fake OpenCode: `the_number_of_a_harness_switches_to_its_first_project`, `a_switch_selects_the_newest_session_only_when_sessions_has_focus` and `a_switch_to_a_harness_without_a_tab_or_data_does_nothing`.
  - New: `the_shown_harness_becoming_unavailable_gives_way_to_an_available_one`.
- [x] A live `Snapshot` keeps the selection by id, and a vanished selection clamps to the same index.
  - Implemented in `keep_selection` (now clones the `Arc`) and `reselect`.
  - Tests: `a_live_snapshot_keeps_the_selected_project_and_session_by_id*`, `a_vanished_session_selection_clamps_to_the_same_index`, `a_vanished_project_selection_clamps…` and `selector::tests::reselect_*`.
- [x] `Stats` adds `<n> turns · <d>`, and the formatter gives `45s`/`12m`/`1h 05m`/`1 turn`.
  - Tests: `stats_events_fill_the_matching_row` and `session_list::tests::{durations_use_seconds_minutes_and_hours, stats_read_turns_and_duration, stats_need_ten_cells_for_the_title}`.
  - New: `stats_of_vanished_sessions_are_dropped_on_a_snapshot`.
- [x] `●` shows for an active session with an injected `now`, and clears on a later `Tick` after 60 s.
  - Tests: `an_active_session_shows_the_marker_and_the_flag_clears_after_60_seconds` and `the_active_flags_are_pushed_only_when_they_change`.
- [x] The layout formulas hold: 120x40 gives content width 112, Projects 36, Sessions 75 and panes 24 rows high; 80x24 gives 24 and 47.
  - Tests: the `layout::tests` pane-width, pane-height and offset tests.
- [x] Every UI state has a snapshot, and per-theme snapshots render `App::view` for all seven built-in themes.
  - States 1–7 are in `src/app/selector_tests.rs`.
  - State 4 is now `the_active_marker_in_an_unselected_row_above_a_band_120x40_styled`, where the `primary` `●` on s1 sits above the s2 band. It is backed by `sessions_focused_120x40_styled` and the cell check `the_active_dot_is_primary_outside_the_band_and_inverted_inside_it`.
  - The new `the_unfocused_selection_in_sessions_is_bold_secondary_without_a_band` covers the mirror case of §8.2 with the distinct-role theme.
  - Component snapshots are present.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. The reviewer re-ran all three with `INSTA_UPDATE=no`: all green, no `.snap.new` files, tree clean.

## Findings
None.
