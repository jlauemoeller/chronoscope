VERDICT: FAIL

## Summary
The clipboard side is correct and well tested: the `Clipboard` trait, lazy `arboard`, the OSC52 env rules, byte-exact OSC52 with the tmux DCS wrap, the toasts and the snapshots. `y`/`Y` go through the keymap, and the truncation row copies its parent. The test gate passes (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `INSTA_UPDATE=no cargo test`, with 1401 lib tests passing). One acceptance criterion is not met. A folded user prompt is copied at unbounded width, so for the common prompt shape `y` copies far more than the screen shows. The end-to-end test asserts this wrong behaviour.

## Acceptance criteria
- [ ] Copy-as-displayed (unit tests on the `Transcript` text functions):
  - [x] A folded tool call gives exactly its description line. Covered by `plain::tests::a_folded_tool_call_is_exactly_its_description`.
  - [x] An unfolded Shell call gives the description, `Command:`, the command, `Result:` and the result lines, without markers or borders. Covered by `plain::tests::an_unfolded_shell_call_has_labels_command_and_result_without_chrome` and `transcript::tests::copy::y_copies_everything_shown_when_the_section_is_unfolded`.
  - [ ] A folded user prompt gives its two lines plus `…`. This holds only when every paragraph fits on one screen row. For a one-paragraph prompt (the fixture `LONG_PROMPT`), the screen shows two rows ending in `…`, but `y` copies the whole paragraph with no `…`. `browser_tests::copy::y_copies_the_selected_section_as_displayed` asserts exactly that (`Copied 1 line`). See finding 1.
  - [x] A truncated output includes the `… N more lines` line. Covered by `plain::tests::a_truncated_output_keeps_its_more_lines_row` and `transcript::tests::copy::on_the_more_row_y_copies_the_tool_call_as_displayed_with_the_more_row`.
- [x] Full copy of a folded, truncated tool call gives the full input and the untruncated result, including a lazy output over 4 KiB. Implemented in `plain::full_text` (`folded: false`, `expanded: true`, `truncate: usize::MAX`). Covered by `plain::tests::full_content_of_a_folded_truncated_call_has_the_whole_output` and `full_content_of_a_lazy_output_over_4_kib_is_read_from_the_transcript`.
- [x] `y`/`Y` return `Msg::CopyText(text)` from `Transcript::on`, and with the truncation row selected they copy the parent tool section. Implemented in `transcript/mod.rs` (`Action::Copy` / `Action::CopyFull`) and `plain::find` (`more_parent`). Covered by `transcript::tests::copy::*`, including both `on_the_more_row_*` tests and the remap test.
- [x] The `App` with a fake clipboard puts the text on it and shows `Copied <n> lines`, and the singular form is tested. Implemented in `app/copy.rs`. Covered by `browser_tests::copy::the_text_goes_to_the_clipboard_and_a_toast_counts_the_lines` and `one_line_is_singular`.
- [x] OSC52 encoding is byte-exact, and the tmux form doubles `ESC` and uses `ESC P tmux; … ESC \`. Implemented in `clipboard::osc52`. Covered by `clipboard::tests::osc52_is_esc_52_c_base64_bel`, `osc52_base64_has_padding_and_encodes_utf8` and `under_tmux_osc52_is_wrapped_with_every_esc_doubled`.
- [x] The OSC52 decision follows the env rules. Implemented in `ClipboardEnv::wants_osc52`, with the env injected. Covered by `clipboard::tests::osc52_is_emitted_when_tmux_is_set`, `osc52_is_emitted_on_linux_without_a_display_variable` and `osc52_is_not_emitted_with_a_display_and_no_tmux_while_arboard_works`.
- [x] A clipboard failure with no OSC52 shows `Copy failed: …`. Covered by `browser_tests::copy::a_clipboard_failure_without_osc52_shows_copy_failed`.
- [x] Every UI state has a snapshot: `copied_toast_80x24_text`, `copied_toast_120x40_text`, `copied_toast_120x40_styled` and `copy_failed_toast_80x24_text`. The toast is a 3-row rounded box at the bottom right, with its bottom border on row h − 2.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass (re-run by the reviewer).

## Findings
1. [must-fix] **A folded user prompt is not copied as displayed.**
   - **Where:** `plain::displayed_text` (`src/components/turn_block/plain.rs`) renders every section at `PLAIN_WIDTH = u16::MAX`. `UserPromptSection`'s fold cut (`FOLDED_LINES = 2` non-blank rows plus `…`) therefore runs on paragraphs, not on screen rows.
   - **Result:**
     - A one-paragraph prompt: the screen shows two rows ending in `…` (see `copied_toast_80x24_text.snap`, rows 8–9). `y` copies the entire paragraph with no `…`, the same text as `Y`.
     - A multi-paragraph prompt whose first paragraph wraps: the screen shows two rows of paragraph 1 plus `…`. `y` copies all of paragraph 1, then paragraph 2, then `…`.
   - **Requirements broken:**
     - SPEC §6.4: "as displayed (folded: summary line)".
     - Planner note: "a folded section copies exactly its summary line(s) … for a folded user prompt that is the first two lines plus `…`".
     - The acceptance criterion.
   - **Why the tests miss it:** the unit test uses only short paragraphs. `browser_tests::copy::y_copies_the_selected_section_as_displayed` locks in the wrong result.
   - **Change:**
     - Give `plain::displayed_text` the transcript's current text width as an extra `width: u16` parameter. It stays pure and reusable for task 026.
     - For a folded `SectionContent::Prompt`, render with `ctx.width = width` and return the rows `UserPromptSection` keeps, without the lead, newline-joined, with the trailing `…` exactly as drawn.
     - Keep `PLAIN_WIDTH` for every other section and for the unfolded prompt.
     - In `Transcript::displayed_text` (`src/components/transcript/copy.rs`), pass `text_width(self.cache.width())` (the same text width the blocks are laid out at).
     - Add a unit test in `plain.rs`: a single long paragraph (e.g. `LONG_PROMPT`) at a fixed width, e.g. 72. It should give exactly the two screen rows, the second ending in `…`.
     - Add a second unit test: a multi-paragraph prompt whose first paragraph wraps.
     - Update `browser_tests::copy::y_copies_the_selected_section_as_displayed` to expect the two rows shown in the 80x24 snapshot ("I have a Logitech BRIO web camera attached to this machine via USB. It" / "works fine - both camera and microphone - but I don't have a way to…") and `Copied 2 lines`.
2. [nit] The OSC52 sequence is written in `App::update` (`src/app/copy.rs`), not "after the frame" through the terminal adapter as the planner note words it. It still lands between frames and is flushed, so it behaves the same. Change: optionally add a sentence to the `copy_text` doc comment noting the deviation, or move the write after `draw` in `run_loop`.
3. [nit] `src/components/transcript/tests/keys.rs`: `keys_of_other_contexts_do_nothing` no longer presses an action of a later task. Change: add the `save` key (`s`) to the loop, so the "other tasks do nothing yet" coverage stays until task 026.
4. [nit] `app/copy.rs::copied_text` reports `Copied 1 line` for an empty string (`.max(1)`). The transcript never sends empty text today, so this is cosmetic. Change: optionally make it `Copied 0 lines`, or leave it with a comment.
5. [nit] OSC52 payloads are not size-capped. Large `Y` copies (e.g. a 1 MB tool output) may exceed terminal or tmux limits and be dropped silently. Change: optionally document the limit in `doc/clipboard.md`.
