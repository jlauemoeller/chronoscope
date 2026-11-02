VERDICT: FAIL

## Summary
The round-2 defect is fixed. Hits on lines that inline markdown changes now land on their own row: code spans, bold, links after the hit, and the lines of a multi-line JSON string input. The suffix cases (`build done` / `done`, `error: usb disconnected` / `usb disconnected`) land on their own row, both within and beyond the truncation limit. The snapshots are unchanged and correct, and `INSTA_UPDATE=no cargo test` passes. The new anchored fallback has one gap. It counts matches whose text is not drawn, such as a link URL or a fence info string. It then searches with no upper bound. So such a match lands on the row of a later, unrelated match. A drawn match that follows an undrawn one on the same line is pushed onto that later row as well, with its own row off screen.

## States checked
- Prompt open with `usb` typed @ 80x24: snapshots `prompt_with_usb_typed_80x24_text` / `_styled`. ok, unchanged since round 2.
- After `Enter`, first match selected with highlights and `1/5` @ 120x40: snapshots `after_enter_the_first_match_is_selected_and_highlighted_120x40_text` / `_styled`. ok.
- After `n` into the folded tool call, unfolded with `2/5` @ 120x40: snapshot `after_n_into_the_folded_tool_call_it_is_unfolded_and_highlighted_120x40_text` / `_styled`. ok.
- After `j` off it, refolded with `2/5` @ 120x40: snapshot `after_moving_off_the_tool_call_it_is_folded_again_and_the_position_stays_120x40_text`. ok.
- After `Esc`, no highlights and no position @ 80x24: snapshot `after_escape_there_are_no_highlights_and_no_position_80x24_styled`. ok.
- Test gate: `INSTA_UPDATE=no cargo test` passes (1322 lib tests plus integration tests). There are no `.snap.new` files, the tree is clean, and the round-3 commit touches no `.snap`. ok.
- Round-2 finding re-run in the scratch crate @ 80x17. All ok: each hit row is on screen and highlighted.
  - ``Plug it into the `usb` port.``
  - `**USB**` bold.
  - A trailing link.
  - A trailing code span.
  - A `>` quote line.
  - A `1.` list line.
  - A generic tool's multi-line JSON `body` with the hit on line 100.
- Suffix cases at 500/350 and 150/120, for `done` and for `usb disconnected`, @ 80x17, rendered in the scratch crate. ok: `2/2` lands on the hit row, and the output is expanded beyond the limit.
- Tool renderers in a tall section @ 80x17: Edit, Write, Read, Grep, and a generic tool with a long key. ok.
- A markdown table cell hit and a `##` heading hit in a tall section @ 80x17. ok.
- A wrapped paragraph with a code span and bold before the hit, and a later hit, @ 40x10 and 30x8. ok.
- Fixture cycle @ 60x13 and 60x8, `n` ×5 with wrap-around. ok.
- Long outputs (150/120 and 400/300) @ 80x17 and 60x13, with `j`/`k` refold and re-reveal. ok.
- Label queries (`result`, `command`) and a multi-word query wrapped across rows. ok.
- An undrawn match (link URL, fence info) before or on the same line as a drawn match, in a tall assistant text section @ 80x17, rendered in the scratch crate. **issue** (finding 1).

## Findings
1. [must-fix] Transcript, jump to a match (`Enter` / `n` / `N`) in a tall assistant text section, at transcript viewport 80x17 (the 80x24 screen). See `ui/undrawn-match-shifts-anchored-placement-80x17.txt`.

   **Problem.** In `SectionView::anchored` (`src/components/transcript/search.rs`), the count `before` includes every match after the anchor line, including matches whose text is not drawn: link URLs and fence info strings. That count then indexes occurrences in the drawn rows, and the search has no upper bound. The result:
   - **An undrawn match borrows a later match's row.** Paragraph `Read [the spec](https://usb.org/spec) first.` sits mid-section. At `1/2` the view scrolls to the `2/2` row `the second usb hit`, 42 rows further down (case B). In round 2's `md` scene, `1/4` (URL) and `2/4` (fence info) now show the screens of `3/4` and `4/4`. In round 2 they revealed the section top, so this is a regression (case D).
   - **A drawn match is pushed off its own row.** Two examples:
     - Paragraph `See [the docs](https://usb.org) for the usb port.` mid-section. `2/3` is the drawn `usb port`, but the view scrolls to the `3/3` row, and `See the docs for the usb port.` is off screen (case A).
     - Paragraph ``Use `lsusb` from [usbutils](https://github.com/gregkh/usbutils) to list usb devices.`` `4/5` is `usb devices`, on the same row as `1/5`..`3/5`. The jump goes to offset 81, the `5/5` row, so its own row is off screen (case C).

   This breaks the round-3 invariant: "after `Enter`/`n`/`N`, the row drawn from the match's own source line is on screen". Links whose URL repeats the link text are common in assistant answers (`[usbutils](…/usbutils)`), so this hits ordinary sessions.

   **Change.** Keep it local to `SectionView::anchored`:
   - **Bound the window from above.** It ends where the drawing of the next source line after `found.line` that `place` finds begins. Look ahead at most `ANCHOR_LOOKBACK` lines; if none is found, the window runs to the section end.
   - **Take the j-th occurrence of the query within that window.** If fewer occurrences are drawn there (some matches in the window are not drawn), clamp to the last occurrence in the window.
   - **If the window holds no occurrence,** return the window's first row: the match's own paragraph, where a link's text is drawn. Do not return a later row, and do not return the section top.

   With this change, the cases above behave as follows:
   - case A `2/3` lands on the `See the docs for the usb port.` row;
   - case B `1/2` lands on `Read the spec first.`;
   - case C `4/5` lands on the `lsusb … usb devices` row;
   - the `md` scene's `1/4` lands on `See the docs and code.`

   **Tests.** Add component tests in `src/components/transcript/tests/search.rs` at viewport 80x17. Each puts the paragraph mid-section, between 20 filler paragraphs before it and 20 after, plus a later drawn hit. Each asserts that the expected row *text* is on screen, not just that some `search_match` cell is:
   - `See [the docs](https://usb.org) for the usb port.`: at `2/3`, that row is on screen and highlighted.
   - `Read [the spec](https://usb.org/spec) first.`: at `1/2`, `Read the spec first.` is on screen.
   - ``Use `lsusb` from [usbutils](https://github.com/gregkh/usbutils) to list usb devices.``: at `4/5`, that row is on screen.

   Keep `a_hit_that_is_not_drawn_does_not_shift_the_ones_after_it` and the round-3 tests green.

Round-2 nits 2 and 3 (label highlighting, wrapped-hit highlighting) were dropped by the verdict and are not raised again.
