STATUS: DONE

## Changes
- `src/app/index_port.rs` (new): `IndexUpdate` (Arc-wrapped `IndexEvent`, equality by sequence number) and `IndexPort` (sync `Poll` over the task-010 receiver; never blocks, a disconnected worker is not an error). `UserEvent::Index(IndexUpdate)` in `src/app/mod.rs`.
- `src/components/global_events.rs`: subscribes to the index user events by discriminant (not `Any`, which would duplicate keys and ticks) and turns them into `Msg::Index`.
- `src/app/selector.rs` (new): `Pane`, `SelectorMsg` (`ProjectSelected`, `SessionSelected`, `Focus`, `SwitchHarness`), the selector's domain state (per-harness snapshots, stats map, current harness, selected project/session ids, focused pane, last pushed active ids) and the handlers: snapshot, stats, selection kept by id (else same index, clamped), focus (moves focus with `Application::active`, selects the newest session when Sessions is focused with nothing selected), harness switching, activity refresh on `Tick`, and pushing rows/selection/cwd/tabs/visible-row counts into the components through `attr`.
- `src/app/mod.rs`: new `Id`s, `Msg::Index`/`Msg::Selector`, mounts `HarnessTabs`, `ProjectList`, `SessionList`, `WorkingDir`, focus on `ProjectList` at startup, injectable clock (`set_clock`), `TimestampFormatter` from the config, the `IndexHandle` owned by the app (`set_index`/`shutdown`), `production_listener(index)` adds the `IndexPort`.
- `src/app/startup.rs`, `src/harness/mod.rs`: `AppInit.harnesses` (registered harnesses, via the new `registered_harnesses(config)`).
- `src/app/runtime.rs`: starts the `SessionIndex` over the enabled harnesses, hands its receiver to the port, calls `shutdown()` on exit so the stats cache is saved.
- `src/app/layout.rs`: `selector_areas(overlay)` (pure) with the planner-note offsets; `layout()` adds `HarnessTabs`, `ProjectList`, `SessionList`, `WorkingDir` between the frame and the help line.
- `src/components/`: `harness_tabs.rs`, `project_list.rs`, `session_list.rs`, `working_dir.rs`, and `list_pane.rs` (shared: `ListCursor` with scroll offset, key resolution through the keymap, pane border/title, row layout and styles). Data goes in through `Attribute::Custom` payloads and every one can be queried back.
- Tests: `src/app/fixtures.rs` and `src/app/selector_tests.rs` (update tests, loop test through the real `IndexPort`, screen snapshots, cell-level style checks) plus per-component tests/snapshots.
- Existing screen snapshots in `src/app/snapshots` were re-accepted: the only difference is the new tabs/panes/Working Directory box on the previously empty overlay.

Decisions where the spec was silent:
- Selected tab: `primary` plus the `━` row, as in the planner note; no `UNDERLINED` modifier (it would double the underline).
- Stats are dropped when fewer than 10 cells would remain for the title after a 2-cell gap between title and stats.
- Pane title with no project: `Sessions`. A project that vanishes in a live update counts as a move to another project, so the session selection is cleared (and, if Sessions has focus, the newest session of the successor is selected).
- Pressing the number of the harness already shown does nothing. Before the first snapshot arrives all tabs are muted; the first available registered harness by number is shown when snapshots arrive and none is shown yet.
- Scrolling: `App` pushes the number of visible rows (pane height - 3) on resize; the list updates its stored offset when moving or when rows are set, and `view` only reads it (it applies a pure correction so the selection is always drawn).
- Enter in Projects moves focus to Sessions; Enter in Sessions does nothing yet (open comes with a later task).
- `Config.utc`/`timestamp_format` drive the rows; tests set `utc = true`.
- Working Directory text width is the box width minus 6 (starts at column 3, 3-cell right margin).

## Self-check
- AC: Index events reach the App via Port -> UserEvent -> GlobalEvents -> Msg; snapshot message asserts domain state and pushed rows — `a_snapshot_travels_from_the_port_through_global_events_to_update`, `a_snapshot_message_sets_the_domain_state_and_fills_the_components`, `an_index_user_event_becomes_an_index_message`, `index_port::tests::*`
- AC: Initial state, focusing Sessions selects newest — `before_any_snapshot_nothing_is_selected_and_the_tab_is_muted`, `a_snapshot_message_sets_the_domain_state_and_fills_the_components`, `focusing_sessions_selects_the_newest_session`, `focusing_projects_keeps_the_session_selection`, `focusing_the_pane_that_has_focus_does_nothing`
- AC: list `on()` keys — `project_list::tests::{down_and_j_move_the_selection_and_report_the_project, up_and_k_*, there_is_no_wrap_around, tab_shift_tab_right_l_and_enter_focus_sessions, left_and_h_do_nothing_in_projects, the_list_scrolls_to_keep_the_selection_visible, keys_follow_the_keymap}`, `session_list::tests::{down_and_up_*, there_is_no_wrap_around, tab_shift_tab_left_and_h_focus_projects, right_and_l_do_nothing_in_sessions, the_list_scrolls_*}`, `list_pane::tests::*`
- AC: project selection pushes sessions and cwd — `moving_the_project_selection_pushes_its_sessions_and_cwd`, `moving_to_another_project_clears_the_session_selection`, `keys_drive_the_selection_through_the_focused_component`
- AC: harness switching — `the_number_of_a_harness_switches_to_its_first_project`, `a_switch_selects_the_newest_session_only_when_sessions_has_focus`, `a_switch_to_a_harness_without_a_tab_or_data_does_nothing` (fake OpenCode registered in the test)
- AC: live snapshot keeps selection by id / clamps — `a_live_snapshot_keeps_the_selected_project_and_session_by_id`, `a_live_snapshot_keeps_the_selection_by_id_when_indices_move_down`, `a_vanished_session_selection_clamps_to_the_same_index`, `a_vanished_project_selection_clamps_to_the_same_index_and_clears_the_session`, `selector::tests::reselect_*`
- AC: stats and formatter — `stats_events_fill_the_matching_row`, `a_stats_event_updates_a_row_that_is_shown_and_survives_a_snapshot`, `session_list::tests::{durations_use_seconds_minutes_and_hours, stats_read_turns_and_duration, stats_need_ten_cells_for_the_title}`
- AC: active marker with injected now, clears after 60 s — `an_active_session_shows_the_marker_and_the_flag_clears_after_60_seconds`, `the_active_flags_are_pushed_only_when_they_change`, `a_harness_that_says_it_is_not_idle_is_active_whatever_its_age`
- AC: layout formulas — `layout::tests::{panes_at_120x40_are_36_and_75_columns_and_24_rows, panes_at_80x24_are_24_and_47_columns, projects_width_is_a_third_clamped_to_20_and_36, components_are_placed_at_the_planner_offsets, the_working_directory_box_is_followed_by_a_blank_row_and_the_help_line}`
- AC: snapshots for all states, per-theme for all seven themes — see UI below
- UI: Startup 120x40 text + styled — `startup_120x40_text`, `startup_120x40_styled`; 80x24 text — `startup_80x24_text`; 200x50 text — `startup_200x50_text`
- UI: Sessions focused 120x40 text + styled — `sessions_focused_120x40_text`, `sessions_focused_120x40_styled`; 80x24 text — `sessions_focused_80x24_text`
- UI: Long titles 80x24 — `long_titles_80x24_text` (plus `long_titles_with_stats_120x40_text` showing the stats rule where there is room)
- UI: Active `●` unselected row / inside band, styled 120x40 — `the_active_marker_in_an_unselected_row_120x40_styled`, `the_active_marker_inside_the_band_120x40_styled`, with cell-level checks in `the_active_dot_is_primary_outside_the_band_and_inverted_inside_it`
- UI: Scrolled list (40 sessions, 80x24) — `a_scrolled_list_80x24_text`
- UI: Long working directory (80x24) — `a_working_directory_longer_than_the_box_is_cut_from_the_left_80x24_text`
- UI: Per-theme (default, terminal, catppuccin, gruvbox, nord, tokyo-night, solarized) — `sessions_focused_theme_*`
- UI: Component snapshots — `harness_tabs::tests::one_selected_and_one_muted_tab_snapshot` (width 60), `project_list::tests::{focused,unfocused}_list_snapshot`, `session_list::tests::{rows_with_stats,rows_without_stats}_snapshot` (width 60, height 10), `working_dir::tests::box_with_a_directory_snapshot`
- Cell-level §8.2 checks (band columns 4..=W-4, marker never inverted, unfocused bold secondary, ring/border, tab colours) — `the_focused_selection_is_one_inverted_band_with_a_primary_marker`, `the_unfocused_selection_is_bold_secondary_without_a_band`, `the_focused_pane_has_the_ring_border`, `the_selected_tab_is_primary_with_a_primary_underline_of_its_label_width`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (695 passed, 0 failed)
