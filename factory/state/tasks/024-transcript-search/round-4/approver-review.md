## Rework note
Fix the two remaining ways a search jump lands on the wrong row. Change only `Source`, `section_source` and `SectionView` in `src/components/transcript/search.rs`, plus tests in `src/components/transcript/tests/search.rs`.
- Keep everything else from round 3 as it is:
  - the strict gutter (`is_gutter`);
  - `is_drawing_of`;
  - the lookback anchor;
  - fold and expansion handling;
  - stepping and highlighting;
  - the snapshots.
- Do not change the renderers.

**What is wrong.**
1. **Markers are stripped everywhere.** `Source::new` strips markdown markers (`# `, `- `/`* `/`+ `, `> `, `N. `/`N) `) from every line of every section, through `marker_len`.
   - Only the markdown renderer draws those markers as a gutter or drops them, and it does so only outside code blocks. Shell and tool output, tool input, system content and fenced code draw them verbatim, and `is_gutter` rejects `#`, `*`, `>` and `)`.
   - So `# done` in a log is counted as a drawing of `done`, but its row `#done` is not accepted. `n` then shows `2/3` while the screen shows the next `done`.
   - A unique `# usb here` inside the truncation limit is not placed at all, so the output is expanded needlessly.
2. **The anchored j-count has no upper bound.** In `anchored`, `before` counts every match after the anchor, including matches whose text is not drawn (link URLs, fence info strings). The row search then runs to the section end.
   - So ``Use [serde](https://serde.rs) or `serde_json`.`` at `3/4` lands on a later `serde` paragraph far below.
   - A paragraph with only a URL hit borrows the row of the next match.

**Invariant (unchanged).** After `Enter`/`n`/`N`, the row drawn from the match's own source line is on screen and highlighted with `search_match`. If the line is cut off by the truncation, the output is first expanded temporarily.
- A match whose text is not drawn at all (URL, fence info) shows the rows where its own line is drawn: the link's paragraph, the code block.

**Change 1: strip markers only where markdown draws them.**
- `Source::new` takes whether the section is drawn as markdown. Derive this in `section_source` from the `Section`:
  - yes for `Prompt`, `Text`, `Thinking`, `Compaction`, and `Tool` with `ToolKind::Subagent` (its prompt and result go through `render_markdown`);
  - no for every other tool kind, `System`, `Stats` and `ModelChange`.
  - Where it is no, `bodies` is 0 for every line.
- In markdown sections, track fenced code blocks while splitting, as CommonMark does:
  - A fence opens on a line that starts, after up to 3 spaces, with 3 or more `` ` `` or `~`.
  - It closes on a line of the same character that is at least as long.
  - Do not strip markers on the fence lines or on any line between them.
  - Indented code blocks need no tracking; Change 2 catches them.
- A line that is not stripped keeps its full squashed text. Then `is_drawing_of` compares the same per-line form that `is_drawing` sees on the drawn side.

**Change 2: a count check in `place`, replacing the general clamp.**
For the squashed line, compute two counts:
- `written`: how many source lines of the section `is_drawing_of` accepts for it, including the line itself and the lines after it;
- `drawn`: how many drawings `is_drawing` accepts in the joined rows.

Then:
- `drawn > written`: return `None`, in a truncated view too. Some accepted row belongs to no such source line, for example the wrapped last row of a longer line that reads exactly like this one.
- `drawn == written`: return the `earlier`-th drawing, as now.
- `drawn < written` in a truncated view: as now. Return the `earlier`-th drawing if it exists, otherwise `None`, so that the caller expands the output and asks again.
- `drawn < written` in an untruncated view:
  - **Edit and Write sections** (`ToolKind::Edit` / `ToolKind::Write`) keep the clamp to the last drawing. The diff's context lines are written in both `old_string` and `new_string` but drawn once. Add a comment there: when a context line repeats in the file, the surplus occurrences land on the last copy's row.
  - **All other sections** return `None`, so the anchored fallback runs.

Pass the section kind to `SectionView` (or put it in `Source`) for this.

**Change 3: bound the anchored window** in `SectionView::anchored`.
- **Start.** As now: the row after the anchor's drawing, or the section's first row when there is no anchor.
- **End.** Look at the source lines after `found.line`, at most `ANCHOR_LOOKBACK` of them. Take the first one that `place` places *and* whose drawing starts after the window start. The window ends just before the row where that drawing starts.
  - Skip a later line that is drawn at or above the window start. Source order is not draw order: an Edit's `new_string` comes before its `old_string` in `section_text`.
  - If no such line exists, the window runs to the section end.
- **j** stays as now: the number of matches on source lines after the anchor that come before `found`, including earlier matches on `found`'s own line.
- **Result.** Return the row of occurrence j (0-based) of the query in the window's rows. Skip labels as now, and use the same smart-case rule.
  - If it holds none, return the window's first row.
  - If it holds at least one but no more than j, return the row of the last one in it.
  - Never return a row after the window. Return `None` only when the section has no row at or after the window start.

**Tests to add** in `src/components/transcript/tests/search.rs`.
- All tests run at the 80x17 transcript viewport.
- Each asserts the `i/n` position, and that the *expected row text or its neighbourhood* is on screen. Matching any `search_match` cell is not enough.
- Each asserts the hit is highlighted where it is drawn. `assert_hit_shown` does this. Matches that are not drawn (URL, fence info) get only the on-screen assertion.
- `filler(count, at, hit)` is 1-based, so `lines[k]` is line k+1.

*Markers in tool output* (`log_of`):
1. `filler(150, 100, "done")` with `lines[4] = "# done"` and `lines[139] = "done"`. Search `done`: `1/3`. Press `n`: `2/3`, with `line 99` or `line 101` on screen.
2. As test 1, with `lines[4] = "* done"`.
3. `filler(500, 350, "done")` with `lines[4] = "# done"` and `lines[399] = "done"`. Press `n`: `2/3`, `t.content.folds.is_expanded(&tool())`, and `line 349` or `line 351` on screen.
4. `filler(150, 120, "# usb setup")` with `lines[4] = "usb setup"`. Search `usb`: `1/2`. Press `n`: `2/2`, with `line 119` or `line 121` on screen.
5. As test 4, with `filler(500, 350, "# usb setup")`: `2/2`, expanded, and `line 349` or `line 351` on screen.
6. `filler(150, 120, "* usb-fix")` with `lines[4] = "  usb-fix"`: `2/2`, with `line 119` or `line 121` on screen.
7. `filler(500, 21, "# usb here")`. Search `usb`: `1/1`, `!t.content.folds.is_expanded(&tool())`, and the `# usb here` row on screen and highlighted.

*Markers in fenced code* (`markdown_turn(&text, 80)`):

8. The text is `Intro.`, a blank line, then a ` ```python ` fence holding the 60 lines `x1`..`x60`, then the closing fence.
   - Replace `x3` with `# done`, and `x20` and `x50` with `done`.
   - Search `done`: `1/3`. Press `n`: `2/3`, with `x19` or `x21` on screen.
9. The text is `usb setup`, then the paragraphs `filler 1`..`filler 30`, then ` ```sh ` / `# usb setup` / `make` / ` ``` `. Paragraphs are separated by blank lines.
   - Search `usb`: `1/2`. Press `n`: `2/2`, with `make` on screen.

*Anchored window* (`markdown_turn(&text, 80)`):
- Unless a test says otherwise, the text is these paragraphs, separated by blank lines:
  - `filler 0`..`filler 19`;
  - the hit paragraph;
  - `more filler 0`..`more filler 19`;
  - `the second usb hit`;
  - `tail filler 0`..`tail filler 19`.
- Search `usb`.

10. The hit paragraph is `See [the docs](https://usb.org) for the usb port.`
    - At `1/3` and at `2/3`, `See the docs for the usb port.` is on screen.
    - At `3/3`, `the second usb hit` is on screen.
11. The hit paragraph is `Read [the spec](https://usb.org/spec) first.`
    - At `1/2`, `Read the spec first.` is on screen.
    - At `2/2`, `the second usb hit` is on screen.
12. The hit paragraph is ``Use `lsusb` from [usbutils](https://github.com/gregkh/usbutils) to list usb devices.``
    - After `Enter` and `n` three times, at `4/5`, `Use lsusb from usbutils to list usb devices.` is on screen.
    - At `5/5`, `the second usb hit` is on screen.
13. The text starts with two paragraphs: ``See [the docs](https://usb.org/spec) and `code`.`` and a ` ```usb ` fence holding `plain block`.
    - Then come `filler 0`..`filler 19`, `the usb hit you want first`, `more filler 0`..`more filler 19`, `the second usb hit`, and `tail filler 0`..`tail filler 19`.
    - Expected rows on screen:
      - `1/4`: `See the docs and code.`
      - `2/4`: `plain block`
      - `3/4`: `the usb hit you want first`
      - `4/4`: `the second usb hit`
14. The text is ``See [docs](https://usb.example.com) for the `usb` port.``, then `filler 1`..`filler 30`, then `the far usb line`.
    - `1/3`. Press `n`: `2/3`, with `for the usb port` on screen.
15. The text is ``Use [serde](https://serde.rs) or `serde_json`.``, then `filler 1`..`filler 30`, then `the far serde line`.
    - Search `serde`: `1/4`. Press `n` twice: `3/4`, with `or serde_json` on screen.
16. A genuine j > 0 case with nothing undrawn. The text is:
    - `filler 0`..`filler 19`;
    - ``the `usb` one``;
    - 30 paragraphs ``more `code` 1``..``more `code` 30``;
    - ``the `usb` two``;
    - `tail filler 0`..`tail filler 19`.

    No line between the two hits can be placed, so the second hit has j = 1.
    - At `1/2`, `the usb one` is on screen and `the usb two` is not.
    - At `2/2`, `the usb two` is on screen and `the usb one` is not.

*Count check*:

17. A line is accepted as drawn but written nowhere as its own line.
    - A long line whose wrapped last row reads exactly `done`, followed later in the same section, more than 17 rows further down, by a separate line `done`.
    - Build it from the body width the test renders at, either a char-wrapped `log_of` line or a word-wrapped `markdown_turn` paragraph.
    - Search `done` and step to the separate line. Its own neighbourhood is on screen, not the wrapped row's.

**Must stay green and unchanged:**
- From round 2:
  - `a_query_that_a_label_also_contains_finds_the_real_match_beyond_the_limit`
  - `a_label_is_not_taken_for_a_match_within_the_limit_either`
  - `a_hit_that_is_not_drawn_does_not_shift_the_ones_after_it`
  - `a_hit_that_wraps_across_rows_does_not_shift_the_ones_after_it`
  - `the_same_line_twice_finds_each_occurrence`
  - `a_match_far_down_a_tall_section_is_scrolled_into_view`
  - the truncation and expansion tests
- Every test added in round 3: the suffix cases, the inline-markdown and JSON-input cases, `two_hits_with_code_spans_each_land_on_their_own_row`, and the Read, Edit `+`, bullet, quote and numbered-list cases.
- The five UI snapshots and the styled companion of snapshot 3.

**Done when** all of the above pass, along with `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test`.

**Optional:**
- Precompute the placement of each source line once per `SectionView`, so that `place` is a lookup. The lookback, the new lookahead and the count check each call it.
- Do this only if it stays simple; it is not required.

**Dropped, do not do:**
- Layout provenance, where each row records its source line. It is out of proportion for this task.
- Content-only label highlighting and highlighting a hit that wraps across two rows. The planner's per-row rule allows the current behaviour.
- Changing `is_label`. The bounded window confines its effect.
- Restricting the clamp in any way other than Change 2. The clamp stays for Edit/Write, because a diff draws its context lines once.

**Accepted limits, do not chase:** in a paragraph that wraps over several rows, an undrawn match (URL) before a drawn one can put the drawn one on another row of the same paragraph. That paragraph is on screen either way.
