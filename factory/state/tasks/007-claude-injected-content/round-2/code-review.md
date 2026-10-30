VERDICT: FAIL

## Summary
The round-1 must-fix is fixed correctly. In `flush_current`, entries flushed into a User turn after its prompt now merge into a trailing `Injected` item added by an earlier post-prompt flush. The `user_flushed` flag keeps the pre-prompt item separate, so a live batch boundary gives the same turns as parsing the whole file. I traced the flag through compaction, orphan summaries and open Assistant turns and found no path where a stale `true` reaches the User branch. Round-1 nits 2, 3, 4, 5 and 8 are addressed; 6, 7 and 9 were declined with reasons, which is acceptable. One problem remains: no committed test checks the flag reset in `start_user_turn`. The build notes say a mutation check covered it, but that check does not hold.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [must-fix] `tests/claude_injected.rs`, `entries_before_and_after_a_prompt_stay_in_separate_items`: the reset `self.user_flushed = false;` in `start_user_turn` (`src/harness/claude/assemble.rs:471`) has no test.
   - **Why it matters:** this reset keeps entries before and after a prompt in separate `Injected` items on every User turn after the first, which covers almost every real session.
   - **Why the existing test misses it:** it covers only the first User turn, where `user_flushed` is still `false` from `new()`.
   - **Evidence:**
     - Scratch copy of HEAD with line 471 deleted: all 45 committed tests in `claude_injected` still pass, and the fixture tests do not catch it either.
     - A probe with the sequence `prompt u1, hook a1 "A", reply r1, hook x1 "X", prompt u2, hook b1 "B", reply r2` shows the difference.
       - HEAD: the second User turn has `[Injected[X], Injected[B]]`, which is correct.
       - With the reset removed: `[Injected[X, B]]`. The post-prompt entry wrongly merges into the pre-prompt item.
     - The build-notes claim "Mutation-checked: disabling the merge or the flag reset makes them fail" does not hold for the reset.
   - **Change:** add a test, or extend this one, that feeds the sequence above through `assemble(...).into_turns()`. Assert that:
     - `item_names(&user(&turns[2]).items) == ["injected", "injected"]`;
     - `injected(&items[0])` has the single text `"X"`;
     - `injected(&items[1])` has the single text `"B"`.

     Confirm that this test fails when `self.user_flushed = false;` is removed from `start_user_turn`.
2. [nit] `src/harness/claude/assemble.rs:147-148`, doc comment of `into_turns`: "Injected entries still waiting for a turn are placed in the current one" is wrong when there is no turn yet or a Compaction turn is last. Those entries are dropped, and now logged at debug. Reword it, e.g. "…placed in the current turn; with no turn yet, or a Compaction turn last, they are dropped (logged at debug)".
3. [nit] `src/harness/claude/assemble.rs:541-546`, `end_of_input`:
   - In live mode, the debug message repeats on every `flush_held()` for as long as entries stay held after a compaction.
   - "1 injected entries" is ungrammatical.

   Low priority. Consider logging only from `finish()` / `into_turns()`, or only when the held count changes, and writing "injected entr{y,ies}" with the right number.
