STATUS: DONE

## Changes
- `components/transcript/updates.rs`: `change_turns` re-evaluates following after a refresh when the user was away from the end, so an `Updated` batch that brings the end into view resumes following and resets the count.
- `components/transcript/follow.rs`: `at_bottom` only trusts the prepared window when its offset equals the transcript's.
- `components/browser_header.rs`: the right-aligned items must fit the row; the note is dropped first, then `● live`, so the row is always `w` cells and ends in `│`.
- `app/mod.rs`: the `NewSections` arm requests a redraw; docs of `Msg::NewSections`; `components/transcript/mod.rs`: docs of `on` and `NEW_SECTIONS_ATTR`.
- Tests: transcript follow tests, header narrow-width test, a styled screen snapshot of `↓ N new`.

## Self-check
- Review (feature, must-fix 1: following not re-evaluated after an in-place update) — `follow::an_update_that_brings_the_end_into_view_resumes_following` (verified to fail without the fix)
- Review (feature, must-fix 2: selection moves to the new last section of a growing turn) — `follow::the_selection_moves_to_the_new_last_section_of_a_growing_turn`, `a_selection_not_on_the_last_section_stays_when_the_last_turn_grows`
- Review (code nit 1, `on` docs) — done. (nit 2, stale window) — offset compared. (nit 3) — same as must-fix 1. (nit 5) — `Msg::NewSections` and `NEW_SECTIONS_ATTR` docs. (nit 6 / feature nit 3, redraw) — `Dirty` requested in the arm.
- Review (code nit 4, `Reset` returns to the top until `Loaded`) — kept deliberately: a reload is treated as a fresh load, as the file was shrunk or replaced; for the Approver to decide if "stay at the end" is wanted.
- Review (feature nit 4/5) — no change; the order against a search item cannot be tested before browser search exists.
- Review (UI nit 1, narrow header) — `browser_header::tests::a_row_too_narrow_for_the_note_and_the_marker_drops_the_note_first`
- Review (UI nit 2 / feature nit 6, styled `↓ N new`) — `live::the_indicator_is_drawn_in_help_text_80x24_styled`
- Round-1 UI snapshots are unchanged.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1427 passed, 0 failed)
