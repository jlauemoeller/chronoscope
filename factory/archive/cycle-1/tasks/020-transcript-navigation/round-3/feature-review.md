VERDICT: PASS

## Summary
Round 3 does all five rework items and changes nothing else. `jump_turn` now uses `Reveal::InPlace`, so `]`/`[` into a block whose first section is taller than the viewport keep the `╭─` border on the top row. Three tests were strengthened or added, plus an update test for `Msg::TranscriptChanged`. I re-ran the three required mutations in a scratch copy. Each one makes exactly its target test fail (162 passed, 1 failed in each case). The gate passes, and no `.snap` file changed this round.

## Acceptance criteria
- [x] `j`/`↓`, `k`/`↑` move across sections and blocks, and a section taller than the viewport is paged through first. Implemented in `nav.rs` `move_selection` / `page_through`. Covered by `three_presses_scroll_by_whole_pages_through_a_three_page_section` and `three_presses_page_through_a_tall_section_and_the_fourth_selects_the_next_one`. Unchanged this round.
- [x] `]`/`[` jump between User/Assistant/Compaction blocks, never stop on system content, and put the top border at the viewport top when possible. Implemented in `paging.rs` `jump_turn` / `turn_stop`; the fallback is now `Reveal::InPlace`.
  - `nav.rs` `reveal` keeps the block-top offset when a tall section's first row is on screen. For sections that fit the viewport it reveals minimally, as before.
  - New test `a_jump_into_a_block_with_a_tall_first_section_keeps_the_top_border_on_top` covers both `]` (from `p1`) and `[` (from `r1` after `G`). It asserts the selection is `a1/big`, the offset equals `top(1)`, and row 0 starts with `╭`. It fails with `Reveal::Top` (mutation confirmed).
  - The round-2 test `a_jump_past_a_tall_unfolded_system_section_keeps_the_prompt_on_screen` still passes.
- [x] `Ctrl-d`/`Ctrl-u` scroll half a page and `PgDn`/`PgUp` a full page, clamped, and the selection moves to the topmost fully visible section. Implemented in `paging.rs` `scroll_pages` / `select_in_view`. Covered by the round-1 and round-2 tests. Unchanged this round.
- [x] `g`/`Home` and `G`/`End` select the first/last section and scroll to the start/end. Implemented in `paging.rs` `jump_to_start` / `jump_to_end`.
  - `g_on_a_tall_last_section_still_shows_its_end` now uses a last section of 35 rows in a 10-row viewport. It asserts the section is taller than the viewport, that `G` selects it, and that `offset == total - 10`.
  - It fails when the `rows.len() <= height &&` guard is removed (mutation confirmed). The dead `let _ = tall;` is gone.
- [x] `Enter`/`Space` toggle, `←`/`h` fold, `→`/`l` unfold, `-` folds all and `+`/`=` unfold all. Non-foldable sections ignore them, and the screen row is kept. Implemented in `folding.rs`. Covered by the round-1 and round-2 tests. Unchanged this round.
- [x] `s` hides and shows system sections, `show_system = false` starts them hidden, and a hidden selected system section moves to a neighbour. Implemented in `folding.rs` `toggle_system` and `updates.rs` `set_show_system` / `restore_selection`.
  - `showing_system_content_scrolls_a_pushed_down_selection_into_view` now presses `+` first, so showing the content pushes `t6` off screen.
  - It fails when the reveal in the show branch is a no-op (mutation confirmed). This resolves round-2 must-fix 1.
- [x] Remapping: `[keys.browser] next = "J"` makes `J` move and `j` do nothing. Covered by `a_remapped_key_works_and_the_old_one_does_nothing`. Unchanged.
- [x] Every UI state has a snapshot. The nine `.snap` files in `key_snapshots.rs` exist and were not touched this round; `git show --stat HEAD` lists no `.snap` file.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass: lib 1046 passed, 0 failed, all integration suites ok. Separately, `a_transcript_change_asks_for_a_redraw` in `src/app/tests.rs` pins the `TranscriptChanged` → `RedrawRequest::Dirty` contract.

## Findings
None.
