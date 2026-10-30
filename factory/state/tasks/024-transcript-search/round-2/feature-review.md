VERDICT: FAIL

## Summary
Round 2 fixes the round-1 code-review must-fix. When a fold command folds a section the search had expanded (`h`, `-`, or `Enter` on a section the search only expanded), the expansion is now dropped, and tests cover this. The round-1 nits were also addressed. fmt, clippy and `INSTA_UPDATE=no cargo test` pass (1307 lib tests, plus the integration tests).

The new way of finding a match's row (`SectionView::row_of`) still puts the jump on the wrong row in a common case. That case is a short output line whose text, ignoring white space, is the end of an earlier drawn row. A match of that kind beyond the truncation limit is also not expanded. I reproduced both with probe tests in a scratch copy.

## Acceptance criteria
- [x] Smart-case rule — `SearchQuery` in `src/transcript_search.rs`. Covered by `a_lower_case_query_matches_any_case` and `a_query_with_an_upper_case_letter_matches_that_case_only`. They still check the rule after the round-2 removal of `is_case_sensitive`/`is_match`.
- [x] Search covers folded sections, tool input, full output (lazy over 4 KiB, beyond truncation), system content and compaction summaries, and excludes hidden system content. This is unchanged since round 1, and the same tests still pass. The matching is correct. Showing a match beyond the limit has the bug in finding 1.
- [x] Off the UI thread via Port → `UserEvent` → `GlobalEvents` → `Msg`, and old generations are ignored. Unchanged since round 1; the update tests still pass.
- [x] `Enter` jumps to the first match, `n`/`N` wrap, the help line shows `i/n`, and no match shows `0/0` plus a toast. The position logic is correct and tested. The row that `n`/`N` scroll to can be wrong (finding 1).
- [x] Jumping into a folded section unfolds it, moving away refolds it, and `Enter` in between keeps it unfolded. Unchanged. New tests cover the expansion being dropped on fold: `folding_the_section_drops_the_expansion_the_search_made`, `fold_all_drops_the_expansion_the_search_made`, `enter_on_a_section_the_search_only_expanded_folds_it_and_the_expansion_goes` and `unfold_all_keeps_what_the_search_opened`. I traced `fold_command_on_temp` and `forget_temp_unfold(folding)` for `Fold`, `Toggle`, `Unfold`, fold-all and unfold-all, and they match the build notes.
- [x] `Esc` removes highlights, the position item and temporary unfolds. Unchanged; the tests pass.
- [x] Typing `n`, `q` or `t` in the prompt inserts them and triggers no action. `printable_keys_go_into_the_prompt_and_trigger_no_action` now runs a search first and asserts `1/5` stays. It also asserts that no selector is mounted after `t`, so the test would now fail if the behaviour were wrong.
- [x] Every UI state has a snapshot. The five states are unchanged, and there is a new styled companion for state 3.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran them all.

## Findings
1. [must-fix] **The row of a match is wrong when its line is the end of an earlier drawn row, and a match beyond the limit is not expanded.** The fault is in `SectionView::row_of` / `nth_drawn` / `is_drawing` in `src/components/transcript/search.rs`. The two counts it compares use different rules:
   - `earlier` counts the earlier source lines whose squashed text is *exactly equal* to the match's line.
   - `nth_drawn` counts every *drawing*: any place in the joined rows where the line ends a row and starts within `GUTTER` (12) bytes of the row start. That accepts the tail of an unrelated longer row.

   For example, the drawn row `build done` squashes to `builddone`. That counts as a drawing of the line `done`, because it starts 5 bytes in. The n-th drawing is then not the n-th source line.

   The trigger is easy to hit: short output lines such as `done`, `ok`, `}`, `true` or `error`, after any earlier row that ends with the same text. The binding planner rules it breaks are "A match beyond the truncation limit also expands the output temporarily" and "`n`/`N` … scroll the match's line into view". When it happens, the help line shows `2/2` while the earlier hit is the one in view.

   Reproduction, added to `src/components/transcript/tests/search.rs` in a scratch copy. Both tests fail at HEAD. The screen stays at the top of the output, on `build done`, and in the first test `expanded=false`.
   ```rust
   #[test]
   fn probe_suffix_line_beyond_limit() {
       let mut lines = filler(500, 350, "done");
       lines[4] = "build done".to_owned();
       let mut t = log_of(&lines);
       assert_eq!(search(&mut t, "done").as_deref(), Some("1/2"));
       ch(&mut t, 'n');
       assert_eq!(shown(&t).as_deref(), Some("2/2"));
       let screen = rows(&mut t);
       assert!(t.content.folds.is_expanded(&tool()), "expanded\n{screen}");
       assert!(screen.contains("line 349") || screen.contains("line 351"), "{screen}");
   }

   #[test]
   fn probe_suffix_line_within_limit() {
       let mut lines = filler(150, 120, "done");
       lines[4] = "build done".to_owned();
       let mut t = log_of(&lines);
       assert_eq!(search(&mut t, "done").as_deref(), Some("1/2"));
       ch(&mut t, 'n');
       let screen = rows(&mut t);
       assert!(screen.contains("line 119") || screen.contains("line 121"), "{screen}");
   }
   ```
   Change: make the drawing that is picked correspond to the match's own source line. Any of these would do:
   - (a) Allow only gutter characters before the line in `is_drawing`: digits, `+`, `-`, `│`, `|`, `:` and the like, never letters. Then real text such as `build` cannot pass for a gutter.
   - (b) Count `earlier` with the same acceptance rule that `nth_drawn` applies. That rule is: an earlier source line counts if its squashed text ends with the line and the extra prefix is at most `GUTTER` bytes.
   - (c) Align the lines in order. Place each source line `0..=found.line` at or after the end of the drawing of the line before it, so an earlier row cannot be taken for a later line.

   Add the two tests above, with real names, to `src/components/transcript/tests/search.rs`. Check that the hit's row is highlighted, as the round-2 tests do.
2. [nit] Labels (`Result:`, `Command:`) are still highlighted when the query hits them. This is carried over from round 1, follows the planner's per-row rule, and is acknowledged in the build notes. Change, if wanted: restrict `TurnBlock::highlight` to content spans.
3. [nit] A hit that wraps across two rows gets no highlight. This is carried over, the planner note allows it, and the build notes acknowledge it. Change, if wanted: highlight a row-ending prefix together with the next row's matching start.
4. [nit] The first `Enter` on a section the search unfolded changes nothing visible, and the second one folds it. It is now documented in the module doc and in `fold_command_on_temp`. It is listed only so the Approver knows; no change needed.
5. [nit] `SectionView::row_of` calls `self.source.split('\n').collect()` and squashes every earlier line on each jump. It also gets the source again for each `section_view` call (`derive_sections` + `section_text`, which materialises lazy output). For a large expanded output this is O(lines) work on the UI thread for each `n`. That is fine for now. Consider caching the squashed lines per section for the duration of a search.
