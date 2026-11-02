VERDICT: FAIL

## Summary
Round 4 makes the three changes in the rework note, and only in `Source`, `section_source` and `SectionView`. Markers are now stripped only in markdown sections and outside fenced code. `place` compares drawings against written lines, and the clamp is kept only for Edit/Write. The anchored window now has an end. All 17 requested scenarios are present, and 16 of them fail on the round-3 `search.rs` (I checked this in a scratch clone). The protected tests and snapshots are unchanged and green, and fmt, clippy and `INSTA_UPDATE=no cargo test` pass. The behaviour is right. The one must-fix is in the tests: the note made highlight assertions mandatory, and most of the new tests leave them out.

## Acceptance criteria
- [x] Smart-case rule. Implemented by `SearchQuery` in `src/transcript_search.rs` and covered by `a_lower_case_query_matches_any_case` and `a_query_with_an_upper_case_letter_matches_that_case_only`. Unchanged this round.
- [x] Search covers folded sections, tool input, full output (lazy over 4 KiB, beyond truncation), system content and compaction summaries, and leaves out hidden system content. The matching is unchanged and its tests pass. This round fixes where some of those matches are placed on screen: markers in tool output and fenced code, and URL or fence-info hits.
- [x] The search runs off the UI thread via Port → `UserEvent` → `GlobalEvents` → `Msg`, and results from an old generation are ignored. Unchanged; the update tests pass.
- [x] `Enter` jumps to the first match, `n`/`N` wrap around, the help line shows `i/n`, and no match shows `0/0` plus a toast. Unchanged and tested. I also probed `N` backwards through the `# done` log case, and each step lands on its own row.
- [x] Temporary unfold and refold; pressing `Enter` in between makes the unfold explicit. Unchanged; the component tests pass.
- [x] `Esc` removes highlights, the position item and temporary unfolds. Unchanged; the component test passes.
- [x] `n`/`q`/`t` typed in the prompt are inserted and trigger no action. Unchanged; the update test passes.
- [x] Every UI state has a snapshot. No snapshot file changed in this commit, and all six (five states plus the styled companion of state 3) pass.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass. I re-ran all three: 1339 lib tests pass, and so do the integration suites.

What I checked against the round-4 rework note:
- **Scope.** The commit touches only `src/components/transcript/search.rs`, `src/components/transcript/tests/search.rs` (one hunk that only appends) and the build notes. The renderers, `is_gutter`, `is_drawing_of`, the lookback, folding, stepping and highlighting are untouched.
- **Change 1.**
  - `section_source` marks Prompt, Text, Thinking, Compaction and Subagent tool calls as markdown, and every other kind as not markdown. This matches the renderers: `render_markdown` is used only in `user.rs`, `text.rs`, `thinking.rs`, `compaction.rs` and `tool/subagent.rs`.
  - `fence_of` and `Source::new` follow the CommonMark rules: an opening fence is indented by at most 3 spaces and has 3 or more `` ` `` or `~`; the closing fence uses the same character, is at least as long, and has nothing else on the line. No markers are stripped on fence lines or inside the block.
  - A probe with a `~~~~` fence holding `> done` among plain `done` lines lands every step on its own row.
- **Change 2.** `place` handles all four branches as specified:
  - `drawn > written` returns `None`, including in a truncated view;
  - equal returns the n-th drawing;
  - fewer in a truncated view returns the n-th drawing or `None`;
  - fewer in an untruncated view returns the last drawing for Edit/Write (with the requested comment) and `None` otherwise.
- **Change 3.** `anchored` starts the window after the anchor and ends it before the first later placed line within `ANCHOR_LOOKBACK`. It returns occurrence j, the last occurrence, or the window's first row, and returns `None` only when `first >= rows.len()`.

## Findings
1. [must-fix] Most of the new tests do not assert that the hit is highlighted where it is drawn. The binding rework note (round-4 `approver-review.md`, line 65) requires this of each new test: "Each asserts the hit is highlighted where it is drawn. `assert_hit_shown` does this." The build notes' self-check heading says this was done, but only `a_unique_hash_line_within_the_limit_is_found_without_expanding` and `an_undrawn_url_before_a_code_span_stays_on_the_paragraph_and_the_far_line_is_not_reached` assert a highlight. The other 15 tests check only the position and the on-screen text.

   In a scratch clone I added the missing assertions, and they all pass at HEAD. The behaviour is therefore correct, and the gap is in the tests only.

   Change, in `src/components/transcript/tests/search.rs`:
   - **`near`.** Give it a `hit: &str` parameter and end it with `assert!(highlighted(t).iter().any(|m| m == hit), ...)`. Pass `"done"` or `"usb"` from the six marker-in-output tests and from `a_row_that_reads_like_a_line_but_ends_a_longer_one_is_not_taken_for_it`.
   - **The two fenced-code tests.** Assert `"done"` highlighted after `n` in `a_hash_line_in_a_fenced_code_block_is_not_taken_for_the_plain_lines_after_it`, and `"usb"` highlighted in `a_hash_line_in_a_fenced_block_after_a_paragraph_is_found_on_its_own_row`.
   - **`walk`.** Give each expected entry a flag for whether the match is drawn, and call `assert_hit_shown(&mut t, &pos, row, "usb")` for drawn matches. Skip only the undrawn ones:
     - `1/3` in `a_url_hit_before_a_drawn_one_…`;
     - `1/2` in `a_paragraph_with_only_a_url_hit_…`;
     - `1/5` in `several_undrawn_and_drawn_hits_…`;
     - `1/4` and `2/4` in `a_link_and_a_fence_info_string_…`.
   - **`a_url_and_a_link_text_before_a_code_span_stay_on_the_paragraph`.** Replace the `contains("or serde_json")` check with `assert_hit_shown(&mut t, "3/4", "or serde_json", "serde")`.
   - **`the_second_hit_with_nothing_undrawn_between_is_the_second_occurrence`.** Assert `"usb"` highlighted at `1/2` and at `2/2`.
2. [nit] The window's end condition differs from the note's wording. `SectionView::anchored` (search.rs ~line 893) keeps a later line whose drawing starts *at* the window start (`row >= first`). The note (line 54) says to skip a line drawn "at or above the window start", which means `row > first`. The code's choice gives a one-row window instead of looking further, which seems the safer behaviour, and my probe of a fence right after a paragraph without a blank line lands correctly. Change: keep `>=`, and extend the comment above `until` to say that a line drawn at the window start ends the window on purpose. Alternatively, switch to `>` to match the note.
3. [nit] Each `place` call now scans every source line (`written`) and the whole joined text with no early return. `anchored` can call it up to 400 times (200 back, 200 ahead). I measured a 20 000-line markdown list where no line can be placed:
   - debug build: `Enter` 1.35 s and `n` 1.0 s;
   - release build: 0.21 s and 0.16 s;
   - expanded 20 000-line log: 0.46 s in debug.

   This is no worse than round 3 on the same probe (2.3 s and 1.3 s in debug), and the note makes the improvement optional. Change, if wanted: precompute `place` for each source line once per `SectionView`, as the note's optional item suggests.
4. [nit] In `place` (search.rs ~line 800), `for (i, _) in self.source.squashed.iter().enumerate()` discards the item. Change: `for i in 0..self.source.squashed.len()`.
