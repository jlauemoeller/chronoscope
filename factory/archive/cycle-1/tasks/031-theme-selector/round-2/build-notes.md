STATUS: DONE

## Changes
- `src/components/theme_selector/mod.rs`: a new `move_cursor` puts the cursor on a theme and, when it is the first or last theme that can be selected, also scrolls the unselectable (invalid) rows before/after it into view (`ListCursor` is untouched; `view` still has no side effects). It runs on every cursor step and again when the visible rows are announced.
- `src/components/theme_selector/rows.rs`: labels wider than the overlay are cut with `truncate_with_ellipsis` (inner width less the 4-column indent and 1 column gap).
- `src/config/edit.rs`: adding a `theme` key now splices one line into the original text (after the last top-level key-value, else at the top before the first table, else at the end) instead of re-rendering the document, so mixed line endings and everything else stay byte-identical; the line ends like the file's first line. `restore_line_endings` is gone. Replacing a non-value `theme` entry logs a warning.
- `src/app/theme_selector.rs`: `THEMED` is `pub(super)`, doc comment of `open_theme_selector` corrected. `src/components/mod.rs`: dropped the unused `THEME_SELECTOR_MIN_WIDTH` re-export.
- `src/app/browser_tests/theme_selector.rs`: the test iterates the production `THEMED` instead of its own copy.

## Self-check
- Review (ui must-fix 1, invalid rows at the ends of a scrolled list) — components::theme_selector::tests::{a_trailing_invalid_theme_is_scrolled_into_view_with_the_last_valid_one, a_leading_invalid_theme_is_scrolled_back_into_view_with_the_first_valid_one, the_invalid_rows_show_when_the_rows_are_announced_after_the_cursor_moved, several_trailing_invalid_themes_show_when_they_fit, a_long_run_of_invalid_themes_never_hides_the_cursor} and the new text snapshot snapshot_scrolled_with_a_trailing_invalid_theme_60x20_text (`zz-broken (invalid)` on the last row, cursor on `User 09`)
- Review (ui nit 3, long label) — a_label_wider_than_the_overlay_is_cut_with_an_ellipsis
- Review (code nit 2, mixed line endings) — config::edit::tests::{mixed_line_endings_stay_as_they_are, the_theme_goes_after_the_last_top_level_key_not_into_a_table, a_file_of_only_comments_gets_the_theme_at_the_end}
- Review (code nit 3) — warning logged when a non-value `theme` is replaced
- Review (code nit 4 / feature nit 2) — test uses `THEMED` directly
- Review (code nits 5, 6) — re-export dropped, doc comment fixed
- Review (code nit 7, in-place write) — left as is on purpose: keeps symlinked dotfiles intact
- Review (nit 1 of all reviews, 12 themes at 60x20) — task text contradiction, unchanged; see round 1 notes. Not a code change.
- Review (ui nit 4, help line under the overlay) — no change, matches the help overlay.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2108 passed, 0 failed; 1 pre-existing ignored)
