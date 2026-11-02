VERDICT: PASS

## Summary
Round 3 made exactly the four test and comment changes the approver review asks for, and no production behaviour changed. Every acceptance criterion traces to code and to a test, the gate and the release budgets pass, and all nine source changes I tried made tests fail. Two small test-code nits remain, and neither needs another round.

## Test gate
Run on an export of `HEAD` (12b2095) with `INSTA_UPDATE=no`, in my own target directory. Nothing was run from the repo.
- `cargo fmt --check`: pass
- `cargo clippy --all-targets --locked -- -D warnings`: pass
- `cargo test --locked`: pass (3016 passed, 0 failed, 2 ignored; no `.snap.new` or `.pending-snap` file appeared)
- `cargo test --release --locked -- --ignored budgets --nocapture`: pass (exit status captured directly).
  - Mixed session: first content 2.2 ms, full parse 134 ms, loaded with rows 141 ms, peak 0.22x the file, worst sidebar frame 0.10 ms.
  - Dense session: first content 1.8 ms, full parse 558 ms, loaded with rows 460 ms, peak 0.63x, worst sidebar frame 0.10 ms.

## Round-3 follow-up
Each "Done when" item of `approver-review.md`, checked directly:
- `git diff 8fbdca1..HEAD -- src tests` touches six files: the tests in `background_fill.rs`, `turn_sidebar/tests.rs`, the test constant in `browser_tests.rs`, the doc comment in `mouse.rs`, and two new `.snap` files. In `background_fill.rs` only `mod tests` changed. No existing snapshot changed, and no `.snap.new` or `.pending-snap` file is tracked.
- `cargo test --lib background_fill` passes with six tests.
- The default-theme snapshot has the background on `1..2` of rows 1 and 2 only, with rows 0 and 3 all `Reset`. The transparent snapshot is `0..5 Reset/Reset/NONE` on every row.
- Deleting `if row.is_none() { self.selected = None; }` from `TurnSidebar::sync` fails exactly `a_selected_turn_that_loses_its_entry_is_deselected` and `a_selected_turn_that_is_not_in_new_rows_is_deselected` (61 passed, 2 failed).
- The test constant `BROWSER_IDS` is `[Id; 5]`, with `Id::TurnSidebar` and `Id::SidebarGap` and without `Id::EmptySession`. Leaving the gap unmounted now fails `opening_a_session_mounts_the_browser_unmounts_the_selector_and_starts_a_load` directly.
- The `hit_test` doc comment reads as specified.

Other source changes tried in the export, each alone, running the tests matching one filter (the failures listed are within that filter):
- `BackgroundFill::perform` returning `NoChange`: fails `it_ignores_events_and_commands`.
- The fill's transparent guard removed: fails `a_transparent_theme_paints_nothing` and `the_fill_in_a_transparent_theme`.
- The fill painting `Surface`: fails `it_paints_only_its_area_with_the_background` and `the_fill_in_the_default_theme`.
- `derive_sections(turn, true)` in `entry_for`: fails `toggling_system_content_leaves_the_entries_unchanged` and the no-entry test.
- `Updated` not forwarded to the sidebar: fails 2 app tests.
- No rebuild when a subagent closes: fails both tests that go back from a subagent.
- A threshold of 91 columns: fails 17 tests.

## Acceptance criteria
- [x] Layout tests cover 89 and 90 columns, the 30-column pane, the 1-column gap, full-width header and help line, and mouse routing; `hit_test` finds `Id::TurnSidebar` in the pane, nothing in the gap, and the transcript beyond — `browser_areas`, `layout()` and `hit_test`; `the_sidebar_is_hidden_at_89_columns_and_shown_at_90`, `the_sidebar_is_30_columns_with_a_one_column_gap_and_as_tall_as_the_transcript`, `the_header_and_the_help_line_stay_full_width_next_to_the_sidebar`, `hit_testing_finds_the_sidebar_in_the_pane_nothing_in_the_gap_and_the_transcript_beyond`, `hit_testing_at_89_and_90_columns_has_the_sidebar_only_from_90`.
- [x] Component snapshots at 120x40 and 90x24 in `default` and `terminal` cover focused and unfocused selection on each selectable kind, no selection, a long list at each end, and the widest durations — `src/components/turn_sidebar/snapshots.rs`, 48 files.
- [x] Row tests cover each kind and its order, `Start` and `End` not selectable, connectors around a Compaction, redacted-only and system-only turns having no entry, the system toggle, and a gained duration — `entry_for` and `TurnRows` in `rows.rs`; `each_kind_is_listed_in_transcript_order_between_start_and_end`, `start_and_end_are_not_selectable_and_have_no_entry`, `connector_rows_join_dots_and_a_compaction_has_blank_rows_around_it`, `a_turn_with_only_redacted_thinking_or_only_system_content_has_no_entry`, `toggling_system_content_leaves_the_entries_unchanged`, `an_assistant_turn_gaining_a_duration_shows_it_in_place`.
- [x] Row tests cover adjacent Compactions, a Compaction after `Start` and before `End`, a notice-only turn, an `Updated` turn gaining its first section in position, and an `Appended` turn with a known id — `TurnRows::set` and `TurnRows::apply`, which match `Content::append` and `Content::update`; `two_adjacent_compactions_have_one_blank_row_between_them`, `a_compaction_right_after_start_and_right_before_end`, `a_turn_with_only_a_notice_has_an_entry`, `an_updated_turn_gaining_its_first_section_appears_in_its_transcript_position`, `an_appended_turn_with_a_known_id_updates_its_row_instead_of_adding_one`.
- [x] An Assistant entry without stats shows no duration, and it appears in place once known, in the shared format, ending at the right margin — `labelled` with `format_duration` in `line.rs`; `an_assistant_entry_without_stats_shows_no_duration_and_gets_it_in_place`, `a_duration_ends_at_the_right_margin_in_the_shared_format`.
- [x] Building rows neither lays out nor renders — `entry_for` calls only `derive_sections(turn, false)`; `building_rows_neither_lays_out_nor_renders_nor_reads_output`, with `the_counters_of_the_test_above_move_when_work_is_done` as its control. I traced `turn_stop` through `sections_of`, `Content::section_rows` and `TurnBlock::layout`, which `block_work` counts, so an unchanged `block_work` shows `turn_stop` was not reached. The three counters are thread-local, so parallel tests cannot disturb them.
- [x] A turn update rebuilds only what changed — `TurnRows::apply`; `a_turn_update_rebuilds_only_the_turn_that_changed` (5 000 turns, `entry_builds` counter, every other entry compared) and `the_sidebar_applies_an_update_by_touching_only_the_turn_named`.
- [x] The sidebar draws only the rows it shows — `draw_rows`; `the_sidebar_builds_only_the_rows_it_shows` (10 000 turns, exactly 36 lines built) and `a_list_of_10_000_turns_is_drawn_and_updated_quickly`.
- [x] App tests cover below 90 columns, 59 columns at 90, `EmptySession` and scrolling — `src/app/browser_tests/sidebar.rs`: `below_90_columns_it_is_not_in_the_layout_and_the_transcript_has_the_full_width`, `at_90_columns_the_transcripts_viewport_is_59_wide`, `it_is_not_drawn_when_the_empty_session_replaces_the_transcript`, `scrolling_the_transcript_does_not_move_the_pane`.
- [x] An open subagent shows its turns, and going back shows the parent's, including turns received while covered — `push_sidebar_rows` on opening and closing, and covered updates stop at `covered_session_update`; `an_open_subagent_shows_its_turns_and_going_back_shows_the_parents_including_late_ones`, `a_nested_subagent_lists_its_own_turns_and_going_back_each_level_restores_the_one_below`, `updates_of_the_transcript_on_top_reach_the_sidebar_during_a_subagent`.
- [x] A theme preview repaints the sidebar — `Id::TurnSidebar` and `Id::SidebarGap` in `THEMED`; `a_theme_preview_repaints_the_sidebar`, `previewing_a_theme_changes_the_cells_of_the_sidebar`.
- [x] A click on the transcript at 120 columns hits the same section, shifted by the sidebar — `a_click_on_the_transcript_at_120_columns_hits_the_same_section_shifted_by_the_sidebar` compares against the 89-column layout; also `clicks_on_the_sidebar_and_in_the_gap_do_nothing`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — see Test gate.
- [x] `cargo test --release -- --ignored budgets --nocapture` stays within budget, with rows built during the load — `load_with_sidebar` and `SidebarScenario`, used in `tests/budgets.rs`; figures under Test gate.
- [x] Snapshot changes are limited to the new sidebar snapshots and browser screens at 90 columns or wider — against `main`, 53 added and 43 modified. Of the modified ones, I measured 41 as 120 columns wide and 2 as 200. The added ones are 48 sidebar snapshots, 2 `BackgroundFill` snapshots and 3 browser screens (`empty_session_120x40_text`, `loaded_browser_90x24_text`, `loaded_browser_89x30_text`). No existing snapshot below 90 columns changed.

## Findings
1. [nit] `src/components/turn_sidebar/tests.rs:598`, in `the_title_reads_the_key_and_has_two_spaces_before_the_rule`, rebinds a variable only to make it mutable. Change: write `let mut relabelled = TurnSidebar::new(theme()).key_label("2");` on line 597 and delete line 598.
2. [nit] `src/app/layout.rs:570`: the test name `the_browser_is_header_transcript_and_a_two_row_help_line` no longer lists everything it checks, because its body now also asserts `Id::TurnSidebar` and `Id::SidebarGap`. Change: rename it to `the_browser_is_header_sidebar_transcript_and_a_two_row_help_line`.
