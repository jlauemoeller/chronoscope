VERDICT: PASS

## Summary
Round 2 fixes both round-1 UI must-fix findings. Folded thinking now shows the first line of the rendered markdown, and unfolded tool descriptions are word-wrapped. It also clears the round-1 feature nits: the folded prompt skips blank separator rows and its `…` keeps the `foreground` token. Every acceptance criterion is still implemented, and each is covered by a test that would fail if the behaviour were wrong. The new behaviour has its own targeted tests. I re-ran the gate and it is green: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `INSTA_UPDATE=no cargo test` with 0 failures and no files changed. I found two small nits.

## Acceptance criteria
- [x] Section derivation: each `Item` gives its section kind with the §5.2 default fold, redacted thinking gives no section, and injected items give none when `show_system` is false. Implemented in `section.rs` (`derive_sections`, which replaces the old `as` alias). Tests: `every_item_becomes_its_section_kind_and_stats_come_last`, `redacted_thinking_yields_no_section`, `injected_items_yield_no_section_when_show_system_is_false`, `default_folds_follow_the_section_table`.
- [x] Folded user prompt shows exactly 2 wrapped lines, with `…` only when there is more; unfolded shows all lines. Implemented in `renderers/user.rs`, which now drops blank rows before taking 2 content rows. Tests: the round-1 tests still pass, plus `folded_prompt_skips_blank_separator_rows`. That test covers two paragraphs (no `…`), three paragraphs (`…` in `foreground`), a heading first, and unfolded keeping its blank rows.
- [x] Folded and unfolded forms of thinking, tool call, compaction and system sections, with their tokens. Implemented in `renderers/{thinking,tool,compaction,system}.rs`. Tests: the round-1 styled tests, plus `folded_thinking_shows_the_rendered_first_line_not_markdown_source` (bold, list, heading, inline code, a long line cut to one row with `…`), `folded_thinking_stays_in_the_thinking_token` (uses `distinct_theme`), `unfolded_tool_description_is_word_wrapped_at_the_text_width`, and `blank_descriptions_fall_back_to_the_tool_name_and_tabs_agree`.
- [x] Stats footer shows `5m 02s · 112 messages · 45.2k tokens`. Implemented in `renderers/stats.rs`, now word-wrapped and using `saturating_add`. Tests: `stats_footer_formats_the_spec_example`, `stats_footer_and_model_note_wrap_at_word_boundaries`, `token_counts_near_u64_max_do_not_overflow`.
- [x] Model change `model → <model>` at column 4. Implemented in `renderers/text.rs`. Test: `model_change_is_at_column_4` at widths 80 and 120.
- [x] Block geometry: titles, text at column 4 wrapping at `W − 5`, tool-body content at column 7, and the row structure, at widths 80 and 120.
  - Implemented in `title.rs` and `widget.rs`. Interior content is now clipped to columns 2..=W−5.
  - `TitleBar::line` now cuts the model text first, then drops the timestamp, then cuts the left title. Its width is always exact: `used <= room`, so `width - 4 - used >= 1` and cannot underflow.
  - Tests: the round-1 geometry tests, plus `the_top_border_is_always_the_width_and_ends_in_a_corner` (widths 2..=140 for all three title kinds), `long_model_is_cut_before_the_timestamp_is_dropped`, and `text_never_reaches_the_padding_at_tiny_widths`.
- [x] `TurnBlock::height` equals the rows drawn. The `fixtures::draw` assertion applies to every snapshot. `height_equals_the_rows_drawn_for_many_widths_and_fold_states` still passes.
- [x] Clipped rendering draws exactly the matching rows of the full rendering. The `clipped_rendering_...` cell-by-cell test still passes. The new `render_layout` and `&TurnBlock` paths are checked against the direct render by `rendering_a_cached_layout_equals_rendering_the_block`.
- [x] Selection and dimming: `▒` in `selection_marker` on exactly the selected rows, bold text, and the `dim_unselected` true/false behaviour. Implemented in `emphasis.rs` and `widget.rs`, unchanged in substance. Tests and the styled snapshots for states 4 and 5 pass. The only snapshot changes are the three `assistant_turn_at_120_*` files, where the folded-thinking row now shows the rendered first paragraph.
- [x] Every UI state has a snapshot. States 1–10 are all present in `snapshots.rs`, and no `.snap.new` files exist.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran all three: clean.

## Findings
1. [nit] `word_wrap` in `src/components/turn_block/renderers/mod.rs` calls `textwrap::wrap(&text, width)` with the default `HyphenSplitter`. When an identifier with hyphens does not fit, it is split at a hyphen. A scratch probe with textwrap 0.16 gave:
   - `model → claude-sonnet-5.5` at width 20: `model → claude-` / `sonnet-5.5`. This is the split that round-1 UI nit 6 cited.
   - a description path at width 30: `/home/jacob/projects/foo-` / `bar/src/main.rs`.

   The task-016 markdown prose wrapper (`src/markdown/prose.rs`, `words`) splits only at spaces. So tool descriptions, model notes and the footer break hyphenated words differently from assistant text and prompts.

   Change: wrap with `textwrap::Options::new(usize::from(width.max(1))).word_splitter(textwrap::WordSplitter::NoHyphenation)`. `break_words` stays on, so over-long tokens are still broken. Add a narrow-width assertion with a hyphenated model name to `stats_footer_and_model_note_wrap_at_word_boundaries`.
2. [nit] `ThinkingSection::first_line` in `src/components/turn_block/renderers/thinking.rs` finds the first paragraph with `split("\n\n")`. This has two effects:
   - With CRLF text (`\r\n\r\n`), the whole thought is markdown-rendered on every layout just to get one row.
   - A leading fenced code block with a blank line inside is cut mid-fence. It still renders acceptably.

   Change: optionally normalise `\r\n` first, or stop at the first blank line found with `lines()`/`trim().is_empty()`. That bounds the work for long thoughts.
