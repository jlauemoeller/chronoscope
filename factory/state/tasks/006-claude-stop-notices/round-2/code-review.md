VERDICT: PASS

## Summary
All round-1 findings are fixed, each with a test that exercises the fix: distinct error blocks without `apiBlockIndex` are joined, invalid UTF-8 in a top-level `isApiErrorMessage` keeps the line parsed, and a flag arriving after its text no longer shows the error twice. `remove` keeps `Builder.index` and `CallRef` positions right, and the test gate and budgets pass. What remains are small edge cases and documentation, confirmed with probe tests in a scratch copy of the tree; none of them occurs in the data the spec describes.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass
- cargo test --release -- --ignored budgets --nocapture: pass (worst across both fixtures: full parse 463.6 ms against 2000 ms, first content 2.3 ms against 300 ms, memory peak 0.58x against 3x, frame 4.89 ms against 16 ms)

No `.snap` or `.snap.new` file is in the diff, and the gate wrote none.

## Findings
1. [nit] `src/harness/claude/assemble.rs:812-816` (`take_message_text`) — When the flag first appears, all earlier text of the message moves into the error, not only text equal to or a prefix of it, which is what round-1 feature finding 2 prescribed. A probe with `A` and a tool call on real-model lines, then `C` and a flagged `C!` at the same `apiBlockIndex`, gives the notice `A\nC!`, so text from unflagged lines is drawn as error text. No text is lost, and the planner note ("the message's text blocks joined with newlines", with the flag held per message id) supports this reading, so it is not a must-fix here. Change: none needed for code review; the feature reviewer decides whether the round-1 prescription still stands. If it does, keep other text as `Item::Text` and remove only equal or prefix blocks.
2. [nit] `src/harness/claude/assemble.rs:1188-1204` (`error_block_key`) — Without `apiBlockIndex` the search takes the newest stored block that the new text extends, so it can prefer a prefix over an exact match. Flagged lines `API Error: x`, `API Error`, `API Error: x`, `API Error` give the notice `API Error: x\nAPI Error: x\nAPI Error` (confirmed by probe), when `API Error: x\nAPI Error` is meant. It needs a block that is a prefix of an earlier one, re-sent in alternation. Change: look for a stored block with exactly the same text first (still skipping `taken`), and only then for one the text extends.
3. [nit] `src/harness/claude/wire.rs:257-268`, `wire.rs:273-281`, `wire.rs:246-247` — The docs on `read_entry`, `lenient_entry` and `MessageWithoutUsage` still describe only `toolUseResult` and `usage`. The check parse now also skips the top-level `isApiErrorMessage`, and `MessageWithoutUsage` also skips `message.stop_reason` and `message.isApiErrorMessage`, so "only the damage inside `usage` is forgiven" is no longer accurate. Change: name these fields in all three comments.
4. [nit] `tests/claude_notices.rs:625` (`busy_transcript`) — The chunking tests do not reach the code added this round. Change: add a flag-after-text sequence and two distinct flagged lines without an index, so these tests also cover `take_message_text` and `error_block_key`.
