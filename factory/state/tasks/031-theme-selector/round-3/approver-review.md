## Rework note
Fix the truncation of invalid theme rows so the ` (invalid)` suffix always shows. Make only the changes below and leave the rest of the code as it is.

1. **Keep the suffix when truncating** in `src/components/theme_selector/rows.rs` `ThemeRows::line`:
   - For a row that is not selectable (`!item.selectable`), truncate only `item.id` with `truncate_with_ellipsis`. The width is `width − ROW_CHROME − " (invalid)".width()`, computed with `saturating_sub`. Always append `" (invalid)"` after the truncated id.
   - Build the row text in one small helper that both the cursor branch and the muted branch use.
   - Keep `ThemeItem.label` as the full `<id> (invalid)`, so the overlay width rule (`max(41, longest label + 10)`, `mod.rs` around line 100) still counts the whole suffix.
   - If even the suffix doesn't fit (the label space is narrower than `" (invalid)"` plus one column for the id), fall back to truncating the whole label as it does now. This cannot happen in the app at 60 columns or more, but the component can be rendered at any width, so the code must not underflow or panic.
2. **Fix the test.** In `src/components/theme_selector/mod.rs` `a_label_wider_than_the_overlay_is_cut_with_an_ellipsis`, change the `rows[4]` assertion so the row ends in `… (invalid) │`. With 34 label columns that should be about `│    an-extremely-long-inval… (invalid) │`. Take the exact text from what `truncate_with_ellipsis` actually produces, not from this note.
3. **Add a snapshot.** Add a 60x20 text snapshot through `App::view`, over the browser fixture like the other 60x20 snapshots, with a user theme whose invalid id is long enough to be truncated (e.g. `catppuccin-mocha-custom-high-contrast-v2-test.toml`, 45 columns). The row must read `<truncated id>… (invalid)`, with one blank column before the right border.
4. **Make three small code-review nits** in `src/config/edit.rs` and the test file:
   - `with_theme`, `Some(Item::Value(old))` arm: handle the case where `old.span()` is `None` explicitly, with the same `DocumentMut` replacement the `Some(_)` arm uses, so control can never fall through into the insertion path and write a second `theme` line.
   - `with_theme`, `Some(_)` arm: reword the warning to match what reaches it, e.g. "config: replacing the `theme` table with the chosen theme; its contents are lost".
   - `src/app/browser_tests/theme_selector.rs` `every_id_is_themed_but_the_global_events`: reword the doc comment to say what the test actually checks, namely that every entry of `THEMED` is a themed `Id`. Do not refactor `THEMED` into a predicate: `apply_theme` also iterates the numbered `Subagent(n)` ids.
5. **Leave these unchanged on purpose:**
   - The line ending of an inserted `theme` line: cosmetic, and every original byte is kept.
   - The single-valid-theme-between-invalid-runs corner case: rare, and the cursor stays visible.
   - Long runs of 13 or more invalid themes: the fix would change key behaviour beyond this task's scope.
   - The help line under the overlay: matches the help overlay.
   - The in-place config write: keeps symlinked dotfiles intact.
   - The 16-theme scrolled snapshot: the task text is wrong, not the code.

**Done when:**
- A long invalid id renders as `<truncated id>… (invalid)` in the component test and in the new 60x20 snapshot.
- Every existing snapshot is unchanged.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` all pass, with no `.snap.new` files left behind.
