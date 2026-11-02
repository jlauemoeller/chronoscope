VERDICT: FAIL

## Summary
The toast UI is unchanged from round 1 and still matches the `UI` section. All four snapshots pass with `INSTA_UPDATE=no`, and the gate passes: fmt, clippy and 1415 lib tests, with no `.snap.new` files. The round-1 must-fix is mostly fixed. A folded prompt is now copied at the transcript's width, cut where the fold cuts it, and ends in `…`. The fixture prompt at 80x24 copies exactly the expected line. One problem remains in the new `soft_joined`, which joins wrapped rows back into logical lines:
- It always adds a space at a soft-wrap break, even when the wrap broke inside a word. This happens with a long URL, path or hash, and with CJK text. The copied text then contains a character that is in neither the source nor the screen.
- It never joins the continuation rows of a wrapped quote.

The "join with a single space" wording came from my round-1 finding. That wording was imprecise and is the root cause of the first problem.

## States checked
- Toast `Copied 42 lines` over the browser @ 80x24: snapshot `copy__copied_toast_80x24_text`. ok, unchanged.
- Toast `Copied 42 lines` @ 120x40: snapshots `copy__copied_toast_120x40_text` and `copy__copied_toast_120x40_styled`. ok, unchanged.
- Toast `Copy failed: no clipboard available` @ 80x24: snapshot `copy__copy_failed_toast_80x24_text`. ok, unchanged.
- Folded fixture prompt, `y` @ 80x24: repo test `browser_tests::copy::y_copies_the_selected_section_as_displayed`. ok. It gives one line ending `… but I don't have a way to…` and `Copied 1 line`, matching the snapshot rows.
- Folded prompt, URL after a word, at 60x20, 80x24 and 120x40: rendered in scratch crate. ok. The break falls at a real space, and the copy matches the two rows with `…`.
- Folded prompt that is one long token (a 220-char URL) @ 80x24: rendered in scratch crate. **issue** (finding 1).
- Folded CJK paragraph without spaces at 60x20, 80x24 and 120x40: rendered in scratch crate. **issue** (finding 1).
- Folded prompt starting with a long quote at 60x20 and 80x24: rendered in scratch crate. **issue** (finding 1). At 120x40 it fits on one row and is ok.
- Folded prompt starting with a long list item at 60x20, 80x24 and 120x40: rendered in scratch crate. ok. Gives `• first item … when the terminal is…`, joined correctly.
- Folded prompt starting with a code block at every size: rendered in scratch crate. ok. Gives `  fn a() {}` / `  fn b() {}…`, keeping the code indent.
- Folded prompt with single-newline lines (soft breaks): rendered in scratch crate. ok. Gives one line, the same as the screen.
- Folded repeated-word paragraph at every size: rendered in scratch crate. ok. At 120x40 it fits two rows, and the copy has no `…`, like the screen.
- Folded prompt with `**bold**`, `*em*` and inline code: rendered in scratch crate. ok. The markup is dropped, as on screen.
- Folded thinking at 60x20, 80x24 and 120x40: rendered in scratch crate. ok. The copy is the ellipsized `Thinking: …` header, exactly as drawn.
- Folded and unfolded compaction, and unfolded thinking, @ 80x24: rendered in scratch crate. ok. The blank row between paragraphs is kept, and `Y` gives the unfolded text.
- Unfolded long-token prompt, `y` and `Y` @ 80x24: rendered in scratch crate. ok. Both give one unbroken logical line.

## Findings
1. [must-fix] Browser, folded user prompt, `y`, at any width where the first two rows wrap. Function: `soft_joined` in `src/components/turn_block/plain.rs`. See `ui/folded-softwrap-joins-80x24.txt`.

   **a) A space is inserted where the wrap broke inside a word.**
   - Long token @ 80x24: the copy reads `…abcdefghijab cdefghij…`.
   - CJK @ 80x24: the copy reads `…スペース なしで…`. The same happens at 60x20 and 120x40.
   - A copied URL, path or hash is therefore corrupted. This breaks the planner note "logical lines (no soft-wrap breaks)". The round-1 instruction "join with a single space" was wrong for these breaks.
   - Change: join a continuation row with the whitespace the source has at that point. `found` already computes `skipped`, the leading whitespace of the rest of the logical line. Return it along with the end, and push a space only when `skipped > 0`. When `skipped == 0` (a mid-word or CJK break), append the row directly. In both cases, append the row with its leading whitespace trimmed.
   - Expected @ 80x24 for `https://example.com/` + `abcdefghij`×20: one line `https://example.com/abcdefghij…abcdefghijabc…` with no space, which is a prefix of the source plus `…`.

   **b) A soft-wrapped quote is copied as two lines, each with `│ `.**
   - @ 80x24 the copy is `│ a quoted line … one row at` / `│ sixty or eighty columns wide…`.
   - This breaks the same binding rule: logical lines, with the `│ ` prefix kept once.
   - The third assertion of `soft_joined_matches_rows_to_the_unwrapped_lines` locks this in.
   - Change: before matching a row as a continuation, strip a leading quote prefix (`│ `, repeated for nested quotes) from the row when the logical line it continues starts with the same prefix. Then join the row as in a).
   - Expected at 80x24 and at 60x20: one line, `│ a quoted line that is long enough to wrap over more than one row at sixty or eighty columns wide…`, with toast `Copied 1 line`.
   - Rows that really do not match anything (none were found in these renders) may still stay apart.

   **Tests to add or change in `plain::tests`:**
   - A folded one-token prompt, for example a 200-character URL, at block width 80. Assert that `source.starts_with(copy.trim_end_matches('…'))` and that the copy has no space.
   - A folded CJK paragraph without spaces at block width 80, with the same prefix assertion.
   - A folded prompt whose first block is a wrapped quote at block width 80. Assert that the copy is the single line above.
   - In `soft_joined_matches_rows_to_the_unwrapped_lines`, add a mid-word case (`["abcdef", "ghi…"]` against `"abcdefghijk"` gives `"abcdefghi…"`). Change the quote assertion to expect one joined line.
   - The existing `a_wider_block_shows_more_of_a_folded_prompt` checks this prefix property, but it passes only because `LONG_PROMPT` breaks at spaces.
2. [nit] Unchanged from round 1, finding 3: a horizontal rule is 80 cells in the plain text, while the screen draws it at the content width. It was left as is on purpose, and the spec does not pin it.

Round-1 nit 2 (the full failure reason in the log) is fixed: `log::warn!` in `App::copy_text`.
