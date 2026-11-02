VERDICT: FAIL

## Summary
Round 3 fixes the chord must-fix and does everything in the approver's note: `z j`, `z k` and `j i` each open and close the overlay, a single `details = "j"` still scrolls, and all gates pass. One gap remains. Round 3 made `desired_size(max_width)` in `open_details` the only thing that sets the overlay's size at opening, and no test covers that line, because every test that draws the overlay first sends `Msg::Resized`, which recomputes the size. The code is correct today; the test is what is missing.

## Acceptance criteria
- [x] Keymap tests and the action table include `details`; help content lists it from the active keymap — `src/keymap/defaults.rs`, `EXPECTED` in `src/keymap/tests.rs:73`, `the_details_key_is_remappable_and_resolves_to_the_action`, `help_overlay::content::tests::the_sections_group_lists_the_details_key_from_the_active_keymap`.
- [x] Component snapshots cover every kind at both sizes and themes, including content taller than the screen — `details_overlay::snapshots::every_kind_of_section_at_both_sizes_in_both_themes` (36 files), `content_taller_than_the_screen_at_80x24_scrolled_to_the_top_and_to_the_end` (4 files).
- [ ] `App::update` tests cover opening, closing with both keys, focus returning, a `More` row and no selection — closing, focus, the `More` row and no selection are covered (`browser_tests::details::{esc_closes_the_overlay_and_returns_the_focus_to_the_transcript, the_details_key_closes_it_too_and_does_not_reopen_it, a_more_row_opens_the_details_of_its_tool_call, with_no_selection_the_key_does_nothing}`). Opening is covered for content and focus, but not for the size it opens at on a screen where values wrap. See finding 1.
- [x] A remapped `details` key opens and closes the overlay, and the help overlay shows the remapped key — `a_remapped_details_key_opens_and_closes_the_overlay`, `a_details_key_bound_to_a_chord_opens_and_closes_the_overlay`, `a_details_chord_that_ends_in_an_overlay_key_opens_and_closes_the_overlay`, `chords_with_overlay_keys_open_and_close_the_overlay_with_the_same_keys`, `the_help_overlay_shows_the_remapped_details_key`.
- [x] A test shows the overlay is a snapshot — `what_it_shows_does_not_change_when_the_turn_does`, with a control that a reopened overlay shows the new value.
- [x] A test shows copy, save and export exclude the content and their keys do nothing while it is open — `copy_save_and_export_do_nothing_while_it_is_open`, `transcript::tests::details::the_content_is_not_part_of_copy_save_or_export`.
- [x] Mouse tests cover the wheel, a left press outside, focus return, and nothing under the overlay getting events — `the_wheel_scrolls_the_overlay_and_a_press_outside_closes_it`, `nothing_under_the_overlay_gets_the_mouse`, `mouse_tests::nothing_under_an_overlay_is_a_target`. Removing the overlay from `OVERLAYS` in a scratch copy fails three tests.
- [x] A test shows `q`, `t`, `?` and `Ctrl-o` do nothing while it is open and `Ctrl-c` quits — `the_global_keys_do_nothing_while_it_is_open`, `ctrl_c_quits_from_the_overlay`. Removing the overlay from `key_clause` in a scratch copy fails the first.
- [x] Row tests cover group order, an empty group, a missing field, no `cost`, and Turn group presence — `details_overlay::rows::tests`, `content::tests::a_group_without_rows_is_left_out`, `transcript::tests::details::the_groups_follow_the_kind_of_the_section`.
- [x] `HelpOverlay` and `DetailsOverlay` share one frame and scrolling implementation, and the help snapshots differ only by the new row — `src/components/overlay_frame.rs`. Comparing main and HEAD, the 200x60 help interior differs by exactly the `details` row. The other four help snapshots show the same one-row shift in the diff, and round 3 changes none of them.
- [x] The README's browser table has the `details` row, and `tests/release_docs.rs` passes — `README.md`, `readme_keybindings_match_the_default_keymap`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — rerun here with `INSTA_UPDATE=no`: 2774 passed, 0 failed. That is one more than the note's 2773, because the build added `chords_with_overlay_keys_open_and_close_the_overlay_with_the_same_keys`.
- [x] `cargo test --release -- --ignored budgets --nocapture` stays within budget — rerun here: first content worst 2.1 ms, full parse worst 300.9 ms, frames worst 4.94 ms, memory peak 0.58x.
- [x] Snapshot changes are limited to the new overlay snapshots and the help snapshots that gain the row — `git diff main...HEAD` adds 44 and modifies 5, all help. The round-3 commit changes no `.snap` file, and there are no `.snap.new` files.

Approver's round-3 requests, checked in the code:
- `KeyResolver::completes` and its test are in `src/keymap/mod.rs` and `src/keymap/tests.rs`, as written in the note.
- `DetailsOverlay::on` feeds every key to the resolver once, before the frame, and the `reset()` call is gone. The four component tests and the app test are present.
- `the_overlay_keys_come_first` and `an_overlay_key_drops_a_pending_prefix` are unchanged and pass.
- Both temporary edits from the note, rerun in a scratch copy, fail the named tests. The second also fails `an_overlay_key_drops_a_pending_prefix` and `chords_with_overlay_keys_open_and_close_the_overlay_with_the_same_keys`.
- Further key probes in the scratch copy behave correctly: `j i` with content that fits (`j` answers nothing, `i` closes), `j k`, `j j`, `z esc`, `z z j`, `z pgdn`, and `g j` bound to `top` (`j` scrolls and does not close).
- The wrapping and width nits are done. Undoing the `char_wrap` change fails `a_space_the_cut_falls_on_does_not_start_the_next_row`, and undoing the `widest_row` change fails `what_is_not_drawn_does_not_widen_the_overlay`.
- `payload_value`, the removed theme clone in `HelpOverlay::view`, the doc rewraps and the single size path in `open_details` are all in the diff.

## Findings
1. [must-fix] No test covers the size the overlay opens at when its values wrap. The planner's "Wanted size" note and the task's UI section require the overlay to be as high as its wrapped content.
   - **Why this is new:** round 3 removed `refresh_details_size()` from `open_details` (`src/app/details.rs`), as the approver asked. `overlay.desired_size(max_width)` at `src/app/details.rs:39` is now the only source of the size at opening. Before, the refresh call corrected any mistake in this line.
   - **Probe:** in a scratch copy, changing that line to `overlay.desired_size(u16::MAX)` fails no test: the full `cargo test` suite still passes, 2774 of 2774. With that change, and a long model value at 80x24, the overlay opens with height 12 and gets height 14 only after a resize.
   - **Why the existing tests miss it:** `overlay_rows` and `screen_text` go through `text(...)`, which sends `Msg::Resized`. That runs `push_viewports` and `refresh_details_size`, and repairs the size before anything is asserted. The running app sends no resize after opening, so such a regression would hide the last rows under `↓ more` until the terminal is resized.

   Change: in `src/app/browser_tests/details.rs`, in `a_wider_screen_gives_the_overlay_the_width_its_rows_want` (line 572), read the rect straight after opening, before any `overlay_rows` call:
   ```rust
   let mut rig = open_long_model_at(80, 24);
   // Before anything draws: `overlay_rows` sends `Msg::Resized`, which asks the
   // overlay for its size again. The wrapped model takes its rows from the start.
   let at_open = rect_of(&rig);
   assert_eq!(at_open.width, 76);
   assert_eq!(at_open.height, 14);
   ```
   This replaces the existing `assert_eq!(rect_of(&rig).width, 76);` line; the rest of the test stays. Height 14 is what the code gives today. Check that the test fails with `desired_size(u16::MAX)` at `src/app/details.rs:39`, restore the line, and say so in the build notes. No `.snap` file changes.
