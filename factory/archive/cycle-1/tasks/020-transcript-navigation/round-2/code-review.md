VERDICT: FAIL

## Summary
Round 2 fixes most round-1 findings cleanly: `Msg::TranscriptChanged` gives the redraw contract, the screen-row test is no longer vacuous, the visibility and duplication nits are resolved, `turn_stop` derives sections once, and there are new tests for tiny viewports and the `s` toggle. The test gate passes. However, the new reveal in `jump_turn` uses `Reveal::Top`, which introduces a regression. `]`/`[` into a block whose first section is taller than the viewport now scrolls past the block's top border and title, even though both fit together with the start of the section. This breaks AC2 ("top border at the viewport top when possible"). No test covers this case.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (lib 1044 passed, 0 failed, 1 ignored; all other suites pass; run with `INSTA_UPDATE=no`, no `.snap.new` files)

## Findings
1. [must-fix] `src/components/transcript/paging.rs:62` (`jump_turn`, `self.offset = reveal(self.offset, height, &span, Reveal::Top)`)
   - **Cause.** For a span taller than the viewport, `reveal(.., Reveal::Top)` always returns `span.start`, even when the section's first row is already on screen with the block's top border at the top. Round 1's code-review prescribed `Reveal::Top`, and that prescription was wrong. The Builder followed it.
   - **Effect.** Any `]`/`[` into a block whose first selectable section is taller than the viewport drops the top border and title row off screen. Examples are a long prompt or a long unfolded assistant text, which is common at 80x24.
   - **Reproduction** (scratch worktree): `tall_section_transcript()` (80x10), with `p1` selected, then `]`. The selection is `a1/big` (rows 7..42) and the block top is 5. The offset becomes 7 instead of 5, so the screen shows only `▒ • item 1` … `item 10`, with no `╭─ Assistant` border.
   - **Why the tests miss it.** The `]` snapshot, `right_bracket_and_left_bracket_jump_between_blocks_with_the_top_border_on_top` and the two new jump tests only use sections that fit the viewport.
   - **Change.**
     - Use `Reveal::InPlace` instead of `Reveal::Top` at line 62. `InPlace` keeps the block-top offset when the section's first row is on screen, and otherwise scrolls the section's top into view. For a section that fits, both modes behave the same. I verified this in a scratch worktree: with `InPlace`, all 162 transcript tests pass, including `a_jump_past_a_tall_unfolded_system_section_keeps_the_prompt_on_screen`.
     - Update the `jump_turn` doc comment to say that the selection is scrolled into view only when its first row would be off screen.
     - Add a regression test in `src/components/transcript/tests/keys.rs`. Using `tall_section_transcript()`, press `]` from `p1` and assert:
       - the selection is `item_section("a1", "big")`;
       - `t.offset() == t.measured(|c, m| c.top(1, m))`;
       - drawn row 0 starts with `╭`.

       Then select `r1` (e.g. `G`), press `[` back to `a1`, and assert the same three things.
2. [nit] `src/components/transcript/tests/keys.rs`, `g_on_a_tall_last_section_still_shows_its_end`: `let _ = tall;` is a leftover dead binding. Change: destructure as `let (mut t, _) = tall_section_transcript();` and drop the `let _ = tall;` line.
3. [nit] There is no `App::update` test for the new `Msg::TranscriptChanged` arm (`src/app/mod.rs:481`). SPEC §9.6 asks for update tests. `PromptMoved` has none either, so this is minor. Change: add a test to `src/app/tests.rs` that calls `app.update(Msg::TranscriptChanged)` after a `take_redraw()` and asserts `app.take_redraw() == RedrawRequest::Dirty`.
4. [nit] `src/components/transcript/folding.rs:149-153`, `toggle_system` when showing:
   - **Behaviour.** The offset is kept as-is. Mid-transcript, system sections that appear in blocks above the viewport therefore push the content being read down by their total height, so the view jumps. Before, the selected section kept its screen row. The selection is only revealed (at the bottom edge) if it is pushed fully off screen.
   - **Status.** This follows ui-review round 1's option "apply the anchor only when hiding", and the spec does not pin it down.
   - **Change, if wanted:** keep the screen-row anchor when showing too, but skip it (or clamp to offset 0) when the anchor would hide the top of the transcript or the selected block's top border while the newly shown rows fit above. This keeps both the round-1 UI fix and a stable view mid-transcript.
