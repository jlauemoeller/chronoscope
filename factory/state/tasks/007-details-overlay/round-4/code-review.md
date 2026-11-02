VERDICT: PASS

## Summary
Round 4 does what the approver's note asks, and its code and tests match the note. Values are now measured and cut by grapheme with the same filter and width function that ratatui draws with, which fixes round 3's nit 2 (raised as a must-fix by the UI review) and removes nit 3. The size at opening now has a test, which fixes round 3's must-fix, and all gates pass with no snapshot changed.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2776 passed, 0 failed, 2 ignored, matching the build notes)

Also run: `INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture` passes. The second, larger run of the budget test shows first content 1.8 ms, full parse 432.1 ms, frames 1.45 ms and peak memory 0.58x. The worst frame across both runs is 4.82 ms.

Checks on the earlier findings and this round's change:
- **Round-3 finding 1 (must-fix):** fixed. `a_wider_screen_gives_the_overlay_the_width_its_rows_want` reads the rect before anything draws and asserts width 76 and height 14.
- **Round-3 finding 2 (nit):** fixed. `columns` and `char_wrap` go through `drawn_graphemes`.
- **Round-3 finding 3 (nit):** gone. `UnicodeWidthChar` is no longer imported.
- **Same measure as the drawing code:** in ratatui-core 0.1.2, `Span::styled_graphemes` applies the same control-character filter. Paragraph drawing and `reflow.rs` in ratatui-widgets 0.3.2 measure with `cell_width`. The filter also keeps the `debug_assert!` in `cell_width` from firing on a control character.
- **Snapshots:** `git diff 5b18587 HEAD -- '*.snap'` is empty, no `.snap.new` file exists, and the working tree stayed clean after the gates.

Temporary edits, run on a `git archive` export of `HEAD` in the scratchpad with `cargo test --lib details` (112 tests pass unedited):
- Per-`char` measuring fails `a_grapheme_is_measured_as_it_is_drawn` and `a_value_is_drawn_to_its_end_however_its_graphemes_are_measured`.
- `g.width()` in place of `cell_width` fails `a_value_is_drawn_to_its_end_however_its_graphemes_are_measured`.
- `overlay.desired_size(u16::MAX)` in `open_details` fails `a_wider_screen_gives_the_overlay_the_width_its_rows_want`.
- Removing the control-character filter fails `what_is_not_drawn_does_not_widen_the_overlay`.

## Findings
1. [nit] `src/components/details_overlay/content.rs:513-516,534-537` — `what_is_not_drawn_does_not_widen_the_overlay` and `a_grapheme_is_measured_as_it_is_drawn` each define the same `wanted` closure (the second is as the round-4 note gives it). Change: move it to a `fn wanted(value: &str) -> Size` helper in the `tests` module, next to `row`, and use it in both tests.
