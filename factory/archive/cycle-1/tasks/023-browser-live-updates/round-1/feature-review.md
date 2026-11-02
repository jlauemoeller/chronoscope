VERDICT: FAIL

## Summary
Follow mode, the `↓ N new` count, in-place updates and the `● live` marker are implemented largely as the spec and planner notes describe. The six required snapshots are present, and the gate passes: `cargo fmt --check` is ok, `cargo clippy --all-targets -- -D warnings` is clean and `INSTA_UPDATE=no cargo test` is all green. Two things block a pass:
- An in-place update that brings the end into view does not resume following. I confirmed this with a probe test on a scratch copy: the next appended turn is counted and stays off screen.
- The binding rule "a growing turn moves the selection to the new last section" has no test.

## Acceptance criteria
- [x] At the bottom, an `Appended` turn scrolls the view so the new last row is visible, and the selection follows if it was on the last section.
  - Implemented in `updates.rs::change_turns` (`pinned`, `was_last`, `pin_to_bottom`).
  - Tests: `transcript::tests::follow::at_the_bottom_an_appended_turn_scrolls_into_view`, `the_selection_follows_when_it_was_on_the_last_section`, `a_selection_elsewhere_is_kept_while_the_view_follows`, `a_growing_last_turn_keeps_the_view_at_the_end`, and `live::at_the_bottom_new_turns_scroll_into_view_through_the_app`.
  - The `Updated` (growing turn) half of the planner's rule has no test; see finding 2.
- [x] Scrolled up, an `Appended` turn keeps the scroll offset and selection, and the count goes up by the number of new sections. `G` jumps to the end, resets the count and resumes following.
  - Implemented in `follow.rs` (`tail_sections`, `count_new`, `set_following`) and in `perform`, which calls `update_following`. The key reports `Msg::NewSections`.
  - Tests: `follow::scrolled_up_an_appended_turn_keeps_the_offset_and_the_selection_and_is_counted`, `a_turn_with_several_sections_counts_them_all`, `new_sections_inside_an_updated_last_turn_are_counted`, `g_jumps_to_the_end_resets_the_count_and_resumes_following`, `the_end_key_does_the_same`.
- [x] The `App` shows `↓ 3 new` in the help line at 3 and removes it at 0.
  - Implemented in `browser.rs::set_new_sections` and `refresh_status`.
  - Tests: `live::the_new_sections_message_shows_and_removes_the_indicator`, `scrolled_up_new_turns_are_counted_in_the_help_line_and_g_resets_it`, and the snapshot `scrolled_up_after_three_new_sections_80x24_text`.
- [x] `Updated` for a visible turn (a tool result arriving) keeps the selection, fold states and scroll offset.
  - Test: `follow::an_arriving_tool_result_keeps_selection_folds_and_scroll_offset`. It also checks the expansion state and that the result is drawn.
  - Also covered by `an_update_of_a_visible_turn_does_not_move_the_viewport_when_scrolled_up` and the two "keeps the screen still" tests for a selected section above the top row.
- [x] During the initial streaming load the view stays at the top. After `Loaded`, follow mode is on only if the last row is visible.
  - Implemented with a `loading` flag that is set at mount and cleared by `finish_loading` on `Loaded`. The App now forwards `Loaded` to the transcript.
  - Tests: `follow::while_the_initial_load_streams_the_view_stays_at_the_top`, `after_loaded_a_tall_transcript_does_not_follow_and_stays_at_the_top`, `after_loaded_a_transcript_whose_last_row_is_visible_follows`, `a_reset_starts_a_load_again`, and the `live::` counterparts.
  - A `Reset` reload ends in `Loaded` (`watch.rs::reload` and `load.rs`), so the transcript is never left stuck in loading.
- [x] `● live` appears when `Activity.last_modified` is within 60 s of the injected now, goes out on a `Tick` after 60 s, and stays on when `idle == Some(false)`.
  - Implemented in `browser.rs::refresh_live`, which runs from `tick_elapsed` and on `Activity` and uses `model::is_active_at`.
  - Tests: `live::the_marker_shows_for_activity_within_60_seconds`, `the_marker_goes_out_on_the_tick_after_60_seconds` (checks the 60 s boundary), `not_idle_keeps_the_marker_however_old_the_activity_is`, and `the_marker_is_re_evaluated_only_on_tick_changes_and_new_activity` (checks that the value is pushed only when it changes).
- [x] Every UI state has a snapshot:
  - live at the bottom: 120x40 text and styled, and 80x24 text;
  - scrolled up with `↓ 3 new` at 80x24 text, with rows 0–22 asserted unchanged;
  - the note plus `● live` at 120x40 text;
  - after `G` at 80x24 text.
  
  The `browser_header` unit tests also assert the header placement: the marker ends at column `w − 3`, there is a 2-cell gap to the note, `●` is primary and `live` is muted_foreground.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I ran all three.

## Findings
1. [must-fix] Following is not re-evaluated after an in-place update. This contradicts §7.2 ("if the viewport is at the end of the transcript, new content scrolls into view automatically") and the planner rule that `following` is true while the viewport shows the last row.
   - Where: `src/components/transcript/updates.rs::change_turns`. `update_following` runs only after a `Cmd`, a viewport change or `Loaded`. Suppose the user is scrolled a few rows above the end and an `Updated` batch makes a turn shorter. `refresh` then clamps the offset, or the shorter content simply ends inside the viewport, so the last row is on screen while `following` stays false. The next `Appended` turn is counted (`↓ 1 new` shows while the user sits at the end) and is not scrolled into view.
   - Reproduced on a scratch copy at 80x24 with turns r1–r6 short and r7 tall:
     1. `G`, then two `scroll_up`; `following` is false.
     2. `Updated(r7 short)`. The result is offset 11, total 35, so the end is visible, but `following` is still false.
     3. `Appended(r8)`. The offset stays at 11, the total is now 40, `new_sections` is 1 and `following` is false.
   - Change: in `change_turns`, after `self.refresh()`, call `self.update_following()` when `counting` was true. This also resets the count, and `App::sync_new_sections` already reads it back. Add a component test of the scenario above: after the shrink, assert `t.following()`; after the append, assert `shows_last_row(&mut t, 24)` and `t.new_sections() == 0`.
2. [must-fix] The binding planner rule for growing turns has no test. The rule: "While following, `Appended` turns and growing turns keep the view pinned to the last row. When the selection was the last section before the update, it moves to the new last section".
   - The code does handle it: `was_last` applies to `Updated` as well. I confirmed this with a probe in which `Updated(t8 with a new item t2)` moves the selection from `t8/t1` to `t8/t2`.
   - No test would fail if the move were limited to `Appended`. `the_selection_follows_when_it_was_on_the_last_section` uses only `Appended`. `a_growing_last_turn_keeps_the_view_at_the_end` grows text without adding a section, so it only checks that the selection stays on its turn.
   - This is the common live case: a new tool call or text item arrives in the current assistant turn while the user sits on its last section.
   - Change: in `src/components/transcript/tests/follow.rs`, add a test that starts from `at_the_end()` with the selection on `last_section_of(&t, "t8")`. Apply `ModelUpdate::Updated` with `t8` carrying an extra item, for example `blocks::text("t2", ...)` after `t1`. Assert that the selection equals `last_section_of(&t, "t8")` for the new item, that `shows_last_row(&mut t, 24)` holds, and that `new_sections() == 0`.
3. [nit] `src/app/mod.rs`: the `Msg::NewSections(count)` arm only calls `set_new_sections`, which asks for a redraw only when the count differs from the App's copy. The key that sent the message also moved the view (for example `G`). So the redraw depends on the App's copy and the transcript's count always being in sync. They are in sync today, but nothing enforces it. Change: also call `self.request_redraw(RedrawRequest::Dirty)` in that arm, as `TranscriptChanged` does.
4. [nit] The App reads the count back after forwarded updates (`sync_new_sections` queries `NEW_SECTIONS_ATTR`), while the planner wording is "reports the count to the App as a `Msg`".
   - This is a reasonable deviation: an `attr` push cannot return a `Msg`. It goes through the public `query` API, and `TURN_COUNT_ATTR` already sets the precedent, so it does not reach into internals (§1.2).
   - Change: none required. Optionally, say so in the `NEW_SECTIONS_ATTR` doc comment.
5. [nit] As the build notes say, the "`↓ N new` before any search position" order cannot be tested yet, because the browser has no search position item. Add an order test when browser search adds its item.
6. [nit] There is no styled snapshot of the `↓ N new` item. The task lists that state as text only, and the `help_text` colour comes from the existing `HelpLine` status rendering, so this is not required. A styled variant of `scrolled_up_after_three_new_sections` would lock the "`↓ N new` in `help_text`" requirement.
