STATUS: DONE

## Changes
New component module `src/components/turn_block/` (re-exported from `components`):
- `section.rs`: `SectionId`/`SectionKey`, `SectionKind`, `Section`/`SectionContent`, pure `derive_sections(turn, show_system)` (redacted or blank thinking, blank text and hidden injected items yield no section; User turns list items before the prompt; stats footer last) and `FoldState` (overrides over the §5.2 defaults; non-foldable kinds never fold).
- `sections/`: one renderer type per kind (`UserPromptSection`, `AssistantTextSection`, `ModelChangeSection`, `ThinkingSection`, `ToolCallSection` with the generic Input/Result body, `StatsSection` with `format_duration`/`format_tokens`/`format_stats`, `CompactionSection`, `SystemSection`), each `lines(&SectionCtx) -> Vec<Line>` starting at block column 2 (marker or two spaces, text at column 4, text width `W-8`, body width `W-11`).
- `title.rs`: `TitleBar` (top border with per-span colours; the timestamp is dropped and then the left title cut with `…` at tiny widths).
- `emphasis.rs`: `emphasize` (selected = bold, others = `muted_foreground` fg when `dim_unselected`, modifiers kept).
- `widget.rs`: `TurnBlock` widget with `layout(width)` (`BlockLayout::height()`, `section_rows()` for task 018), `height(width)`, and `skip_rows(k)` plus the render area height for clipping. Borders are drawn by hand (not ratatui `Block`) so the top row can be clipped and `▒` can replace `│`.
- Tests: `fixtures.rs`, `tests.rs` (behaviour), `snapshots.rs` (UI states); `.snap` files under `snapshots/`.

Interpretation choices (spec silent):
- No blank row after `▼ Thinking`, the compaction divider or the system header (only the tool description and labels are followed by a blank row, as in the planner notes).
- Unfolded system section: header, then per entry its kind line at column 4 and its text at column 7, with a blank row between entries (also for a single entry).
- Markdown soft breaks join lines (task 016 behaviour), so a typed prompt with single newlines folds as one paragraph.
- `1 message` / `1 token` are singular; message count 0 is omitted as unknown.
- Tool result with `status == Error` uses `error_output` (status, not `result.is_error`); a finished call without a result shows `(pending)` too; empty output shows `(no output)`.
- Blank user prompts and blank assistant text get no section.
- Folded tool call / thinking / system headers are cut to one row with `…`.
- Timestamps keep the `Z` that `TimestampFormatter` adds with `utc = true`.

## Self-check
- AC: derivation, default folds, redacted/injected rules — `user_turn_lists_injected_items_before_the_prompt`, `injected_items_yield_no_section_when_show_system_is_false`, `every_item_becomes_its_section_kind_and_stats_come_last`, `redacted_thinking_yields_no_section`, `compaction_turn_has_one_summary_section`, `default_folds_follow_the_section_table`, `fold_state_overrides_foldable_sections_only`
- AC: prompt folded 2 lines + `…`, none when it fits, unfolded all — `folded_prompt_shows_two_lines_and_an_ellipsis_when_longer`, `folded_prompt_without_more_lines_has_no_ellipsis`, `folded_prompt_of_three_lines_ends_the_second_with_an_ellipsis`, `folded_prompt_ellipsis_is_truncated_to_fit_a_full_line`, `unfolded_prompt_shows_every_line`
- AC: thinking/tool/compaction/system folded+unfolded with tokens — `thinking_folded_and_unfolded_use_the_thinking_token`, `folded_thinking_is_truncated_to_one_row`, `tool_call_folded_shows_the_description_in_its_token`, `tool_call_unfolded_has_the_generic_body_in_its_tokens`, `tool_result_is_error_output_when_the_status_is_error`, `pending_tool_shows_muted_pending_at_column_7`, `compaction_folded_and_unfolded`, `system_section_headers_and_unfolded_entries`
- AC: stats footer `5m 02s · 112 messages · 45.2k tokens` — `stats_footer_formats_the_spec_example`, `stats_footer_adds_cost_and_omits_unknown_parts`, `durations_and_token_counts_are_formatted`, `footer_is_muted_and_indented_by_two_spaces`
- AC: model change at column 4 — `model_change_is_at_column_4`
- AC: block geometry at 80 and 120 — `titles_have_the_planner_notes_format`, `title_colours_come_from_their_tokens`, `rows_are_blank_sections_separated_by_blanks_and_a_blank_before_the_bottom`, `text_starts_at_column_4_and_wraps_at_w_minus_5`, `prose_wraps_so_that_no_text_passes_column_w_minus_5`, `tool_body_labels_are_at_column_4_and_content_at_column_7`, `tool_body_wraps_json_to_the_body_width`, `an_empty_block_is_three_rows`
- AC: `TurnBlock::height` equals rows drawn — the `draw` helper asserts it for every snapshot (bottom border at row `height-1`, nothing below in a taller buffer); `height_equals_the_rows_drawn_for_many_widths_and_fold_states` (widths 8..=130, folded/unfolded), `show_system_false_removes_the_injected_rows_from_the_height`
- AC: clipping — `clipped_rendering_draws_the_matching_rows_of_the_full_rendering` (cell-by-cell, including cuts through the selected rows), `skipping_past_the_end_draws_nothing`
- AC: selection and dimming — `the_marker_covers_exactly_the_rows_of_the_selected_section`, `an_unfolded_tool_selection_covers_the_description_through_the_last_body_row`, `no_selection_draws_no_marker`, `selected_text_is_bold_with_its_token_colours_and_others_are_not`, `dim_unselected_mutes_every_other_section_but_not_titles_or_borders`, `dimming_keeps_modifiers`, `without_dimming_unselected_sections_keep_their_token_colours`, `thinking_gets_the_thinking_token_when_not_dimmed`; styled snapshots below
- UI 1 (folded prompt @ 80, 120) — `user_prompt_folded_at_80`, `user_prompt_folded_at_120`
- UI 2 (unfolded @ 80; selected, styled) — `user_prompt_unfolded_at_80`, `user_prompt_unfolded_and_selected_at_80_styled`
- UI 3 (injected + prompt @ 80) — `injected_item_before_the_prompt_at_80`
- UI 4 (assistant @ 120, text + styled, unfolded tool selected, dim) — `assistant_turn_at_120_text`, `assistant_turn_at_120_selected_tool_dimmed_styled`
- UI 5 (dim_unselected = false, styled) — `assistant_turn_at_120_selected_tool_not_dimmed_styled`
- UI 6 (unfolded thinking; unfolded 3-entry system @ 80) — `assistant_turn_with_unfolded_thinking_at_80`, `assistant_turn_with_unfolded_system_section_at_80`
- UI 7 (error unfolded styled; pending unfolded @ 80) — `error_tool_call_unfolded_at_80_styled`, `pending_tool_call_unfolded_at_80_styled` (both with `dim_unselected = false` so token colours show)
- UI 8 (compaction folded/unfolded @ 80) — `compaction_folded_at_80`, `compaction_unfolded_at_80`
- UI 9 (redacted-only, 3-row block @ 80) — `redacted_thinking_only_is_an_empty_block_at_80`
- UI 10 (no effort @ 80) — `assistant_title_without_effort_at_80`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1006 passed, 0 failed)
