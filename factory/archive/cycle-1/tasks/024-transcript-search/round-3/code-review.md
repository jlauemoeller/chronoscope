VERDICT: FAIL

## Summary
Round 3 makes the rework change in `SectionView`/`row_of`. The gutter is now strict (`is_gutter`), `earlier` is counted with a predicate (`Source::is_drawing_of`), the anchored fallback replaces the section top, and the source is split and squashed once per jump. All requested tests were added. The round-2 tests and snapshots are unchanged: `git diff 9d8bf86 HEAD` on `tests/search.rs` and `*.snap` contains only additions. The code-review must-fix from round 2 (`build done` / `error: usb disconnected`) is fixed and covered.

One must-fix remains. `Source::new` strips markdown markers (`# `, `* `, `> `, `N) `) from every source line, but the drawn side only accepts `is_gutter` characters. So in any section that draws those markers verbatim, the two counts disagree. That covers shell and tool output and fenced code in assistant text. I reproduced `n` landing on the wrong match row in three cases.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1322 lib tests passed, 1 ignored; all integration tests pass; working tree clean, no `.snap.new` files)

## Findings
1. [must-fix] `src/components/transcript/search.rs`: `Source::new` / `marker_len`, together with `Source::is_drawing_of` and `SectionView::is_drawing`.

   **Problem.** `marker_len` strips `>`, `- `/`* `/`+ `, `#… ` and `N. `/`N) ` from the start of *every* line of the section's plain text. It does this whatever draws the line. But only the markdown renderer turns those markers into a gutter or removes them, and only outside code blocks. The shell/log renderer, generic tool input and fenced code blocks draw them verbatim. On the drawn side, `is_drawing` accepts only `is_gutter` characters in front of the line. `-`, `+` and `.` are among them, which is why `- ` and `N. ` happen to stay consistent. `#`, `*`, `>` and `)` are not. So the rework's "one acceptance rule on both sides" does not hold for those markers:
   - **(a) Wrong row.** A line such as `# done` or `* done` is counted in `earlier` as a drawing of `done`. Its drawn row `#done` is rejected. A later `done` therefore gets the next drawing's row, and the hit itself is not on screen. This breaks the round-3 invariant ("the row drawn from the match's own source line is on screen"). I reproduced it in a scratch copy:
     - `log_of(filler(150, …))` with line 5 = `# done` and lines 100 and 140 = `done`. Search `done` (`1/3`), then `n`. It shows `2/3`, but the screen shows lines 132–148 (line 140) and line 100 is not shown. With line 5 = `* done` the result is the same.
     - Beyond the limit: `filler(500, …)` with line 5 = `# done` and lines 350 and 400 = `done`. After `n` it shows `2/3`, the output is expanded, and the screen shows lines 392–408 (line 400) instead of line 350.
     - Assistant text with a fenced ` ```python ` block of 60 lines: line 3 = `# done`, lines 20 and 50 = `done`. After `n` it shows `2/3`, and the screen shows `x48`…`x51` (line 50) instead of line 20. `#` comments in code blocks are common, so this is not a corner case.
   - **(b) Needless expansion.** A line within the limit such as `# usb here` (in a 500-line log, line 21) is not placed. `row_of` returns `None` because the view is truncated, and the output is expanded, although the line was already drawn (`folds.is_expanded(&tool())` is true after `1/1`).

   **Change:**
   - Strip markers only where the markdown renderer really transforms them. Pass `Source::new` whether the section is drawn through markdown: assistant text, prompt, thinking and compaction summary, but not tool input/output or system content where these are drawn raw. Within markdown sections, track ` ``` `/`~~~` fences while splitting and do not strip inside them.
   - Lines that are not stripped keep their full squashed text. `is_drawing_of` then compares the same per-line form that the drawn side sees, so the two counts are one rule again.
   - If some raw-drawn marker should still count as a gutter, add it to `is_gutter` on both sides instead.
   - Add regression tests to `src/components/transcript/tests/search.rs` for the cases above:
     - the `# done` log within the limit and beyond the limit (assert `2/3`, the `line 99`/`line 101` or `line 349`/`line 351` neighbourhood on screen, and expansion beyond the limit);
     - the `* done` variant;
     - the fenced-code `# done` case in a `markdown_turn`;
     - a `# usb here` line within the limit of a truncated log, asserting `!t.content.folds.is_expanded(&tool())` with the hit on screen and highlighted.

2. [nit] `src/components/transcript/search.rs`, `SectionView::anchored` → `place`.
   - Each lookback step calls `place`, which counts `earlier` over all preceding lines and, for a line that is not drawn, scans the whole `joined` text. The worst case is O(`ANCHOR_LOOKBACK` × section size) on the UI thread.
   - Measured in a debug build on a 20 000-line expanded log: a jump took about 0.85 s when no line could be placed, against about 0.5 s when every line could. That is noticeable but not pathological, and the base cost (layout plus row text of the whole expanded section) was there before.
   - Consider precomputing, once per `SectionView`, the drawing offsets of each source line or a map from squashed line to its drawing positions, so that `place` is a lookup.
3. [nit] `src/components/transcript/search.rs`, `is_label`. The anchored count skips any drawn row of 12 bytes or less that is a capitalised word followed by `:`. A real content row such as `Error:` or `Note:` in an unplaced stretch would also be skipped. If the query hits it, the j-th occurrence count shifts by one. Consider skipping only the labels the tool renderers actually draw (e.g. a const list shared with the renderers).
4. [nit] `src/components/transcript/search.rs`, `SectionView::place` with `clamp`. When a line is written more often than it is drawn (the context lines of an Edit diff appear in both `new_string` and `old_string`), every surplus occurrence maps to the *last* drawing. With a context line that repeats in the file, the `old_string` occurrences of the first copy land on the last copy's row. This is documented and acceptable for this task. A comment noting that limitation where `clamp` is passed would help the next reader.
