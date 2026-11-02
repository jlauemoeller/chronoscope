STATUS: DONE

## Changes
- `tests/claude_injected.rs`: added `entries_before_and_after_a_later_prompt_stay_in_separate_items` (the test from the approver note) and a live variant, `entries_before_and_after_a_later_prompt_stay_apart_across_a_live_flush` (`flush_held()` after the second prompt's hook). 47 tests in the file.
- `src/harness/claude/assemble.rs`: `into_turns` doc comment now says held entries go into the current User or Assistant turn if there is one and are otherwise dropped (logged at debug). The `end_of_input` debug message uses "1 injected entry has" / "N injected entries have". Placement logic untouched. The repetition on every `flush_held()` is left as is (optional).
- `tests/claude_injected.rs`: the debug-log test expects the new wording.

## Self-check
- Review (required 1): both new tests pass at HEAD. Mutation check actually run: deleted `self.user_flushed = false;` from `start_user_turn`; result: exactly the two new tests failed (`entries_before_and_after_a_later_prompt_stay_in_separate_items`, `entries_before_and_after_a_later_prompt_stay_apart_across_a_live_flush`), 45 others passed. The line was then restored (file copied back; `git diff` shows only the intended changes). Correction: the round-2 claim that the reset was mutation-checked was wrong; my earlier check set it to `true` instead of removing it, which only the first-turn test caught. No other mutation checks were run this round.
- Review (required 2): `into_turns` doc comment corrected.
- Review (required 3): grammar fixed, covered by `entries_left_without_a_turn_are_logged_at_debug_level` (asserts "1 injected entry has no turn").
- Out of scope items untouched.
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (283 passed, 0 failed)
