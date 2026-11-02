STATUS: DONE

## Changes
Only `Source`, `section_source`, `SectionView` in `src/components/transcript/search.rs`, plus tests in `src/components/transcript/tests/search.rs`. Renderers, the strict gutter, `is_drawing_of`, the lookback anchor, fold/expansion handling, stepping, highlighting and snapshots are untouched.
- **Change 1, markers only where markdown draws them.** `Source::new(text, markdown, diff)`; `section_source` derives `markdown` from the section (Prompt, Text, Thinking, Compaction, and Subagent tool calls; not other tool kinds, System, Stats, ModelChange). In markdown sections fenced code blocks are tracked while splitting (open: up to 3 spaces then 3+ of `` ` `` or `~`; close: same character, at least as long, nothing else on the line); fence lines and lines inside keep their full squashed text.
- **Change 2, count check in `place`.** `written` (source lines `is_drawing_of` accepts, incl. this one) is compared with `drawn` (drawings `is_drawing` accepts): `drawn > written` -> `None`, also when truncated; equal -> the n-th drawing; fewer and truncated -> the n-th drawing if present else `None` (caller expands and retries); fewer and untruncated -> the last drawing for Edit/Write sections (diff context lines are written twice, drawn once; commented), `None` for all others so the anchored fallback runs.
- **Change 3, bounded window in `anchored`.** Window = rows after the anchor's drawing up to (excluding) the row of the first later source line (at most `ANCHOR_LOOKBACK` ahead) that is placed and drawn at or after the window start; else to the section end. Result: occurrence j in the window (labels skipped, smart case); none -> the window's first row; fewer than j+1 -> the last one; `None` only if the section has no row at or after the window start.

## Self-check (80x17, each asserts the `i/n` position and the expected row text on screen; highlight where the text is drawn)
- Markers in tool output: `a_hash_line_in_tool_output_is_not_taken_for_the_plain_line_after_it`, `a_star_line_in_tool_output_...`, `a_hash_line_before_a_plain_one_beyond_the_limit_is_expanded_and_shown`, `a_plain_line_before_a_hash_line_within_the_limit_is_shown`, `..._beyond_the_limit_is_expanded_and_shown`, `an_indented_line_before_a_star_line_is_shown`, `a_unique_hash_line_within_the_limit_is_found_without_expanding`.
- Markers in fenced code: `a_hash_line_in_a_fenced_code_block_is_not_taken_for_the_plain_lines_after_it`, `a_hash_line_in_a_fenced_block_after_a_paragraph_is_found_on_its_own_row`.
- Anchored window: `a_url_hit_before_a_drawn_one_on_the_same_line_stays_on_the_paragraph`, `a_paragraph_with_only_a_url_hit_does_not_borrow_the_next_match`, `several_undrawn_and_drawn_hits_on_one_line_stay_on_it`, `a_link_and_a_fence_info_string_before_the_hit_do_not_shift_it`, `an_undrawn_url_before_a_code_span_stays_on_the_paragraph_and_the_far_line_is_not_reached`, `a_url_and_a_link_text_before_a_code_span_stay_on_the_paragraph`, `the_second_hit_with_nothing_undrawn_between_is_the_second_occurrence`.
- Count check: `a_row_that_reads_like_a_line_but_ends_a_longer_one_is_not_taken_for_it`.
- 16 of these new tests fail on the round-3 code (checked by restoring it); all round-2/round-3 tests and the six snapshots stay green.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (1339 lib tests, 0 failed; integration tests pass)
