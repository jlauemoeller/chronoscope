VERDICT: PASS

## Summary
Both round-1 must-fixes are resolved.
- Folded thinking now shows the rendered first line (`▶ Thinking: Investigating USB descriptors`), not markdown source.
- Unfolded tool descriptions are word-wrapped at the text width. Continuation lines start at column 4.

All 10 UI states still have their text snapshots, plus styled snapshots where the task specifies colours. The three changed `assistant_turn_at_120_*` snapshots match the new folded-thinking behaviour, and there are no pending `.snap.new` files. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass. `INSTA_UPDATE=no cargo test` passes with 1017 passed and 0 failed. In one of four full runs, an unrelated `index_stub` test from task 010 failed while a concurrent build was running (nit 3).

Round-1 findings:
- Resolved: must-fix 1 and 2; nits 3, 5 (tab mismatch and blank-description fallback; ANSI residue is deferred to task 019 as suggested), 6 and 7.
- Partly open: nit 4. The timestamp is now kept by cutting the model first, but that creates the new unevenness in nit 1 below, and a cut left title still runs into the fill.

## States checked
- 1. User prompt folded @ 80 / 120 — snapshots `user_prompt_folded_at_80`, `user_prompt_folded_at_120` — ok (unchanged).
- 2. User prompt unfolded @ 80 (text) and selected (styled) — snapshots `user_prompt_unfolded_at_80`, `user_prompt_unfolded_and_selected_at_80_styled` — ok. `▒` is in `primary` (229,229,229 in `default`) on exactly the prompt rows, and the text is bold `foreground`.
- 3. Injected item before the prompt @ 80 — snapshot `injected_item_before_the_prompt_at_80` — ok.
- 4. Full Assistant turn @ 120, text and styled, unfolded tool selected, dim on — snapshots `assistant_turn_at_120_text`, `assistant_turn_at_120_selected_tool_dimmed_styled` — ok. Folded thinking now reads `▶ Thinking: The user wants the UVC extension units. Start with lsusb.` because the soft-broken first paragraph is one rendered line, and it is drawn muted.
- 5. State 4 with dim off @ 120 (styled) — snapshot `assistant_turn_at_120_selected_tool_not_dimmed_styled` — ok. Thinking is in `thinking` and not bold.
- 6. Unfolded thinking and unfolded 3-entry system section @ 80 — snapshots `assistant_turn_with_unfolded_thinking_at_80`, `assistant_turn_with_unfolded_system_section_at_80` — ok.
- 7. Error and pending tool calls unfolded @ 80 (styled) — snapshots `error_tool_call_unfolded_at_80_styled`, `pending_tool_call_unfolded_at_80_styled` — ok.
- 8. Compaction folded / unfolded @ 80 — snapshots `compaction_folded_at_80`, `compaction_unfolded_at_80` — ok.
- 9. Redacted-only Assistant turn @ 80 — snapshot `redacted_thinking_only_is_an_empty_block_at_80` — ok (3 rows).
- 10. Assistant title without effort @ 80 — snapshot `assistant_title_without_effort_at_80` — ok.
- Rendered in the scratch crate (`/tmp/factory-ui-017-turn-block-sections`):
  - Folded thinking @ 80 / 40 — ok. Checked first lines that are `**bold**`, a list, a `#` heading, a fenced code block, a block quote, inline code and a link, leading blank lines, `\r\n` line endings, a table and an HTML comment. Each shows the rendered text, cut with `…` at the text width. Styled @ 80 with the section selected: bold `thinking` (muted), `▒` in `primary`.
  - Unfolded tool descriptions @ 80 / 60 / 40 / 20 — ok. Checked a 100-character prose description, a long path, a description with an embedded newline and a tab, a blank description (falls back to `Shell`) and CJK text. Prose wraps at word boundaries, continuation lines start at column 4, CJK never passes the text width, and `▒` covers the whole wrapped description. Folded @ 60 — ok.
  - Model-change note and stats footer @ 80 / 40 / 30 / 20 / 12 — ok. They wrap at words, and break inside a word only when a word is longer than the width.
  - Titles: the fixture model @ 80 / 64 / 60 / 50 / 46 / 44 / 42 / 40, a 50-character model @ 120 down to 2, and the User title @ 30 down to 2 — every top border is exactly W cells and ends in `╮`; issue (nit 1).
  - Tiny widths 2–12: text never reaches the right padding — ok.
  - Folded prompt @ 80: two paragraphs, three paragraphs, heading first, code first, list, exactly two lines, long second paragraph — ok. Blank rows are skipped, and `…` appears only when there is more and stays in `foreground`.
  - HTML-comment-only thinking and text, folded and unfolded — the comment renders literally in both forms (consistent); see nit 2.

## Findings
1. [nit] Assistant title, widths about 40–64 (any model id). This corrects my round-1 nit 4. Cutting the model first, as I suggested, now gives uneven results.
   - With the fixture model `claude-opus-5.5 @ high`, at W = 44 / 46 / 50 the model shrinks to a useless stub (`c…`, `cla…`, `claude-…`) to keep the timestamp.
   - At W = 42 the full `claude-opus-5.5 @ high` is shown and the timestamp is dropped.
   - So a narrower block shows more of the model than a wider one.

   Change: in `TitleBar::line`, keep the timestamp only when at least about 10 cells of model text (or the whole model, if shorter) still fit beside it. Otherwise drop the timestamp and give the model the freed room. At 44–50, that would draw `╭─ Assistant ── claude-opus-5.5 @ high ──…╮`.

   Also still open from round 1: a left title cut with `…` runs straight into the fill with no trailing space (`╭─ Assistant…──╮`, `╭─ Use…──╮`). Keep one space after the `…`, as `cut_model` already does for the model text.

   See `ui/title-model-cut-40-64.txt`.
2. [nit] Doc comment on `derive_sections` (`src/components/turn_block/section.rs`). It gives "text that is only an HTML comment" as an example of markdown that renders to no rows. The markdown renderer draws HTML comments literally: `<!-- c -->` appears as a text row in both the folded thinking header and the unfolded and assistant-text forms, so the example is wrong. Change: use an example that really renders to nothing (for instance an empty string or whitespace-only text), or drop the example.
3. [nit] Test gate, not caused by this task. `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` (`tests/index_stub.rs:339`, task 010) failed once with `assertion left == right` during a full `cargo test` run, while another cargo build was running at the same time. It then passed 15 isolated reruns and two more full runs (1017 passed, 0 failed). This branch does not touch `tests/`. Recording it so the Approver recognises it if CI flakes, and recommending a separate fix to that test's timing.
4. [nit] Tool call, folded, multi-line description. A description with an embedded newline (`Multi\nline …`) folds to `▶ Multi` with no `…`, so nothing shows that more text is hidden. Unfolded, it correctly shows both lines. Change: in the folded form, add `…` when the description has more non-blank lines than the first. This is low impact, because real descriptions are almost always one line.
