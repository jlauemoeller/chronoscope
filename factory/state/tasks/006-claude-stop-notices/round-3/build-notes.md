STATUS: DONE

## Changes
- `src/harness/claude/assemble.rs`:
  - `remove` now renews positions through a small helper `shift_after`, and when the removal leaves two `Item::Injected` side by side it merges the second into the first (first id kept, second's id dropped from `Builder.index`, positions in `b.index` and `calls` renewed a second time).
  - `error_block_key` (no `apiBlockIndex`) looks first for a stored block with exactly the same text, then for one the text extends, skipping blocks numbered on the same line.
- `src/harness/claude/wire.rs`: comments only. `read_entry`, `lenient_entry` and `MessageWithoutUsage` now name the fields the check parse skips (`usage`, `message.stop_reason`, both `isApiErrorMessage` fields) and say that leaving them out of `MessageWithoutUsage` is deliberate.
- `tests/claude_notices.rs`: `meta_line` helper, the tests from the rework note, and `busy_transcript` extended with the flag-after-text, distinct-block and meta/merge sequences.
- Not changed: `take_message_text`, per-turn `stop`, empty `stop_reason`, stop-reason logic, wire leniency, views, snapshots.

## Self-check
- Note 1: `a_call_sent_again_after_a_removed_notice_replaces_itself`, `a_text_sent_again_after_the_text_taken_into_an_error_replaces_itself`.
- Note 2: `injected_items_a_removal_leaves_side_by_side_collapse`, `injected_items_collapse_before_a_notice_that_comes_after_them`, `a_merged_injected_item_keeps_a_later_call_and_its_result_attachable`.
- Note 3: `an_exact_repeat_is_preferred_over_a_shorter_block_it_extends`; the distinct, repeated and grown tests still pass.
- Note 4: `any_chunking_gives_the_same_final_turns_with_notices` (ids `msg_e`, `msg_2`, `msg_f`, `msg_d`, exactly one `Injected` with 2 entries).
- Note 5: doc comments in `wire.rs` updated, code untouched.
- Mutation checks (done by temporarily editing the code, then restored from a backup copy; the final diff has no trace of them):
  - `b.index` loop deleted: the two note-1 tests fail (and the merged-call test).
  - merge removed: the first two note-2 tests fail (plus the third and the chunking test).
- UI: none. No `.snap` changed, no `.snap.new` left.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (2659 passed, 0 failed)
- INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture: pass (full parse worst 460 ms vs 2000 ms, first content 2.1 ms, memory 0.58x, worst frame 5.4 ms)
