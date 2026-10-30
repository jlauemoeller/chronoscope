VERDICT: FAIL

## Summary
The `TurnBlock` widget matches the planner-notes geometry. That covers the titles, the column 2/4/7 layout, the `W − 5` wrap, the blank-row rhythm, the exact height, clipping, the `▒` coverage and both `dim_unselected` modes. All 10 UI states have text snapshots, plus styled snapshots where colours matter. The gate passes (`INSTA_UPDATE=no cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`).

Two renderers break the markdown and wrapping rules of §5.5 in common, real cases:
- Folded thinking shows raw markdown source.
- Unfolded tool descriptions are char-wrapped mid-word at width 80.

Neither case is covered by the fixtures.

## States checked
- 1. User prompt folded @ 80 / 120. Snapshots `user_prompt_folded_at_80`, `user_prompt_folded_at_120`: ok. Two lines, the second ends in `…`, text at col 4, `▶` at col 2, wrap at col 75 / 115.
- 2. User prompt unfolded @ 80 (text). Snapshot `user_prompt_unfolded_at_80`: ok.
- 2. User prompt unfolded and selected @ 80 (styled). Snapshot `user_prompt_unfolded_and_selected_at_80_styled`: ok. `▒` in `selection_marker` (primary) on exactly the 8 prompt rows, text bold `foreground`, title `user_title`, timestamp `timestamp`, border `border`.
- 3. Injected item before the prompt @ 80. Snapshot `injected_item_before_the_prompt_at_80`: ok (`▶ file: …` precedes the prompt).
- 4. Full Assistant turn @ 120 (text + styled, dim on). Snapshots `assistant_turn_at_120_text`, `assistant_turn_at_120_selected_tool_dimmed_styled`: ok.
  - Title is ` Assistant ── claude-opus-5.5 @ high ` with the `──` joiner in `border`.
  - `▒` runs from `▼ description` through the last result row.
  - Labels at col 4 and content at col 7, body wrap at col 115.
  - Unselected text, thinking, model note and footer are all `muted_foreground`.
  - Selected `label`/`output` keep their token colour and are bold.
- 5. State 4 with dim off @ 120 (styled). Snapshot `assistant_turn_at_120_selected_tool_not_dimmed_styled`: ok. Text and folded tool are `foreground` and not bold, thinking is `thinking`, model note and footer are muted.
- 6. Unfolded thinking and unfolded 3-entry system section @ 80. Snapshots `assistant_turn_with_unfolded_thinking_at_80`, `assistant_turn_with_unfolded_system_section_at_80`: ok.
- 7. Error tool unfolded and pending tool unfolded @ 80 (styled). Snapshots `error_tool_call_unfolded_at_80_styled`, `pending_tool_call_unfolded_at_80_styled`: ok. Result in `error_output` (destructive); `(pending)` in `muted_foreground` at col 7.
- 8. Compaction folded and unfolded @ 80. Snapshots `compaction_folded_at_80`, `compaction_unfolded_at_80`: ok. Also rendered styled in the scratch crate: title in `system_title`, divider and summary muted, selected = muted + bold with `▒`.
- 9. Redacted-only Assistant turn @ 80. Snapshot `redacted_thinking_only_is_an_empty_block_at_80`: ok (3 rows).
- 10. Assistant title without effort @ 80. Snapshot `assistant_title_without_effort_at_80`: ok.
- Timestamps carry a `Z` suffix because the snapshots use `utc = true` (§8.6, binding snapshot convention): ok.
- Rendered in the scratch crate (`/tmp/factory-ui-017-turn-block-sections`):
  - Full turn at widths 60, 40, 30, 20, 14, 12, 10, 9, 8, 6, 4, 3 and 2: ok down to 8; issues below 8 (nit 7) and with word splitting (finding 2, nits 4 and 6).
  - Clipped rendering with skip 0/1/13/20, and rendering into an offset `Rect` inside a larger buffer: ok.
  - Selection and dimming at 60 for selected thinking (`▒` also on the blank row inside the section), selected system section (dim off), selected footer, and selected assistant text with heading, inline code and a fenced code block. Syntax colours are muted when dimmed and kept when not: ok.
  - Folded thinking whose first line is markdown: issue (finding 1).
  - Unfolded tool with a long description @ 80 / 120: issue at 80 (finding 2).
  - User prompt edge cases @ 80:
    - two paragraphs, heading first, code block first: nit 3;
    - hard breaks, exactly 2 lines (no `…`), 200-char unbroken token, CJK: ok.
  - `show_system = false` on a turn with only an injected item: ok (3-row block).
  - Tool edge cases @ 60:
    - empty description, empty output (`(no output)`), CJK in input and output (no overflow past the body width): ok;
    - tabs and ANSI escapes in output: nit 5. ESC is stripped and never reaches a cell.
  - Long model id @ 120 / 80 / 60 / 40, and User title at 30..6: nit 4.
  - All 7 built-in themes @ 80 (styled): ok. Colours all come from theme tokens and roles; the only hard-coded style is the `BOLD` modifier the spec requires.

## Findings
1. [must-fix] Assistant turn, folded readable thinking, any width (seen at 80). The folded line takes `first_line()` of the raw thinking **source**, so markdown syntax shows literally: `▶ Thinking: **Investigating USB descriptors**`, `▶ Thinking: - first bullet`. §5.5 requires thinking to be rendered as markdown. The unfolded form, and the folded User prompt via `render_markdown`, already do this. Claude's readable thinking usually opens with a `**Title**` line, so this hits the default folded state of most thinking sections. The fixtures use plain text, so no test catches it.
   Change: in `ThinkingSection::lines` (`sections/thinking.rs`), build the folded line from the first non-blank line of `render_markdown(text, ctx.width, thinking_style, theme)`. Prefix it with `Thinking: `, ellipsize the result to one row, and keep it all in `thinking`. It should read `▶ Thinking: Investigating USB descriptors` (and `▶ Thinking: • first bullet` or `first bullet`). Add a fixture or test with a `**bold**` first line.
   See `ui/thinking-folded-markdown-80.txt`.
2. [must-fix] Assistant turn, unfolded tool call, width 80 (a listed size), also at 60 and 40. The `▼ <description>` row is char-wrapped (`char_wrap` in `ToolCallSection::description`). A ~100-char description splits mid-word at 80: `…list every exten` / `sion unit with its controls`. At 40, `…descriptors for th` / `e BRIO`. When the break falls on a space, the continuation line starts with that space, so its text begins at col 5 instead of col 4. A description is prose. §5.5 says "Prose is word-wrapped", and the planner notes char-wrap only the `Input:` JSON and the result text.
   Change: word-wrap the unfolded description at the text width `W − 8`, using the same prose wrapper as the markdown renderer or `textwrap`. Continuation lines start at col 4 with no leading space, and char-wrapping is a fallback only for single tokens longer than the width. Add a test at width 80 with a description longer than 72 cells.
   See `ui/tool-description-unfolded-80.txt` and `ui/narrow-wrapping-40.txt`.
3. [nit] User turn, folded prompt, 80. When the rendered prompt's second line is the blank separator after a paragraph or heading, the folded form is the first line plus a lone `…` row (`▶ Para one.` / `  …`, `▶ Bug report` / `  …`). Multi-paragraph prompts are common, so the fold shows only one line of content. This follows the planner wording literally ("first 2 wrapped lines"), hence a nit. Suggest skipping blank rendered lines when picking the two folded lines.
   See `ui/user-prompt-folded-paragraphs-80.txt`.
4. [nit] Assistant title, narrow widths or long model ids. When both titles do not fit, the timestamp is dropped before the left title is truncated. With the fixture model it disappears below about 64 columns (e.g. at 60). With a ~50-char model id it disappears at 80. The spec is silent; suggest truncating the model text with `…` first and dropping the timestamp only when ` Assistant ` alone no longer fits. Also, a truncated left title runs straight into the border (`@ hi…──╮`) without its trailing space.
   See `ui/title-long-model-80.txt`.
5. [nit] Tool call, folded vs unfolded description with a tab. Folded replaces the tab with one space (`Run things`), while unfolded expands it to a tab stop (`Run    things`). ANSI SGR residue (`[31m`) stays visible in result text. ESC itself is stripped, so this is not a security issue; worth handling in task 019's output bodies. An empty description renders as a bare `▼ ` / `▶ ` row; consider a fallback such as the tool name.
6. [nit] Stats footer and `model → …` note, narrow widths (40 and below). Both are char-wrapped and split mid-word (`45.2k to` / `kens`, `claude-` / `sonnet-5.5`). The footer is specified as "one muted line"; consider word-wrapping both, or ellipsizing the footer to one row.
7. [nit] Any block, widths below 8. At W = 4 or less, the top border loses its `╮` corner (`╭─…─`), because `TitleBar::line` emits more cells than the width. At W ≤ 6, `text_width().max(1)` puts text in the right padding, against the border (`│   I│`). The tests cover 8..=130 and the Transcript will likely gate tiny sizes with `EmptyState`, but `TitleBar::line` should never exceed `width` and should always end in `╮` when `width >= 2`.
   See `ui/tiny-widths-6-4.txt`.
