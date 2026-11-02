STATUS: DONE

## Changes
- `src/components/turn_block/plain.rs` (new): pure `displayed_text` and `full_text` (turn + section id + fold state + settings -> `Option<String>`), reusable by task 026. Sections are rendered by the screen's own renderers at a width no line reaches (`u16::MAX`), so only logical line breaks remain. The chrome is dropped: the 2-column lead (first span of every non-blank row), the tool/system body indent, borders and markers. The content's own indentation, bullets, `│ ` quotes, labels and blank rows stay. Horizontal rules are cut to 80 cells. The truncation row's id stands for its tool call.
- `renderers/tool/mod.rs`: `ToolCallSection::render` is refactored onto `parts()`, shared with the new `plain_lines()`, so the two cannot drift. The `… N more lines` text is `more_text()`. `renderers/system.rs`: `SystemSection::plain_lines`.
- `Full content` uses `folded: false, expanded: true, truncate: usize::MAX`, with the full description and no subagent hint. Results come from `ToolOutput::text()`.
- `components/transcript/copy.rs` (new): `Transcript::displayed_text` / `full_text`. `Transcript::on` answers `Msg::CopyText` for the `copy` / `copy_full` actions (resolved through the keymap); nothing selected gives `None`.
- `src/clipboard.rs` (new): `Clipboard` trait returning a `CopyOutcome { system, osc52 }`; `ClipboardEnv` (injected `tmux`, `display`, `linux`) with `wants_osc52`; byte-exact `osc52()` (BEL-terminated; tmux DCS wrap with every ESC doubled); `Arboard` (one lazily created `arboard::Clipboard` kept for the app's life, failure remembered); `SystemClipboard<S: TextSink>`; `NoClipboard`. Added the `base64 = "0.22"` dependency (already in the lockfile).
- `src/app/copy.rs` (new), `app/mod.rs`: `Msg::CopyText(String)`; `App` owns `clipboard` and an optional `osc52_out` writer, set with `set_clipboard`. The app starts with `NoClipboard` and no writer, so tests never touch the real clipboard or stdout. The OSC52 sequence is written and flushed in `update`, between frames. Toast `Copied <n> line(s)`, or `Copy failed: <reason>` when neither the system clipboard nor OSC52 took the text, or the OSC52 write failed while the system clipboard had failed too. `runtime.rs` installs `SystemClipboard::new()` and stdout in production.
- `doc/clipboard.md` (new): tmux settings (`set-clipboard on`, `allow-passthrough on`, `escape-time`).
- Tests: `turn_block/plain.rs` (unit), `transcript/tests/copy.rs` (component), `clipboard.rs` (OSC52 and env rules), `app/browser_tests/copy.rs` (update tests and snapshots). Removed `y` from the "does nothing" key test.

## Self-check
- AC: folded tool call is exactly its description — `plain::tests::a_folded_tool_call_is_exactly_its_description`
- AC: unfolded Shell call, no markers/borders — `plain::tests::an_unfolded_shell_call_has_labels_command_and_result_without_chrome`
- AC: folded user prompt gives two lines plus `…` — `plain::tests::a_folded_user_prompt_is_two_lines_and_an_ellipsis`
- AC: truncated output includes `… N more lines` — `plain::tests::a_truncated_output_keeps_its_more_lines_row`
- AC: full copy of folded, truncated tool call incl. lazy output over 4 KiB — `plain::tests::full_content_of_a_folded_truncated_call_has_the_whole_output`, `…full_content_of_a_lazy_output_over_4_kib_is_read_from_the_transcript`
- AC: `y`/`Y` return `Msg::CopyText`; truncation row copies the parent — `transcript::tests::copy::*` (`y_copies_…`, `capital_y_…`, `on_the_more_row_*`)
- AC: App with fake clipboard shows `Copied <n> lines`, singular tested — `browser_tests::copy::the_text_goes_to_the_clipboard_and_a_toast_counts_the_lines`, `one_line_is_singular`, key paths `y_copies_the_selected_section_as_displayed`, `capital_y_copies_the_full_section`
- AC: OSC52 byte-exact, tmux wrap — `clipboard::tests::osc52_is_esc_52_c_base64_bel`, `osc52_base64_has_padding_and_encodes_utf8`, `under_tmux_osc52_is_wrapped_with_every_esc_doubled`, `browser_tests::copy::the_system_clipboard_under_tmux_writes_the_wrapped_sequence_to_the_terminal`
- AC: OSC52 decision from injected env — `clipboard::tests::osc52_is_emitted_when_tmux_is_set`, `…on_linux_without_a_display_variable`, `…is_not_emitted_with_a_display_and_no_tmux_while_arboard_works`, `…when_arboard_failed`
- AC: failure with no OSC52 shows `Copy failed: …` — `browser_tests::copy::a_clipboard_failure_without_osc52_shows_copy_failed`, `the_app_starts_with_a_clipboard_that_fails`
- AC: `Ctrl-c` never copy, still quits — `transcript::tests::copy::ctrl_c_is_never_a_copy_key`, `browser_tests::copy::ctrl_c_is_not_copy_it_still_quits`
- UI: toast `Copied 42 lines` @ 80x24 text — `browser_tests::copy::copied_toast_80x24_text`
- UI: same @ 120x40 text and styled — `copied_toast_120x40_text`, `copied_toast_120x40_styled`
- UI: `Copy failed: no clipboard available` @ 80x24 text — `copy_failed_toast_80x24_text`

## Judgment calls and notes
- "As displayed" is computed at an unbounded width, so soft-wrap breaks never appear. A consequence: markdown soft line breaks join into one line (as on screen), and a folded prompt that is one long paragraph copies that whole paragraph (no `…`), whereas the screen cuts it at two wrapped rows. A folded prompt of several paragraphs copies its first two with `…` on the second.
- The env rule makes OSC52 always accompany an arboard failure, so `Copy failed` arises from `SystemClipboard` only when there is no terminal writer or the write fails. The tests and the snapshot reach it through the fake clipboard / default `NoClipboard`.
- OSC52 is written in `update` (between frames, flushed) rather than inside `run_loop` after `draw`; this is equivalent for ordering and needs no `run_loop` signature change.
- herdr's OSC52 behaviour is NOT verified (cannot be from here); `doc/clipboard.md` says so.
- Very large OSC52 payloads may exceed terminal or tmux limits; not handled.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1401 passed lib, 0 failed; all other targets pass)
