VERDICT: APPROVE

## Assessment
**Code review: PASS.**
- Every item of the round-2 rework note (copied into `round-3/approver-review.md`) is done:
  - `jump_turn` uses `Reveal::InPlace`, and its doc comment is updated.
  - The new `]`/`[` regression test is in place.
  - The `+` was added to `showing_system_content_scrolls_a_pushed_down_selection_into_view`.
  - `g_on_a_tall_last_section_still_shows_its_end` was rewritten with a 35-row last section in a 10-row viewport, and the dead binding is gone.
  - The `Msg::TranscriptChanged` update test was added.
- The reviewer reproduced all three required mutation checks on their own, plus one more for the redraw test.
- [nit] The tall `a1` assistant block is built inline in `g_on_a_tall_last_section_still_shows_its_end` with the same expression `tall_section_transcript()` uses; the reviewer suggests a `tall_assistant_block()` helper. **Ignored.** This is test-only duplication of a single expression, it has no effect on behaviour or coverage, and the reviewer marks the change as optional.

**Feature review: PASS.** Every acceptance criterion is checked off, and each round-3 test has a mutation check that makes it fail. No findings.

**UI review: PASS.**
- All nine snapshots are unchanged and pass.
- A scratch render at 80x6 (`ui/turn-jump-tall-first-section-80x6.txt`) shows that `]`/`[` into blocks whose first section is tall keep `╭─ <Kind>` on row 0.
- The round-1/2 check (a prompt pushed down by unfolded injected content) still holds.
- No findings.

**My own check.**
- The round-3 code diff touches only these lines:
  - the one-line `Reveal::Top` → `Reveal::InPlace` change and the doc comment in `src/components/transcript/paging.rs`;
  - the tests in `src/components/transcript/tests/keys.rs` and `src/app/tests.rs`.
- No `.snap` file changed in round 3.
- `INSTA_UPDATE=no cargo test` passes: lib 1046 passed, 0 failed, 1 ignored, and every other suite is ok. The working tree is clean afterwards.

Findings that earlier rounds deliberately left unchanged are still unchanged, as decided in the round-2 verdict:
- the minimal reveal for a pushed-down prompt on `]`;
- `s` keeping the offset when it shows system content;
- page keys moving the selection away from a tall selected section;
- `]`/`[` selecting a block's first non-system section.

None of them is a blocker.

## Conflicts
None. All three reviews pass, and the only remaining finding is an optional test-helper nit.

## Rework budget
Earlier REWORK verdicts: 1 of 2 (round 2).
