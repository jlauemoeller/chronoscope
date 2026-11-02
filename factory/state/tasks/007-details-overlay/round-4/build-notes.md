STATUS: DONE

## Changes
- `src/components/details_overlay/content.rs`: values are measured and wrapped by grapheme, counted as the buffer draws them (`drawn_graphemes`: graphemes with a control character are dropped, the rest are whole, width from `CellWidth::cell_width`). `columns` and `char_wrap` use it, so the end of a value is never lost (emoji with a variation selector, ZWJ sequences, halfwidth katakana with a sound mark). Imports changed as the note says (`unicode_segmentation`, `CellWidth`).
- `src/app/browser_tests/details.rs`: `long_model_turn` split into `turn_with_model`; a new test draws values to their end; `a_wider_screen_gives_the_overlay_the_width_its_rows_want` now reads the rect (width 76, height 14) at opening, before anything draws.
- The all-spaces value behaviour (ui-review finding 2), `src/markdown/wrap.rs`, `open_details`, chord handling and all snapshots are unchanged, as the note says.

## Self-check
- `content::tests::a_grapheme_is_measured_as_it_is_drawn` and `browser_tests::details::a_value_is_drawn_to_its_end_however_its_graphemes_are_measured` (the sign with a variation selector and the halfwidth katakana, at 80x24, 120x40 and 200x30).
- `browser_tests::details::a_wider_screen_gives_the_overlay_the_width_its_rows_want` covers the size at opening.
- Temporary edits (each made, checked, restored; the files were restored from copies):
  - per-`char` measuring with `ch.width().unwrap_or(0)`: fails `a_grapheme_is_measured_as_it_is_drawn` and `a_value_is_drawn_to_its_end_however_its_graphemes_are_measured`.
  - `g.width()` in place of `cell_width()`: fails `a_value_is_drawn_to_its_end_however_its_graphemes_are_measured`.
  - `desired_size(u16::MAX)` in `open_details`: fails `a_wider_screen_gives_the_overlay_the_width_its_rows_want`.
- `what_is_not_drawn_does_not_widen_the_overlay` and every other existing test pass unchanged. No `.snap` file changed and no `.snap.new` is left.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (2776 passed, 0 failed)
- INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture: within budget
