VERDICT: PASS

## Summary
The round-2 must-fix is resolved. `ThemeRows::label_text` (`src/components/theme_selector/rows.rs`) now cuts only the id of an invalid theme and always keeps ` (invalid)`, with a `saturating_sub` fallback that cannot underflow or panic at any width. The updated component test, the new narrow-width test and the new 60x20 snapshot (`catppuccin-mocha-custom-high-contrast-… (invalid) │`) would all fail on the round-2 code. The three nits are done as the rework note asked, and no existing snapshot changed (commit 9ba68ff adds one `.snap` file and modifies none).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2110 passed, 0 failed, 1 pre-existing ignored; working tree clean, no `.snap.new` / `.pending-snap` files)

## Findings
1. [nit] `src/config/edit.rs`: two doc comments overstate what is kept.
   - `save_theme`'s doc comment says "An existing file keeps everything but the value of `theme` byte for byte".
   - `with_theme`'s doc comment says "Everything outside the old value (or the new line) is copied from `text` as it is, so comments, spacing and line endings of any kind survive".

   Neither holds on the `replaced_by_document` path, which is taken when `theme` is a table or an array of tables. That path re-renders the whole document, as the helper's own comment says. Change: add a clause to both comments, for example "except when `theme` is a table, which is replaced and the document re-rendered through `DocumentMut`".
2. [nit] `src/components/theme_selector/mod.rs` `an_invalid_row_in_a_narrow_overlay_keeps_the_suffix_or_falls_back_without_panicking`. The test covers widths 25 (id branch) and 16 (fallback), but not the exact boundary where `room == suffix + 1`. Change (optional): also assert two more cases, so an off-by-one in `room > suffix` would be caught:
   - An area 18 wide renders the row as `│    … (invalid) │`.
   - An area 17 wide takes the fallback.
