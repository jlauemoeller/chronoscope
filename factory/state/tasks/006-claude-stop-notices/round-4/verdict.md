VERDICT: APPROVE

## Assessment

All three reviews pass, all acceptance criteria are met, and the only two remaining findings are nits I judge minor enough to ignore. I would ignore them even if rework rounds remained, so the spent budget is not the reason for this verdict.

**Code review: PASS**, two nits.
- Finding 1 (nit), `stop_changed` in `src/harness/claude/assemble.rs` is true for any non-empty reason, so the name overstates what it means: ignored. It is a local variable name with no effect on behaviour, and the code reads correctly next to its one use.
- Finding 2 (nit), a whitespace-only block beside real error text stays in the notice: ignored.
  - I reproduced it on both paths. `"  "` and `"boom"` give the text `"  \nboom"`, whether the blank block is flagged or arrives before the flag.
  - The effect is one blank row above the error, which `word_wrap_unhyphenated` draws as `["", "boom"]`. No text is lost.
  - It matches the planner note as written ("text blocks joined with newlines"), and the reviewer calls it a judgement call, not a defect.
  - The one flagged line in the local transcripts is a single line with its own message id (round-2 verdict), so this path does not arise there.
  - It is not an A/B choice for the human, because the two outcomes differ only by that row.

**Feature review: PASS**, no findings. Its round-3 nit on `b.index.remove(&second)` is closed by `a_merged_injected_items_id_is_free_again`.

**UI review: PASS**, no findings. The task is `UI: none`, and no view code or snapshot changed.

**Round-3 must-fix: closed.** Deleting `c.turn == b.idx &&` from `shift_after` now fails `a_removal_in_a_later_turn_leaves_a_call_of_an_earlier_turn_attachable`. The code and feature reviewers each ran all six mutations from the round-3 note in isolated copies, and each mutation failed exactly its named test. I did not repeat them, because the two reports agree and nobody disputes them.

**Independent checks**, on a clean export of `HEAD` with its own target directory and `INSTA_UPDATE=no`:
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.
- `cargo test` gives 2664 passed, 0 failed, 2 ignored, and no `.snap.new` file was written.
- `git diff main...HEAD` has no `.snap` file and no file under `tests/fixtures`, so no README line is needed.
- Outside `factory/`, the round-4 `src/` diff is the `error_block_key` comment re-wrap only.
- The merge base equals `main`, so the branch is current.
- I read the task's production diff in `assemble.rs` and `wire.rs` and found nothing the reviews missed.
- I did not rerun the budgets. Round 4 changed no production code, and the builder and two reviewers report them within budget (worst full parse 331.9 ms against 2000 ms).

**Not blocking, but these need a home.** The task 005 verdict carried two items forward to this task. They never reached `task.md`, and neither appears in `factory/state/backlog.md` or in any other task's `task.md`, so approving this task leaves them untracked. Claude notices now carry a message's own text through these paths, so both are reachable. Each needs a backlog entry or a later task:
- **Notice text normalisation in one place** (`NoticeSection`, `section_text`, the blank check in `item_content`). Trimming both ends gives `"boom"` from `"  \nboom"`, so this would also remove the blank row in code finding 2, except for a blank block in the middle of a notice (`"boom\n  \nbang"`). Line feeds must survive it, because two error blocks draw as two rows.
- **CommonMark escaping in `emphasized_lines`**, and indents of four or more columns. I did not probe this. The task 005 verdict describes it as wrong rendering in exported files, with no notice text going missing.

Neither is a security issue on the drawing path: `wrap_with` replaces control characters before drawing, and a probe of `a\x1b[31mb` drew `a [31mb`.

## Conflicts

None.

## Rework budget

Earlier REWORK verdicts: 2 of 2 (`round-2/verdict.md` and `round-3/verdict.md`). Round 1 has no `verdict.md`, and the `approver-review.md` files are copied rework notes, not verdicts.
