VERDICT: PASS

## Summary
Round 2 fixes both round-1 must-fix findings. `change_turns` now re-evaluates following after an in-place update when the user is away from the end, and new component tests pin the rule that a growing turn moves the selection to its new last section. The round-1 nits from all three reviews were also taken up: the redraw in the `NewSections` arm, the stale-window guard in `at_bottom`, the docs, the narrow header and the styled `↓ N new` snapshot. I ran the gate myself and it passes: `cargo fmt --check` ok, `cargo clippy --all-targets -- -D warnings` clean, `INSTA_UPDATE=no cargo test` all green, with no `.snap.new` files and a clean worktree.

## Acceptance criteria
- [x] At the bottom, an `Appended` turn scrolls the view so the new last row is visible, and the selection follows if it was on the last section.
  - Implemented in `src/components/transcript/updates.rs::change_turns` (`pinned`, `was_last`, `pin_to_bottom`).
  - Covered by `follow::at_the_bottom_an_appended_turn_scrolls_into_view` and `the_selection_follows_when_it_was_on_the_last_section`.
  - The growing-turn half of the planner rule is now covered by `follow::the_selection_moves_to_the_new_last_section_of_a_growing_turn`. Its `assert_ne!` shows that `t8` gained a new last section, and its `assert_eq!(t.selected(), Some(&last))` shows that the selection moved there.
  - If `was_last` applied only to `Appended`, `restore_selection` would keep the old section, because `t1` still exists, and the `assert_eq!` would fail.
  - `a_selection_not_on_the_last_section_stays_when_the_last_turn_grows` covers the "otherwise kept" branch.
- [x] Scrolled up, an `Appended` turn keeps the offset and selection, and the count goes up by the number of new sections. `G` jumps to the end, resets the count and resumes following.
  - Implemented in `follow.rs` (`tail_sections`, `count_new`, `set_following`).
  - Covered by `follow::scrolled_up_an_appended_turn_keeps_the_offset_and_the_selection_and_is_counted`, `a_turn_with_several_sections_counts_them_all`, `new_sections_inside_an_updated_last_turn_are_counted`, `g_jumps_to_the_end_resets_the_count_and_resumes_following` and `the_end_key_does_the_same`.
- [x] The `App` shows `↓ 3 new` when the `Transcript` reports 3, and removes it at 0.
  - Implemented in `src/app/browser.rs`: `set_new_sections`, `sync_new_sections` and `refresh_status`.
  - Covered by `live::the_new_sections_message_shows_and_removes_the_indicator`, `scrolled_up_new_turns_are_counted_in_the_help_line_and_g_resets_it`, the snapshot `scrolled_up_after_three_new_sections_80x24_text`, and the new styled snapshot `the_indicator_is_drawn_in_help_text_80x24_styled`.
  - The styled test asserts `help_text` for columns 72..79 on row 23.
- [x] `Updated` for a visible turn (a tool result arriving) keeps the selection, fold states and scroll offset.
  - Covered by `follow::an_arriving_tool_result_keeps_selection_folds_and_scroll_offset`, plus the "keeps the screen still" tests for a selected section above the top row.
  - The new `update_following` call after `refresh` runs only when not following, and it can only turn following on when the last row is actually on screen. It does not move the offset, so this criterion still holds.
- [x] During the initial streaming load the view stays at the top. After `Loaded`, follow mode is on only if the last row is visible.
  - Implemented with the `loading` flag and `finish_loading`.
  - Covered by `follow::while_the_initial_load_streams_the_view_stays_at_the_top`, `after_loaded_a_tall_transcript_does_not_follow_and_stays_at_the_top` and `after_loaded_a_transcript_whose_last_row_is_visible_follows`.
  - `update_following` returns early while `loading`, and `counting` is false then, so the new call cannot turn following on during the load.
- [x] `● live` appears when `Activity.last_modified` is within 60 s of the injected now, goes out on a `Tick` after 60 s, and stays on when `idle == Some(false)`.
  - Implemented in `browser.rs::refresh_live` with `model::is_active_at`.
  - Covered by `live::the_marker_shows_for_activity_within_60_seconds`, `the_marker_goes_out_on_the_tick_after_60_seconds`, `not_idle_keeps_the_marker_however_old_the_activity_is` and `the_marker_is_re_evaluated_only_on_tick_changes_and_new_activity`.
- [x] Every UI state has a snapshot. All four states from the task's `UI` section are present and accepted:
  - live at the bottom, 120x40 text and styled, and 80x24 text;
  - scrolled up with `↓ 3 new`, 80x24 text;
  - the note plus `● live`, 120x40 text;
  - after `G`, 80x24 text.

  Round 2 adds a styled `↓ N new` snapshot.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran all three.

## Round-1 findings
- Feature must-fix 1 (following not re-evaluated after an in-place update): fixed in `updates.rs`, where `if counting { self.update_following(); }` runs after `self.refresh()`.
  - `follow::an_update_that_brings_the_end_into_view_resumes_following` reproduces the scenario from round 1: r1–r6 short, r7 tall, `G`, two scroll-ups, then r7 shrinks.
  - It asserts `following()` and a count of 0 after the shrink, then that the next `Appended` is shown and not counted. Without the new call, nothing on the `Updated` path sets `following` to true, so `assert!(t.following())` would fail. (This is reasoned from the code; I did not run it against the old code.)
  - The App learns of the reset count through `sync_new_sections`, which runs after every forwarded `Updated`, `Appended` and `Reset` (`browser.rs` lines 293, 305 and 310).
- Feature must-fix 2 (no test for the selection moving with a growing turn): fixed by the two new tests above.
- Code nit 2 (stale window): `at_bottom` now also requires `w.offset == self.offset`.
- Code nit 6 / feature nit 3 (redraw): the `Msg::NewSections` arm in `src/app/mod.rs` now requests `RedrawRequest::Dirty`.
- Docs: the `on`, `Msg::NewSections` and `NEW_SECTIONS_ATTR` doc comments are updated.
- UI nit 1 (narrow header): `browser_header::cwd_row` drops the note first, then the marker.
  - Covered by `a_row_too_narrow_for_the_note_and_the_marker_drops_the_note_first`, which checks that the row is `w` cells wide and ends in `│` at widths 44, 40, 30, 16 and 8.

## Findings
1. [nit] `src/components/transcript/updates.rs`, `reset` → `start_loading`. This round-1 nit is still open by choice. A `Reset` while following returns the view to the top until `Loaded`. The build notes leave it to the Approver. Change: none required, unless the Approver wants "stay at the end after a reload".
2. [nit] The planner's order rule, "`↓ N new` before any search position item", still has no test, because the browser has no search position item yet. Change: add an order test in the browser search task.
3. [nit] `src/components/transcript/mod.rs`, the doc comment of `AppComponent::on` for `Transcript`. After the edit, the sentence is wrapped unevenly: "...else [`Msg::TranscriptChanged`]\n/// when that changed what\n/// is shown, so that the App redraws." Change: reflow the paragraph.
4. [nit] There is no App-level test that the `↓ N new` item disappears from the help line when an `Updated` batch brings the end into view. The component test covers the count, and `sync_new_sections` after `Updated` is already exercised by other tests, so this is optional. Change: optionally add a `live::` test that applies a shrinking `Updated` through the rig and asserts the help line no longer shows `↓`.
