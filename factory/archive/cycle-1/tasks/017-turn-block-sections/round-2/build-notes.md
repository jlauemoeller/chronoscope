STATUS: DONE

## Changes
Round 2 addresses the round-1 reviews (feature review PASS, code review PASS, UI review FAIL with two must-fix findings). No human or approver note was present in this round directory.
- `renderers/thinking.rs`: the folded header is built from the first non-blank line of the rendered markdown (first paragraph only, rendered at 512 cells, then cut to one row with `…`), so `**Plan**` reads `▶ Thinking: Plan`.
- `renderers/tool.rs`: the unfolded description is word-wrapped (`textwrap`, via the new `word_wrap` helper) at the text width; a token longer than the width is still broken. Blank descriptions fall back to the tool name, and control characters (tabs) become spaces in both folded and unfolded forms.
- `renderers/user.rs`: the folded prompt skips blank separator rows, so two paragraphs show as two rows; the `…` is appended only to a content row and so keeps the `foreground` token.
- `renderers/text.rs`, `renderers/stats.rs`: the model-change note and the stats footer are word-wrapped instead of char-wrapped; `format_tokens` uses `saturating_add`.
- `title.rs`: `TitleBar::line` always returns exactly `width` cells and ends in `╮` from width 2. When the titles do not fit, the model text is cut with `…` first (keeping spaces), then the timestamp is dropped, then the left title is cut.
- `widget.rs`: interior content is clipped to columns 2 to W-4 so it never reaches the padding at tiny widths; added `TurnBlock::render_layout(&BlockLayout, area, buf)` for cached layouts and `Widget for &TurnBlock`; no more double span clones.
- Renamed the `sections` renderer module to `renderers` and the derivation function to `derive_sections` (no `as` alias); fixed the module docs; removed the unused `TEXT_COLUMN`; `components::turn_block` is now `pub`. Documented that `BlockLayout::section_rows()` (not `derive_sections`) lists the sections that have rows.
- Snapshots: the three `assistant_turn_at_120_*` snapshots changed (folded thinking now shows the rendered first paragraph). Others unchanged.

## Self-check
- Review (UI 1, must-fix): folded thinking shows markdown source — `folded_thinking_shows_the_rendered_first_line_not_markdown_source`, `folded_thinking_stays_in_the_thinking_token`
- Review (UI 2, must-fix): tool description char-wrapped — `unfolded_tool_description_is_word_wrapped_at_the_text_width` (width 72/80/32 and a single long token)
- Review (UI 3 / feature 1 / code 8, nit): blank row and unstyled `…` in folded prompt — `folded_prompt_skips_blank_separator_rows`
- Review (feature 2, nit): same as UI 1.
- Review (feature 3, nit): same as UI 2.
- Review (UI 4, nit): title truncation — `long_model_is_cut_before_the_timestamp_is_dropped`
- Review (UI 5, nit): tab mismatch and blank description fixed (`blank_descriptions_fall_back_to_the_tool_name_and_tabs_agree`); ANSI SGR residue in tool results is left for task 019's output bodies.
- Review (UI 6, nit): `stats_footer_and_model_note_wrap_at_word_boundaries`
- Review (UI 7 / code 9, nit): `the_top_border_is_always_the_width_and_ends_in_a_corner`, `text_never_reaches_the_padding_at_tiny_widths`
- Review (code 1, 2, 3, 4, 5, 10, 11, nits): dead constant removed; `derive_sections` rename; `saturating_add` (`token_counts_near_u64_max_do_not_overflow`); `render_layout` and `&TurnBlock` (`rendering_a_cached_layout_equals_rendering_the_block`); single clone; selectable sections documented.
- Review (code 6, 7): same as UI 2 and UI 1.
- All AC and UI tests from round 1 remain and pass.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1017 passed, 0 failed)
