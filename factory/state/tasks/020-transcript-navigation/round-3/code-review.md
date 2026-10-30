VERDICT: PASS

## Summary
Round 3 does exactly what the rework note asks and nothing more. `jump_turn` now uses `Reveal::InPlace`, so `]`/`[` into a block whose first section is taller than the viewport keeps the `╭─` border on the top row. The doc comment is updated. The three weak tests now exercise what their names claim, and a new `App::update` test pins the `TranscriptChanged` redraw contract. I re-ran all three required mutation checks, plus one for the new redraw test, in a scratch copy of HEAD, and each one makes its target test fail (for the three transcript mutations, 162 passed and 1 failed). No `.snap` file changed since round 2, and the test gate passes.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (lib 1046 passed, 0 failed, 1 ignored; all other suites pass; run with `INSTA_UPDATE=no`; working tree clean afterwards, no `.snap.new` files)

## Round-2 findings
- Code-review 1 (must-fix, `Reveal::Top` regression): fixed at `src/components/transcript/paging.rs:62`. Covered by `a_jump_into_a_block_with_a_tall_first_section_keeps_the_top_border_on_top`, which checks both `]` and `[` for the selection, the offset (equal to the block top) and a `╭` on row 0. The round-2 test `a_jump_past_a_tall_unfolded_system_section_keeps_the_prompt_on_screen` still passes.
- Code-review 2 (nit, dead `let _ = tall;`): removed.
- Code-review 3 (nit, no update test for `Msg::TranscriptChanged`): added `a_transcript_change_asks_for_a_redraw` in `src/app/tests.rs`.
- Code-review 4 (nit, `s` keeping the offset when showing): left unchanged on purpose, as the verdict says.
- Feature-review 1 (must-fix, a test that could not fail): `showing_system_content_scrolls_a_pushed_down_selection_into_view` now presses `+` first. Mutation-verified.
- Feature-review 2 (nit, tall last section): `g_on_a_tall_last_section_still_shows_its_end` now builds a 35-row last section in a 10-row viewport, asserts that the section is taller than the viewport, and checks `offset == total - 10`. Mutation-verified.

## Mutation checks (independently reproduced in a scratch copy of HEAD)
- `Reveal::InPlace` changed to `Reveal::Top` in `jump_turn`: only `a_jump_into_a_block_with_a_tall_first_section_keeps_the_top_border_on_top` fails.
- The `reveal(..)` line removed from the show branch of `toggle_system`: only `showing_system_content_scrolls_a_pushed_down_selection_into_view` fails.
- `rows.len() <= height &&` removed from `jump_to_end`: only `g_on_a_tall_last_section_still_shows_its_end` fails.
- (Extra, not in the rework note) the `Msg::TranscriptChanged` arm in `src/app/mod.rs` made a no-op: `a_transcript_change_asks_for_a_redraw` fails.

## Findings
1. [nit] `src/components/transcript/tests/keys.rs`, `g_on_a_tall_last_section_still_shows_its_end`: the test builds the tall `a1` block with the same `arc(with_id(blocks::assistant(.., blocks::text("big", &tall_text_of(35)) ..), "a1"))` expression that `tall_section_transcript()` uses. Change, if wanted: extract a small `tall_assistant_block()` helper and use it in both places.
