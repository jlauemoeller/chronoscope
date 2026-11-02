VERDICT: FAIL

## Summary
Round 3 rewrote `SectionView`/`row_of` as the rework note asked. The gutter check is strict, one predicate is used on both sides, there is an anchored fallback, and the source is split once per jump. All the requested tests are present and green, as are the protected round-2 tests and snapshots. fmt, clippy and `INSTA_UPDATE=no cargo test` pass (1322 lib tests, plus the integration tests).

Two parts of the new placement still put a jump on the wrong row: the markdown-marker stripping and the anchored j-count. In both cases the help line shows `i/n` while a different hit is on screen. I reproduced 7 cases with probe tests in a scratch clone. All 7 fail at HEAD and pass with the round-2 `search.rs` (HEAD~1), so they are regressions from this round. That breaks the rework note's binding invariant ("the row drawn from the match's own source line is on screen").

## Acceptance criteria
- [x] Smart-case rule — `SearchQuery` in `src/transcript_search.rs`. Covered by `a_lower_case_query_matches_any_case` and `a_query_with_an_upper_case_letter_matches_that_case_only`. Unchanged.
- [x] Search covers folded sections, tool input, full output (lazy over 4 KiB, beyond truncation), system content and compaction summaries, and excludes hidden system content. The matching is unchanged and its tests pass. Showing some matches is affected by findings 1 and 2.
- [x] Off the UI thread via Port → `UserEvent` → `GlobalEvents` → `Msg`, and old generations are ignored. Unchanged; the update tests pass.
- [x] `Enter` jumps to the first match, `n`/`N` wrap, the help line shows `i/n`, and no match shows `0/0` plus a toast. The position logic is correct and tested. The row scrolled to is wrong in the cases of findings 1 and 2.
- [x] Temporary unfold and refold, and `Enter` in between makes it explicit (component tests). Unchanged.
- [x] `Esc` removes highlights, the position item and temporary unfolds (component test). Unchanged.
- [x] `n`/`q`/`t` typed in the prompt are inserted and trigger no action (update test). Unchanged.
- [x] Every UI state has a snapshot. `--stat` shows no snapshot file changed this round, so the five snapshots and the styled companion of state 3 are unchanged and pass.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran them all.

What I verified from the rework note:
- The test diff only appends. The protected tests are untouched and green: `a_query_that_a_label_also_contains_…`, `a_label_is_not_taken_…`, `a_hit_that_is_not_drawn_…`, `a_hit_that_wraps_…`, `the_same_line_twice_…`, `a_match_far_down_…`, and the truncation and expansion tests.
- Note cases 1–8 are all present: the four suffix/prefix tests, the four paragraph tests, the generic JSON tool test, the two-code-span test, and Read, Edit `+`, bullet and quote (plus a numbered list).
- The strict gutter works where the source line has no stripped marker. My probes with a `- ` prefix and with a digit prefix (`2 done` before `done`, beyond the limit) both land on the right row and expand.

## Findings
1. [must-fix] **Markdown markers are stripped from every source line, including text that is drawn verbatim, so a line such as `# x` or `* x` lands on the row of an earlier plain `x`.**

   How it happens, in `src/components/transcript/search.rs`:
   - `Source::new` (line ~610) applies `marker_len` (line ~577) to every line of every section. That includes Bash/tool output, generic tool input, and lines inside fenced code blocks of assistant text, none of which the markdown renderer turns into markers.
   - So `# usb setup` gets the body `usbsetup`, and `* usb-fix` gets `usb-fix`. Each is then the same as an earlier plain line `usb setup` / `usb-fix`, and `earlier` counts that line.
   - The drawn row keeps its `#` / `*` / `>`. Those are not `is_gutter` characters (line ~555), so `nth_drawn` finds only the earlier row.
   - Without truncation, `place` (line ~733) then clamps (`last.filter(|_| clamp)`, line ~763) to that earlier drawing. It does not return `None`, so the anchored fallback never runs.

   The result: `n` shows `2/2` but scrolls to the earlier line. This is the same symptom as round 2's must-fix.
   - Beyond the truncation limit, the output is expanded and the earlier row is still the one shown.
   - A unique `# …` line early in a truncated output (line 10 of 500) also fails `place`, so the output is expanded needlessly before the anchored fallback finds it.
   - Triggers are everyday text: shell, Python, YAML and TOML comments (`# …`) in tool output and in code blocks, `git branch` output (`* main`), and `>` lines in output.
   - All of these probes pass with round-2's `search.rs`.

   Reproduction (append to `src/components/transcript/tests/search.rs`). Each fails at HEAD.
   ```rust
   #[test]
   fn probe_hash_line_within_limit() {
       let mut lines = filler(150, 120, "# usb setup");
       lines[4] = "usb setup".to_owned();
       let mut t = log_of(&lines);
       assert_eq!(search(&mut t, "usb").as_deref(), Some("1/2"));
       ch(&mut t, 'n');
       assert_eq!(shown(&t).as_deref(), Some("2/2"));
       let screen = rows(&mut t);
       assert!(screen.contains("line 119") || screen.contains("line 121"), "{screen}");
   }
   // Same with filler(500, 350, "# usb setup"): expect the output expanded and
   // "line 349"/"line 351" on screen. Same with filler(150, 120, "* usb-fix")
   // and lines[4] = "  usb-fix".

   #[test]
   fn probe_hash_comment_in_code_block() {
       let mut text = String::from("usb setup\n\n");
       text.push_str(&(1..=30).map(|n| format!("filler {n}")).collect::<Vec<_>>().join("\n\n"));
       text.push_str("\n\n```sh\n# usb setup\nmake\n```\n");
       let mut t = markdown_turn(&text, 80);
       assert_eq!(search(&mut t, "usb").as_deref(), Some("1/2"));
       ch(&mut t, 'n');
       assert_eq!(shown(&t).as_deref(), Some("2/2"));
       let screen = rows(&mut t);
       assert!(screen.contains("make"), "{screen}"); // fails: screen shows the top
   }

   #[test]
   fn probe_hash_line_early_in_truncated_output() {
       let mut t = log_of(&filler(500, 10, "# usb setup"));
       assert_eq!(search(&mut t, "usb").as_deref(), Some("1/1"));
       assert!(!t.content.folds.is_expanded(&tool())); // fails: expanded needlessly
   }
   ```
   Change:
   - Strip markers (`bodies` ≠ 0) only for lines that the markdown renderer draws as markdown: assistant/user text sections, and not inside fenced code blocks. Use `bodies = 0` for tool input and output, system content and lines inside fences.
   - Make the clamp in `place` safe. Apply it only where a line can really be drawn fewer times than written (the Edit/Write diff context lines). Elsewhere, a missing n-th drawing returns `None`, so the anchored fallback runs.

   Add the five probes above as named tests: `#` within and beyond the limit, `*`, the fenced comment, and no needless expansion.

2. [must-fix] **The anchored fallback's j-count includes matches whose text is not drawn, so a drawn hit on a line with a link before it lands on a later hit far below.**

   How it happens, in `SectionView::anchored` (line ~782 and the `before` count at line ~795):
   - Every match on the source lines after the anchor counts, including matches on the found line itself that come earlier.
   - A hit inside a link URL is in the source but not drawn. So for the next hit on the same line, `before` is one too high, and the search takes the next drawn occurrence. That one is further down, often off screen, and belongs to a different match.
   - Example: ``Use [serde](https://serde.rs) or `serde_json`.`` has 3 source matches but 2 drawn ones. `3/4` (on `serde_json`) scrolls to the next `serde` paragraph far below.
   - Round 2's tail lookup placed both probes below correctly.

   The existing `two_hits_with_code_spans_each_land_on_their_own_row` does not guard the j-count. Each hit there has its own anchor (the paragraph before it), so j is always 0.

   Reproduction (both fail at HEAD and pass at HEAD~1):
   ```rust
   #[test]
   fn probe_url_hit_and_text_hit_on_one_line() {
       let mut text = String::from("See [docs](https://usb.example.com) for the `usb` port.\n\n");
       text.push_str(&(1..=30).map(|n| format!("filler {n}")).collect::<Vec<_>>().join("\n\n"));
       text.push_str("\n\nthe far usb line\n");
       let mut t = markdown_turn(&text, 80);
       assert_eq!(search(&mut t, "usb").as_deref(), Some("1/3"));
       ch(&mut t, 'n');
       assert_eq!(shown(&t).as_deref(), Some("2/3"));
       let screen = rows(&mut t);
       assert!(screen.contains("for the usb port"), "{screen}"); // shows "the far usb line"
   }

   #[test]
   fn probe_serde_link() {
       let mut text = String::from("Use [serde](https://serde.rs) or `serde_json`.\n\n");
       text.push_str(&(1..=30).map(|n| format!("filler {n}")).collect::<Vec<_>>().join("\n\n"));
       text.push_str("\n\nthe far serde line\n");
       let mut t = markdown_turn(&text, 80);
       assert_eq!(search(&mut t, "serde").as_deref(), Some("1/4"));
       ch(&mut t, 'n');
       ch(&mut t, 'n');
       assert_eq!(shown(&t).as_deref(), Some("3/4"));
       let screen = rows(&mut t);
       assert!(screen.contains("or serde_json"), "{screen}"); // shows "the far serde line"
   }
   ```
   Change: bound the anchored search window. Search only the rows from after the anchor up to the drawing of the next source line after `found.line` that is placed, or the section end if none is. If fewer than j+1 occurrences are drawn in that window, take the last one in it. Do not continue into rows that belong to later lines.

   Add the two probes as named tests. Add one more where j > 0 without undrawn hits: two inline-code hits on one line, or on consecutive lines with no placeable line between them. It must check that each lands on its own row.

3. [nit] Performance off the common path. `anchored` may call `place` for up to `ANCHOR_LOOKBACK` (200) earlier lines. Each call is O(index) for `earlier` plus a full scan of `joined`. `is_drawing` also re-squashes the last row for every candidate (`squash(&self.rows[last].1)`). For a large expanded output with several unplaceable lines in a row, that is noticeable work on the UI thread per `n`. Change, if wanted: precompute the squashed row lengths in `SectionView::new`, and compute the placements of the lines `from..found.line` in one pass.

4. [nit] Carried over and allowed by the planner's per-row rule: labels such as `Result:` are highlighted when the query hits them, and a hit wrapped across two rows gets no highlight. No change needed.
