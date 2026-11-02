STATUS: DONE

## Changes
- `src/keymap/mod.rs`: `KeyResolver::completes(keymap, event, action)`: whether the key completes a chord bound to `action` with the pending prefix (not the key alone). Test: `keymap::tests::completes_is_about_the_chord_and_not_about_the_key_alone`.
- `src/components/details_overlay/mod.rs`: `on` feeds every key to the overlay's resolver once, before the frame. A key that completes a `details` chord closes the overlay whatever the key means in the overlay; otherwise the overlay keys come first, then a single `details` key closes it. The `reset()` call is gone (`feed` drops a prefix). `DESIRED_SIZE_ATTR` is answered with `payload_value`.
- `src/components/details_overlay/content.rs`: `widest_row` measures `display_lines` with `columns` (counted as `char_wrap` counts), so `\r` and control characters do not widen the overlay; `char_wrap` pushes no blank last row after a cut; the two existing wrapping tests got the additions from the note.
- `src/components/help_overlay/mod.rs`: no theme clone per draw; `on_mouse` doc rewrapped. `src/app/layout.rs`, `src/app/mouse.rs`: doc comments rewrapped.
- `src/app/details.rs`: `open_details` has one path to the size (max width set on the overlay, then `desired_size`); `push_viewports` keeps `refresh_details_size`.

## Self-check
- Review: a chord with an overlay key does not close — `details_overlay::tests::{a_chord_that_ends_in_an_overlay_key_closes_it, the_overlay_key_of_such_a_chord_scrolls_without_the_prefix, a_chord_that_starts_with_an_overlay_key_scrolls_and_then_closes_it, the_prefix_of_another_chord_does_not_make_a_single_overlay_key_close_it}`; app level `browser_tests::details::{a_details_chord_that_ends_in_an_overlay_key_opens_and_closes_the_overlay, chords_with_overlay_keys_open_and_close_the_overlay_with_the_same_keys}` (`z j` in the first; `z k` and `j i` in the second, with the same keys for open and close). One test more than the note counted (2774 instead of 2773).
- Review: wrapping and width nits — `content::tests::{a_space_the_cut_falls_on_does_not_start_the_next_row, a_wrapped_value_with_spaces_loses_only_the_spaces_at_the_cuts, what_is_not_drawn_does_not_widen_the_overlay}`.
- Review: theme clone, `payload_value`, doc rewraps, single size path in `open_details` — code changes as listed; existing tests unchanged and passing.
- Temporary edits (each made, checked, restored):
  - `on` closing only when the resolver was pending and `feed` answered `Action(Details)`: fails `the_prefix_of_another_chord_does_not_make_a_single_overlay_key_close_it`.
  - without the `completes` early return, feeding the resolver only in the `FrameKey::Unbound` arm: fails `a_chord_that_ends_in_an_overlay_key_closes_it`, `a_chord_that_starts_with_an_overlay_key_scrolls_and_then_closes_it`, the new app test `a_details_chord_that_ends_in_an_overlay_key_opens_and_closes_the_overlay`, and also `an_overlay_key_drops_a_pending_prefix` (which only holds with the resolver fed on every key).
- No `.snap` file changed in this round and no `.snap.new` is left.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (2774 passed, 0 failed)
- INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture: within budget
