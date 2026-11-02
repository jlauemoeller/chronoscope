VERDICT: REWORK

## Assessment
**Code review: FAIL.**
1. [must-fix] `jump_turn` uses `Reveal::Top`, so `]`/`[` into a block whose first section is taller than the viewport drops the block's top border. **To be fixed.**
   - I confirmed it from the code. In `nav.rs`, `reveal(.., Reveal::Top)` returns `span.start` unconditionally for a span taller than the viewport. With `tall_section_transcript()` (80x10), `]` from `p1` first sets the offset to the block top (5) and then moves it to the start of `a1/big` (7). Row 0 is then `▒ • item 1`, not `╭─ Assistant`.
   - Round 1 kept the border here, so this is a regression against AC2 ("top border at the viewport top when possible"). It is common in practice: any long prompt or unfolded assistant text at 80x24 triggers it.
   - The `Reveal::Top` came from round-1 code-review's prescription. Round-1 UI review had suggested `InPlace`.
2. [nit] Dead `let _ = tall;` in `g_on_a_tall_last_section_still_shows_its_end`. **To be fixed** together with feature nit 2, since the test is rewritten anyway.
3. [nit] There is no `App::update` test for `Msg::TranscriptChanged`. **To be fixed.** It is cheap, and it pins the redraw contract that task 021 will rely on.
4. [nit] `toggle_system` keeps the offset when it shows system content, so content mid-transcript is pushed down. **Ignored.**
   - The spec does not say what should happen here.
   - The current behaviour was round-1 UI review's own suggested option, and round-2 UI review checked it and passed it.
   - Re-anchoring would reopen round-1 UI nit 2 (the first block's title dropping off the top).

**Feature review: FAIL.**
1. [must-fix] `showing_system_content_scrolls_a_pushed_down_selection_into_view` cannot fail. The system sections are folded, so the `t6` prompt stays on screen and the reveal in `toggle_system`'s show branch is never exercised. **To be fixed:** unfold everything first so that the reveal is needed.
2. [nit] `g_on_a_tall_last_section_still_shows_its_end` does not test its title, because its last section (`r1/t1`) is short. **To be fixed** with a fixture whose last section is taller than the viewport.
3. [nit] Two interpretation notes, both unchanged since round 1. **Ignored**, because both follow the binding planner notes and the reviewer asked for no change.
   - Page keys treat a tall selected section as "not fully visible".
   - `]`/`[` select a block's first non-system section.

**UI review: PASS.**
1. [nit] When `]` has to reveal a prompt that an unfolded system section pushed off screen, the prompt lands on the bottom row rather than the top. **Ignored.**
   - The reviewer calls this spec-correct and cosmetic.
   - Minimal reveal is how the rest of this component scrolls a selection into view.
   - This round should stay narrow: an added prescription in round 1 is what caused this round's regression.
   - The reviewer's other option, a doc comment that doesn't suggest top placement, is covered by the rework note.

## Conflicts
The reviews do not really conflict. The closest case is code-review finding 1 against UI nit 1:
- Code review asks for `Reveal::InPlace`, which reveals a short off-screen prompt minimally.
- UI review would put that prompt at the top of the viewport instead.

Code correctness ranks above UI, and the UI item is a cosmetic nit, so the placement change is dropped and `InPlace` with minimal reveal stays.

## Rework budget
Earlier REWORK verdicts: 0 of 2. Neither round 1 nor round 2 had a `verdict.md` before this one.

## Rework note
Fix a regression in `]`/`[`, and make three tests actually test what their names claim. Keep this round narrow: change nothing beyond the items below.

1. **`]`/`[` must keep the block's top border on top when the jumped-to section is tall.**
   - **Bug.** In `src/components/transcript/paging.rs` `jump_turn`, the fallback calls `reveal(self.offset, height, &span, Reveal::Top)`. For a section taller than the viewport, `Reveal::Top` always scrolls to the section's first row, even when that row is already on screen below the block's top border. So `]` into a block whose first section is tall (a long prompt or an unfolded long assistant text) scrolls the `╭─ <Kind>` border and title off screen. This breaks AC2: "top border at the viewport top when possible".
   - **Change.** Pass `Reveal::InPlace` instead of `Reveal::Top`. `InPlace` keeps the block-top offset when the selected section's first row is on screen, and otherwise scrolls the section's top into view. For sections that fit the viewport it behaves exactly like `Top`, so the round-2 fix for a prompt pushed down by an unfolded system section still holds. Keep the minimal reveal for a short off-screen prompt; do not move it to the top row.
   - **Doc comment.** Update the comment on `jump_turn`: the block's top border goes on the top row, and the selected section is scrolled into view only when it would otherwise be off screen.
   - **Regression test** in `src/components/transcript/tests/keys.rs`, using `tall_section_transcript()` (80x10). With `p1` selected, press `]` and assert:
     - the selection is `item_section("a1", "big")`;
     - `t.offset() == t.measured(|c, m| c.top(1, m))`;
     - row 0 of the drawn frame starts with `╭`.

     Then press `G` (selects `r1`) and `[` back to `a1`, and assert the same three things.

2. **Make `showing_system_content_scrolls_a_pushed_down_selection_into_view` able to fail.**
   - **Problem.** The system sections are folded there, so showing them moves the `t6` prompt by only 2 rows and it never leaves the viewport. The reveal in `src/components/transcript/folding.rs` `toggle_system` (show branch) is never exercised.
   - **Change.** Add `ch(&mut t, '+');` right after `let mut t = session_transcript(80, 12).show_system(false);` and before the `t.select(...)`. `+` unfolds hidden system sections too. Keep `ch(&mut t, 's')` and `assert_selection_on_screen(&mut t, 12)`.

3. **Make `g_on_a_tall_last_section_still_shows_its_end` test a tall last section.**
   - **Problem.** Its last section (`r1/t1`, "Done.") is short.
   - **Change.**
     - Build a transcript whose last section is taller than the viewport, e.g. a prompt followed by the tall `a1` assistant block from `tall_section_transcript()` as the last turn, at 80x10.
     - Select the first section, press `G`, and assert that the selection is that tall last section and that `t.offset() == total_height(&mut t) - 10`, i.e. its end is shown.
     - Remove the dead `let _ = tall;` binding.

4. **Add an update test for the redraw contract.** In `src/app/tests.rs`, call `app.take_redraw()` to clear any pending request, then `app.update(Msg::TranscriptChanged)`, and assert that `app.take_redraw() == RedrawRequest::Dirty`.

5. **Mutation check (required).** Show in the build notes that each fixed or new test fails when its fix is reverted. Revert each fix temporarily, one at a time, and restore it afterwards:
   - `Reveal::InPlace` back to `Reveal::Top` in `jump_turn`: the new test in item 1 fails;
   - the `reveal(..)` line in `toggle_system`'s show branch removed: the test in item 2 fails;
   - the `rows.len() <= height &&` guard in `jump_to_end` removed: the test in item 3 fails.

**Done when:**
- the items above are in place and the mutation checks are reported;
- no existing `.snap` file changes;
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass.

**Deliberately not changed** (the reviewers raised these; leave them as they are):
- **Prompt placement on `]`.** A prompt revealed by `]` stays where the minimal reveal puts it (possibly the bottom row). The UI reviewer's suggestion to put it at the top is cosmetic and spec-correct either way.
- **`s` when showing system content.** It keeps the current offset; do not re-add the screen-row anchor. The spec doesn't specify this, and anchoring would hide the transcript's top block title again.
- **Page keys** keep moving the selection away from a selected section taller than the viewport (binding planner rule).
- **`]`/`[`** keep selecting a block's first non-system section.
