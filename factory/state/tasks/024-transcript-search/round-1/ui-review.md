VERDICT: FAIL

## Summary
All five required snapshots exist and match the `UI` section (prompt, `search_match` colours, the right-aligned `i/n` item in `help_text`, temporary unfold, refold, `Esc`), and fmt, clippy and `INSTA_UPDATE=no cargo test` pass. Rendering in a scratch crate turned up a navigation bug. A jump scrolls to "the k-th occurrence of the query in the section's rendered rows", so a label row containing the query, a hit hidden by markdown or a hit split by wrapping sends `Enter`/`n`/`N` to the wrong row of a tall section, and the position item then disagrees with the screen.

## States checked
- Prompt open, `usb` typed @ 80x24 — snapshots `prompt_with_usb_typed_80x24_text` / `_styled` — ok. The prompt is on the help-line row: `/` fg (229,229,229) = `help_key`, the query, and the cursor cell bg (115,115,115) = `cursor`, inset 1 like the help line.
- After `Enter`: first match selected, highlights in the unfolded assistant text, `1/5` @ 120x40 — snapshots `after_enter_…_120x40_text` / `_styled` — ok. `▒` is on the selected section, each `USB`/`usb` is (23,23,23) on (229,229,229), and `1/5` is at cols 116..119 in `help_text`.
- After `n` into the folded tool call: temporarily unfolded, `2/5` @ 120x40 — snapshot `after_n_…_120x40_text` — ok. The snapshot is text only, so I checked the highlight in the scratch crate: `usb` in the result line carries `search_match` (`ui/after-n-tool-result-highlight-120x33-styled.txt`, transcript viewport of the 120x40 screen).
- After `j` off it: refolded, `2/5` still shown @ 120x40 — snapshot `after_moving_off_…_120x40_text` — ok.
- After `Esc`: no highlights and no position item @ 80x24 — snapshot `after_escape_…_80x24_styled` — ok.
- Minimum screen 60x20 (transcript viewport 60x13), cycling `n` through all 5 matches with wrap-around — rendered in scratch crate — ok. Each hit is scrolled into view and highlighted, and the unfold and refold work.
- Long tool output: 150 lines with a hit at line 120, and 400 lines with a hit at line 300 (beyond `truncate_output_lines = 200`). Transcript viewports 80x17 (80x24 screen) and 60x13 (60x20 screen) — rendered in scratch crate — ok. The hit row is centred and highlighted, the output beyond truncation is expanded temporarily, and after `G` the section is folded and collapsed again.
- Help line with `2/5`, `↓ 3 new  12/170` and `0/0` @ 60, 80, 120 columns — rendered in scratch crate — ok. The status is right-aligned, the order and two-space separator are correct, and hints drop out before they would overlap the status. The toast sits above the help-line row, so it does not cover `0/0` (`ui/help-line-and-prompt-widths.txt`).
- Prompt with a long query @ 60 columns — rendered in scratch crate — ok. It scrolls inside the inset, and the cursor cell stays visible.
- CJK, emoji and tab before a hit on the same row — rendered in scratch crate — ok. The highlight columns line up with the glyphs.
- `dim_unselected = true` — rendered in scratch crate — ok. Hits in unselected sections keep the full `search_match` colours.
- Smart case at render level: `USB` highlights only the 3 upper-case hits, and `zebra` draws no highlight — rendered in scratch crate — ok.
- A label containing the query, a hit hidden by markdown, a hit split by wrapping, each in a tall section, transcript viewport 80x17 (80x24 screen) — rendered in scratch crate — **issue** (finding 1).

## Findings
1. [must-fix] Transcript, jump to a match (`Enter` / `n` / `N`) in a section taller than the viewport. Reproduced at transcript viewport 80x17, which is the 80x24 screen from the `UI` section.

   **Cause.** The row to scroll to is taken as the k-th occurrence of the query in the section's rendered rows, where k is the match's index within its section (`Occurrences::row_of(nth)` in `src/components/transcript/search.rs`). That count only agrees with the search's matches when the drawn rows and the searched text agree, and in these cases they don't:

   - **(a) A label row holds the query.**
     - Query `result` against a Shell call whose output has `result` only on line 120: the section has 1 match (`1/1`). The `Result:` label row is counted as occurrence 0, so the jump stays at the top of the section and highlights the label. The real hit is off screen.
     - The same happens with `command` and the `Command:` label. When that hit is beyond `truncate_output_lines`, the output is not even expanded, because the label "satisfies" the count, so the real match is never drawn.
     - Likewise any label row that contains the query.
     - See `ui/jump-lands-on-label-80x17.txt`.
   - **(b) An earlier hit is not drawn.** For example, `usb` inside a markdown link URL and in a ```` ```usb ```` fence info string, followed by two visible hits. `1/4` scrolls to the third hit and `2/4` to the fourth. `3/4` and `4/4` fall back to the top of the section, where no hit is visible. See `ui/hidden-hit-shifts-later-matches-80x17.txt`.
   - **(c) An earlier hit wraps across two rows.** This needs a multi-word query. Every later match is shifted by one, and the last falls back to the section top. See `ui/wrapped-hit-shifts-later-matches-80x17.txt`.

   This breaks the planner's binding rule that `n`/`N` "scroll the match's line into view", and the `i/n` item names a different match from the one on screen. The builder's documented fallback ("if rendering hides it the section is revealed instead") applies only to the hidden match itself. Here every later match in the section is mis-placed.

   **Change.** Find the row for match *i* from that match's own position, not from a count of rendered occurrences.
   - Map `Match { line, range }` to the rendered row that draws that source text. For example, record which rendered rows each source line of the section produced, and look for the occurrence only in the rows of source line `line`. Alternatively, count occurrences only in content rows (never labels or other chrome), and treat source matches that are not drawn as hidden instead of letting them consume a rendered occurrence.
   - Expand truncated output whenever the match's own line is beyond the limit.
   - Fall back to revealing the section only for a match whose own text is not drawn.
   - Add component tests at a small viewport (for example 80x17). Each should assert that the match's row is on screen and highlighted after the jump, for:
     - query `result` against a tool output whose hit is beyond the viewport;
     - query `command` with its hit beyond the truncation limit;
     - a hit in a link URL before a visible hit;
     - a wrapped multi-word hit before a visible hit.
2. [nit] Transcript, any active search — Label text is highlighted when it is not a match. Examples are `Result:` and `Command:` with the query `result` or `command`. With `0/0` or `1/1`, cells that are not among the matches light up. This follows the planner's per-row rule, so it is not a defect in itself. Change, if wanted: apply the highlight only to the content spans of a row (text, output, input values), not to labels.
3. [nit] Transcript, a hit wrapped across two rows — It gets no highlight at all. The planner note permits highlighting it "as two parts", and the builder documented the gap. It is cosmetic. Change: when a row ends with a prefix of the query and the next row of the same section starts with the rest, highlight both parts.
4. [nit] Snapshot 3 (`after_n_…_120x40_text`) — The `UI` section only asks for a text snapshot, so nothing in the repo pins the `search_match` style on a hit inside tool output, which is drawn with the `output` token. Change: add a styled companion `after_n_into_the_folded_tool_call_it_is_unfolded_and_highlighted_120x40_styled`.
