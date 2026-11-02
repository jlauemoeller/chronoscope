VERDICT: PASS

## Summary
`TurnStats.tokens` is now `Option<TokenUsage>`, and the footer shows `tokens` (input + output + cache writes) and a `cached` part that is hidden at zero, with cost last. Reasoning is counted once on each harness. All acceptance criteria are met, with tests that would fail if the behaviour were wrong, and no existing snapshot changed. The only findings are three nits.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass
- cargo test --release -- --ignored budgets --nocapture: pass (worst of two runs: first content 5.4 ms, full parse 655.1 ms, frame 5.91 ms, peak memory 0.83x)

## Findings
1. [nit] `src/harness/claude/assemble.rs` `token_usage` and `src/harness/opencode/assemble.rs` `Tokens::usage` — a usage object with no readable number (for example `{}` or `{"input_tokens":"many"}`) becomes `Some(TokenUsage::default())`, so the footer shows `0 tokens` rather than leaving the part out. This follows the planner's "a missing number counts as zero", and `{}` already showed `0 tokens` before this change, so it breaks no requirement. Found by reading the code; no test covers this case. Change: if `0 tokens` is unwanted, have both adapters record no usage when every numeric field is missing, and add a test on each harness.
2. [nit] `src/harness/opencode/assemble.rs` `mod replace_tests` — `cache_tokens_are_summed_over_the_rows`, `opencode_reasoning_is_added_to_output_once`, `token_fields_of_an_unexpected_type_count_as_missing_and_keep_the_row_parsed` and `rows_without_tokens_leave_the_token_figures_unknown` do not test row replacement, so the module name misdescribes them. Change: move them, with their helpers, to a `mod token_tests` or to `src/harness/opencode/tests.rs`.
3. [nit] `src/harness/shared.rs:11` and `:22` — `lenient` and `lenient_object` are `pub` in a `pub mod` (`src/lib.rs:13`, `src/harness/mod.rs:7`), so these internal serde helpers are exported from the library crate, although only the two adapters use them. Change: make both `pub(crate)`.
