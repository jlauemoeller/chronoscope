## Rework note
Fix how a search match is placed on a drawn row: `SectionView` and `row_of` in `src/components/transcript/search.rs`. Everything else from round 2 stays as it is. That covers the fold and expansion handling, the stepping, the highlighting and the snapshots.

**What is wrong.** `row_of` fails in two opposite ways.
1. **Too loose.**
   - `earlier` counts the earlier source lines whose squashed text is *equal* to the line.
   - `nth_drawn`/`is_drawing` accept any row that ends with the line and has up to 12 bytes of *any* text in front of it.
   - So `build done` or `error: usb disconnected` stands in for a later line `done` or `usb disconnected`. Then `n` shows `2/2` but scrolls to the earlier row. Beyond the truncation limit, the output is not expanded and the match is never drawn.
   - The `nth_ending` fallback has the same mismatch.
2. **Too strict.**
   - Some lines are drawn differently from their source after the hit. Examples are an inline code span, `**bold**`, a link, or one line of a multi-line string in a generic tool's JSON input, which is drawn with `\n` escapes.
   - Neither lookup finds such a line. `row_of` returns `None`, and the jump shows only the section top while the hit is drawn further down, off screen.

**Invariant to meet.**
- After `Enter`/`n`/`N`, the row drawn from the match's own source line is on screen and highlighted with `search_match`.
- If that line is cut off by the truncation, the output is first expanded temporarily, as now.
- Falling back to the section top is allowed only when the match's text is not drawn at all, such as a link URL or a fence info string.

**How.** Keep the change local to `SectionView` and do not change the renderers.
- **Strict gutter.** In `is_drawing`, the part of the first row in front of the line may hold only the non-letter characters that the renderers draw before a line:
  - ASCII digits (the Read line-number gutter);
  - the diff marks `+` and `-` (Edit/Write);
  - the markdown bullet `•`, the `.` of a list number, and the blockquote bar `│`.

  White space is already squashed. Check the Read, Edit, Write and markdown renderers (`src/markdown/layout.rs`, `BULLET`/`QUOTE_BAR`) for the exact set. Never accept letters or other text. Remove the "12 bytes of anything" rule (`GUTTER`).
- **One acceptance rule on both sides.**
  - Count `earlier` with the same predicate. An earlier source line counts when its squashed text is allowed gutter characters followed by this line. Then the n-th drawing is the n-th source occurrence.
  - Make the tail fallback (`nth_ending`) consistent in the same way.
  - Do not switch to an alignment that places the source lines in order, where each lookup starts after the row of the line placed before it. The order of the lines in `section_text` is not the order in which rows are drawn:
    - tool input values follow serde_json's sorted key order, so an Edit's `new_string` comes before its `old_string`;
    - the Edit diff interleaves old and new lines and draws context lines once.
- **Anchored fallback instead of the section top.** Use it when the line is found neither verbatim nor by its tail and the output is not truncated.
  1. Take the nearest preceding source line that is placed, or the section start if none is.
  2. From the end of its drawing, look for the query in the drawn rows, with the same smart-case rule as `SearchQuery`.
  3. Pick the j-th occurrence. Here j is the number of matches before this one on source lines after that anchor, including earlier matches on the same line.
  4. Return `None` (section top) only if no such occurrence is drawn.

  When the output is truncated and the line is not found, keep the current expand-and-retry.
- **Split and squash the source lines once per `SectionView`**, not on every lookup.

**Tests to add** in `src/components/transcript/tests/search.rs`. Each runs at transcript viewport 80x17 with a section taller than the viewport, and asserts three things:
- the `i/n` position;
- the hit's row is on screen and drawn with `search_match`;
- where the hit lies beyond `truncate_output_lines`, that `t.content.folds.is_expanded(&tool())` holds.

The cases:
1. `filler(500, 350, "done")` with `lines[4] = "build done"`, using `log_of`. Search `done` (`1/2`), press `n`. Expect `2/2`, the output expanded, and `line 349` or `line 351` on screen.
2. `filler(150, 120, "done")` with `lines[4] = "build done"`. After `n`, `line 119` or `line 121` is on screen.
3. `filler(500, 350, "usb disconnected")` with `lines[10] = "error: usb disconnected"`. Search `usb`, press `n`. Expect `2/2`, the output expanded, and `line 349` or `line 351` on screen.
4. As case 3, with `filler(150, 120, …)`. `line 119` or `line 121` is on screen.
5. An assistant text section of 41 paragraphs built with `markdown_turn`: `filler 0`..`filler 19`, one hit paragraph, then `more filler 0`..`more filler 19`. One test for each hit paragraph, each expecting `1/1` with the hit row on screen:
   - ``Plug it into the `usb` port.``
   - `Plug it into the **USB** hub.` (query `USB`)
   - `The usb hub, see [the docs](https://example.org).`
   - ``The usb hub is `fine` here.``
6. A generic tool call, `ToolKind::Other` such as `mcp__x__post`. Its input is `{"title": "x", "body": <120 lines, line 100 = "the usb here">}` and its result is `ok`. Search `usb`: expect `1/1` with the hit row on screen.
7. A tall assistant text section with two hits, each on a line with an inline code span, far apart. `1/2` and `2/2` each land on their own row. This guards the j-th-occurrence count of the anchored fallback.
8. The stricter gutter must not break placement in other renderers. Each of these, in a tall section, lands on its own row:
   - a Read output with Claude numbering (`     N→text`);
   - an Edit diff `+` line;
   - a markdown `- ` bullet line;
   - a markdown `> ` quote line.

These tests and snapshots must stay green and unchanged:
- `a_query_that_a_label_also_contains_finds_the_real_match_beyond_the_limit`
- `a_label_is_not_taken_for_a_match_within_the_limit_either`
- `a_hit_that_is_not_drawn_does_not_shift_the_ones_after_it`
- `a_hit_that_wraps_across_rows_does_not_shift_the_ones_after_it`
- `the_same_line_twice_finds_each_occurrence`
- `a_match_far_down_a_tall_section_is_scrolled_into_view`
- the truncation and expansion tests
- the five UI snapshots and the styled companion of snapshot 3

**Done when** all of the above pass, along with `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test`.

**Dropped, do not do:**
- Highlighting only content spans (not labels such as `Result:`/`Command:`), and highlighting a hit that wraps over two rows. The planner's per-row highlighting rule allows the current behaviour.
- The UI reviewer's preferred fix of having the layout record the source line of every row. It would mean threading provenance through the markdown pipeline and every tool renderer, including pretty-printed JSON. That is out of proportion for this task, and the local fix above is the alternative the UI reviewer offered.
- Caching the materialised lazy output across `section_view` calls. It is a nit; do it only if it falls out of the change.
