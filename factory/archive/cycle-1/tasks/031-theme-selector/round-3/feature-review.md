VERDICT: PASS

## Summary
Round 3 fixes the one must-fix from round 2: a long invalid theme id is now cut on its own, and ` (invalid)` is always appended, so the suffix always shows. The component test asserts the corrected row, and a new 60x20 snapshot through `App::view` shows `catppuccin-mocha-custom-high-contrast-… (invalid)` with one blank column before the border. The three code nits from the rework note were made as asked. I re-ran the gate: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean, and `INSTA_UPDATE=no cargo test` reports 2110 passed and 0 failed. The working tree is clean afterwards, there are no `.snap.new` files, and the only snapshot change since round 2 is the one new `.snap` file.

## Acceptance criteria
- [x] Built-ins in §8.3 order with display names, then user themes alphabetically; an invalid theme shows `<id> (invalid)` muted and the cursor skips it.
  - Round-2 gap closed: `ThemeRows::label_text` in `src/components/theme_selector/rows.rs` builds the text for both the cursor row and the muted rows. For a theme that can't be selected, it cuts only `item.id`, to `room − " (invalid)".width()` columns, then appends `INVALID_SUFFIX`. When the space is too narrow even for the suffix, it falls back to cutting the whole label, using `saturating_sub` so nothing underflows. `ThemeItem.label` is still the full `<id> (invalid)`, so the overlay width rule is unchanged.
  - Tests: `a_label_wider_than_the_overlay_is_cut_with_an_ellipsis` (now `│    an-extremely-long-inval… (invalid) │`), `an_invalid_row_in_a_narrow_overlay_keeps_the_suffix_or_falls_back_without_panicking` (widths 0–16), `a_long_invalid_id_keeps_its_invalid_suffix_at_60x20` (app-level, with a snapshot), plus the existing order, skip and muted tests from earlier rounds.
- [x] On open, the cursor is on the active theme. Unchanged since round 2. Tests: `the_cursor_starts_on_the_active_theme` (component and app), `an_active_user_theme_starts_the_cursor_on_it`.
- [x] Moving the cursor emits `PreviewTheme`, and the App applies it to every mounted component. Unchanged. Tests: `moving_the_cursor_applies_the_theme_to_every_mounted_component`, `the_previewed_screen_is_the_screen_of_an_app_started_with_that_theme`, and the styled snapshot `the_themes_after_three_j_120x40_gruvbox_styled`.
- [x] `Esc` restores the original theme exactly; `Enter` keeps the new one and closes. Unchanged. Tests: `esc_restores_the_original_theme_exactly_and_returns_the_focus`, `esc_saves_nothing`, `enter_keeps_the_theme_closes_the_selector_and_returns_the_focus`.
- [x] Saving.
  - An existing config changes only in its `theme` value, a missing config is created as exactly `theme = "<id>"\n`, and an invalid TOML file is left untouched with a toast.
  - Change this round in `src/config/edit.rs` `with_theme`: a value with no span now goes through `replaced_by_document`, the same replacement the non-value arm uses. It can no longer fall through into the insertion path and add a second `theme` key.
  - The existing byte-exact tests still pass.
- [x] `t` opens the overlay from the browser and the selector; `q` does nothing while it is open; `Ctrl-c` quits. Unchanged; the tests still pass.
- [x] With more themes than fit, the list scrolls to keep the cursor visible. Unchanged. The scrolled snapshot uses 16 themes, because 12 fit exactly at 60x20 under the binding height rule. The Approver accepted this deviation in round 2.
- [x] Every UI state has a snapshot: states 1–5 as before, the trailing-invalid snapshot from round 2, and the new long-invalid-id snapshot at 60x20.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran them with `INSTA_UPDATE=no`: 2110 passed, 0 failed, 1 pre-existing ignored.

## Findings
1. [nit] `src/config/edit.rs` `with_theme`, `Some(_)` arm: the new warning says "the `theme` table". That arm is also reached for an array of tables (`[[theme]]`), so "table" is not quite exact there. Change (optional): say "entry" instead of "table", e.g. "config: replacing the `theme` entry (not a string) with the chosen theme; its contents are lost".
2. [nit] `src/components/theme_selector/mod.rs` `an_invalid_row_in_a_narrow_overlay_keeps_the_suffix_or_falls_back_without_panicking`: the fallback case at width 16 checks only that the row starts with `│    some` and contains `…`. It does not pin the exact text, so a change in where the fallback cuts would go unnoticed. Change (optional): assert the exact row text, as the 25-column case does.
