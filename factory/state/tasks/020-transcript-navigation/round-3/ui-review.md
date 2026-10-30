VERDICT: PASS

## Summary
The round-3 fix (`Reveal::InPlace` in `jump_turn`) works as intended. `]`/`[` into a block whose first section is taller than the viewport now keep the `╭─ <Kind>` top border on row 0 with the selection visible below it. The round-2 behaviour, where a prompt pushed below the viewport by an unfolded system section is scrolled into view, still holds. No `.snap` file changed this round, and all nine snapshots for the `UI` section still pass. Gate: `cargo fmt --check` ok, `cargo clippy --all-targets -- -D warnings` ok, `INSTA_UPDATE=no cargo test` ok (lib 1046 passed, 0 failed, no `.snap.new`).

## States checked
- After `j` x3, `▒` on a section of the second block @ 80x24 — snapshot `after_j_three_times_the_selection_is_in_the_second_block_at_80x24_text` — ok (unchanged, passes)
- Paging through a tall section after the 1st and 2nd `j` @ 80x24 — snapshots `paging_through_a_tall_section_after_the_first_j_at_80x24_text`, `..._after_the_second_j_at_80x24_text` — ok
- After `]` x2, the third block's top border on row 0 @ 80x24 — snapshot `after_the_right_bracket_twice_the_third_block_is_at_the_top_at_80x24_text` — ok
- After `Ctrl-d`, selection on the topmost fully visible section @ 80x24 — snapshot `after_ctrl_d_the_selection_is_the_topmost_fully_visible_section_at_80x24_text` — ok
- After `+` / after `-` @ 120x40 — snapshots `after_plus_everything_is_unfolded_at_120x40_text`, `after_minus_everything_is_folded_at_120x40_text` — ok
- Default vs after `s` @ 80x24 — snapshots `with_system_sections_shown_by_default_at_80x24_text`, `after_s_the_system_sections_are_hidden_at_80x24_text` — ok
- `]`/`[` into blocks whose first section is taller than the viewport (a 30-line assistant text and a 30-line prompt, unfolded with `+`) @ 80x6, keys `+ ] j j ] j G [ [ [ [`. Rendered in a scratch crate with a synthetic copy of `session.jsonl`. Every jump puts `╭─ Assistant` / `╭─ User` on row 0 with `▒` on the section rows below it. `j` then pages through the tall section and moves on, and `[` back into the tall blocks does the same — ok. See `ui/turn-jump-tall-first-section-80x6.txt`.
- `+`, `]`, `]`, `[`, `]` in `injected.jsonl` @ 80x12 (round-1 must-fix regression check). Rendered in the scratch crate: the selected prompt "Now run the tests." is on screen with `▒` after every jump, unchanged from round 2 — ok
- Sweep of `+`, `]` x8, `G`, `[` x8 over `session.jsonl`, `injected.jsonl` and `compaction.jsonl` @ 80x6, 80x8 and 80x12. Rendered in the scratch crate: the selection is on screen in every frame. Row 0 is the target block's top border whenever the block fits and the end of the transcript allows it. The only exceptions are prompts that an unfolded leading system section pushes down; those are revealed minimally, as the round-2 verdict decided — ok

## Findings
None.
