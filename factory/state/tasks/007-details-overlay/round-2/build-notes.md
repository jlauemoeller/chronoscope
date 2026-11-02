STATUS: DONE

## Changes
- Chord `details` key (code-review 1, feature-review 1, ui-review 1): `DetailsOverlay` has a `KeyResolver` for the `browser` context. A key that is not an overlay key is fed to it and `Resolved::Action(Details)` answers `Msg::CloseDetails`; any overlay-context key drops a pending prefix.
- Resize (feature-review 2, ui-review 2, code-review 8): the overlay answers its wanted size through `DESIRED_SIZE_ATTR` for the width it is given with `MAX_WIDTH_ATTR`. `App::refresh_details_size` asks for it on every resize (first thing in `push_viewports`, before the routing layout is refreshed) and keeps only the size, not the content. The previous "fixed at opening" choice is dropped.
- Wrapping (ui-review 3, 4): a space a cut falls on is dropped so continuation rows start at the value column; a tab is shown as a space, `\r` is dropped and trailing newlines are not rows. The `notice_120x40_{default,terminal}` snapshots changed by that one continuation row (one column left).
- Too small (ui-review 5): `open_details` does nothing on a screen below the minimum size.
- Shared margin (code-review 7, feature-review 6): `layout::overlay_max_width` / `OVERLAY_MARGIN_COLUMNS`, used by `help_overlay_rect`, `open_details` and the snapshot tests.
- Nits: no theme clone per draw (2); removed `interior_from_value`, `DetailsContent::is_empty` and the unused `components` re-exports (3, feature-review 3); help doc link (4); `push_help_interior` uses `interior_of` (5); `open_help` doc (6); doc comments rewrapped (feature-review 7).
- Tests: `Ctrl-s` instead of `Ctrl-w` in the copy/save/export test (feature-review 4); the guard test is split, sends `Msg::OpenDetails` while open and covers help, theme selector and prompt guards, the `rig.down(0)` no-op is gone (9); the wheel/press-outside checks also use a transcript cell outside the overlay and compare the presentation (10, feature-review 5).

## Self-check
- Review: chord key does not close — `details_overlay::tests::{a_details_key_bound_to_a_chord_closes_it, a_lone_prefix_or_a_second_key_alone_does_not_close_it, an_overlay_key_drops_a_pending_prefix}`, `browser_tests::details::a_details_key_bound_to_a_chord_opens_and_closes_the_overlay`.
- Review: resize — `browser_tests::details::{a_wider_screen_gives_the_overlay_the_width_its_rows_want, a_narrower_screen_makes_the_overlay_as_high_as_the_wrapped_rows}` (rect, wrap, no `more`, routing of a press in the new rect), `details_overlay::tests::it_answers_the_size_it_wants_for_the_width_it_may_have`.
- Review: wrap and control characters — `content::tests::{a_space_the_cut_falls_on_does_not_start_the_next_row, a_wrapped_value_with_spaces_loses_only_the_spaces_at_the_cuts, a_tab_is_a_space_and_trailing_newlines_are_not_rows}`.
- Review: too small — `browser_tests::details::it_does_not_open_on_a_screen_that_is_too_small_to_show_it`.
- Review: guard and mouse test nits — `an_open_request_while_it_is_open_changes_nothing`, `it_does_not_open_over_the_help_overlay_the_theme_selector_or_a_prompt`, `the_wheel_scrolls_the_overlay_and_a_press_outside_closes_it`.
- Round 1 criteria are unchanged and still covered by the round-1 tests (see round-1/build-notes.md).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2766 passed, 0 failed)
- cargo test --release -- --ignored budgets --nocapture: within budget
