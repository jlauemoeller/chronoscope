VERDICT: PASS

## Summary
All six items of the round-3 rework note are done as written: outside `factory/`, this round adds only the specified tests to `tests/claude_notices.rs` and re-wraps one comment in `src/harness/claude/assemble.rs`. The round-3 must-fix is closed, because deleting the `c.turn == b.idx` check now fails the new test, and each of the other required mutations also fails exactly its named test. I found no wrong behaviour on this branch, so only two nits remain.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2664 passed, 0 failed, summed from the per-binary results)
- cargo test --release -- --ignored budgets --nocapture: pass (worst across both fixtures: full parse 291.1 ms against 2000 ms, first content 2.5 ms against 300 ms, memory peak 0.58x against 3x, frame 4.94 ms against 16 ms)

No `.snap` or `.snap.new` file is in `git diff main...HEAD`, and the gate wrote none to the repo.

## Rework note and mutation checks
Each mutation was made in a private export of `HEAD` with its own Cargo target directory, then `tests/claude_notices.rs` was run. The baseline there is 50 passed, 0 failed. Each mutation failed one test, the one listed. A first run in a shared Cargo target directory gave crossed results and was discarded; this table is from the isolated rerun.

| Mutation | Failing test |
|---|---|
| `c.turn == b.idx &&` deleted from `shift_after` | `a_removal_in_a_later_turn_leaves_a_call_of_an_earlier_turn_attachable` |
| Merged entries reversed in `remove` | `injected_items_a_removal_leaves_side_by_side_collapse` |
| `b.index.remove(&second);` deleted | `a_merged_injected_items_id_is_free_again` |
| `text.trim().is_empty()` changed to `text.is_empty()` | `a_flagged_message_with_only_whitespace_has_no_notice` |
| `stop_reason` added to `MessageWithoutUsage` | `invalid_utf8_in_the_stop_reason_keeps_the_line_parsed` |
| `isApiErrorMessage` added to `MessageWithoutUsage` | `invalid_utf8_in_the_flag_inside_the_message_keeps_the_line_parsed` |

Probes on the unchanged code also gave the right results:
- a warning, then a removal, then a different warning, with a result arriving after it;
- a warning on a message later flagged as an error;
- thinking and a tool call after text was taken into an error;
- message ids sharing a prefix (`m` and `m:7`).

The `apiBlockIndex` overflow that the round-3 verdict dropped is unchanged, as that verdict instructs, so it is not raised again here.

## Findings
1. [nit] `src/harness/claude/assemble.rs:818` — `stop_changed` is true whenever the line carries a non-empty reason, including one equal to the stored value, so the name overstates what it means. Change: rename it `has_stop_reason`.
2. [nit] `src/harness/claude/assemble.rs:1087` (`message_notice`) — A whitespace-only block beside real error text stays in the notice: `"  "` and `"boom"` give `"  \nboom"`, whether the blank block arrives on the flagged line or before the flag. This matches the planner note as written ("text blocks joined with newlines"), so it is a judgement call and not a defect. A message with only whitespace already has no notice. Change: add `.filter(|t| !t.trim().is_empty())` before the `collect` in `message_notice`, with a test for each path. Tried in a scratch copy: both paths give `"boom"`, and the full suite still passes 2664 of 2664.
