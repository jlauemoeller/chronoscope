VERDICT: FAIL

## Summary
The clipboard side is well done. It covers the `Clipboard` trait and its fakes, the injected `ClipboardEnv`, byte-exact OSC52 with the tmux DCS wrap, the lazy `arboard` kept for the app's life, the toasts and the snapshots. The `parts()` refactor of the tool renderer is clean. The plain-text functions still have two problems:
- `y` on a folded user prompt does not copy what is displayed.
- `Y` does not copy the full result verbatim: tabs, control characters and very long lines are changed by `char_wrap`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (lib 1401 passed, all other targets pass; no `.snap.new` files)

## Findings
1. [must-fix] `src/components/turn_block/plain.rs:53-71` (`displayed_text`) and `src/components/turn_block/renderers/user.rs:19-32`: a folded user prompt is not copied as displayed.
   - **Problem.** Rendering at `PLAIN_WIDTH = u16::MAX` turns a one-paragraph prompt into one row. So `y` copies the whole paragraph, with no `…`. This is the most common prompt shape. Prompts made of single-newline lines join into one markdown paragraph and behave the same way.
   - **Rule it breaks.** The binding planner note says a folded section copies "exactly its summary line(s)… For a folded user prompt that is the first two lines plus `…`". The screen shows two rows ending in `…`. The accepted 80x24 snapshot shows `…but I don't have a way to…`.
   - **Tests that miss it.** `browser_tests/copy.rs::y_copies_the_selected_section_as_displayed` asserts the opposite: `copied[0] == LONG_PROMPT`. The unit test `a_folded_user_prompt_is_two_lines_and_an_ellipsis` passes only because its prompt is made of several paragraphs.
   - **Change.**
     - Give `plain::displayed_text` the transcript's real content width (the width the section is laid out at on screen).
     - Have `Transcript::displayed_text` pass it in.
     - For a folded prompt, render at that width and copy exactly the two non-blank rows shown, the second ending in `…`. Drop the lead and trailing spaces as now.
     - Add a unit test with one long paragraph: it should give two rows plus `…`, not the whole paragraph.
     - Change `y_copies_the_selected_section_as_displayed` to expect the two shown lines.
     - Unfolded sections may stay at the unbounded width, so their logical lines are not broken.
2. [must-fix] `src/components/turn_block/plain.rs:78-89` (`full_text`) and `src/components/turn_block/renderers/tool/mod.rs` (`plain_lines`, which uses `result_block` → `text_block` → `char_wrap`): `Y` changes the full result.
   - **Problem.** `char_wrap` (`src/markdown/wrap.rs`) does three things to the text:
     - It expands every `\t` to four spaces. The existing test `char_wrap_applies_the_style_and_expands_tabs` shows this.
     - It drops control characters.
     - It breaks any line longer than about 65,530 cells (`PLAIN_WIDTH - BODY_INDENT`) into several lines.

     So a `Y` copy of a Shell or Read result containing Go code, a Makefile, TSV output or a long minified JSON line is not the full result. The same goes for the command and for Write content, which goes through `wrap_styled_lines`.
   - **Rule it breaks.** The planner note says the full result is taken "via `ToolOutput::text()`". Task 026 reuses `full_text` for save and export, where exact content matters even more.
   - **Change.**
     - In full mode, build the text blocks from the raw strings, split only on `\n`, with no tab expansion and no width wrapping: the result from `ToolOutput::text()`, minus its trailing newline, and the command/input string. For example, add a "plain" flag to `SectionCtx`, or a non-wrapping path in `text_block`/`result_block`, that `plain_lines` uses when building full text.
     - Stripping ANSI escape sequences is acceptable. `\t` must be kept, and lines must not be broken.
     - Add unit tests: a Shell result with `a\tb` copies with the tab intact under `Y`, and a result line longer than 70,000 characters stays one line.
3. [nit] `src/components/turn_block/renderers/thinking.rs` / `system.rs` (folded headers): at `u16::MAX` the folded `Thinking: …` and system headers copy their whole first line, not the ellipsized row on screen. If the width from finding 1 is applied to every folded summary, this becomes exact too.
4. [nit] `src/app/copy.rs:29-37`: there is no size limit on the OSC52 payload. Very large `Y` copies (a 100 MB lazy output) produce a sequence that tmux and many terminals silently drop, while the toast still says `Copied N lines`. Either cap the OSC52 payload and log or toast when the cap is exceeded, or document the limit in `doc/clipboard.md`. The build notes mention it.
5. [nit] `src/components/transcript/copy.rs:28-33`: `full_text` reads a lazy output from disk synchronously inside `Transcript::on`, on the UI thread. That is fine for typical sizes. For very large outputs, consider noting it for the §9.2 budgets.
6. [nit] `src/app/copy.rs:47-51` (`copied_text`): empty text reports `Copied 1 line` because of `.max(1)`. Consider `Copied 0 lines`, or not copying empty text at all.
7. [nit] `src/app/mod.rs:13`: `pub mod copy;` only adds `impl App` blocks, so `mod copy;` is enough. The same applies to `pub const PLAIN_WIDTH` in `plain.rs:43`, which is only used inside that module.
8. [nit] `src/components/transcript/tests/keys.rs:861-866`: `/` was dropped from the key list along with `y`. It still belonged in that test, which checks that a key does not change the view. Consider keeping `/` there.
