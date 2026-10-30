VERDICT: FAIL

## Summary
Round 2 fixes the round-1 UI must-fix. `ThemeSelector::move_cursor` now scrolls invalid rows into view when they sit before the first selectable theme or after the last one. This happens on cursor moves and when the visible rows are announced. `ListCursor` is unchanged, `view` has no side effects, and the new tests and 60x20 snapshot would fail without the fix. The code-review nits are addressed: `save_theme` now inserts the new `theme` line into the original text (I probed it with dotted keys, multi-line values, arrays of tables, a BOM, CRLF and inline tables, and every result was valid TOML with only one line added), and `THEMED` is shared with its test. One new problem blocks: the new label truncation cuts the ` (invalid)` suffix off long invalid theme ids, and a test asserts that wrong output.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2108 passed, 0 failed, 1 pre-existing ignored; no `.snap.new` / `.pending-snap` files)

## Findings
1. [must-fix] `src/components/theme_selector/rows.rs` `ThemeRows::line` (`truncate_with_ellipsis(&item.label, …)`) together with `ThemeItem::invalid`, whose label is `format!("{id} (invalid)")`. Truncating the whole label removes the ` (invalid)` suffix first. A long invalid id therefore renders as `an-extremely-long-invalid-theme-i…`, a plain muted row that looks like any other unselected theme and that the cursor silently skips.
   - This breaks §8.4 ("listed muted with `(invalid)`") and the task's UI section (`<id> (invalid)` in `muted_foreground`).
   - It is reachable: the overlay is capped at `w − 4`, so a 47+ column invalid id at 60 columns is enough.
   - The test `a_label_wider_than_the_overlay_is_cut_with_an_ellipsis` in `src/components/theme_selector/mod.rs` asserts the wrong row `│    an-extremely-long-invalid-theme-i… │`.

   Change:
   - Truncate only the id and always keep the suffix. For example, keep the id and an `invalid` flag apart when rendering (or reserve `" (invalid)".width()` columns), then render `format!("{} (invalid)", truncate_with_ellipsis(id, width - ROW_CHROME - suffix_width))`.
   - Update the test to expect `│    an-extremely-long-inval… (invalid) │`, or whatever the exact fit is for 34 label columns.
   - Optionally, add a case where the width is so small that only the suffix fits.
2. [nit] `src/config/edit.rs` `with_theme`, `Some(Item::Value(old))` arm. If `old.span()` were `None`, control would fall out of the `match` into the insertion path and add a second `theme = …` line, which is a duplicate key and an invalid config. This can't happen today because `Document::parse` always records value spans, but the fall-through is implicit. Change: handle the no-span case explicitly with the same `DocumentMut` replacement as the `Some(_)` arm (`let mut doc = parsed.into_mut(); doc["theme"] = toml_edit::value(id); return Ok(doc.to_string());`).
3. [nit] `src/config/edit.rs` `with_theme`, `Some(_)` arm. The warning says "replacing the non-string `theme` entry". A non-string value (`theme = 1`) takes the span path and logs nothing; only tables and arrays of tables reach this arm. Change: reword it, e.g. "config: replacing the `theme` table with the chosen theme; its contents are lost".
4. [nit] `src/app/browser_tests/theme_selector.rs` `every_id_is_themed_but_the_global_events`. The new doc comment says "every themed one must be in `THEMED`", but the test only iterates `THEMED` and checks that each entry is classified as themed. If a new `Id` is added to the match as themed but left out of `THEMED`, the test still passes, so the round-1 drift concern is only half covered. Change: either replace `THEMED` with an exhaustive `fn is_themed(Id) -> bool` in `src/app/theme_selector.rs` and have `apply_theme` and the test use it, or reword the comment to say what the test actually checks.
5. [nit] Carried over from round 1, no code change needed. The task's "12 themes at 60x20" scroll example contradicts the binding height rule `min(h − 2, rows + 5)`. The builder documented this and covers both the boundary (`twelve_themes_fit_exactly_at_60x20`) and the scrolled case (16 themes). The Approver and Planner should settle it in the task text.
