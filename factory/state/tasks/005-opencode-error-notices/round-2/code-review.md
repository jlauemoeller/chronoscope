VERDICT: PASS

## Summary
All round-1 findings are addressed, including the feature review's must-fix: with the index rebuild in `remove` deleted in a scratch copy, the new `a_row_pushed_after_a_removed_notice_replaces_the_item_it_names_not_a_shifted_one` fails. The gate and the budgets pass (full parse worst 438.1 ms of 2000, first content worst 2.0 ms of 300, frames worst 4.81 ms of 16, memory at most 0.58x the file), only the three intended snapshots changed since round 1, and no `.snap.new` files exist. The nits are a remaining gap in the export escaping, which none of the local error rows reach, one test that does not catch what its name says (three snapshots do), and small cleanups.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [nit] `src/components/turn_block/export.rs:142`, `emphasized_lines` — only `_` and `*` are escaped, but a notice is plain text, so some messages still export wrongly. I rendered these through `pulldown-cmark`:
   - A line ending in a backslash, `path is C:\dir\`, escapes the closing delimiter: `<p>_path is C:\dir_</p>`. The emphasis is lost and the final backslash is not shown.
   - A backslash before an underscore, `a\_b c_d`, becomes `a\\_b`, where the `_` is live: `_already a\<em>b c_d</em>`.
   - An underscore inside backticks, `` `max_tokens` ``, gets a backslash that a code span shows literally: `<code>max\_tokens</code>`. This one is new in round 2 and alters the identifier in the rendered export.
   - A line indented by four or more spaces after a blank line becomes a code block with literal delimiters: `<pre><code>_indented after blank_</code></pre>`.
   - `<b>tag</b> [x](y)` exports as live HTML and a live link instead of the literal text.

   This is a nit because the real data does not reach it and no line of a notice goes missing. I read the `finish: "error"` rows of the local OpenCode database without modifying it: the 12 rows match the spec's counts (`aborted` 4, `provider.invalid-request` 6, `provider.transport` 2). The opening was immutable, so anything only in its WAL file is not included. Two rows contain a markdown-significant character, both with the span `` `verbose: true` ``, which has no `_` or `*` and renders correctly through the export. Claude's notices in the next task take the message's own text through this path, so the gap is worth closing. Change: escape `\` first, then `` ` ``, `_`, `*`, `<`, `[`, `]` and `&`; cap the kept indent at three spaces; and add the inputs above to `an_exported_notice_keeps_its_indent_outside_the_emphasis_and_escapes_markers`.
2. [nit] `src/components/transcript/tests/notice.rs:268`, `a_hyphenated_word_is_not_split_across_rows_so_a_search_highlights_it` — the test passes when `NoticeSection` goes back to `word_wrap`, because neither of its widths splits a word at a hyphen: at 100 the message fits on one row, and at 40 no row happens to end in `-`. In a scratch copy with that change, the viewport widths that split `non-` / `empty` were 77 to 80, where the position still says `1/1` but nothing is highlighted. The regression is still caught, by the three `error_long_*_at_80` snapshots, so the behaviour is not unprotected. Change: in lines 269-276, build the transcript with `notices().viewport(ratatui::layout::Size::new(80, 30))`, change `draw(&mut t, 100, 30)` to `draw(&mut t, 80, 30)` and the loop to `for x in 0..80`, and keep the assertion that the highlighted text is `non-empty`. On the current code this highlights `non-empty` at width 80.
3. [nit] `src/components/transcript/tests/notice.rs:326` — `assert!(t.select(&error()) || t.select(&item_section("a1", "n1")))` has two identical operands, since `error()` is `item_section("a1", "n1")`. Change: `assert!(t.select(&error()))`.
4. [nit] `src/harness/opencode/assemble.rs:177`, `ending_notice` — `part` returns an owned `String`, so every row with a `finish` allocates, including normal `stop` rows that return `None` straight away. The budgets are not affected. Change: have `part` return `Option<&str>` and allocate only when building the notice's text.
5. [nit] `src/harness/opencode/assemble.rs:634`, doc comment of `replace_row` — this line is visibly longer than the rest of the comment after the round-2 edit. Change: reflow the comment.
