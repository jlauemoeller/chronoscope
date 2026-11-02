STATUS: DONE

## Changes
- `src/components/transcript/mod.rs`: new query-only attribute `SELECTED_SECTION_ATTR` (a `SectionId` payload; absent with nothing selected).
- `src/components/turn_sidebar/mod.rs`: new attribute `SELECTED_TURN_ATTR` (a `TurnId` payload) and `TurnSidebar::select_turn`. A turn with an entry is selected through the existing sync (least-scrolling reveal via `ListCursor`); a turn without an entry, or an unknown one, leaves the selection as it is. Lookup is `TurnRows::entry_index` (hash map plus binary search), no scan.
- `src/app/browser.rs`: `App::sync_sidebar_selection` (reads the top transcript's `SELECTED_SECTION_ATTR`, pushes the turn with `set_attr`, remembers the section pushed in `BrowserState::sidebar_section`) and `sidebar_follows` (the sidebar follows unless it has the focus itself; the one place task 012 extends). `push_sidebar_rows` clears the remembered section so that rows are replaced first and the selection after them (subagent opened or left).
- `src/app/mod.rs`: `App::update` ends with `sync_sidebar_selection()` (before `refresh_routing`). Push happens only when the transcript's selected section changed since the last push, so task 013 can move the sidebar on its own.
- `src/components/mod.rs`: exports.
- Tests: new `src/app/browser_tests/sidebar_follow.rs` (24 tests), component tests in `turn_sidebar/tests.rs` and `transcript/tests.rs`. `sidebar.rs`, `search.rs`, `subagent.rs` test helpers made `pub(super)`. In `browser_tests/sidebar.rs` two 010 tests were adapted: `there_is_no_selection_and_no_focus_in_the_running_app` (now asserts entry 0 is selected, renamed) and `scrolling_the_transcript_does_not_move_the_pane` (compares the pane ignoring the `>` marker).
- Snapshots: 49 existing `.snap` files changed (browser screens at 90 columns or wider); each differs from before only on the pane's selected row (`●` or first `═` becomes `>`, text bold `secondary_foreground` in styled ones), checked by script (changed lines identical to the old ones beyond column 31 in the text snapshots; the styled ones only on the one pane row and its style row). The `assertion_line` metadata that insta adds was removed to keep the diffs minimal. 5 new snapshots under `src/app/browser_tests/snapshots/`.
- Note: the transcript's search is not incremental, so "an incremental search moves it" is covered by a test that has the search prompt hold the focus while a following update moves the selection (`the_sidebar_follows_while_the_search_prompt_has_the_focus`), plus the Enter search.

## Self-check
- AC: follows j/k, ]/[, page keys, search — `j_and_k_move_the_sidebar_selection_with_the_transcripts`, `the_turn_keys_move_the_sidebar_one_entry_at_a_time`, `the_page_keys_and_the_ends_move_the_sidebar_selection_with_the_transcript`, `a_search_moves_the_sidebar_selection_to_the_turn_of_the_match`, `the_sidebar_follows_while_the_search_prompt_has_the_focus`.
- AC: subagent open/leave — `opening_a_subagent_shows_the_row_of_its_selection_and_going_back_restores_the_parents`, `a_nested_subagent_and_each_way_back_follow_the_transcript_on_top`.
- AC: live updates — `a_turn_appended_while_away_from_the_end_adds_a_row_and_changes_neither_selection_nor_offset`, `a_following_transcripts_selection_is_tracked_as_turns_arrive`, `a_new_turn_the_selection_is_not_on_changes_nothing_in_the_selection`.
- AC: away from the end, row added, selection and offset unchanged — `a_turn_appended_while_away_from_the_end_...` (pane text and marker row identical).
- AC: selected row visible, least scrolling, area unchanged — `the_selected_row_stays_visible_and_the_pane_scrolls_by_the_least_needed`, `selecting_a_turn_scrolls_the_list_by_the_least_needed` (component), `scrolling_the_transcript_does_not_move_the_pane` (existing, adapted).
- AC: turn without entry — `a_selection_in_a_turn_without_an_entry_leaves_the_sidebar_unchanged`, `a_turn_without_an_entry_or_unknown_leaves_the_selection_where_it_is`.
- AC: no Msg from the push — `pushing_the_selection_produces_no_message` (`take_queued()` empty after a key and after a live update that moves the selection).
- AC: wheel — `the_transcripts_wheel_moves_the_sidebar_selection_when_it_moves_the_transcripts`.
- Extra: only on a change, and not while the sidebar has the focus — `the_selection_is_pushed_only_when_the_transcripts_section_changed`, `the_sidebar_does_not_follow_while_it_has_the_focus`, `a_turn_pushed_by_attribute_selects_its_row_and_an_unknown_turn_changes_nothing`.
- UI: User row @ 120x40 default — existing `loaded_browser_120x40_text` / `_styled` / `loaded_browser_theme_default` (updated).
- UI: Assistant row @ 120x40 — `the_selection_on_an_assistant_row_120x40_text`, `_styled`.
- UI: Compaction row @ 120x40 — `the_selection_on_a_compaction_row_120x40_text`, `_styled`.
- UI: long list scrolled @ 120x40 — `a_long_list_scrolled_to_keep_the_selected_row_visible_120x40_text`.
- AC: budgets — `cargo test --release -- --ignored budgets --nocapture` passes (sidebar frames worst 0.10 ms; first content, parse, memory, frames within budget).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all targets, 0 failed; lib 2624 passed)
