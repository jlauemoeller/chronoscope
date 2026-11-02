VERDICT: FAIL

## Summary
The round-1 must-fix cases now work. Label rows (`Result:`, `Command:`), hits that are not drawn (link URL, fence info) and wrapped multi-word hits no longer send the jump to the wrong row. Hits beyond the truncation limit expand the output, and folding drops the search's expansion. All snapshots exist and match the `UI` section, and `INSTA_UPDATE=no cargo test` passes. The new line-based placement (`SectionView::row_of`) has a gap of its own. When a match's source line is drawn differently from how it is written, the jump falls back to the top of the section. That happens when inline markdown follows the hit on the same line (code span, bold, link), or for a multi-line string in a generic tool's JSON input. In a section taller than the viewport, `1/1` is then shown while the hit is off screen.

## States checked
- Prompt open with `usb` typed @ 80x24: snapshots `prompt_with_usb_typed_80x24_text` / `_styled`. ok.
- After `Enter`, first match selected with highlights and `1/5` @ 120x40: snapshots `after_enter_the_first_match_is_selected_and_highlighted_120x40_text` / `_styled`. ok.
- After `n` into the folded tool call, unfolded with `2/5` @ 120x40: snapshot `after_n_into_the_folded_tool_call_it_is_unfolded_and_highlighted_120x40_text`, plus the new `_styled` companion. ok. The `usb` in the result line (row 23, cols 34..37) is `search_match` (23,23,23) on (229,229,229), with the output's BOLD.
- After `j` off it, refolded with `2/5` @ 120x40: snapshot `after_moving_off_the_tool_call_it_is_folded_again_and_the_position_stays_120x40_text`. ok.
- After `Esc`, no highlights and no position @ 80x24: snapshot `after_escape_there_are_no_highlights_and_no_position_80x24_styled`. ok.
- Test gate: `INSTA_UPDATE=no cargo test` passes (1307 lib tests plus integration tests), with no `.snap.new` files and a clean tree. ok.
- Round-1 regressions, re-run in the scratch crate at transcript viewport 80x17 (the 80x24 screen):
  - Query `result` with a hit on output line 120 of 150: lands on the hit, not the label. ok.
  - Query `command` with a hit on line 230 of 250, beyond truncation: the output is expanded and the hit is centred. ok.
  - Link-URL and ```` ```usb ```` fence hits that are not drawn: `1/4` and `2/4` reveal the section top, and `3/4` and `4/4` land on their own hits. ok.
  - Multi-word query `a usb` wrapped across rows: later matches are not shifted (`2/3` and `3/3` are correct). ok.
- Long outputs of 150 lines (hit at 120) and 400 lines (hit at 300) @ 80x17 and 60x13, rendered in the scratch crate. ok: the hit is centred and highlighted, `j` refolds, and `k` back re-reveals it.
- Folding drops the search's expansion: 400 lines, search `usb`, then `h`, then `l`, rendered in the scratch crate. ok: the `… 200 more lines` row is back and line 300 is not drawn.
- Fixture cycle @ 60x13 and 60x8 with `n` ×5 and wrap-around, rendered in the scratch crate. ok.
- Tool kinds in a tall section @ 80x17, rendered in the scratch crate. All ok:
  - Edit diff (gutter `-`/`+`) and Write.
  - Read output.
  - Grep output.
  - Generic tool with a long input key on a single-line value.
- Hit on a line with inline markdown (code span, bold, trailing link or code span) in a tall assistant text section @ 80x17, rendered in the scratch crate. **issue** (finding 1).
- Hit on a blockquote line or a numbered-list line in a tall assistant text section @ 80x17, rendered in the scratch crate. ok.
- Hit inside a multi-line string of a generic tool's JSON input in a tall section @ 80x17, rendered in the scratch crate. **issue** (finding 1).

## Findings
1. [must-fix] Transcript, jump to a match (`Enter` / `n` / `N`) in a section taller than the viewport, at transcript viewport 80x17 (the 80x24 screen from the `UI` section).

   **Problem.** `SectionView::row_of` (`src/components/transcript/search.rs`) places a match by finding its source line among the drawn rows, ignoring white space. If the line is not found verbatim and the section is not truncated, it retries with the line's tail from the match on, which must end where a row ends. Both lookups fail when the line is drawn differently after the match. The row is then `None`, and `scroll_to_match` reveals only the section top. The hit is drawn and highlighted further down, but it is off screen while the help line says `1/1`. I confirmed at 80x200 that the hit row exists and carries `search_match`. Reproduced cases:
   - **(a) Inline markdown in assistant text, after the hit on the same line.** Examples:
     - ``Plug it into the `usb` port.``
     - `Plug it into the **USB** hub.`
     - `The usb hub, see [the docs](https://example.org).`
     - ``The usb hub is `fine` here.``

     In each case the section is 41 paragraphs with the hit in the middle, and the jump stops at the section top (offset 7). The hit row is at offset ~39 and is not visible. Blockquote and numbered-list lines, where markdown changes only the line start, are placed correctly. See `ui/inline-markdown-hit-not-scrolled-into-view-80x17.txt`.
   - **(b) Generic tool (`ToolKind::Other`) whose JSON input has a multi-line string.** The input is drawn as pretty JSON, with `\n` escapes inside one wrapped string. A hit on source line 100 of `body` falls back to the section top, and the hit is about 18 rows further down. See `ui/json-input-hit-not-scrolled-into-view-80x17.txt`.

   Inline code spans and bold are everywhere in assistant answers, so this affects ordinary sessions. It breaks the binding planner rule that a jump must "scroll the match's line into view". The build notes' fallback ("otherwise the section is revealed") was meant for lines that are not drawn. These lines are drawn.

   **Change.** Locate the row from what the renderer actually drew for that source line, rather than requiring the drawn text to equal the source text.
   - Preferred: have the turn-block layout record, for each rendered content row, the section-text source line it came from. This covers markdown paragraphs, tool input values (including JSON string lines) and output lines. Then scroll to the first row of the match's `line`, plus the query's occurrence within those rows where it can be found.
   - Alternatively, when both lookups fail and the section is not truncated, find the occurrence of the query among the section's content rows (never labels) that lies nearest to the match's expected position. For example, use the drawn row of the nearest preceding source line that was placed, and search forward from it. Do not fall back to the section top.

   Add component tests in `src/components/transcript/tests/search.rs` at viewport 80x17. Each test uses a section taller than the viewport with a single hit on a line that contains, respectively:
   - a code span;
   - `**bold**`;
   - a trailing link;
   - a line of a multi-line JSON string input.

   Each test asserts that, after the jump, the hit's row is on screen and highlighted with `search_match`.
2. [nit] Transcript, any active search: label text such as `Result:` or `Command:` is highlighted when the query hits it, even though it is not a match. This is carried over from round 1; the builder noted it as not done, and the per-row rule in the planner note permits it. Change, if wanted: apply the highlight only to the content spans of a row, not to labels.
3. [nit] Transcript, a hit wrapped across two rows gets no highlight at all. This is carried over from round 1; the builder noted it as not done, and the planner note permits highlighting it "as two parts". Change: when a row ends with a prefix of the query and the next row of the same section starts with the rest, highlight both parts.

Round-1 nit 4 (a styled companion for snapshot 3) is resolved.
