VERDICT: FAIL

## Summary
The three round-4 changes work for the cases they target:
- Markdown markers in tool output and in fenced code no longer shift a jump.
- The anchored window keeps undrawn hits (link URLs, fence info strings) on their own paragraph.
- The count check rejects a wrapped row that only reads like a later line.

All round-3 scratch scenes still render identically, and the snapshots are unchanged. One regression remains, in Change 2. In Edit/Write sections, `place` now returns the *last* drawing unconditionally whenever a line is drawn fewer times than it is written. Round 3 returned the n-th drawing and fell back to the last one only when there was no n-th. As a result, a context line that repeats in two far-apart hunks always jumps to the second hunk, and the first copy is never shown.

## States checked
- Prompt open with `usb` typed @ 80x24. Snapshots `prompt_with_usb_typed_80x24_text` / `_styled`. ok, unchanged since round 3.
- After `Enter`, the first match selected, highlights, `1/5` @ 120x40. Snapshots `after_enter_the_first_match_is_selected_and_highlighted_120x40_text` / `_styled`. ok.
- After `n` into the folded tool call: unfolded, hit in the result, `2/5` @ 120x40. Snapshot `after_n_into_the_folded_tool_call_it_is_unfolded_and_highlighted_120x40_text` / `_styled`. ok.
- After `j` off it: refolded, `2/5` kept @ 120x40. Snapshot `after_moving_off_the_tool_call_it_is_folded_again_and_the_position_stays_120x40_text`. ok.
- After `Esc`: no highlights and no position @ 80x24. Snapshot `after_escape_there_are_no_highlights_and_no_position_80x24_styled`. ok.
- Test gate:
  - `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass (1339 lib tests plus the integration tests).
  - The round-4 commit touches no `.snap`, and there are no `.snap.new` files. ok.
- Rendered in the scratch crate @ 80x17. Each check asserts the `i/n` read from `SEARCH_POSITION_ATTR`, that the expected row text is on screen, and that a highlight is present.
  - **Markers in shell output** (`# done`, `* done`, `> usb q`, `1. usb n`, `- usb d`, before or after a plain twin, within and beyond the 100-line limit; a unique `# usb here` within the limit is not expanded). ok. Round 3 failed A1–A5 here.
  - **Fenced code in an assistant text** (backtick fence, `~~~` fence, a fence inside a `1.` list item, a fence inside a nested list, an unclosed fence, a fence after a paragraph, markers stripped again after a fence closes). ok.
  - **Anchored window** (URL plus drawn hit on one line; URL-only paragraph; code span plus URL plus hit at `4/5`; the round-2 `md` scene with a link and a ```` ```usb ```` fence; the `serde` case at `3/4`; a URL in a wrapped 200-character paragraph; a URL in a list item). ok. Round 3 failed C1–C7 here.
  - **Count check** (a 69+4 character log line whose wrapped tail row reads `done`, then `done` on line 60; a markdown paragraph wrapping to a `done` row). ok.
  - **Non-markdown sections:** System content with `# usb rule` before `usb rule`. ok.
  - **Markdown in a Subagent call:** a `# usb` heading and a `- usb` item in the prompt, and a `## usb` heading in the result. ok.
  - **Edit with a context line repeated in two hunks.** **issue**, finding 1.
- Regression sweep: every scratch scene from rounds 1–3 run on HEAD and on round 3 (`5ca5d9b`, built from `git archive` outside the repo).
  - Scenes: fixture @ 120x33, 60x13 and 60x8; long outputs; suffix; inline markdown; Read/Write/Edit/Grep/MCP tools; JSON input; narrow @ 40x10 and 30x8; table and heading; wrap/split; labels; CJK; dim; case; leave; fold/expand.
  - Output is byte-identical except `md` and `probe3`, which are the intended round-4 fixes. ok.

## Findings
1. [must-fix] Transcript, jump to a match in an Edit section, @ transcript viewport 80x17 (the 80x24 screen). See `ui/edit-repeated-context-line-80x17.txt`.

   **Problem.** In `SectionView::place` (`src/components/transcript/search.rs`), the `drawn < written`, untruncated, `self.source.diff` branch returns `last` and discards `nth`. That is a regression:
   - **Round 3 behaviour.** `nth_drawn(.., clamp = true)` gave the n-th drawing and fell back to the last one only when there was no n-th.
   - **Why the counts differ.** A diff writes each context line twice (once in `old_string`, once in `new_string`) and draws it once. So whenever a context line appears in two hunks, `drawn` (2) is less than `written` (4). Every occurrence then lands on the last hunk.
   - **Scene.** The Edit has `usb ctx` as the first context line of `@@ -1,24 +1,24 @@` and again in `@@ -54,7 +54,7 @@`, about 47 rows apart.
   - **Result.** At `1/4` (the `new_string` line 0 copy), the view scrolls to offset 54 and shows only the hunk-2 row. Hunk 1's highlighted `usb ctx` row is off screen. Round 3 showed hunk 1 at offset 0. With `n`, the first copy is never shown at any position.
   - **Rules broken.** This breaks the invariant "after `Enter`/`n`/`N`, the row drawn from the match's own source line is on screen and highlighted". It also contradicts the round-4 note. That note says to *keep* the clamp, and its required comment says only "the surplus occurrences land on the row of the last copy". Searches for common code text (`Ok(())`, `return`, `}`) hit repeated context lines in ordinary Edit calls.

   **Change.**
   - In the diff branch of `place`, return `nth.or(last)` instead of `last`. Update the comment to say that only occurrences beyond the drawn copies fall back to the last one.
   - I verified this in a patched copy outside the repo:
     - `1/4` lands on hunk 1 and `2/4` on hunk 2.
     - `3/4` and `4/4` land on hunk 2. These are the `old_string` copies, with `earlier >= 2`, so they are surplus. That is the accepted limit; do not chase it.
     - `INSTA_UPDATE=no cargo test` stays green: all 1339 lib tests and every integration test.

   **Test.** Add a component test in `src/components/transcript/tests/search.rs` at viewport 80x17.
   - **Input.** A `ToolKind::Edit` whose `old_string` and `new_string` are 62 lines:
     - line 0 is `usb ctx`;
     - lines 1..=20 differ (`old N` / `new N`);
     - lines 21..=55 are `c N`;
     - line 56 differs;
     - line 57 is `usb ctx`;
     - the rest are `c N`.
   - **Assertions.** Search `usb`. At `1/4`, `@@ -1,24 +1,24 @@` and the highlighted `usb ctx` row are on screen. Press `n`: at `2/4`, `@@ -54,7 +54,7 @@` is on screen.

The following cases were checked and are not findings:
- **Undrawn context lines.** Edit context lines outside every hunk are searched (the full input is searched) but never drawn. A jump to one lands on the section top. This is the same as in round 3.
- **Split wrapped line.** In the count-check scene, `1/2` highlights `done` on the second row of the wrapped line, which is the real match.
