STATUS: DONE

## Changes
- `paging.rs` `jump_turn`: after putting the block's top border at the top row, the selected section is revealed (`reveal(.., Reveal::Top)` with `span`) if it would be off screen, e.g. an unfolded system section taller than the viewport in front of the prompt. The top border stays on top whenever the selection still fits.
- `paging.rs` `jump_to_end`: a selected section that fits the viewport but is off screen after the jump (one- or two-row viewports) is revealed; a tall last section still shows its end. `Amount::Half` is at least one row.
- `folding.rs` `toggle_system`: hiding keeps the selected section's screen row; showing leaves the view where it is (so the top of the transcript, with the block's title, stays on screen) and only scrolls if the selection would be pushed off screen.
- Redraw contract: new `Msg::TranscriptChanged` (`app/mod.rs`, handled like `PromptMoved`: `request_redraw(Dirty)`); `Transcript::on` returns it when `perform` reports a change (anything but `NoChange`/`Invalid`), else `None`.
- Cleanups: `Cursor.rows`/`pos` private again; `jump_to_start/end` reuse `first_section_from`/`last_section` (now `pub(super)`); `turn_stop` derives the block's sections once instead of per candidate; `FoldOp`/`Amount` are `pub(super)`; `FoldOp` is imported, not path-qualified; `system_selected()` comment reworded.
- Not changed (per the reviews): the selected tall section is "not fully visible" when page-scrolling (follows the binding planner rule; feature-review nit 2); `]`/`[` pick the first non-system section (nit 3, deliberate).

## Self-check
- Review: code-review 1 / feature-review: ui-review 1 [must-fix] `]` leaves the prompt off screen — `a_jump_past_a_tall_unfolded_system_section_keeps_the_prompt_on_screen` (`+`, `]`, `]`, `[` at 80x10, asserts the selected section is inside the viewport each time; verified to fail with the reveal removed), `a_jump_keeps_the_top_border_on_top_while_the_prompt_still_fits`
- Review: code-review 2 (redraw) — `a_key_that_changes_the_view_answers_transcript_changed`, `a_key_that_changes_nothing_answers_nothing`
- Review: code-review 3 / feature-review 1 (vacuous assertion) — `the_screen_row_is_kept_only_when_the_end_of_the_transcript_allows_it` now asserts `offset == total - 24` and the screen row
- Review: code-review 4, 5, 6, 7 — see Changes
- Review: ui-review 2 (`s` anchor hides the title) — `showing_system_content_keeps_the_top_of_the_transcript_on_screen`, `showing_system_content_scrolls_a_pushed_down_selection_into_view`
- Review: ui-review 3 (tiny viewports) — `g_in_a_one_row_viewport_shows_the_selected_last_section`, `g_on_a_tall_last_section_still_shows_its_end`, `half_a_page_is_at_least_one_row`
- Review: ui-review 4 — comment reworded
- All round-1 AC and UI snapshot coverage is unchanged (no snapshot changed).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1257 passed, 0 failed; lib 1044)
