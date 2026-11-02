VERDICT: FAIL

## Summary
The approver's round-3 steps 1 to 3 and the round-2 code-review nits 1 to 7 are all done, and the shipped code behaves correctly: chords that use an overlay key open and close the overlay, and the gate and budgets pass. One must-fix remains, a test gap this round opened. Removing `refresh_details_size()` from `open_details` left `desired_size(max_width)` as the only path to the height on opening, and no test fails if that height is wrong. The other finding is a nit about variation-selector characters in the wrap code.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2774 passed, 0 failed, 2 ignored, matching the build notes)

Also run: `cargo test --release -- --ignored budgets --nocapture` passes (worst first content 2.1 ms, full parse 416.7 ms, frame 4.96 ms, peak memory 0.58x). The round-3 commit changes no `.snap` file, and no `.snap.new` file exists. The help snapshot changes on this branch are the new `details` row and the rows it moves down.

I ran these temporary edits in a scratch copy of the branch, not in the repo:
- Dropping the `completes_chord` early return fails `a_chord_that_ends_in_an_overlay_key_closes_it` and both app chord tests.
- `was pending && resolved == Action(Details)` fails `the_prefix_of_another_chord_does_not_make_a_single_overlay_key_close_it`.
- `completes` ignoring its `action` fails `completes_is_about_the_chord_and_not_about_the_key_alone`.
- The unconditional last `push` in `char_wrap` fails `a_space_the_cut_falls_on_does_not_start_the_next_row`.
- The raw `widest_row` measure fails `what_is_not_drawn_does_not_widen_the_overlay`.
- Removing `push_details_interior` from `open_details` fails four app tests.
- Removing `drop_details` fails `opening_another_session_drops_the_overlay`.

The one edit no test caught is in finding 1. In the scratch copy, 3 `tests/release_docs.rs` tests fail with or without any edit, because the copy leaves out files those tests read. They pass in the repo.

## Findings
1. [must-fix] `src/app/browser_tests/details.rs:575` (`a_wider_screen_gives_the_overlay_the_width_its_rows_want`) — No test covers the overlay's height on opening when a value wraps. The planner's wanted-size rule says "as high as the content plus the border", and the only code doing this at opening is now `overlay.desired_size(max_width)` at `src/app/details.rs:39`. With that line changed to `overlay.desired_size(u16::MAX)`, no test fails because of the edit. Opening the 110-column model at 80x24 then gives a rect 12 rows high instead of 14, two rows short of the wrapped content. Existing tests miss it for these reasons:
   - This test asserts only the width at opening.
   - Its later `overlay_rows` call corrects the size (the interior reads 12 after that call, even with the edit), so `narrow` does not show the short rect.
   - `a_narrower_screen_makes_the_overlay_as_high_as_the_wrapped_rows` opens at 200x50 and goes through the resize path.
   - The `desired_size` unit tests do not cover what `open_details` passes.

   The code is correct today; the test is what is missing. Change: directly after `assert_eq!(rect_of(&rig).width, 76);` at line 575 and before the `overlay_rows` call, add `assert_eq!(rect_of(&rig).height, 14, "as high as the wrapped rows");`. A probe of this path gives 14 with the code as it is and 12 with the edit above.
2. [nit] `src/components/details_overlay/content.rs:249-251,258-279` (`columns`, `char_wrap`) — Both count widths per `char`, but the terminal buffer draws by grapheme, so a character with a variation selector is undercounted. For example, `❤` + U+FE0F counts 1 column and draws 2. In a probe, a value of 50 of these measured 50 columns (`UnicodeWidthStr` gives 100), so it did not wrap far enough, and only 26 of the 50 were drawn. The rest were clipped, against "never hidden" (spec/SPEC.md §5.5). This task's values (model, effort, formatted numbers) cannot realistically hit it, but tasks 008 and 009 pass raw harness strings such as `cwd` and `gitBranch`. Change: in both functions, iterate `unicode_segmentation::UnicodeSegmentation::graphemes(text, true)` (already a dependency), measure each grapheme with `UnicodeWidthStr::width`, and count a grapheme containing a control character as 0 so that `what_is_not_drawn_does_not_widen_the_overlay` keeps passing. Add a wrap test with this value.
3. [nit] `src/components/details_overlay/content.rs:10-11` — Two `use` lines import from `unicode_width`. Change: merge them into `use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};`.
