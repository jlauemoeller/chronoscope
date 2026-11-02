VERDICT: REWORK

## Assessment
All three reviews FAIL on the same verified bug, and the fix is one clear change, so another round can settle it. Everything else from round 1 is resolved: invalid rows at either end of a scrolled list now scroll into view, `save_theme` inserts one line into the original text, `THEMED` is shared with its test, and the re-export and doc comment are fixed. The gate is green (2108 passed, 0 failed).

**Code review: FAIL**
1. [must-fix] Long invalid ids lose ` (invalid)` when truncated. **To be fixed.** I confirmed it in the code. `ThemeItem::invalid` builds the suffix into `label`, and `ThemeRows::line` in `src/components/theme_selector/rows.rs` runs `truncate_with_ellipsis(&item.label, …)` on the whole label, so the suffix is cut first. The test `a_label_wider_than_the_overlay_is_cut_with_an_ellipsis` asserts that wrong row. This breaks §8.4 and the first acceptance criterion. Both rows are muted, so the suffix is the only thing that marks a row as invalid; under `NO_COLOR` nothing else does.
2. [nit] No-span fall-through in `with_theme`. **To be fixed.** It is unreachable today, but it is a small, safe change that makes the code easier to read.
3. [nit] The warning wording in `with_theme`'s `Some(_)` arm is inaccurate. **To be fixed.** A one-line wording change.
4. [nit] The doc comment of `every_id_is_themed_but_the_global_events` claims more than the test checks. **To be fixed, by rewording the comment only.** I am not asking for an `is_themed(Id)` refactor: `apply_theme` also has to iterate the numbered `Subagent(n)` ids, so a predicate does not simply replace the list, and that refactor carries more risk than this nit is worth.
5. [nit] The task says 12 themes at 60x20. **Ignored here and referred to the planner.** See the deviations below.

**Feature review: FAIL**
1. [must-fix] Same as code finding 1. **To be fixed.**
2. [nit] 12 vs 16 themes at 60x20. **Ignored here.** See the deviations below.
3. [nit] With exactly one selectable theme between two invalid runs, the trailing run can stay hidden. **Ignored.** It is a rare corner case, the cursor is always visible, and the UI reviewer rendered that state and found it acceptable.
4. [nit] A new `theme` line takes its line ending from the file's first line. **Ignored.** Every original byte is kept, so the only effect is cosmetic, and only in mixed-ending files. Round 2's regression came from fixing a nit, so I am keeping the scope tight.
5. [nit] A missing key is added by splicing raw text instead of using `DocumentMut`. **Accepted.** See the deviations below.

**UI review: FAIL**
1. [must-fix] Same as code finding 1. **To be fixed**, together with the 60x20 snapshot of a long invalid id that the reviewer asks for.
2. [nit] 12 vs 16 themes. **Ignored here.** See the deviations below.
3. [nit] A run of 13 or more consecutive invalid themes is never fully shown. **Ignored.** It needs 13 or more broken theme files at the smallest size, the errors are logged, and the suggested fix (scrolling without moving the cursor) would change key behaviour beyond this task's scope.
4. [nit] The screen's help line stays visible under the overlay. **Ignored.** This matches the existing help overlay, and the spec doesn't address it.

Round 1 code nit 7 (atomic write): the builder kept the in-place write on purpose, so symlinked dotfiles stay intact. **Accepted.**

**Deviations from the task text (neither blocks):**
- **16 themes instead of 12 at 60x20 (AC and UI state 5).** The binding height rule `min(h − 2, rows + 5)` fits 12 themes at 60x20 (17 rows of 18), so the task's example cannot scroll. The builder kept the binding formula, proved the boundary with `twelve_themes_fit_exactly_at_60x20`, and took the scrolled snapshot with 16 themes. That meets the criterion's intent. The planner should correct the task text.
- **Raw-text splice instead of `DocumentMut::insert` for a missing `theme` key.** This departs from a binding planner note, but the note exists so that everything else stays byte-identical, and the splice keeps more bytes the same than re-rendering does (mixed line endings included). The insertion point is still found through `toml_edit` spans, and the Code Reviewer (higher priority) recommended this approach in round 1. Accepted.

## Conflicts
None. All three reviews raise the same must-fix and ask for the same change. The `DocumentMut` deviation (feature nit 5) is in line with the Code Reviewer's round-1 recommendation, and the Feature Reviewer asks for no change, so there is nothing to resolve.

## Rework budget
Earlier REWORK verdicts: 0 of 2. Round 1 has no `verdict.md`. This verdict is the first REWORK.

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
