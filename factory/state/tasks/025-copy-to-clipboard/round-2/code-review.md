VERDICT: FAIL

## Summary
Round 2 fixes all three round-1 problems. `Y` is now verbatim through `SectionCtx::verbatim`: tabs, very long lines and Write content are kept exactly. `y` on a folded prompt now cuts at the transcript's block width, and the fixture copies exactly the text the 80x24 snapshot shows. The round-1 nits are also addressed (failure reason logged, `Copied 0 lines`, private `mod copy` and `PLAIN_WIDTH`, OSC52 size limit documented, `/` and `w` back in the keys test). One new bug remains. `soft_joined` always puts a space between two joined rows, so a folded prompt whose row break falls inside a word gets an extra space in the copy. A word is split across rows when it is char-wrapped: CJK text, a long identifier in a code block, or a long URL or path.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (lib 1415 passed, 1 ignored; all other targets pass; no `.snap.new` files, working tree clean)

## Findings
1. [must-fix] `src/components/turn_block/plain.rs:109-145` (`soft_joined`, the `last.push(' ')` at line 136): a space is inserted at every soft-wrap join, even where the logical line has no whitespace.
   - **Effect.** When a row break falls inside a word, the copy gets a space that is neither in the prompt nor on the screen. That happens with CJK text, which wraps between characters, and with any token longer than the row (a code-block line, a long identifier, a path), which is char-wrapped. This breaks the planner note "logical lines (no soft-wrap breaks)": the copied text is changed, not unwrapped.
   - **Reproduced** in a scratch copy, block width 60, `displayed_text(..., Some(60))` on a folded prompt:
     - `"日本語のテキストはスペースなしで折り返されます。".repeat(8)` copies as `…折り返されます。日本 語のテキスト…`, with an extra space after `日本`.
     - A fenced code line `fn main() { let a_very_long_line_of_code = something_else_that_is_long(...); }` copies as `… = somethi ng_else_that_is_long(…`, with an extra space inside the identifier.
   - **Change.**
     - Have the `found` closure also return whether it skipped whitespace before the match (`skipped > 0`). It already computes `skipped` at line 124.
     - In the join branch, push `' '` only when whitespace was skipped. Otherwise append `row.trim_start()` directly.
     - Add cases to `soft_joined_matches_rows_to_the_unwrapped_lines`. For example, logical `["abcdef"]` with rows `["abc", "def"]` must give `["abcdef"]`, and logical `["ab cd"]` with rows `["ab", "cd"]` must still give `["ab cd"]`.
     - Optionally add a `plain` test of a folded CJK or code-block prompt at a fixed width.
2. [nit] `src/components/turn_block/renderers/tool/read.rs:96-125` (`gutter_rows`): under `verbatim` (`Y`), a Read result still goes through the gutter. Claude's `     12→text` comes out as `12  text`, reformatted and right-aligned. The text is complete and the label matches the unfolded body, so this is acceptable. If `Y` and the later save/export should give the result exactly "via `ToolOutput::text()`", bypass `numbered`/`gutter_rows` when `ctx.verbatim` and use `text_block("Content:", &text, …)` on the raw result.
3. [nit] `src/components/turn_block/plain.rs:109-145` (`soft_joined`): wrapped rows of a quoted paragraph (`│ …`) are not matched to their logical line, so they stay two lines (`│ a quoted … will\n│ wrap around …`). This is documented ("a quote bar, say") and matches the screen, so it is fine as is. Matching after a common `│ ` prefix would make it consistent with plain paragraphs and list items, which do get joined.
4. [nit] `src/components/turn_block/plain.rs:127-132`: in the `Some((line, from))` arm, `.filter(|_| !out.is_empty())` is redundant: the following `if let (Some(next), Some(last)) = (next, out.last_mut())` already requires a previous row. It can be dropped.
5. [nit] Round-1 code finding 5 (a lazy output read synchronously on the UI thread for `Y`) is unchanged, and the build notes say so. It is still acceptable for this task. Keep it in mind for the §9.2 budgets when task 026 reuses `full_text` for save and export.
