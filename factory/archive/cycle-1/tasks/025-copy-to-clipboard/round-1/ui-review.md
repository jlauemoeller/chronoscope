VERDICT: FAIL

## Summary
The toast UI matches the `UI` section. All four required snapshots exist and are correct: `Copied 42 lines` at 80x24 text and 120x40 text and styled, and `Copy failed: no clipboard available` at 80x24 text. They use the 3-row rounded box in `toast` style, bottom-right, with the bottom border on row h−2, in the same colours as the task 012 toast snapshots. Scratch-crate renders confirm that `y` and `Y` copy the right text for tool calls, truncated output, markdown and multi-paragraph prompts. One copy-as-displayed case is wrong: `y` on a **folded single-paragraph user prompt** copies the whole paragraph, while the screen shows two rows ending in `…`. A repo test locks in this wrong behaviour.

## States checked
- Toast `Copied 42 lines` over the browser @ 80x24: snapshot `copy__copied_toast_80x24_text`. ok
- Toast `Copied 42 lines` @ 120x40: snapshots `copy__copied_toast_120x40_text` and `copy__copied_toast_120x40_styled`. ok. The toast cells are fg (250,250,250) on bg (18,18,18), the same as `toast__tests__*` from task 012, and the bottom border is on row 38.
- Toast `Copy failed: no clipboard available` @ 80x24: snapshot `copy__copy_failed_toast_80x24_text`. ok
- Toast `Copied 2 lines` @ 60x20 (smallest browser size): rendered in scratch crate. ok
- Toast `Copy failed: <long arboard-style reason>` @ 80x24, 60x20 and 120x40: rendered in scratch crate. ok. The box is capped at 60 columns (56 at 60x20) and the text is cut with `…`.
- `y` / `Y` on a folded multi-paragraph prompt @ 80x30: rendered in scratch crate. ok. `y` gives the two shown rows, ending in `…`. `Y` gives the full prompt.
- `y` on a folded single-paragraph prompt @ 80x30: rendered in scratch crate. **issue** (finding 1).
- `y` / `Y` on a folded Shell call (500-line output): rendered in scratch crate. ok. `y` gives only the description. `Y` gives description, `Command:`, command, `Result:` and all 500 lines (`Copied 508 lines`).
- `y` on the unfolded, truncated Shell call: rendered in scratch crate. ok. It includes labels, blank rows and the `… 300 more lines` row, without markers, borders or the body indent. The content's own leading spaces are kept.
- `y` on unfolded markdown (heading, table, rule, quote, code block, ordered and nested list): rendered in scratch crate. ok, apart from the rule length (finding 3).
- `y` while the search prompt is open: rendered in scratch crate. ok. It types `y` into the prompt and copies nothing.
- `y` while the selector is open: rendered in scratch crate. ok. Nothing is copied.

## Findings
1. [must-fix] Browser, folded user prompt that is one long paragraph (any width; seen at 80x30 and in the fixture at 80x24). `y` copies the whole paragraph. The fold shows only the first two wrapped rows, the second ending in `…`.
   - This breaks the acceptance criterion "a folded user prompt gives its two lines plus `…`" and the planner note "A folded section copies exactly its summary line(s)". It also breaks SPEC §6.4 ("folded: summary line").
   - Here `y` gives the same text as `Y`.
   - `plain::displayed_text` renders at `PLAIN_WIDTH = u16::MAX`, so the fold's two-row cut and `…` never happen for a single paragraph. The same over-copy happens when a multi-paragraph prompt's first paragraph wraps.
   - The key test `app::browser_tests::copy::y_copies_the_selected_section_as_displayed` asserts the wrong behaviour: it expects the full `LONG_PROMPT` and `Copied 1 line`. The `copy__copied_toast_80x24_text` snapshot shows that same prompt cut at `… but I don't have a way to…`.

   Change: for a folded user prompt, build the copy from the summary rows the screen draws, at the transcript's current content width.
   - Use the same `render_markdown`, skip blank rows, keep 2 rows and add `…` as in `UserPromptSection::lines`.
   - Join rows that are soft-wrap continuations of the same logical line with a single space. Keep real line breaks.
   - Drop the `▶ ` lead.
   - Pass the width from the `Transcript`, which already knows it, into `displayed_text` for this case.
   - Expected for the fixture at 80x24: one line, `I have a Logitech BRIO web camera attached to this machine via USB. It works fine - both camera and microphone - but I don't have a way to…`, with toast `Copied 1 line`.
   - Update the key test's assertion to match.
   - Add a `plain` unit test with a single long paragraph that checks the copy ends in `…` and stops where the fold does.

   See `ui/folded-long-prompt-80x30.txt`.
2. [nit] `App::copy_text`, `Copy failed: <reason>`. The toast caps at 60 columns, so a long `arboard` error is cut, for example `Copy failed: Unknown error while interacting with the c…`, and the full reason appears nowhere. Change: `log::warn!` the full reason when the copy fails.
3. [nit] `y` / `Y` on a section with a horizontal rule. The plain text has an 80-cell rule (`RULE_CELLS`), while the screen draws 72 cells at 80 columns. The spec doesn't pin this, so it is cosmetic. Keeping it fixed at 80 is fine. If it should match the screen, use the content width.
