VERDICT: REWORK

## Assessment
- **Code review: FAIL.**
  - Finding 1 [must-fix], no test for the `self.user_flushed = false;` reset in `start_user_turn` (`src/harness/claude/assemble.rs:471`): **to be fixed.**
    - The behaviour at HEAD is correct.
    - This reset is the only thing that keeps the pre-prompt and post-prompt `Injected` items apart in every User turn after the first, which covers nearly every real session.
    - Two reviewers each deleted the line on their own, and the whole suite still passed. So the round-1 fix has no regression guard for its common case.
    - The build notes say the reset was mutation-checked. That claim is wrong, so it can't be accepted as cover.
    - Not minor enough to ignore.
  - Finding 2 [nit], the `into_turns` doc comment (around line 147) says held entries "are placed in the current one": **to be fixed.**
    - The comment is factually wrong today. With no turn yet, or a Compaction turn last, the entries are dropped and logged at debug.
    - The fix is one sentence, and the rework round is needed anyway.
  - Finding 3 [nit], the `end_of_input` debug message:
    - The grammar part ("1 injected entries") is **to be fixed**, since it is trivial.
    - The repetition on every `flush_held()` is **optional**. It is debug level only, and `flush_held` and `finish` both go through `end_of_input`. Restructuring that is not worth mandating.
- **Feature review: FAIL.**
  - Finding 1 [must-fix]: the same test gap as code finding 1, **to be fixed.** The reviewer supplied a test and confirmed that it passes at HEAD and fails with the reset removed. AC 1 stays unchecked until it is in.
  - Finding 2 [nit]: the same as code finding 2, **to be fixed.**
  - Finding 3 [nit]: the same as the repetition part of code finding 3, **optional.**
  - Finding 4 [nit], unknown blocks on a re-sent assistant line counted once per re-send: **ignored.** The impact is low, real data shows 0 unparsed entries over all local sessions, and both reviewers accepted that the builder declined it.
- **UI review: PASS.** The task is `UI: none`, and there are no findings.
- Round-1 nits 6, 7 and 9 were declined with reasons, and both reviewers accepted that. **Ignored.**

## Conflicts
None. The code and feature reviews agree on the must-fix and on the doc-comment and logging nits. The UI review has no findings.

## Rework budget
Earlier REWORK verdicts: 0 of 2. There is no `verdict.md` in `round-1`. This verdict uses the first.

## Rework note
The behaviour is correct. This round adds a missing regression test and fixes two small doc and log issues. Do not change the placement logic.

1. **Required: test for the `user_flushed` reset.**
   - Add this test to `tests/claude_injected.rs` next to `entries_before_and_after_a_prompt_stay_in_separate_items`. The existing test only covers the first User turn, where `user_flushed` is still `false` from `new()`, so it cannot catch a missing reset.

     ```rust
     #[test]
     fn entries_before_and_after_a_later_prompt_stay_in_separate_items() {
         let text = lines(&[
             prompt("u1", "one"),
             hook("a1", "A"),
             reply("r1", "msg_1", "hi"),
             hook("x2", "X"),
             prompt("u2", "two"),
             hook("b2", "B"),
             reply("r2", "msg_2", "ho"),
         ]);
         let turns = assemble(&text).into_turns();
         let items = &user(&turns[2]).items;
         assert_eq!(item_names(items), ["injected", "injected"]);
         assert_eq!(injected(&items[0])[0].text, "X");
         assert_eq!(injected(&items[1])[0].text, "B");
     }
     ```

   - **Done when:**
     - The test passes at HEAD.
     - It fails when you temporarily delete `self.user_flushed = false;` from `start_user_turn` in `src/harness/claude/assemble.rs`. Run that mutation check, then restore the line.
   - In the build notes, report only mutation checks you actually ran, and what each one showed.
   - Optional: a live variant that calls `flush_held()` after `hook("b2", "B")` and asserts the same result.
2. **Required: correct the `into_turns` doc comment** in `src/harness/claude/assemble.rs` (around line 147). It currently says held entries "are placed in the current one". Make it say:
   - held entries go into the current User or Assistant turn if there is one;
   - with no turn yet, or a Compaction turn last, they are dropped and logged at debug.
3. **Required: fix the grammar of the `end_of_input` debug message.** Write "1 injected entry" and "N injected entries" according to the count.
   - Optional: avoid repeating this message on every `flush_held()` while the same entries stay held, for example by logging only when the held count changes. Do not restructure `flush_held`, `finish` or `end_of_input` for this.
4. **Out of scope:** do not change the re-sent unknown-block counting (feature nit 4) or round-1 nits 6, 7 and 9.
5. **Gate:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` all pass.
