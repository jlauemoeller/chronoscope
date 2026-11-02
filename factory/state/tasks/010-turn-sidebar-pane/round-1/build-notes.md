STATUS: DONE

## Changes
- `src/components/turn_sidebar/` (new): the `TurnSidebar` component.
  - `rows.rs`: `TurnRows` (one slot per turn, a map from `TurnId` to slot, a sorted list of the slots that have an entry), `Entry`, `Row`, `entry_for` (built on `derive_sections(turn, false)`; never lays out or renders). The list is `Start`, the entries, `End` with exactly one row between neighbours (`│`, or blank when a neighbour is a Compaction), so item `k` is on row `2k` and a row is found without a row vector. `apply` handles `Appended` (a known id updates its slot), `Updated` (unknown ids ignored), `Reset`.
  - `line.rs`: one row as a styled line (columns 3 / 7 / right margin `width - 4`, band, unfocused bold, durations right-aligned).
  - `mod.rs`: the component (`THEME_ATTR`, `Focus`, `KEY_LABEL_ATTR` default `1`, `TURN_ROWS_ATTR`, `MODEL_UPDATE_ATTR`, `SELECTED_ATTR` as an entry, `VISIBLE_ROWS_ATTR`); reuses `titled_block`, `border_role` and `ListCursor`. It keeps no `Arc<Turn>`, draws only the visible rows, and paints the background over the pane and the gap column after it. It handles no keys and no mouse yet.
- `src/app/layout.rs`: `SIDEBAR_WIDTH` 30, `SIDEBAR_GAP` 1, `SIDEBAR_MIN_SCREEN_WIDTH` 90; `BrowserAreas` gains `sidebar` and `body`; the transcript area is narrow from 90 columns on regardless of an empty session (so the viewport never needs a re-announce when the session stops being empty); `layout()` draws the sidebar after the header and before the transcript, and leaves it out with `EmptySession`, which keeps the full width.
- `src/app/{mod,browser,selector,theme_selector}.rs`: `Id::TurnSidebar`; mounted in `mount_browser`, in `BROWSER_IDS` and `THEMED`; `Appended`/`Updated`/`Reset` of the transcript on top are forwarded to it (not the covered ones); on opening and closing a subagent the App queries `TURN_ROWS_ATTR` of the transcript on top and pushes the rows; `push_viewports` announces the narrow transcript area and the sidebar's list rows and moves a focused, now hidden sidebar's focus to the transcript.
- `src/components/transcript/mod.rs`: answers `TURN_ROWS_ATTR` with one pass over its turns.
- Test instrumentation (cfg(test)): `turn_block::block_work` (layouts + renders), entry-build and line-build counters.
- `src/bench_support/mod.rs`, `tests/budgets.rs`: `load_with_sidebar` builds rows during the load; `SidebarScenario` times selection moves + frames. `docs/performance.md` notes the result.
- Existing tests: coordinates/columns at 120 and 200 columns adjusted without changing what they assert (`overlay_mouse.rs` points now at the right edge of the transcript because the overlay hides the left; `mouse.rs` `transcript_rows` slices the transcript's columns; viewport 120 -> 89). 43 browser screen snapshots at 120x40/200x50/200x60 re-accepted one by one: rows 0-4 and the help line rows unchanged, only the sidebar columns and the transcript re-wrap at 89 columns differ. No snapshot below 90 columns changed.

## Self-check
- AC: layout tests (89/90, 30+1, full-width header/help, hit_test pane/gap/transcript) — `app::layout::tests::the_sidebar_is_hidden_at_89_columns_and_shown_at_90`, `..._is_30_columns_with_a_one_column_gap_and_as_tall_as_the_transcript`, `the_header_and_the_help_line_stay_full_width_next_to_the_sidebar`, `an_empty_session_has_no_sidebar_...`, `app::mouse_tests::hit_testing_finds_the_sidebar_in_the_pane_nothing_in_the_gap_and_the_transcript_beyond`, `hit_testing_at_89_and_90_columns_...`
- AC: component snapshots 120x40 and 90x24, `default` and `terminal` — `turn_sidebar::snapshots::*` (focused/unfocused on User/Assistant/Compaction, no selection, long list top/end/middle, widest durations, widest selected; 48 snapshot files)
- AC: row tests (kinds and order, Start/End not selectable, connectors, redacted-only/system-only, system toggle, duration gained) — `turn_sidebar::tests::each_kind_is_listed_...`, `start_and_end_are_not_selectable_...`, `connector_rows_join_dots_...`, `a_turn_with_only_redacted_thinking_or_only_system_content_has_no_entry`, `toggling_system_content_leaves_the_entries_unchanged`, `an_assistant_turn_gaining_a_duration_shows_it_in_place`
- AC: adjacent Compactions, Compaction after Start / before End, notice-only entry, Updated gaining first section in place, Appended known id — `two_adjacent_compactions_have_one_blank_row_between_them`, `a_compaction_right_after_start_and_right_before_end`, `a_turn_with_only_a_notice_has_an_entry`, `an_updated_turn_gaining_its_first_section_appears_in_its_transcript_position`, `an_appended_turn_with_a_known_id_updates_its_row_instead_of_adding_one`
- AC: no duration without stats, appears in place, ends at the right margin — `an_assistant_entry_without_stats_shows_no_duration_and_gets_it_in_place`, `a_duration_ends_at_the_right_margin_in_the_shared_format`
- AC: building rows neither lays out nor renders — `building_rows_neither_lays_out_nor_renders_nor_reads_output` (`block_work`, `text_calls`)
- AC: update rebuilds only what changed — `a_turn_update_rebuilds_only_the_turn_that_changed`, `the_sidebar_applies_an_update_by_touching_only_the_turn_named`
- AC: draws only the rows shown — `the_sidebar_builds_only_the_rows_it_shows` (36 lines for 10 000 turns), `a_list_of_10_000_turns_is_drawn_and_updated_quickly`
- AC: App tests (below 90, at 90 = 59 wide, EmptySession, scrolling) — `browser_tests::sidebar::below_90_columns_...`, `at_90_columns_the_transcripts_viewport_is_59_wide`, `it_is_not_drawn_when_the_empty_session_replaces_the_transcript`, `scrolling_the_transcript_does_not_move_the_pane`, `the_sidebar_comes_when_the_first_turns_do_and_the_viewport_was_already_narrow`
- AC: subagent shows its turns, back shows the parent's including late ones — `browser_tests::sidebar::an_open_subagent_shows_its_turns_and_going_back_shows_the_parents_including_late_ones`, `a_nested_subagent_...`
- AC: theme preview repaints — `a_theme_preview_repaints_the_sidebar`, `previewing_a_theme_changes_the_cells_of_the_sidebar`
- AC: mouse shifted by the sidebar — `a_click_on_the_transcript_at_120_columns_hits_the_same_section_shifted_by_the_sidebar`, `clicks_on_the_sidebar_and_in_the_gap_do_nothing`, plus the adjusted `browser_tests::mouse::*`
- AC: gate and budgets — see below (budgets: sidebar frames 0.10 ms worst, rows built during the load, load 141 ms mixed / 413 ms dense)
- AC: snapshot scope — only 120x40 / 200x50 / 200x60 browser screens changed plus the new ones; no 80x24 or 60x20 snapshot touched
- UI: focused/unfocused selection on User, Assistant, Compaction @ 120x40 and 90x24, default and terminal — `turn_sidebar::snapshots::{focused,unfocused}_selection_on_*`
- UI: no selection — `snapshots::no_selection`; long list top / end — `a_long_list_at_the_top`, `a_long_list_scrolled_to_the_end` (shows `● End` on the last row); widest durations — `the_widest_durations`, `the_widest_duration_selected_in_the_focused_pane`
- UI: browser screens 120x40 with sidebar — `loaded_browser_120x40_{text,styled}`, `loaded_browser_theme_*`; 89 columns without it — `loaded_browser_89x30_text`; also `loaded_browser_90x24_text`, `empty_session_120x40_text`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (3006 passed, 0 failed)
- cargo test --release -- --ignored budgets --nocapture: pass

## Notes for the next tasks
- The pane is one cell narrower than the area it paints: `TurnSidebar::view` also paints the gap column after it so no stripe is left unpainted (`every_cell_of_the_browser_is_painted_with_the_background` covers it).
- Selection is held as a `TurnId` and exposed as an entry index (`SELECTED_ATTR`, `TurnSidebar::select`), so it survives inserted entries; `ListCursor` holds the scroll over list rows.
