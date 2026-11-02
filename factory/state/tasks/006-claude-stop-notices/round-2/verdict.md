VERDICT: REWORK

## Assessment

The feature itself is right. All acceptance criteria are ticked, and the real flagged line in the local transcripts gives one correct notice. What remains is around the new `remove` helper: a crash regression no test would catch, and one spec rule it can break. Each has one obvious fix, so this is a rework and not a human choice.

**Code review: PASS**, four nits.
- Finding 1 (nit), `take_message_text` moves all earlier text of the message into the error: ignored, and the rework note says to leave it. No text is lost, the planner note ("the message's text blocks joined with newlines") supports it, and both round-2 reviewers accept it. The real flagged message is a single line with its own message id, so the path does not arise there.
- Finding 2 (nit), `error_block_key` can prefer a prefix over an exact match: not a reason to rework on its own, but it gives wrong text from code new in this task, and the fix is two lines. Included. Probe result on this branch: `API Error: x\nAPI Error: x\nAPI Error`.
- Finding 3 (nit), doc comments in `wire.rs` that no longer say what is skipped: included as a small fix, because "only the damage inside `usage` is forgiven" is now untrue.
- Finding 4 (nit), `busy_transcript` does not reach this round's code: included, because it also puts the new merge under the any-chunking criterion.

**Feature review: FAIL.**
- Finding 1 (must-fix), no test covers the `Builder.index` fix-up in `remove`: to be fixed. The code is correct today, so this is only a test gap, but I do not treat it as minor:
  - With that loop deleted in a scratch copy, all 2653 tests still pass, while the reviewer's two sequences each fail on their own, the first with a panic in `upsert` (`index out of bounds: the len is 2 but the index is 2`).
  - The planner's removal trap names `Builder.index` as one of the two position stores.
  - Task 005 had the same gap (no test for the index rebuild in its `remove`), and it was closed before approval.
- Finding 2 (must-fix), a removal can leave two `Item::Injected` side by side: to be fixed. The impact is small and I want to be plain about that. No data is lost, nothing crashes, and it needs a stop reason or flag that changes between lines of one message, which no local transcript has, with injected entries on both sides. I still would not ignore it, because:
  - It breaks an explicit base-spec rule (§5.1, §5.6), through code this task added.
  - The fix has one obvious shape, which I applied in a scratch copy (see below).
  - Task 002 was reworked on the same reasoning.
  I reproduced both of the reviewer's sequences (`[Text, Injected(1), Injected(1)]` and `[Text, Injected(1), Injected(1), Notice]`). The result does not depend on chunking, because `on_assistant` flushes held entries before it removes anything.
- Finding 3 (nit): the same as code finding 4.

**UI review: PASS**, no findings.

**Independent checks**, on a fresh export of `HEAD` in a scratch directory, with `INSTA_UPDATE=no`:
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass. `cargo test` gives 2653 passed, 0 failed, 2 ignored. No `.snap.new` file was written, and no `.snap` file is in `git diff main...HEAD`.
- I did not rerun the budgets. The builder and two reviewers report them within budget, and nobody disputes that.
- The one flagged line in the local transcripts (`<synthetic>`, `stop_sequence`, no `apiBlockIndex`) gives one `Error` notice with its full text and `unparsed_count() == 0`.
- The rework note below was tried in the scratch copy. The `remove` merge and the `error_block_key` change make every new test case pass. Clippy and fmt pass, and the existing tests pass unchanged. The extended `busy_transcript` also passes both chunking tests.

**Not blocking, noted so it isn't lost.** The task 005 verdict carried two items forward to this task: normalising notice text in one place, and escaping the rest of CommonMark's markup in `emphasized_lines`. They never reached this `task.md`, and no reviewer in this round raised them. This task changes no view code, so they remain as that verdict describes them: cosmetic, with no text lost. I read `without_controls` and control characters are replaced before drawing, so no terminal escape reaches the screen. I did not probe them again. They are left out of the rework note and need a later task or backlog entry if they are wanted.

## Conflicts

None between the current reviews. The code review passes and the feature review fails, but they do not make opposing demands.

One older demand is dropped: round-1 feature finding 2 suggested removing only earlier text equal to or a prefix of the error. The round-2 build moved all of it instead, and both round-2 reviewers accept that under the planner note. The rework note keeps the current behaviour, so the builder does not go back to the round-1 suggestion.

## Rework budget

Earlier REWORK verdicts: 0 of 2. Round 1 has no `verdict.md`, and none appears in git history.

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
