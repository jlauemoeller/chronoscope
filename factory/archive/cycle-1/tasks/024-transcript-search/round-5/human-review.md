# Human review

Fix one placement regression and complete the round-4 test assertions. Change only `SectionView::place` in `src/components/transcript/search.rs` and tests in `src/components/transcript/tests/search.rs`. Everything else from round 4 stays as it is: the renderers, `is_gutter`, `is_drawing_of`, the marker and fence handling in `Source::new`, the count check's other branches, `anchored`, fold/expansion handling, stepping, highlighting and the snapshots.

**1. Edit/Write branch of `place`.**
- In the untruncated `drawn < written` branch for `self.source.diff` (now `last`), return `nth.or(last)`. This restores the round-3 behaviour: the n-th drawing when it exists, and the last drawing only when there is no n-th.
- Reword the comment there. A diff draws each context line once, although both the old and the new text contain it. So the occurrences beyond the drawn copies (`earlier >= drawn`) fall back to the last copy's row.

**2. New test: a repeated context line in an Edit**, at the 80x17 viewport, using `tool_turn(crate::model::ToolKind::Edit, json!({"file_path": "/a/b.rs", "old_string": old, "new_string": new}), "ok")`.
- Each string has 62 lines:
  - line 0 is `usb ctx`;
  - lines 1..=20 differ (`old N` / `new N`);
  - lines 21..=55 are `c N`;
  - line 56 differs (`old 56` / `new 56`);
  - line 57 is `usb ctx`;
  - lines 58..=61 are `c N`.
- The diff then draws two hunks, `@@ -1,24 +1,24 @@` and `@@ -54,7 +54,7 @@`, about 47 rows apart. Each holds one ` usb ctx` row.
- Search `usb`:
  - At `1/4`: `assert_hit_shown(&mut t, "1/4", "@@ -1,24 +1,24 @@", "usb")`, and `@@ -54,7 +54,7 @@` is not on screen.
  - Press `n`. At `2/4`: `assert_hit_shown(&mut t, "2/4", "@@ -54,7 +54,7 @@", "usb")`.
- `3/4` and `4/4` (the `old_string` copies) landing on hunk 2 is the accepted limit. Do not assert or chase it.
- This test must fail on the round-4 code.

**3. Highlight assertions in the round-4 tests.** The round-4 note required every new test to assert that a drawn hit is highlighted where it is drawn.
- **`near(t, position, line)`.**
  - Add a `hit: &str` parameter.
  - After the neighbour check, assert `highlighted(t).iter().any(|m| m == hit)`.
  - Pass `"done"` or `"usb"` (the query) from:
    - the six marker-in-output tests (`a_hash_line_in_tool_output_…`, `a_star_line_in_tool_output_…`, `a_hash_line_before_a_plain_one_beyond_the_limit_…`, `a_plain_line_before_a_hash_line_within_the_limit_…`, `a_plain_line_before_a_hash_line_beyond_the_limit_…`, `an_indented_line_before_a_star_line_is_shown`);
    - `a_row_that_reads_like_a_line_but_ends_a_longer_one_is_not_taken_for_it`.
- **`walk(text, expected)`.**
  - Make `expected` a `&[(&str, bool)]`, where the flag says whether the expected row draws the query.
  - At flagged steps, call `assert_hit_shown(&mut t, &pos, row, "usb")` in place of the bare `contains`.
  - The flag is `false` only where the expected row has no drawn `usb`:
    - `1/2` (`Read the spec first.`) in `a_paragraph_with_only_a_url_hit_does_not_borrow_the_next_match`;
    - `1/4` (`See the docs and code.`) and `2/4` (`plain block`) in `a_link_and_a_fence_info_string_before_the_hit_do_not_shift_it`.
  - Every other step is `true`, including the URL steps of `a_url_hit_before_a_drawn_one_…` and `several_undrawn_and_drawn_hits_…`. Their paragraph row also draws `usb`.
- **`a_hash_line_in_a_fenced_code_block_is_not_taken_for_the_plain_lines_after_it`.** After `n`, also assert `"done"` is among `highlighted(t)`.
- **`a_hash_line_in_a_fenced_block_after_a_paragraph_is_found_on_its_own_row`.** Add `assert_hit_shown(&mut t, "2/2", "# usb setup", "usb")` and keep the `make` check.
- **`a_url_and_a_link_text_before_a_code_span_stay_on_the_paragraph`.** Replace the `contains("or serde_json")` check with `assert_hit_shown(&mut t, "3/4", "or serde_json", "serde")`.
- **`the_second_hit_with_nothing_undrawn_between_is_the_second_occurrence`.** At `1/2` and at `2/2`, also assert `"usb"` is among `highlighted(t)`.
- Keep every existing negative assertion (`!contains("the far serde line")`, `!contains("the usb two")`, `!contains("the usb one")`, `!contains("the far usb line")`).

**4. Build notes.** Make the self-check describe what the tests actually assert.

**Optional, only if trivial:**
- In `place`, replace `for (i, _) in self.source.squashed.iter().enumerate()` with `for i in 0..self.source.squashed.len()`.
- Extend the comment above `until` in `anchored`: a later line drawn at the window start ends the window after that row, on purpose. Keep `>=`.

**Must stay green and unchanged:** every test from rounds 1–4 except the assertion additions above, the five UI snapshots and the styled companion of snapshot 3.

**Done when** the new Edit test passes and fails on the round-4 code, all assertions above pass, and `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass.

**Dropped, do not do:** changing `>=` to `>` in `anchored`, nested-fence tracking, precomputing placements, and anything else the round-4 note dropped.
