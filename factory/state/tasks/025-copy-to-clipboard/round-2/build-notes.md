STATUS: DONE

## Changes
- Folded summaries are copied as the screen shows them. `plain::displayed_text` takes the block width (`Option<u16>`, the width the transcript lays its blocks out at; `Transcript::displayed_text` passes it). A folded prompt is rendered at that text width and keeps the two non-blank rows the fold draws, the second ending in `…`. Rows that are soft-wrap continuations of one logical line are joined by a space (`soft_joined`, matched against the unwrapped lines); real line breaks stay. So the fixture prompt at 80 columns copies as one line ending `… but I don't have a way to…` (the ui review's expected text), and several paragraphs copy as `first\nsecond…`. Other folded non-tool summaries (thinking, system, compaction) are cut at the same width. A folded tool call stays its whole description (acceptance criterion: "exactly its description line"). Unfolded sections are never cut at the width. With no width, behaviour is the earlier unbounded one.
- `Y` is verbatim. New `SectionCtx::verbatim` makes tool text (Shell command, `Other` input, Web/Search fields, results, Write content, Edit diff rows, Read content, Subagent prompt/result, system entries) one row per `\n`-separated line, with no wrapping, no tab expansion and no dropped characters (`verbatim_lines`). `full_text` sets it; `displayed_text` and the screen do not.
- App: the full failure reason is logged (the toast is cut at 60 columns); `Copied 0 lines` for empty text; `mod copy;` and `PLAIN_WIDTH` made private; the `copy_text` doc says the OSC52 write happens in `update`, between frames, and is not size-limited; `doc/clipboard.md` documents the OSC52 size limit.
- Tests: `/` and `w` (save, a later task) added back to the "keys that do not change the view" test.

## Self-check
- Review code 1 / feature 1 / ui 1 (folded prompt not as displayed) — `plain::tests::a_folded_one_paragraph_prompt_is_what_the_fold_shows_as_one_line`, `a_wider_block_shows_more_of_a_folded_prompt`, `a_folded_prompt_of_several_paragraphs_keeps_its_two_rows`, `a_folded_prompt_whose_first_paragraph_wraps_shows_only_what_the_fold_shows`, `a_wrapped_first_paragraph_and_a_second_one_are_two_lines`, `soft_joined_matches_rows_to_the_unwrapped_lines`, `transcript::tests::copy::y_on_a_folded_prompt_copies_the_rows_shown_at_the_transcripts_width`, `browser_tests::copy::y_copies_the_selected_section_as_displayed` (now at 80x24, expects the shown text).
- Review code 2 (`Y` not verbatim) — `plain::tests::full_content_keeps_tabs_and_control_free_lines_exactly`, `full_content_does_not_break_a_very_long_line` (70 000 chars), `full_content_of_a_write_keeps_the_file_as_written`, `the_displayed_copy_still_expands_tabs`.
- Review code 3 (folded thinking/system headers) — `plain::tests::a_folded_thinking_header_is_cut_like_the_screen`; tool description stays whole (`a_folded_tool_call_stays_its_whole_description_at_any_width`).
- Review code 4, ui 2 (OSC52 size, failure reason) — documented in `doc/clipboard.md`; reason logged in `app/copy.rs`.
- Review code 5 (lazy read on the UI thread) — left as is; noted here for the §9.2 budgets.
- Review code 6 — `Copied 0 lines` for empty text.
- Review code 7 — `mod copy;` and private `PLAIN_WIDTH`.
- Review code 8, feature 3 — keys test again presses `/` and `w`.
- Feature 2 — kept in `update`; documented in the doc comment.
- UI 3 (rule length) — left at 80 cells; spec does not pin it.
- UI snapshots unchanged (4 toast snapshots from round 1 still pass).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1415 lib tests, 0 failed; other targets pass)
