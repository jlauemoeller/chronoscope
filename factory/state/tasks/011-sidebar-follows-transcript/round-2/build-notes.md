STATUS: DONE

## Changes
- `src/components/turn_sidebar/mod.rs`, `src/components/list_pane.rs`: the pane remembers its selection and scroll offset for a transcript that a subagent covers. New attribute `COVER_ATTR` (`Flag(true)`: a transcript is opened over this one, push the pane onto a stack; `Flag(false)`: it was closed, pop it so the next rows set restore it). `ListCursor::restore(len, selected, offset)` starts from the saved offset and scrolls only as far as needed, so a changed pane size still keeps the selected row visible. The module doc line is rewrapped.
- `src/app/browser.rs`: `push_sidebar_rows(covering)` sets `COVER_ATTR` before replacing the rows (called from `open_subagent` with `true`, `close_subagent` with `false`). `sidebar_section` is now a small enum `SidebarSection { NotPushed, Pushed(Option<SectionId>) }`; its reset in `push_sidebar_rows` is documented as a defence. The unchanged-section comparison no longer clones the section.
- Tests: see the self-check. Test helper `subagent_rig_after` (a parent with turns ahead of the fixture's).

## Self-check
- Review (ui must-fix, feature nits 2 and 3): going back from a subagent restores the pane as it was — `going_back_from_a_subagent_shows_the_pane_as_it_was_with_a_list_longer_than_the_pane` (whole screen equal before and after, parent list 90 rows against a 29-row pane, marker away from the edges; plus the new text snapshot at 120x40), `going_back_scrolls_the_least_needed_when_the_pane_shrank_while_it_was_covered`, `nested_subagents_bring_back_each_pane_as_it_was`, component tests `rows_set_after_an_uncover_bring_back_the_selection_and_offset_of_the_covered_pane` and `a_restored_selection_scrolls_the_least_needed_and_covers_nest`. Mutation: making `cover(false)` restore nothing fails four of these. This also keeps the parent's selection when its covered transcript moved onto a turn without an entry.
- Review (code and feature must-fix): `the_sidebar_does_not_follow_while_it_has_the_focus` rewritten as the review describes (press `G`, move the focus to the sidebar, append; the transcript's section changes, the sidebar's entry stays). Mutation: `sidebar_follows` always `true` now fails it.
- Review: styled snapshot of the long list — `a_long_list_scrolled_to_keep_the_selected_row_visible_120x40_styled`.
- Review: `n` / `N` through matches — added to `a_search_moves_the_sidebar_selection_to_the_turn_of_the_match`.
- Review: first half of `pushing_the_selection_produces_no_message` now updates with `Msg::TranscriptChanged` and asserts `take_queued()` is empty; the weak `Debug` string check is gone.
- Review: `marker_row` dead clause removed; row-count comments corrected (83, 43); `scrolling_the_transcript_does_not_move_the_pane` blanks the marker column instead of replacing `>`; the module doc is rewrapped; the `sidebar_section` enum and the defensive comment are done.
- Review: build-notes count corrected here: round 1 changed 44 existing `.snap` files and added 5 (49 in total); round 2 adds 2 new snapshots.
- Not done (nits for planning, no change in this task): a turn that gains its entry while holding the selection does not pull the sidebar until the section next changes (follows the binding only-on-change note); a selected turn that loses its entry is deselected by the 010 `sync`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all targets, 0 failed; lib 2630 passed)
- cargo test --release -- --ignored budgets --nocapture: pass (sidebar frames worst under 1 ms)
