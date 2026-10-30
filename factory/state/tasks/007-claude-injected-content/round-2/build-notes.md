STATUS: DONE

## Changes
- `src/harness/claude/assemble.rs`:
  - `flush_current` now collapses consecutive entries in a User turn as it does in an Assistant turn: entries flushed after the prompt merge into a trailing `Injected` item added by an earlier post-prompt flush (new `user_flushed` flag, reset by `start_user_turn`). The pre-prompt item stays separate. A live batch boundary (`flush_held`) therefore gives the same turns as parsing the whole file.
  - Unknown attachment without a `type` is counted under `<missing>`.
  - Held entries left without a turn at end of input are logged at debug.
  - `on_attachment` takes `&Entry`; comment on the Assistant-turn merge now says it deliberately goes beyond "only noise between".
- `src/harness/claude/inject.rs`: the `deferred_tools_` prefix check moved into `is_known_noise_attachment`; redundant list entries removed.
- `tests/claude_injected.rs`: 5 new tests (45 total). `tests/fixtures/README.md`: version of the real compaction samples.

## Self-check
- Review (code 1, feature 1, must-fix): User-turn collapse across a batch flush — `a_batch_flush_between_consecutive_entries_in_a_user_turn_still_collapses_them` (live batches with `flush_held` equal the whole-file parse and the replayed updates), `a_footer_between_entries_in_a_user_turn_does_not_split_them`, `entries_before_and_after_a_prompt_stay_in_separate_items`. Mutation-checked: disabling the merge or the flag reset makes them fail.
- Review (code 2): prefix check moved to `inject.rs` (covered by the noise table, which includes `deferred_tools_delta`).
- Review (code 3): `<missing>` label — `an_attachment_without_a_type_is_counted_under_a_readable_label`.
- Review (code 4, feature 2): debug log of dropped entries — `entries_left_without_a_turn_are_logged_at_debug_level`.
- Review (code 5, feature 3): comment corrected.
- Review (code 6, feature 4: unknown blocks counted per re-sent line): not changed; low impact, no real data shows it (0 unparsed over 53 sessions).
- Review (code 7): accepted as is (once per kind per session).
- Review (code 8): `&Entry`.
- Review (code 9): `structured_output` and `read_truncation_notice` are left as unknown kinds on purpose, so the log keeps flagging them for an allowlist decision.
- Review (feature 5): README now names the real sample versions.
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (281 passed, 0 failed)
