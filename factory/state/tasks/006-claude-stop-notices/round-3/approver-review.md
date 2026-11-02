## Rework note

The stop-reason logic, wire leniency, notice placement, render test and snapshots are correct. Do not change them. This round is about `remove` and `error_block_key` in `src/harness/claude/assemble.rs`, tests in `tests/claude_notices.rs`, and three doc comments in `src/harness/claude/wire.rs`.

**1. Test the `Builder.index` fix-up in `remove`.** The code is right, but nothing fails if the `for at in b.index.values_mut()` loop is deleted. Add two tests to `tests/claude_notices.rs`:
- Lines `line(1, "msg_1", r#""max_tokens""#, &text("One"))`, `line(2, "msg_1", "null", &tool_use("c1"))`, `line(3, "msg_1", r#""end_turn""#, "")`, `line(4, "msg_1", "null", &tool_use("c1"))`, `result_line(5, "c1")`. Expect no notice, exactly two items (the text and the call), and the call's status `Ok`.
- Lines `line(1, "msg_e", "null", &text("err"))`, `line(2, "msg_1", "null", &tool_use("c1"))`, `line(3, "msg_1", "null", &text("hello"))`, `api_error_line(4, "msg_e", "null", "err")`, `line(5, "msg_1", "null", &text("hello world"))`, `result_line(6, "c1")`. Expect `texts(&items) == ["hello world"]`, one `Error` notice with `err`, and the call's status `Ok`.

**2. Collapse injected items that a removal leaves side by side** (base spec §5.1 and §5.6: consecutive injected entries collapse into one section). In `remove`, after the item is taken out at `pos`: when `pos > 0`, `pos < turn.items.len()`, and the items at `pos - 1` and `pos` are both `Item::Injected`, move the second's entries onto the end of the first and take the second out of the turn. The merged item keeps the first id, as `flush_current` and `take_held` do. Drop the second's id from `b.index`. The second removal must renew positions in `b.index` and in this turn's `calls` exactly as the first does, so move that renewal into a small helper that `remove` calls for both. `*changed` stays set.

Add a `meta_line(n)` test helper that writes a `user` line with `"isMeta":true`, a `uuid`, a `timestamp` and string content, then these tests:
- `line(1, "z", "null", &text("z"))`, `meta_line(2)`, `line(3, "m", r#""max_tokens""#, "")`, `meta_line(4)`, `line(5, "m", r#""end_turn""#, "")`. Expect exactly `[Text "z", Injected with 2 entries]`.
- `line(1, "z", "null", &text("z"))`, `meta_line(2)`, `line(3, "m", "null", &text("m"))`, `meta_line(4)`, `api_error_line(5, "m", "null", "m")`. Expect exactly `[Text "z", Injected with 2 entries, Error notice "m"]`.
- `line(1, "z", "null", &text("z"))`, `meta_line(2)`, `line(3, "m", r#""max_tokens""#, "")`, `meta_line(4)`, `line(5, "n", "null", &tool_use("c9"))`, `line(6, "m", r#""end_turn""#, "")`, `line(7, "n", "null", &tool_use("c9"))`, `result_line(8, "c9")`. Expect exactly `[Text "z", Injected with 2 entries, call c9 with status Ok]`. The re-sent call and the result check both position stores after the merge.

**3. Prefer an exact match in `error_block_key`.** Without `apiBlockIndex`, the search now takes the newest stored block that the new text extends, so it can pick a prefix over an exact match. Look first for a stored block with exactly the same text, then for one the text extends, skipping `taken` in both. Add a test with `api_error_line` for `API Error: x`, `API Error`, `API Error: x`, `API Error` on one message, expecting one `Error` notice with `API Error: x\nAPI Error`. The existing tests for distinct, repeated and grown blocks must keep passing.

**4. Reach this round's code from the chunking tests.** Append these to `busy_transcript`: `line(10, "msg_f", "null", &text("late"))`, `api_error_line(11, "msg_f", "null", "late")`, `api_error_line(12, "msg_d", "null", "first")`, `api_error_line(13, "msg_d", "null", "second")`, `line(14, "msg_z", "null", &text("z"))`, `meta_line(15)`, `line(16, "msg_m", r#""max_tokens""#, "")`, `meta_line(17)`, `line(18, "msg_m", r#""end_turn""#, "")`. In `any_chunking_gives_the_same_final_turns_with_notices`, the expected notice ids become `notice:msg_e`, `notice:msg_2`, `notice:msg_f`, `notice:msg_d`. Also assert that the turn holds exactly one `Item::Injected`, with 2 entries.

**5. Correct three doc comments in `src/harness/claude/wire.rs`.** The comments on `read_entry`, `lenient_entry` and `MessageWithoutUsage` describe only `toolUseResult` and `usage`. Name what the check parse now also skips: the top-level `isApiErrorMessage`, and `message.stop_reason` and `message.isApiErrorMessage`. "Only the damage inside `usage` is forgiven" must no longer be claimed. The comment on `MessageWithoutUsage` should also say that leaving these fields out is deliberate: invalid UTF-8 in them keeps the line parsed, as it did before this task, so they must not be added there. This replaces the task 002 verdict's advice to add every new `Message` field to both structs, for these lenient fields. Comments only; do not change this code.

**Leave as is.**
- `take_message_text` keeps moving all of the message's earlier text into the error. Do not narrow it to equal or prefix text. Round-1 feature finding 2 suggested that, and this note drops it: the planner note says the error is the message's text blocks joined with newlines, and both round-2 reviewers accept the current behaviour.
- `stop` stays per turn, and an empty `stop_reason` stays no reason.
- Make no view changes. Notice text normalisation and export escaping are not part of this round.

**Done when.**
- All tests above pass, and every existing test in `tests/claude_notices.rs` passes, with `busy_transcript` extended as in step 4.
- With the `b.index` loop deleted, each of the two tests in step 1 fails. With the merge in step 2 removed, the first two tests in step 2 fail. Check both by temporarily editing the code, restore it, and say so in the build notes.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass.
- `INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture` stays within budget.
- No `.snap` file changes in this round, and no `.snap.new` file is left behind.
