STATUS: DONE

## Changes
- `src/components/turn_block/renderers/tool/` (replaces `tool.rs`): `ToolCallSection` lays out the description row plus the labelled blocks of a `ToolBodyRenderer`. One type per kind with its own unit tests: `ShellBody`, `ReadBody` (gutter, Claude `N→` / `N<TAB>` prefixes parsed once, plain output numbered from `offset`, unnumbered trailer lines get an empty gutter), `WriteBody` (`highlight_code` by extension, char-wrapped), `EditBody` (`similar` unified diff, 3 context lines, MultiEdit supported, no "no newline" rows), `GrepBody`, `GlobBody`, `WebFetchBody`, `WebSearchBody`, `OtherBody` (pretty JSON), `SubagentBody` (markdown prompt/result, `<key> to open transcript` hint at the label column). `body_renderer(&ToolKind)` dispatches. Subagent folded/unfolded description gets ` (<agent type>)` (`subagent_type` or `agent`), kept when the description is cut.
- Truncation: `truncate_output_lines` logical lines are shown (counted before wrapping), then a `… N more lines` row (thousands grouped with a space, `line` for 1). Applies to results, Read content, Write content and the Edit diff. Subagent markdown is not truncated. `ToolCallSection::render` returns the row apart (`ToolRendered::more`).
- `renderers/mod.rs`: `SectionCtx` gains `truncate`, `expanded`, `open_key` (+ `SectionCtx::new`); `render_section(section, ctx) -> Rendered { lines, more }`.
- `section.rs`: `SectionKey::More(ItemId)` (layout-only, never derived), `SectionId::more_row()/more_parent()`; `FoldState` gets the per-`SectionId` expansion set (`is_expanded/set_expanded`), independent of fold state.
- `widget.rs`: `TurnBlock::truncate_output_lines()` / `open_subagent_key()`; `layout` adds the more row as its own selection stop directly after the tool section (no blank row), so `▒` marks only that row when it is selected.
- `transcript/`: passes both settings to blocks; `TRUNCATE_OUTPUT_LINES_ATTR` now invalidates all heights and restores the selection; new `OPEN_SUBAGENT_KEY_ATTR`; `Cmd::Submit` on the more row expands (stored in `FoldState`), removes the row and selects the tool section without moving the viewport; folding/an update/a raised limit that removes the row moves the selection to the tool section.
- `markdown/wrap.rs`: `wrap_styled_lines` (public) to char-wrap highlighted code.
- `model/output.rs`: test-only thread-local counter `text_calls()` to prove folded sections never call `ToolOutput::text()`.
- A materialisation error renders `(output unavailable: <error>)` in `error_output` and is logged with `log::warn!`.
- Existing Shell tests/snapshots moved from the generic `Input:` body to `Command:` (5 snapshots re-accepted; diffs inspected: only `Input:` + JSON replaced by `Command:` + raw command text, and the matching row shifts); the JSON-wrapping test now uses an `Other` tool.
- The App does not mount `Transcript` yet, so the settings/key label are props only (attrs); no App wiring.

## Self-check
- AC: each kind renders its table body, labels at 4, content at 7, tokens — `shell::tests::labels_columns_and_tokens`, `read::tests::{claude_numbering_is_parsed_into_the_gutter_once,labels_and_path_token}`, `write::tests::labels_columns_and_path_token`, `edit::tests::a_diff_has_headers_and_coloured_plus_and_minus_lines`, `search::tests::{grep_labels_columns_and_tokens,glob_shows_the_path_but_no_filters}`, `web::tests::{web_fetch_labels_columns_and_tokens,web_search_labels_columns_and_tokens}`, `other::tests::labels_columns_and_tokens`, `subagent::tests::unfolded_shows_prompt_result_and_the_hint`, `tool::tests::dispatch_picks_a_renderer_for_every_kind`.
- AC: 200-char path char-wrapped at body width — `shell::tests::a_long_path_is_char_wrapped_at_the_body_width`, `turn_block::tests::tool_body_labels_are_at_column_4_and_content_at_column_7`.
- AC: Read Claude numbering once / plain numbered from offset — `read::tests::{claude_numbering_is_parsed_into_the_gutter_once,tab_separated_numbering_is_parsed_too,plain_output_is_numbered_from_the_input_offset,plain_output_without_an_offset_starts_at_one}`.
- AC: Write highlighted by extension — `write::tests::rust_content_is_highlighted_by_extension`.
- AC: Edit `+`/`-`/`@@` styling — `edit::tests::a_diff_has_headers_and_coloured_plus_and_minus_lines` (+ styled snapshot).
- AC: Subagent fold/unfold/hint, remapped key — `subagent::tests::*`, `turn_block::tests::the_open_transcript_hint_follows_the_remapped_open_subagent_key` (real `Keymap` from TOML), `transcript::tests::truncation::the_transcript_passes_the_open_subagent_key_to_subagent_sections`.
- AC: 1 434 lines / 200 → 200 + `… 1 234 more lines`; Move + Submit expands; survives fold/unfold — `transcript::tests::truncation::{a_long_result_shows_200_lines_and_a_more_row,the_more_row_is_a_stop_of_its_own_after_the_tool_section,moving_selects_the_more_row_and_only_it_gets_the_marker,submit_on_the_more_row_expands_all_lines_and_selects_the_tool,the_expansion_survives_folding_and_unfolding}` plus setting/update/fold edge cases in the same file and `turn_block::tests::{a_truncated_call_has_a_second_stop_directly_below_it,selecting_the_more_row_marks_that_row_only,an_expanded_call_has_all_its_lines_and_no_more_row}`.
- AC: folded never calls `text()`, unfolding does — `truncation::a_folded_call_never_materialises_its_output_and_unfolding_does` (lazy output + call counter); unreadable output — `truncation::an_output_that_cannot_be_read_is_reported_in_error_output`.
- AC: layout-cache heights match rendering for new bodies — `transcript::tests::property::cached_heights_equal_rendered_heights` extended to all ten kinds, small random truncate limits, random expansion, plus a check that selection stops are ordered and non-overlapping.
- UI 1 Shell long command + result (text + styled @100) — `shell_unfolded_with_a_long_command_at_100_text`, `..._styled`.
- UI 2 Read numbered (text) — `read_unfolded_with_numbered_content_at_100_text`.
- UI 3 Write `.rs` (styled) — `write_unfolded_with_rust_content_at_100_styled`.
- UI 4 Edit diff (text + styled) — `edit_unfolded_with_a_diff_at_100_text`, `..._styled`.
- UI 5 Grep/Glob/WebFetch/WebSearch (text) — `grep_unfolded_at_100_text`, `glob_unfolded_at_100_text`, `web_fetch_unfolded_at_100_text`, `web_search_unfolded_at_100_text`.
- UI 6 Other `mcp__` pretty JSON (text) — `other_mcp_tool_unfolded_with_pretty_json_at_100_text`.
- UI 7 Subagent folded and unfolded (text) — `subagent_folded_at_100_text`, `subagent_unfolded_at_100_text`.
- UI 8 truncated, row selected @80x24 Transcript (text + styled) and after expansion (text) — `truncated_result_with_the_more_row_selected_at_80x24_text`, `..._styled`, `expanded_result_after_submitting_the_more_row_at_80x24_text`.
- UI 9 failed Shell (styled) — `failed_shell_call_at_100_styled` (also the existing 80-column one).

## Judgement calls (spec silent)
- Unfolded subagent description also shows ` (<agent type>)`.
- The subagent hint sits at the label column (4), after a blank row.
- Write/Edit show no `Result:` (table lists none).
- Read with only `offset` shows `(from line N)`; only `limit` shows `(lines 1–N)`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (951 unit tests + integration tests, 0 failed)
