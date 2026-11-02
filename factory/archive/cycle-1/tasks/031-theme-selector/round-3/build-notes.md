STATUS: DONE

## Changes
- `src/components/theme_selector/rows.rs`: `ThemeRows::label_text` builds the row text for both the cursor row and the muted rows. For a theme that cannot be selected only the id is cut (`width - 5 - " (invalid)".width()` columns, `saturating_sub`) and ` (invalid)` is always appended; if not even one id column fits beside the suffix it falls back to cutting the whole label (no underflow or panic). `ThemeItem.label` stays `<id> (invalid)`, so the overlay width rule is unchanged.
- `src/config/edit.rs`: a value with no span now takes the same `DocumentMut` replacement as the table arm (new helper `replaced_by_document`), so it can never fall through to the insertion path; warning reworded to "replacing the `theme` table with the chosen theme; its contents are lost".
- `src/app/browser_tests/theme_selector.rs`: doc comment of `every_id_is_themed_but_the_global_events` now says what it checks (`THEMED` stays a list).
- Left unchanged on purpose, as the rework note says: inserted line ending, single valid theme between invalid runs, 13+ invalid runs, help line under the overlay, in-place write, the 16-theme snapshot.

## Self-check
- Review (round 2 ui/code must-fix: suffix cut off) — components::theme_selector::tests::a_label_wider_than_the_overlay_is_cut_with_an_ellipsis (row now `│    an-extremely-long-inval… (invalid) │`), an_invalid_row_in_a_narrow_overlay_keeps_the_suffix_or_falls_back_without_panicking (also widths 0..16)
- Review (new 60x20 snapshot, 45-column invalid id over the browser) — browser_tests::theme_selector::a_long_invalid_id_keeps_its_invalid_suffix_at_60x20 (row reads `catppuccin-mocha-custom-high-contrast-… (invalid) │`, one blank column before the border)
- Review (code nits 2, 3) — config::edit changes above; existing config::edit tests still pass
- Review (code nit 4) — doc comment reworded
- Every existing snapshot is unchanged (only one new .snap file).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (2110 passed, 0 failed; 1 pre-existing ignored)
