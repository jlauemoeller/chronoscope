VERDICT: PASS

## Summary
The round-2 must-fix is resolved: a Claude line whose only invalid UTF-8 is inside `message.usage` now parses at all four sites through `read_entry`, keeps its readable token figures, and every line with bad bytes elsewhere keeps its previous outcome. The implementation matches the rework note, the doc fixes are done, and the round changes no snapshots. The test gate and release budgets pass, and only three small nits remain.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass
- cargo test --release --test budgets -- --ignored budgets --nocapture: pass (worst first content 1.8 ms, full parse 560.6 ms, frame 3.77 ms, peak memory 0.83x)

## Checks on earlier findings and test quality
- Round 2 finding 1: fixed. No `from_slice::<Entry>` remains in `src/harness/claude` outside `read_entry`, except one test in `wire.rs` that asserts the plain parse fails. `from_slice` for `Head`, `ScanHead` and `IgnoredAny` skips the bad bytes and needs no change.
- Round 2 nits 2, 3 and 4: the requested doc comments are in place. The behaviour stays as the rework note told it to.
- Mutations, run on a scratch copy of the branch (the repository was not changed). Each one made the named tests fail:
  - No retry in `read_entry`: `invalid_utf8_inside_usage_keeps_the_line_parsed`, `a_result_is_read_from_a_line_with_invalid_utf8_in_usage`, `a_prompt_is_found_on_a_line_with_invalid_utf8_in_usage`.
  - Retry without the guard parse: `invalid_utf8_outside_usage_keeps_its_outcome`, `a_result_is_not_read_from_a_line_with_invalid_utf8_in_the_result`.
  - `TokenUsage::total` adding `reasoning`: `the_footer_total_is_input_plus_output_plus_cache_writes`, `the_spec_example_figures_give_1_5m_tokens_and_359_0m_cached`, `opencode_reasoning_is_added_to_output_once`.
  - Claude adding thinking tokens to `output`: `claude_thinking_tokens_are_not_added_to_output`.
  - OpenCode not adding reasoning to `output`: `opencode_reasoning_is_added_to_output_once`.
- Snapshots: the branch adds five accepted `.snap` files and modifies none. No `.snap.new` file is tracked.

## Findings
1. [nit] `src/harness/claude/wire.rs:140` — the doc comment on `entry_with_invalid_utf8_in_usage` says "as it is on `main`". That refers to a branch, and it stops meaning anything once this work is merged. The wording came from the rework note. Change: replace it with a lasting reason, for example "unparsed, so it is skipped and counted (base spec §9.5)".
2. [nit] `src/harness/claude/scan.rs:111` — `prompt_of_queued` is the only one of the four `read_entry` call sites without a test for invalid UTF-8 in `usage`. The rework note did not ask for one, and the code path is the same one-line call as `prompt_of_user`. Change, if wanted: add a test next to `a_prompt_is_found_on_a_line_with_invalid_utf8_in_usage`, using a human `queued_command` attachment line with `"message":{"usage":{"service_tier":"\xff"}}`, and assert `first_prompt` is kept.
3. [nit] `src/harness/claude/wire.rs`, `Message` and `MessageWithoutUsage` — the guard parse only works while `MessageWithoutUsage` has every field of `Message` except `usage`, and nothing enforces that. If a later task adds a field to `Message` alone (§4.2's `stop_reason` is a likely one), a line with invalid UTF-8 in that field would fail the first parse, pass the guard because the guard skips unknown fields, and be repaired with U+FFFD. It would then count as parsed instead of unparsed. Current behaviour is correct. Change: add a line to the doc comments of both structs saying that they must stay in sync, and why.
