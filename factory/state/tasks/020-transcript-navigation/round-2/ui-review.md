VERDICT: PASS

## Summary
The round-1 must-fix is fixed. `]`/`[` into a User block whose leading system section is unfolded and taller than the viewport now scroll the selected prompt into view. When the prompt fits, the block's top border still lands on the top row. Round-1 nits 2 and 3 are also addressed: showing system content keeps the transcript top on screen, `G` in a 1-2 row viewport shows the selected section, and a half page is at least one row. All nine snapshots for the `UI` section are unchanged and still pass. The gate passes: `cargo fmt --check` ok, `cargo clippy --all-targets -- -D warnings` ok, `INSTA_UPDATE=no cargo test` ok (lib 1044 passed, 0 failed, no `.snap.new`).

## States checked
- After `j` x3, `▒` on a section of the second block @ 80x24 — snapshot `after_j_three_times_the_selection_is_in_the_second_block_at_80x24_text` — ok
- Paging through a tall section after the 1st and 2nd `j` @ 80x24 — snapshots `paging_through_a_tall_section_after_the_first_j_at_80x24_text`, `..._after_the_second_j_at_80x24_text` — ok
- After `]` x2, third block's top border on row 0 @ 80x24 — snapshot `after_the_right_bracket_twice_the_third_block_is_at_the_top_at_80x24_text` — ok (still passes with the new reveal)
- After `Ctrl-d`, selection on the topmost fully visible section @ 80x24 — snapshot `after_ctrl_d_the_selection_is_the_topmost_fully_visible_section_at_80x24_text` — ok
- After `+` / after `-` @ 120x40 — snapshots `after_plus_everything_is_unfolded_at_120x40_text`, `after_minus_everything_is_folded_at_120x40_text` — ok
- Default vs after `s` @ 80x24 — snapshots `with_system_sections_shown_by_default_at_80x24_text`, `after_s_the_system_sections_are_hidden_at_80x24_text` — ok
- `+`, `]`, `]`, `[`, `]` in `injected.jsonl` @ 80x12 (round-1 must-fix 1). Rendered in scratch crate: the prompt "Now run the tests." is on screen with `▒` every time — ok (nit 1). See `ui/turn-jump-past-expanded-injected-80x12.txt`.
- `+`, `G`, `[` x3 in `injected.jsonl` @ 80x8. Rendered in scratch crate: each target's prompt or first section is on screen, and top borders are on row 0 where the block fits — ok
- `G`, `g`, `]`, `]`, `Ctrl-d`, `Ctrl-u`, `PgDn` @ 80x1 and `G`, `]`, `[` @ 80x2. Rendered in scratch crate: `G` now shows `▒ Yes.`, and `Ctrl-d` scrolls 1 row — ok
- `show_system = false` start, then `s`, `s`, `j j`, `s` in `session.jsonl` @ 80x12. Rendered in scratch crate: the view stays at offset 0 and the `User` title stays on screen (round-1 nit 2) — ok
- `G`, `s`, `s`, `+`, `s`, `s` in `injected.jsonl` @ 80x12. Rendered in scratch crate: hiding keeps the selected footer's screen row, and showing scrolls only as far as needed to keep the footer on screen — ok

## Findings
1. [nit] Transcript, `]`/`[` into a User block whose unfolded system section pushes the prompt below the viewport, `injected.jsonl` after `+ ] ]` @ 80x12. `jump_turn` passes `Reveal::Top`, but `reveal` uses that only for spans taller than the viewport. For a short prompt it scrolls minimally, so the prompt lands on the last content row of the screen. The reply that follows is entirely below the viewport, and the user has to page on to read the turn they jumped to. This is correct per the spec ("scrolls it into view") and is cosmetic. Change, if wanted: in this fallback, put the selected section's first row at the top of the viewport (`self.offset = span.start`, clamped by `refresh`) instead of the minimal reveal, so the prompt and the content after it are visible. Alternatively, fix the doc comment so it doesn't suggest top placement. See `ui/turn-jump-past-expanded-injected-80x12.txt`.
