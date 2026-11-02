VERDICT: REWORK

## Assessment
All three round-1 must-fix findings are fixed in round 2:
- `y` on a folded prompt now copies the rows shown at the block width.
- `Y` is verbatim.
- The end-to-end key test expects the text on screen.

The clipboard, OSC52, toast and snapshot work is unchanged and still correct, and every reviewer re-ran the gate and it passed. One bug introduced in round 2 remains, and all three reviewers rate it must-fix.

**Code review: FAIL**
1. [must-fix] `soft_joined` always pushes `' '` when it joins a soft-wrap continuation. **To be fixed.** When the wrap broke inside a token (a long URL, path or identifier, a code line, CJK text), the copy contains a space that is neither in the source nor on screen. That breaks the binding planner note "logical lines (no soft-wrap breaks)". I confirmed it by reading `plain.rs`: `last.push(' ')` runs on every join, whatever `skipped` is. All three reviewers reproduced it independently.
2. [nit] Read result under `Y` still goes through the line-number gutter. **Ignored.** The text is complete, and the label matches the unfolded body. Exact raw output for save and export can be decided in task 026.
3. [nit] Wrapped quote rows are not joined. **To be fixed** together with UI 1b; see Conflicts.
4. [nit] The redundant `.filter(|_| !out.is_empty())`. **To be fixed**, because the same lines are being rewritten anyway. It is not a reason to rework on its own.
5. [nit] A lazy output is read synchronously on the UI thread for `Y`. **Ignored.** It is acceptable for this task. Keep it in mind for the §9.2 budgets when task 026 reuses `full_text`.

**Feature review: FAIL**
1. [must-fix] The same `soft_joined` space bug as code 1. **To be fixed.**
2. [nit] The quote join. **To be fixed**, as code 3.
3. [nit] A folded tool call copies its whole description line rather than the screen's `…` cut. **Ignored.** This matches the acceptance criterion "exactly its description line", and the reviewer asks for no change without the planner.
4. [nit] `verbatim_lines` uses `str::lines()`, which turns CRLF into LF, while its doc comment says "no dropped characters". **To be fixed** by rewording the doc comment only. CRLF becoming LF is fine for the clipboard, and the planner note asks only for the full result via `ToolOutput::text()`.
5. [nit] Scratchpad collision. **Ignored.** This is process only, not code.

**UI review: FAIL**
1. a) [must-fix] The same space bug as code 1. **To be fixed.**
1. b) [must-fix] A soft-wrapped quote is copied as two `│ `-prefixed lines. **To be fixed.** The planner note requires logical lines with the `│ ` prefix kept, which means one line with the prefix once.
   - One part of this finding is **not** taken: the instruction to change the third assertion of `soft_joined_matches_rows_to_the_unwrapped_lines`. That assertion's logical lines have no `│`, so it tests rows that match nothing, and it must keep giving two lines. Instead, add a separate quote case.
2. [nit] A horizontal rule is 80 cells in the plain text. **Ignored.** The spec does not pin it, and keeping it was a deliberate choice.

The toast UI and all four snapshots are correct and unchanged.

## Conflicts
There is no real conflict. All three reviewers ask for the same space fix. Code and UI say "push a space only when `skipped > 0`". Feature says "push the whitespace slice the logical line has there". The two differ only where the source has more than one whitespace character at the break, and the logical slice satisfies both, so the note uses it.

The quote join is a difference in severity, not in direction:
- UI rates it must-fix.
- Code calls the current behaviour acceptable but suggests the same change for consistency.
- Feature asks for it if it is cheap.

No reviewer asks for the rows to stay apart, and the binding planner note supports joining them, so the note includes it.

## Rework budget
Earlier REWORK verdicts: 0 of 2. Round 1 had no verdict, because round 2 was the automatic fix round. This verdict uses 1 of 2.

## Rework note
Fix how `soft_joined` in `src/components/turn_block/plain.rs` joins the rows of a folded prompt back into logical lines. Nothing else in the task changes.

1. **Join with the source's own whitespace, not a fixed space.**
   - Today the join branch always does `last.push(' ')`. That adds a space when the screen wrapped inside a token, for example a long URL, path or identifier, a code-block line, or CJK text with no spaces.
   - Have the `found` closure return the whitespace it skipped along with the end, i.e. `&rest[..skipped]`. It already computes `skipped`.
   - In the join branch, push that slice (nothing when `skipped == 0`), then `row.trim_start()`.
   - The result: a joined line is exactly the matching part of the logical line, plus a trailing `…` where the fold cut it.

2. **Join the wrapped rows of a quote.**
   - A continuation row of a quoted paragraph starts with the quote prefix `│ `, repeated for nested quotes. So today it never matches the rest of its logical line and stays a separate `│ …` line.
   - Before matching a row as a continuation, check whether the logical line it would continue starts with a quote prefix (one or more `│ `) and the row starts with the same prefix. If so, strip that prefix from the row.
   - Then match the stripped row and join it as in step 1, appending the stripped row rather than the original. The prefix then appears once, at the start of the line.
   - Rows that still match nothing stay lines of their own, as now.

3. **Tidy-ups in the same code.**
   - Drop the redundant `.filter(|_| !out.is_empty())` in the `Some((line, from))` arm. The `if let (Some(next), Some(last))` already needs a previous row.
   - Update the doc comments of `soft_joined` and `displayed_text`, which now say rows are "joined by a space". They should say rows are joined with the whitespace the source has there.
   - In `src/components/turn_block/renderers/mod.rs`, reword the `verbatim_lines` doc comment to say that a CRLF line ending becomes LF. Do not change the split.

4. **Tests.**
   - In `soft_joined_matches_rows_to_the_unwrapped_lines`:
     - Add a mid-word case: rows `["abc", "def"]` against logical `["abcdef"]` give `["abcdef"]`.
     - Add a spaced case: rows `["ab", "cd"]` against `["ab cd"]` give `["ab cd"]`.
     - Add a quote case: rows `["│ alpha", "│ beta"]` against `["│ alpha beta"]` give `["│ alpha beta"]`.
     - Keep the existing third assertion (`["│ alpha", "│ beta"]` against logical lines without `│` stay apart). It is the unmatched case. This overrides the UI review's request to change it.
   - New `plain::tests` cases with `displayed_text` at block width 80 on a folded prompt. Reference renders are in `factory/state/tasks/025-copy-to-clipboard/round-2/ui/folded-softwrap-joins-80x24.txt`.
     - A single-token prompt, `https://example.com/` followed by `abcdefghij` 20 times. Assert that the copy contains no space and no `\n`, and that `source.starts_with(copy.trim_end_matches('…'))`.
     - A CJK paragraph without spaces, for example `日本語のテキストはスペースなしで長く続きます。` repeated 8 times. Use the same prefix assertion and assert that there is no space.
     - A prompt that is one quoted paragraph long enough to need more than two rows at width 80. Build it from a quote such as `> a quoted line that is long enough to wrap over more than one row at sixty or eighty columns wide`, followed by more words. Assert that:
       - the copy is one line (no `\n`);
       - it contains `│` exactly once, at the start (`│ `);
       - after the `│ `, it is a prefix of the quote's text, ignoring the trailing `…`;
       - it ends in `…`.
     - Optionally, a folded prompt whose first code-block line is wider than the row. It must copy that line's shown prefix with no inserted space.

**Done when:**
- The tests above pass.
- The existing folded-prompt tests, including `browser_tests::copy::y_copies_the_selected_section_as_displayed`, pass unchanged. The fixture breaks at real spaces.
- The four toast snapshots pass with `INSTA_UPDATE=no` and no `.snap.new` files.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

Leave these as they are:
- the Read gutter under `Y`
- the whole description of a folded tool call
- the 80-cell rule
- the synchronous lazy read
