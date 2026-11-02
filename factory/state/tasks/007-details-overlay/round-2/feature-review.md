VERDICT: FAIL

## Summary
Round 2 fixes the resize behaviour, the too-small guard, the wrapping and most of the round-1 nits, each with a test that would fail if the behaviour were wrong, and all gates pass. The round-1 chord must-fix is only partly fixed: `details = "z i"` now closes the overlay, but `details = "z j"`, which loads with no warnings, still opens it and cannot close it, because the overlay context takes `j` before the pending chord can complete. I confirmed this with a probe in a scratch copy of the branch.

## Acceptance criteria
- [x] Keymap tests and the action table include `details`; help content lists it from the active keymap — `src/keymap/defaults.rs`, `EXPECTED` in `src/keymap/tests.rs`, `the_details_key_is_remappable_and_resolves_to_the_action`, `help_overlay::content::tests::the_sections_group_lists_the_details_key_from_the_active_keymap`.
- [x] Component snapshots cover every kind at both sizes and themes, including content taller than the screen — `details_overlay::snapshots::every_kind_of_section_at_both_sizes_in_both_themes` (36 files), `content_taller_than_the_screen_at_80x24_scrolled_to_the_top_and_to_the_end` (4 files). The fixtures hold the groups of each kind, fully populated from plain data.
- [x] `App::update` tests cover opening, closing with both keys, focus returning, a `More` row and no selection — `browser_tests::details::{the_details_key_opens_the_overlay_for_the_selected_section_and_gives_it_the_focus, esc_closes_the_overlay_and_returns_the_focus_to_the_transcript, the_details_key_closes_it_too_and_does_not_reopen_it, a_more_row_opens_the_details_of_its_tool_call, with_no_selection_the_key_does_nothing}`.
- [ ] A remapped `details` key opens and closes the overlay, and the help overlay shows the remapped key — met for single keys and for a chord made of keys the overlay context does not use (`a_remapped_details_key_opens_and_closes_the_overlay`, `a_details_key_bound_to_a_chord_opens_and_closes_the_overlay`, `the_help_overlay_shows_the_remapped_details_key`). A chord containing an overlay key opens and does not close. See finding 1.
- [x] A test shows the overlay is a snapshot — `what_it_shows_does_not_change_when_the_turn_does`, with a control that a reopened overlay shows the new value.
- [x] A test shows copy, save and export exclude the content and their keys do nothing while it is open — `copy_save_and_export_do_nothing_while_it_is_open` (now presses `Ctrl-s`), `transcript::tests::details::the_content_is_not_part_of_copy_save_or_export`.
- [x] Mouse tests cover the wheel, a left press outside, focus return, and nothing under the overlay getting events — `the_wheel_scrolls_the_overlay_and_a_press_outside_closes_it` (now also on a transcript cell outside the overlay, comparing the presentation), `nothing_under_the_overlay_gets_the_mouse`, `mouse_tests::nothing_under_an_overlay_is_a_target`.
- [x] A test shows `q`, `t`, `?` and `Ctrl-o` do nothing while it is open and `Ctrl-c` quits — `the_global_keys_do_nothing_while_it_is_open` (with a control that `q` quits once closed), `ctrl_c_quits_from_the_overlay`.
- [x] Row tests cover group order, an empty group, a missing field, no `cost`, and Turn group presence — `details_overlay::rows::tests::{the_groups_come_in_the_order_of_the_table, a_missing_field_is_left_out, there_is_no_cost_row_without_a_cost, the_turn_group_is_on_every_section_of_an_assistant_turn, a_users_sections_have_no_turn_group, a_compaction_has_no_turn_group_but_its_timestamp}`, `content::tests::a_group_without_rows_is_left_out`.
- [x] `HelpOverlay` and `DetailsOverlay` share one frame and scrolling implementation, and the help snapshots differ only by the new row — `src/components/overlay_frame.rs`. The round-2 commit changes no help snapshot, so the round-1 comparison of those five files still holds.
- [x] The README's browser table has the `details` row, and `tests/release_docs.rs` passes — `README.md`, `readme_keybindings_match_the_default_keymap`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — rerun here with `INSTA_UPDATE=no`: 2766 passed, 0 failed.
- [x] `cargo test --release -- --ignored budgets --nocapture` stays within budget — rerun here: first content worst 2.0 ms, full parse worst 305.1 ms, frames worst 3.44 ms, memory peak 0.58x.
- [x] Snapshot changes are limited to the new overlay snapshots and the help snapshots that gain the row — `git diff main...HEAD` adds 44 snapshots and modifies 5, all help. In round 2 only `notice_120x40_{default,terminal}` changed, by the one continuation row that the wrapping fix moves. No `.snap.new` files.

Round-1 requests checked in the code and tests:
- Resize: `App::refresh_details_size` runs first in `push_viewports`, before `refresh_routing`. Covered by `a_wider_screen_gives_the_overlay_the_width_its_rows_want` and `a_narrower_screen_makes_the_overlay_as_high_as_the_wrapped_rows`.
- Too small: the guard in `open_details`, covered by `it_does_not_open_on_a_screen_that_is_too_small_to_show_it`.
- Wrapping and control characters: `content::tests::{a_space_the_cut_falls_on_does_not_start_the_next_row, a_wrapped_value_with_spaces_loses_only_the_spaces_at_the_cuts, a_tab_is_a_space_and_trailing_newlines_are_not_rows}`.

## Findings
1. [must-fix] A `details` key bound to a chord that contains an overlay key opens the overlay but cannot close it. This breaks SPEC.md §5.1 ("The `details` key also closes it") and the remapped-key criterion, and it is what remains of the round-1 must-fix.
   - **Cause:** `DetailsOverlay::on` (`src/components/details_overlay/mod.rs:161-176`) calls `self.frame.on_key` first and resets the resolver on any overlay-context key, so the resolver never sees the key that would complete the chord.
   - **Probe, second key:** with `details = ["z j"]`, which gives no config warnings, the transcript's resolver answers `Pending` then `Action(Details)`. The overlay answers `None` to `z` and `Some(DetailsScrolled)` to `j`. `"z k"` loads with no warnings too, and neither of its keys closes the overlay.
   - **Probe, first key:** with `details = ["k i"]`, `k` drops out of `prev` with a config warning, the transcript opens on the chord, and the overlay answers `None` to both keys.
   - **Why the planner note does not settle this:** the note resolves `Context::Overlay` first "as in `HelpOverlay::on`", which covers one key with two meanings, as `the_overlay_keys_come_first` pins for a single `details = "j"`. The help overlay's context has no chords, and a pending `z` followed by `j` is not ambiguous, because `z` means nothing in the overlay. The single-key behaviour should stay.

   Change, in `DetailsOverlay::on`: feed every key to the resolver before the frame.
   ```rust
   let completes_chord = self.resolver.is_pending();
   let resolved = self.resolver.feed(self.frame.keymap(), key);
   let is_details = resolved == Resolved::Action(Action::Details);
   if completes_chord && is_details {
       return Some(Msg::CloseDetails);
   }
   ```
   Then call `frame.on_key` as now, drop the `reset()`, and in the `FrameKey::Unbound` arm answer `Msg::CloseDetails` when `is_details`, otherwise `None`. A single `details = "j"` still scrolls, and `an_overlay_key_drops_a_pending_prefix` still holds, because `feed` drops a prefix that a key does not complete. I tried this change in the scratch copy: `"z j"`, `"z k"` and `"k i"` close, a single `"j"` still scrolls, and all tests matching `details` in the library pass.

   Add component tests in `details_overlay/mod.rs`:
   - `"z j"`: `z` gives `None`, `j` gives `Some(Msg::CloseDetails)`, and the offset stays 0.
   - `"k i"`, with the view scrolled down first: `k` gives `Some(Msg::DetailsScrolled)`, `i` gives `Some(Msg::CloseDetails)`.

   Add an app-level test in `src/app/browser_tests/details.rs` that opens and closes the overlay with `"z j"`.
2. [nit] The build notes list code-review nit 2 (no theme clone per draw) as done, but only `DetailsOverlay` was changed. `src/components/help_overlay/mod.rs:62` still has `let theme = self.frame.theme().clone();`. Change: use `let theme = self.frame.theme();` and pass `theme` to `lines`.
3. [nit] The build notes list the doc rewrap as done, but three edited doc lines still run past the wrap width: `src/app/layout.rs:305` (`help_overlay_rect`), `src/app/mouse.rs:220` (`outside_overlay`) and `src/components/help_overlay/mod.rs:101` (`on_mouse`). Change: rewrap them.
