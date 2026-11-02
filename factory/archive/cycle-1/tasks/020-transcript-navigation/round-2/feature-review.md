VERDICT: FAIL

## Summary
Round 2 fixes the round-1 must-fix: `]`/`[` now reveal the selected prompt when an unfolded system section pushes it below the viewport. It also adds `Msg::TranscriptChanged` for redraws, fixes tiny viewports and stops anchoring when `s` shows content. The gate passes and no snapshot changed, so the round-1 AC and snapshot coverage still holds. I tested the new code by mutating it in a scratch copy. The `jump_turn` reveal, the one-row `G` reveal and the `Half.max(1)` change each have a test that fails without them. The new reveal in `toggle_system` (show branch) does not: its only test passes with the reveal removed.

## Acceptance criteria
- [x] `j`/`↓`, `k`/`↑` move across sections and blocks, and a section taller than the viewport is paged through first. Implemented in `nav.rs` `move_selection` / `page_through`. Covered by the tests listed in round 1, among them `three_presses_scroll_by_whole_pages_through_a_three_page_section` and `three_presses_page_through_a_tall_section_and_the_fourth_selects_the_next_one`. Unchanged this round.
- [x] `]`/`[` jump between User/Assistant/Compaction blocks, never stop on system content, and put the top border at the viewport top when possible. Implemented in `paging.rs` `jump_turn` / `turn_stop`; `turn_stop` now derives the block's sections once. Tests: the round-1 tests, plus `a_jump_past_a_tall_unfolded_system_section_keeps_the_prompt_on_screen` (fails with the reveal removed, checked by mutation) and `a_jump_keeps_the_top_border_on_top_while_the_prompt_still_fits`.
- [x] `Ctrl-d`/`Ctrl-u` scroll half a page and `PgDn`/`PgUp` a full page, clamped; the selection moves to the topmost fully visible section. Implemented in `paging.rs` `scroll_pages` / `select_in_view`. Tests: the round-1 tests, plus `half_a_page_is_at_least_one_row` (fails without `.max(1)`).
- [x] `g`/`Home` and `G`/`End` select the first/last section and scroll to the start/end. Implemented in `paging.rs` `jump_to_start` / `jump_to_end`, which now reuse `first_section_from` / `last_section`. Tests: the round-1 tests, plus `g_in_a_one_row_viewport_shows_the_selected_last_section` (fails without the reveal) and `g_on_a_tall_last_section_still_shows_its_end` (see nit 2).
- [x] The fold keys, `Enter`/`Space`, `←`/`h`, `→`/`l`, `-` and `+`/`=`, are ignored by non-foldable sections and keep the screen row. Implemented in `folding.rs`. Tests: the round-1 tests. `the_screen_row_is_kept_only_when_the_end_of_the_transcript_allows_it` now asserts `offset == total - 24` and the screen row, which fixes the vacuous assertion.
- [x] `s` hides and shows system sections, `show_system = false` starts them hidden, and a hidden selected system section moves to a neighbour. Implemented in `folding.rs` `toggle_system` → `updates.rs` `set_show_system` / `restore_selection`. Tests: the round-1 tests, plus `showing_system_content_keeps_the_top_of_the_transcript_on_screen`. Keeping the selection on screen when showing is not effectively tested (finding 1).
- [x] Remapping: `[keys.browser] next = "J"`. Tests: `a_remapped_key_works_and_the_old_one_does_nothing` and related. Unchanged.
- [x] Every UI state has a snapshot. The nine `.snap` files in `key_snapshots.rs` are unchanged this round.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass (lib 1044 passed, 0 failed). The working tree is clean.

## Findings
1. [must-fix] The test `showing_system_content_scrolls_a_pushed_down_selection_into_view` (`src/components/transcript/tests/keys.rs`, near the end of the file) cannot fail.
   - What the test was meant to cover: round 2 dropped the screen-row anchor when `s` shows content. The `reveal(.., Reveal::InPlace)` in `folding.rs` `toggle_system` (the `else if let (Some(size), Some(cur))` branch) is now the only thing that keeps the selected section on screen, which SPEC §6.3 requires and both round-1 reviewers treated as required.
   - Why it misses: in the test the system sections are folded, so showing them pushes the `t6` prompt down by only 2 rows (49..50 → 51..52). The viewport is rows 40..52, so the prompt stays on screen and the reveal never does anything. In a scratch copy, replacing the reveal with a no-op left all 161 transcript tests passing.
   - Evidence that the reveal matters: with system content unfolded, the result differs.
     - Before `s`: offset 82, prompt at 91..92.
     - After `s` without the reveal: offset 82, prompt at 96..97, off screen (viewport 82..94).
     - After `s` with the reveal: offset 87, prompt on screen.
   - Change: in that test, add `ch(&mut t, '+');` right after `let mut t = session_transcript(80, 12).show_system(false);` and before `t.select(&section_id("t6", SectionKey::Prompt))`. `fold_all` also unfolds hidden system sections. Keep `ch(&mut t, 's')` and `assert_selection_on_screen(&mut t, 12)`. Confirm the test fails when the reveal line in `toggle_system` is removed.
2. [nit] `src/components/transcript/tests/keys.rs`, `g_on_a_tall_last_section_still_shows_its_end`.
   - The test does not exercise its title. In a scratch copy I removed the `rows.len() <= height &&` guard in `paging.rs` `jump_to_end`, and all transcript tests still pass. The selected last section `r1/t1` is evidently not taller than the viewport.
   - The test also has a dead `let _ = tall;`.
   - Change: build a transcript whose *last* section is taller than the viewport, press `G`, and assert that the offset is `total - height`, i.e. that the section's end is shown. Then check that the test fails without the guard. Use `let (mut t, _) = ...` instead of `let _ = tall;`.
3. [nit] `src/components/transcript/paging.rs` `select_in_view` and `turn_stop`: the two round-1 interpretation notes still apply and are unchanged.
   - A selected section taller than the viewport is never "fully visible", so page keys move the selection away from it.
   - `]`/`[` select the block's first non-system section.

   No change is required; this is listed only so the Approver sees it.
