VERDICT: FAIL

## Summary
All five items of the round-2 rework note are done: the source change in round 3 is limited to `remove`, `shift_after` and `error_block_key`, `wire.rs` changes in comments only, and the builder's required mutation checks hold when repeated. The code itself looks correct, and I found no wrong behaviour on this branch. One test gap remains in the removal trap: nothing fails if `shift_after` stops checking which turn a call belongs to, so that case is a must-fix under the same bar round 2 applied to the `Builder.index` fix-up.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass
- cargo test --release -- --ignored budgets --nocapture: pass (worst across both fixtures: full parse 462.0 ms against 2000 ms, first content 2.3 ms against 300 ms, memory peak 0.58x against 3x, frame 5.66 ms against 16 ms)

No `.snap` or `.snap.new` file is in the diff, and the gate wrote none.

## Rework note and mutation checks
Each mutation was made in a scratch copy of `HEAD`, and the full suite was run with `--no-fail-fast`.

| Mutation | Result |
|---|---|
| `b.index` loop in `shift_after` deleted | Caught: both step 1 tests, and the merged-call test |
| Merge block in `remove` deleted | Caught: the three merge tests and the chunking test |
| Second `shift_after` in the merge deleted | Caught: the merged-call test |
| Exact-match search in `error_block_key` deleted | Caught: `an_exact_repeat_is_preferred_over_a_shorter_block_it_extends` |
| `c.turn == b.idx` dropped from `shift_after` | **Not caught** (finding 1) |
| Merged entries reversed | Not caught (finding 2) |
| `b.index.remove(&second)` deleted | Not caught (finding 3) |
| `text.trim()` dropped from `message_notice` | Not caught (finding 4) |

Probes on the unchanged code gave the right results for three injected runs with two removals, a notice upserted after a merge, a flagged line with no blocks after text and a call, and invalid UTF-8 in `message.stop_reason` and `message.isApiErrorMessage`.

## Findings
1. [must-fix] `tests/claude_notices.rs`, section "positions after a removal" (guard at `src/harness/claude/assemble.rs:1280`) — No test covers a call in an earlier turn when a removal happens in a later turn. `shift_after` must leave that call's position alone, and the `c.turn == b.idx` check does so correctly today, but all tests still pass with the check deleted. With it deleted, this sequence moves the call `old` from item 2 to item 1, `attach_result` finds a text item there, and the result is silently dropped, leaving the call `Pending`. The planner's removal trap names `CallRef { turn, item }` and requires that a later result still attaches. How often it happens: this needs a result arriving after a new prompt (base spec §5.1 allows a prompt typed while the agent works) plus a removal in that later turn. A count over the 53 top-level local transcripts found 0 of 4855 results in a later turn, and removals themselves occur in no local transcript, as was also true of the round-2 `Builder.index` gap. Change: add this test, and confirm it fails with `c.turn == b.idx &&` deleted:
   ```rust
   #[test]
   fn a_removal_in_a_later_turn_leaves_a_call_of_an_earlier_turn_attachable() {
       let next_prompt = r#"{"type":"user","uuid":"u2","origin":{"kind":"human"},"timestamp":"2026-01-01T00:03:00Z","message":{"role":"user","content":"again"}}"#;
       let lines = [
           line(1, "a", "null", &text("one")),
           line(2, "b", "null", &text("two")),
           line(3, "c", "null", &tool_use("old")),
           next_prompt.to_owned(),
           line(4, "m", r#""max_tokens""#, &text("x")),
           line(5, "m", r#""end_turn""#, ""),
           result_line(6, "old"),
       ];
       let items = items_of(&assemble(&lines));
       assert_eq!(call_status(&items, "old"), Some(ToolStatus::Ok));
   }
   ```
2. [nit] `tests/claude_notices.rs:776` (`shape`) and the merge tests at lines 830-869 — `shape` only counts an `Injected` item's entries, so merged entries in reverse order would still pass, although the rework note says the second's entries go on the end of the first. This is a nit rather than a must-fix because a wrong order loses nothing, no acceptance criterion names it, and `first.extend(entries)` (`assemble.rs:1267`) matches the existing merge in `flush_current`. Change: in `injected_items_a_removal_leaves_side_by_side_collapse`, also assert that the entry texts are `["meta 2", "meta 4"]`.
3. [nit] `src/harness/claude/assemble.rs:1264` (`b.index.remove(&second)`) — No test fails if this line is deleted. It only matters when the second item's id is used again, which needs a repeated entry uuid. Neither spec describes that, unlike the prompt in finding 1, and a count over the same transcripts found none in 951 meta lines, so this is a nit. With the line deleted, the text `t` in the sequence `line(1, "z", "null", &text("z"))`, `meta_line(2)`, `line(3, "m", r#""max_tokens""#, "")`, `meta_line(4)`, `line(5, "m", r#""end_turn""#, "")`, `line(6, "t", "null", &text("t"))`, `meta_line(4)`, `line(7, "u", "null", &text("u"))` is replaced by the injected item (a probe gives `["text z", "injected 2", "injected 1", "text u"]`). Change: add that sequence as a test expecting `shape` to be `["text z", "injected 2", "text t", "injected 1", "text u"]`.
4. [nit] `src/harness/claude/assemble.rs:1090` (`message_notice`) — No test covers a flagged message whose text is only whitespace, so the `trim()` could be dropped unnoticed. Change: add an `api_error_line(1, "msg_e", "null", "   ")` test expecting no items.
5. [nit] `tests/claude_notices.rs:615` — Invalid UTF-8 is tested only in the top-level `isApiErrorMessage`. The doc comment on `MessageWithoutUsage` says `message.stop_reason` and `message.isApiErrorMessage` must stay out of that struct, but no test would fail if they were added. A probe shows the current behaviour is right: both lines stay parsed, and `"max\xff"` gives `stopped: max\u{FFFD}`, as the `lenient_entry` comment describes. Change: add the same test for those two fields, asserting `unparsed_count() == 0`.
6. [nit] `src/harness/claude/assemble.rs:1196`, `:1203` — `base + position` and `last + 1` can overflow on an absurd `apiBlockIndex`. A probe with `18446744073709551615` and two text blocks on one flagged line panics at line 1196 in a debug build. In release, where overflow checks are off, it wraps and gives the notice `second\nfirst` without crashing. `block_id` has the same `base + position` at line 1139, from before this task. Change: use `saturating_add` in `error_block_key`, and optionally in `block_id`.
7. [nit] `src/harness/claude/assemble.rs:1185` — This doc comment line runs to 82 columns, and the comment was not re-wrapped when `(first)` was added. Change: re-wrap the comment.
