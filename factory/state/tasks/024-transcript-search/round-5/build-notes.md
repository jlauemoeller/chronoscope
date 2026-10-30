STATUS: DONE

## Changes
- `SectionView::place` (`src/components/transcript/search.rs`): the untruncated `drawn < written` branch for Edit/Write sections returns `nth.or(last)` again (the n-th drawing when it exists, the last only when there is none). The comment now says that a diff draws each context line once although both texts contain it, and the occurrences beyond the drawn copies (`earlier >= drawn`) fall back to the last copy's row. Optional items done: `for i in 0..self.source.squashed.len()` in `place`, and the comment above `until` in `anchored` says a later line drawn at the window start ends the window after that row on purpose (`>=` kept).
- Tests only otherwise; nothing else changed.

## Self-check (what the tests assert; viewport 80x17)
- New `a_repeated_context_line_of_an_edit_lands_on_its_own_hunk`: Edit with `usb ctx` at lines 0 and 57 of both strings. `1/4` shows `@@ -1,24 +1,24 @@` with `usb` highlighted and `@@ -54,7 +54,7 @@` not on screen; after `n`, `2/4` shows `@@ -54,7 +54,7 @@` with `usb` highlighted. It fails on the round-4 code (checked by restoring it; it is the only test that fails there).
- `near(t, position, line, hit)` now also asserts that `hit` is highlighted; used by the six marker-in-output tests (`done`/`usb`) and `a_row_that_reads_like_a_line_but_ends_a_longer_one_is_not_taken_for_it` (`done`).
- `walk` takes `(row, drawn)` pairs: where `drawn` is true it uses `assert_hit_shown` (position, row on screen, `usb` highlighted); the flag is false only for `Read the spec first.` (`1/2`), `See the docs and code.` (`1/4`) and `plain block` (`2/4`), which draw no `usb`, where position and on-screen row are asserted.
- Highlight assertions added to: `a_hash_line_in_a_fenced_code_block_is_not_taken_for_the_plain_lines_after_it` (`done`), `a_hash_line_in_a_fenced_block_after_a_paragraph_is_found_on_its_own_row` (`assert_hit_shown` on `# usb setup`, plus `make` on screen), `a_url_and_a_link_text_before_a_code_span_stay_on_the_paragraph` (`assert_hit_shown` on `or serde_json`, `serde`), `the_second_hit_with_nothing_undrawn_between_is_the_second_occurrence` (both positions). All negative assertions kept.
- Not asserted (undrawn matches, accepted limit): `3/4`/`4/4` of the Edit case (the `old_string` copies) landing on hunk 2.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (1340 lib tests, 0 failed; integration tests pass)
