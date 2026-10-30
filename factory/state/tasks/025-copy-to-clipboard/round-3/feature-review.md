VERDICT: PASS

## Summary
Round 3 does what the approver's rework note asks, and nothing beyond it. `soft_joined` now joins a wrapped row using the whitespace the source has at that point, which is nothing when the screen broke inside a word. Wrapped quote rows now join under a single `│ ` bar. The redundant filter is gone, and the doc comments are updated. Every acceptance criterion is met and covered by a test, and the test gate passes. Both nits are cosmetic.

## Acceptance criteria
- [x] Copy-as-displayed (unit tests on the `Transcript` text functions):
  - [x] A folded tool call gives exactly its description line. Tested by `plain::tests::a_folded_tool_call_is_exactly_its_description` and `a_folded_tool_call_stays_its_whole_description_at_any_width`.
  - [x] An unfolded Shell call gives the description, `Command:`, the command, `Result:` and the result lines, with no markers or borders. Tested by `plain::tests::an_unfolded_shell_call_has_labels_command_and_result_without_chrome` and `transcript::tests::copy::y_copies_everything_shown_when_the_section_is_unfolded`.
  - [x] A folded user prompt gives its two lines plus `…`. The round-2 bug that inserted a space at mid-token breaks is fixed.
    - The existing tests still pass unchanged, including `browser_tests::copy::y_copies_the_selected_section_as_displayed`.
    - New tests: `a_folded_single_token_prompt_is_copied_without_inserted_spaces`, `a_folded_cjk_prompt_is_copied_without_inserted_spaces`, `a_folded_quoted_prompt_is_one_line_with_one_bar`, `a_folded_code_block_line_wider_than_the_row_has_no_inserted_space`, `soft_joined_joins_with_the_sources_own_whitespace` and `soft_joined_joins_the_rows_of_a_quote_under_one_bar` (which includes a nested quote).
    - The unmatched case, where `["│ alpha", "│ beta"]` stays apart, is kept, as the approver required.
    - The single-token and quote tests would fail against the round-2 code, which inserted `' '` and left the quote rows apart.
  - [x] A truncated output includes the `… N more lines` line. Tested by `plain::tests::a_truncated_output_keeps_its_more_lines_row` and `transcript::tests::copy::on_the_more_row_y_copies_the_tool_call_as_displayed_with_the_more_row`.
- [x] Full copy of a folded, truncated tool call gives the full input and the full untruncated result, including a lazy output over 4 KiB. This is unchanged since round 2. Tested by `full_content_of_a_folded_truncated_call_has_the_whole_output` and `full_content_of_a_lazy_output_over_4_kib_is_read_from_the_transcript`.
- [x] `y`/`Y` return `Msg::CopyText(text)` from `Transcript::on`, and with the truncation row selected they copy the parent tool section. Tested by `transcript::tests::copy::*`, including both `on_the_more_row_*` tests.
- [x] The `App` with a fake clipboard puts the text on the clipboard and shows `Copied <n> lines`. The singular `Copied 1 line` is tested. See `browser_tests::copy::the_text_goes_to_the_clipboard_and_a_toast_counts_the_lines` and `one_line_is_singular`.
- [x] OSC52 is byte-exact, and the tmux form doubles `ESC` and uses `ESC P tmux; … ESC \`. Tested by `clipboard::tests::osc52_is_esc_52_c_base64_bel` and `under_tmux_osc52_is_wrapped_with_every_esc_doubled`.
- [x] The OSC52 decision follows the env rules, with the env injected. Tested by `clipboard::tests::osc52_is_emitted_when_tmux_is_set`, `osc52_is_emitted_on_linux_without_a_display_variable` and `osc52_is_not_emitted_with_a_display_and_no_tmux_while_arboard_works`.
- [x] A clipboard failure with no OSC52 shows `Copy failed: …`. Tested by `browser_tests::copy::a_clipboard_failure_without_osc52_shows_copy_failed`.
- [x] Every UI state has a snapshot: `copied_toast_80x24_text`, `copied_toast_120x40_text`, `copied_toast_120x40_styled` and `copy_failed_toast_80x24_text`. They are unchanged, pass with `INSTA_UPDATE=no`, and leave no `.snap.new` files.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran all three: 1421 lib tests pass, every other target passes, and the working tree is clean afterwards.

## Approver rework note
1. **Source whitespace.** Done. `locate` returns the gap `&rest[..rest.len() - text.len()]`, and the join pushes that gap and then the trimmed row body.
2. **Quote join.** Done. `quote_prefix` finds the logical line's run of `│ ` bars, strips it from the continuation row, and appends the stripped body. A row that matches nothing still stays a line of its own.
3. **Tidy-ups.** Done.
   - The filter is dropped.
   - The `soft_joined` and `displayed_text` doc comments now say the join uses the source's whitespace.
   - The `verbatim_lines` doc comment now says a `\r\n` ending becomes a plain one, and the split itself is unchanged.
4. **Tests.** Done. The mid-word, spaced and quote cases are in two new tests rather than in `soft_joined_matches_rows_to_the_unwrapped_lines`; the coverage is the same. All the folded-prompt `displayed_text` cases are present, including the optional code-block line.

I also probed, on a scratch copy of HEAD (since removed), folded prompts at widths 40 and 80. All of these copied as one line that is an exact prefix of the rendered logical line, with the cut `…` and no inserted or doubled whitespace:
- a quote whose single token breaks mid-word;
- a nested quote;
- a list item;
- a line with a tab;
- a double space in the source;
- a two-line quote;
- CRLF input;
- inline bold and code.

## Findings
1. [nit] `src/components/turn_block/plain.rs`, lines 37–42 (the `displayed_text` doc comment): after the edit, the paragraph breaks oddly at "source has there); without" / "it, summaries are cut …". Change: rewrap the paragraph so each line fills to about 80 columns.
2. [nit] Approver step 4 asked for the mid-word, spaced and quote cases to go inside `soft_joined_matches_rows_to_the_unwrapped_lines`. They are in `soft_joined_joins_with_the_sources_own_whitespace` and `soft_joined_joins_the_rows_of_a_quote_under_one_bar` instead. The coverage is the same, so no change is needed. This note only records the difference from the rework note.
