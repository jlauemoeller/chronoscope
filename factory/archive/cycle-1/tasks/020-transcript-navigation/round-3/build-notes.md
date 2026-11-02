STATUS: DONE

## Changes
- `paging.rs` `jump_turn`: the fallback reveal uses `Reveal::InPlace` instead of `Reveal::Top`, so a block whose first section is taller than the viewport keeps its top border on the top row. Short sections behave as before (minimal reveal). The doc comment now says the selection is scrolled into view only when it would otherwise be off screen.
- Tests (`tests/keys.rs`): new `a_jump_into_a_block_with_a_tall_first_section_keeps_the_top_border_on_top` (`]` from `p1`, then `G` and `[`; asserts selection `a1/big`, offset == block top, row 0 starts with `╭`); `showing_system_content_scrolls_a_pushed_down_selection_into_view` now presses `+` first so the reveal is needed; `g_on_a_tall_last_section_still_shows_its_end` now uses a transcript whose last section (35 rows, 10-row viewport) is taller than the viewport and asserts `offset == total - 10`; the dead `let _ = tall` is gone.
- `src/app/tests.rs`: `a_transcript_change_asks_for_a_redraw` (`take_redraw`, `update(Msg::TranscriptChanged)`, `take_redraw() == Dirty`, then `None`).
- Nothing else changed; no `.snap` file changed.

## Self-check
- Review: approver rework 1 / code-review 1 — `a_jump_into_a_block_with_a_tall_first_section_keeps_the_top_border_on_top`; the round-2 test `a_jump_past_a_tall_unfolded_system_section_keeps_the_prompt_on_screen` still passes.
- Review: approver rework 2 / feature-review 1 — `showing_system_content_scrolls_a_pushed_down_selection_into_view`
- Review: approver rework 3 / feature-review 2 / code-review 2 — `g_on_a_tall_last_section_still_shows_its_end`
- Review: approver rework 4 / code-review 3 — `a_transcript_change_asks_for_a_redraw`
- Review: not changed on purpose, per the note: prompt placement on `]`, `s` keeping the offset when showing, page keys with a tall selection, `]`/`[` first non-system section.

## Mutation checks (each fix reverted alone, then restored)
- `Reveal::InPlace` -> `Reveal::Top` in `jump_turn`: `a_jump_into_a_block_with_a_tall_first_section_keeps_the_top_border_on_top` FAILED (1 failed, 162 passed in the transcript tests).
- `reveal(..)` removed from `toggle_system`'s show branch: `showing_system_content_scrolls_a_pushed_down_selection_into_view` FAILED (the only failure).
- `rows.len() <= height &&` removed from `jump_to_end`: `g_on_a_tall_last_section_still_shows_its_end` FAILED (the only failure).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (1259 passed, 0 failed; lib 1046)
