VERDICT: FAIL

## Summary
All three round-1 must-fix findings have visible fixes:
- The folded prompt is now cut at the transcript's block width.
- `Y` is verbatim, so tabs and very long lines are kept.
- The end-to-end test now expects the text the 80x24 screen shows.

The round-1 nits are handled too: `Copied 0 lines`, the failure reason is logged, `mod copy;` is private, the OSC52 limit is documented, and the keys test presses `/` and `w` (`w` is the default `save` key). The test gate passes.

One must-fix bug remains in the new code. `soft_joined` puts a space between every pair of joined rows, including rows that were split in the middle of a word (char-wrap). So `y` on a folded prompt that starts with a long URL, path or code line copies text with a space that is in neither the source nor the screen. For example, the URL comes out as `https://example.com/aaaaaaaaaaaa aaaa…`.

## Acceptance criteria
- [ ] Copy-as-displayed (unit tests on the `Transcript` text functions):
  - [x] A folded tool call gives exactly its description line. Tested by `plain::tests::a_folded_tool_call_is_exactly_its_description` and `a_folded_tool_call_stays_its_whole_description_at_any_width`.
  - [x] An unfolded Shell call gives the description, `Command:`, the command, `Result:` and the result lines, with no markers or borders. Tested by `plain::tests::an_unfolded_shell_call_has_labels_command_and_result_without_chrome` and `transcript::tests::copy::y_copies_everything_shown_when_the_section_is_unfolded`.
  - [ ] A folded user prompt gives its two lines plus `…`. This is now right for prose. Tested by `a_folded_one_paragraph_prompt_is_what_the_fold_shows_as_one_line`, `a_folded_prompt_whose_first_paragraph_wraps_shows_only_what_the_fold_shows`, `y_on_a_folded_prompt_copies_the_rows_shown_at_the_transcripts_width` and `browser_tests::copy::y_copies_the_selected_section_as_displayed`. It is still wrong when the first row was split mid-token, because a space is inserted at the join (finding 1).
  - [x] A truncated output includes the `… N more lines` line. Tested by `plain::tests::a_truncated_output_keeps_its_more_lines_row` and `transcript::tests::copy::on_the_more_row_y_copies_the_tool_call_as_displayed_with_the_more_row`.
- [x] Full copy of a folded, truncated tool call gives the full input and the full untruncated result, including a lazy output over 4 KiB.
  - Implemented in `plain::full_text`, now with `verbatim: true`.
  - Tested by `full_content_of_a_folded_truncated_call_has_the_whole_output` and `full_content_of_a_lazy_output_over_4_kib_is_read_from_the_transcript`.
  - Verbatim behaviour is tested by `full_content_keeps_tabs_and_control_free_lines_exactly`, `full_content_does_not_break_a_very_long_line` and `full_content_of_a_write_keeps_the_file_as_written`.
- [x] `y`/`Y` return `Msg::CopyText(text)` from `Transcript::on`, and with the truncation row selected they copy the parent tool section. Tested by `transcript::tests::copy::*`, including both `on_the_more_row_*` tests.
- [x] The `App` with a fake clipboard puts the text on the clipboard and shows `Copied <n> lines`, and `Copied 1 line` is tested. See `browser_tests::copy::the_text_goes_to_the_clipboard_and_a_toast_counts_the_lines` and `one_line_is_singular`.
- [x] OSC52 is byte-exact, and the tmux form doubles `ESC` and uses `ESC P tmux; … ESC \`. Tested by `clipboard::tests::osc52_is_esc_52_c_base64_bel` and `under_tmux_osc52_is_wrapped_with_every_esc_doubled`.
- [x] The OSC52 decision follows the env rules, with the env injected. Tested by `clipboard::tests::osc52_is_emitted_when_tmux_is_set`, `osc52_is_emitted_on_linux_without_a_display_variable` and `osc52_is_not_emitted_with_a_display_and_no_tmux_while_arboard_works`.
- [x] A clipboard failure with no OSC52 shows `Copy failed: …`. Tested by `browser_tests::copy::a_clipboard_failure_without_osc52_shows_copy_failed`.
- [x] Every UI state has a snapshot: `copied_toast_80x24_text`, `copied_toast_120x40_text`, `copied_toast_120x40_styled` and `copy_failed_toast_80x24_text`. These are unchanged from round 1, and they pass with `INSTA_UPDATE=no`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran them: 1415 lib tests pass, all other targets pass, and the working tree is clean.

## Findings
1. [must-fix] `src/components/turn_block/plain.rs`, `soft_joined`: joining a row to the row before it always inserts `' '` (`last.push(' ')`). This is wrong when the screen split the line inside a token, which happens with char-wrapped long words, URLs, paths and code-block lines.
   - **Rule it breaks.** The planner note requires "logical lines (no soft-wrap breaks)". The copy adds a character that is in neither the source nor the screen.
   - **Reproduced** on a copy of HEAD, `displayed_text(..., Some(40))` on a folded prompt:
     - A prompt that is only `https://example.com/` followed by 90 `a`s copies as `"https://example.com/aaaaaaaaaaaa aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa…"`.
     - A prompt that is a fenced code block whose first line is `fn main() { let x = 1234567890…(70 digits); }` copies as `"  fn main() { let x = 1234567890 12345678901234567890123456789…"`.
   - **Change.** In `soft_joined`, `found` already computes `skipped`, the whitespace in the logical line between the end of the previous match and the start of this row. Return it from `found` along with the end, then push that logical whitespace slice instead of a fixed `' '`. When `skipped == 0`, push nothing.
   - **Tests.** Add unit tests in `plain::tests`:
     - A folded prompt that is a single URL longer than two rows, at block width 40. It should copy as an exact prefix of the URL with no space, ending in `…`.
     - A folded prompt whose first code-block line wraps. It should copy that line's shown prefix with no inserted space.
     - Extend `soft_joined_matches_rows_to_the_unwrapped_lines` with a mid-word case, for example rows `["alpha be", "ta gamma…"]`, which should give `["alpha beta gamma…"]`.
2. [nit] `soft_joined` keeps the wrapped rows of a quote apart. For example, `> quoted text that is long enough to wrap…` at width 40 copies as `"│ quoted text that is long\n│ enough to wrap around at fort…"`, so a soft-wrap break stays in the copy. This is documented as intended ("a quote bar, say"), and it changes line structure but not content. Change: if it is cheap while fixing finding 1, strip the quote-bar prefix from the continuation row's key before matching, and join on it.
3. [nit] `plain::displayed_text` copies a folded tool call's whole first description line. The screen cuts it with `…` at the block width (`ToolCallSection::description`), and thinking and system headers are now cut at that width. The acceptance criterion says "exactly its description line", so this choice is defensible. Change: either keep it and say so in the `displayed_text` doc comment (it already notes "always its whole description"), or cut it like the other folded headers. Do not change it without the planner.
4. [nit] `src/components/turn_block/renderers/mod.rs`, `verbatim_lines`: `str::lines()` also strips a `\r` before each `\n`. So `Y` on CRLF output is not byte-exact, although the doc comment says "no dropped characters". Change: split on `'\n'` and drop only a final empty piece, or reword the doc comment to say that CRLF becomes LF.
5. [nit] (process, not code) The scratchpad directory is shared across agents. `scratchpad/repo` belongs to another agent's checkout (branch `factory/024-transcript-search`), and an early probe of mine copied tracked files over it before I noticed. Its owner may want to refresh it. All later probing was done in `scratchpad/fr025-r2-feature`.
