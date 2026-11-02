VERDICT: FAIL

## Summary
Round 2 fixes the round-1 must-fix. Folding (`h`, `Enter` on a section the search only expanded, and fold-all) now drops the search's output expansion, and tests cover it. All round-1 nits are addressed (`PromptChanged(_)`, the dead `SearchQuery` API, the `MAX_SKIPPED` cap, comments). The row of a match is now found from its own source line (`SectionView`). This fixes the label, hidden-hit and wrapped-hit cases from round 1, but the new line matcher accepts any drawn row whose squashed text *ends with* the match's line within a 12-byte "gutter". A line that is a suffix of an earlier line is therefore placed on that earlier line. I reproduced this in a scratch copy: a match beyond the truncation limit is again not expanded and never shown, which is the same rule the round-1 must-fix was about.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1307 lib tests passed, 1 ignored; all integration tests pass; working tree clean, no `.snap.new` files)

## Findings
1. [must-fix] `src/components/transcript/search.rs`, `SectionView::row_of` / `nth_drawn` / `is_drawing` (and `GUTTER`).

   **Problem.** `is_drawing` accepts a candidate when:
   - the squashed line ends where a row ends, and
   - it starts no more than `GUTTER` (12) bytes into the row, whatever those bytes are.

   But `earlier` in `row_of` counts only earlier source lines whose squashed text is *equal* to this line. So the n-th drawn candidate and the n-th source occurrence disagree whenever an earlier line is the same line behind a short prefix of non-gutter text. Typical log lines do this, for example `error: usb disconnected` before `usb disconnected`. The matcher then returns the earlier row:
   - **Beyond the limit.** In a scratch copy I used `log_of(&filler(500, 350, "usb disconnected"))` with line 10 set to `error: usb disconnected`, searched `usb` (`1/2`), then pressed `n`. The result was `2/2`, but `folds.is_expanded(c1)` was false. The screen showed line 10 again, and line 350 was never drawn. This breaks the binding planner rule "A match beyond the truncation limit also expands the output temporarily" and the "scroll the match's line into view" rule for `n`/`N`.
   - **Within the limit.** With `filler(150, 120, …)` and the same line 10, `2/2` scrolls to line 10 instead of line 120.

   The `nth_ending` fallback has the same mismatch. It counts earlier source lines that end with `tail`, but any row ending with `tail` is accepted as a drawing.

   **Change:**
   - Make a candidate's skipped prefix a real gutter. In `is_drawing`, require `joined[starts[first]..at]` to consist only of gutter characters: ASCII digits and the diff/number gutter marks the tool renderers actually draw, such as `+`, `-` and `│`. Check the Read/Edit/Write renderers for the exact set. Do not allow arbitrary text.
   - And/or count `earlier` with the same predicate the drawn side uses. That means counting earlier source lines that would themselves be accepted as a drawing of `line`, i.e. lines whose squashed text ends with `line` after an allowed prefix, so that both counts agree.
   - Apply the same consistency to the `nth_ending` fallback.
   - Add component tests in `src/components/transcript/tests/search.rs` using `log_of`:
     - (a) `filler(500, 350, "usb disconnected")` with line 10 = `error: usb disconnected`, search `usb`, press `n`. Assert `2/2`, that the output is expanded, and that the screen contains `usb disconnected` on a row that is not the line-10 row (e.g. `line 349` or `line 351` is on screen).
     - (b) The same with `filler(150, 120, …)`. Assert that `line 119` or `line 121` is on screen after `n`.
2. [nit] `src/components/transcript/search.rs`, `section_view`. Every call re-derives the sections and builds `section_text` for the section, which for a lazy tool output reads the full output back (`ToolOutput::text()`). It also runs an uncached `layout()` of the block. A jump that unfolds and expands calls it three times on the UI thread. Consider computing the source text once per `reveal_match` and passing it in, or caching it alongside the active search.
3. [nit] `src/components/transcript/search.rs`, `SectionView::row_of`. `self.source.split('\n').collect()` and the squashing of every earlier line run on each call. That is fine for normal sections, but it is O(section size) per `n` for a very large expanded output. It could be done once per `SectionView`.
4. [nit] Still open from round 1, and documented in the build notes: labels such as `Result:`/`Command:` are highlighted when the query hits them, and a hit wrapped over two rows gets no highlight. Both are allowed by the planner's per-row rule.
