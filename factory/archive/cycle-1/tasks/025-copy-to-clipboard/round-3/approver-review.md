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
