VERDICT: PASS

## Summary
`HelpLine`, `Toast`, the selector hint builder and the startup toast text are added as separate components and modules. They follow §1.3/§1.4: plain-data props, theme tokens only, no literal key matching, and `Tick`-driven expiry with an injectable clock. The layout function now places the help line and the toast rect exactly as the planner notes say. Every acceptance criterion has a test that would fail if the behaviour were wrong. Every UI state has accepted snapshots (text, plus styled where colours matter), and no `.snap.new` files are committed.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [nit] `src/app/mod.rs` `show_toast_for` / `src/app/hints.rs` `startup_toast_text`: only the startup text has its line breaks folded into spaces. Any later caller of `show_toast` that passes a message with a `\n` (for example an error string in a copy/save/export confirmation) gets a toast whose `Paragraph` wraps onto a second, clipped line. Its layout width also comes from the unnormalised string. Change: fold whitespace (`split_whitespace().join(" ")`) once in `show_toast_for` before the text is stored and passed to `Toast::new`, and drop the duplicate folding from `startup_toast_text`. The `line_breaks_in_a_warning_become_spaces` test can then move to the App level.
2. [nit] `src/components/help_line.rs` `visible_hints` / `status_width`: `HINT_SEPARATOR.len()` and `STATUS_SEPARATOR.len()` are byte lengths that are used as display widths. This is correct only because both constants are ASCII. Change: use `.width()` (already imported via `UnicodeWidthStr`), so that changing a separator later cannot silently break the fit calculation.
3. [nit] `src/components/help_line.rs` `draw_status`: `width as u16` relies on a comment for its safety. Change: `let Ok(width) = u16::try_from(width) else { return };` (or keep the `min` and use `try_from(..).unwrap_or(row.width)`), which avoids an `as` cast.
4. [nit] `src/app/tests.rs` `three_startup_warnings_end_the_toast_with_two_more`: the `ends_with` assert is redundant after the `assert_eq!` on the full text. Change: drop one of the two.
5. [nit] `src/app/snapshots/*startup_frame_80x24_text.snap` / `*startup_frame_120x40_text.snap` and the new `*selector_help_line_80x24_text.snap` / `*selector_help_line_120x40_text.snap` hold identical frames. This is harmless, but a later intentional change will need to be accepted twice. Change (optional): keep one pair, or note in the test why both exist.
6. [nit] (For the planner/approver, no builder change required.) As the build notes point out, the planner-specified toast rect (bottom border on row `h − 2`, top on `h − 4`) covers part of the selector overlay's bottom border at 80x24. At 120x40 it covers the end of the `ESC to Cancel` hint mid-word (see `selector_with_toast_120x40_text.snap`, row 36: `… | ES╭───`). This matches the task's planner notes and the UI section, but later tasks (browser help line, task 021) may want to revisit the placement so a toast does not cut a hint in half.
