STATUS: DONE

## Changes
- `src/model/mod.rs`: new public `TokenUsage` (`input`, `output`, `cache_read`, `cache_write`, `reasoning`, all `u64`) with `total()` (input + output + cache writes; reasoning never added) and `accumulate()`. `TurnStats.tokens` is now `Option<TokenUsage>`.
- `src/harness/shared.rs`: `lenient` / `lenient_object` serde helpers; a value of an unexpected JSON type becomes `None` instead of failing the line/row (`lenient_object` also rejects arrays, which serde would otherwise read into a struct).
- `src/harness/claude/wire.rs`: `Usage` gains `cache_read_input_tokens`, `cache_creation_input_tokens`, `output_tokens_details.thinking_tokens`; every usage field and `message.usage` itself are lenient.
- `src/harness/claude/assemble.rs`: `Builder.usage` keeps the latest `TokenUsage` per message id; `thinking_tokens` fills `reasoning` only (not added to `output`).
- `src/harness/opencode/assemble.rs`: `Tokens` reads `reasoning` and `cache.{read,write}` leniently; `Contribution.tokens` is a `TokenUsage`, `output = tokens.output + tokens.reasoning`; row replacement still replaces its share.
- `src/components/turn_block/renderers/stats.rs`: `format_stats` shows `tokens` from `total()` and `· <n> cached` (hidden at zero) before the cost. Footer search already uses `format_stats`, so it matches the displayed text.
- Tests/fixtures updated for the new type; `tests/fixtures/README.md` sentence about cache counts corrected. Four new footer snapshots.

## Self-check
- AC: assembler tests (cache on both harnesses, counted once, Claude thinking not added, OpenCode reasoning added) — `cache_reads_and_writes_are_summed_over_messages_each_counted_once`, `claude_thinking_tokens_are_not_added_to_output`, `usage_applies_to_a_footer_that_already_exists`, `tokens_count_each_message_once_with_cache_reads_and_writes` (fixture) in tests/claude_assembly.rs; `cache_tokens_are_summed_over_the_rows`, `opencode_reasoning_is_added_to_output_once`, `a_row_updated_in_place_replaces_its_cache_share` in opencode/assemble.rs.
- AC: `format_stats` cached shown/hidden, cost last — `stats_footer_shows_cached_after_tokens_and_hides_it_when_zero`, `stats_footer_keeps_the_cost_last_after_cached`.
- AC: footer search finds the cached part — `searching_the_cached_part_highlights_it_on_the_footer_row` (search finds the Footer section; highlighted cells on the footer row read `359.4M cached`), `a_cached_part_that_is_hidden_cannot_be_found`.
- AC: spec example figures — `the_spec_example_figures_give_1_5m_tokens_and_359_0m_cached`.
- AC: unexpected JSON type — `usage_fields_of_an_unexpected_type_count_as_missing_and_keep_the_line_parsed` (Claude, checks `unparsed_count() == 0`, line content kept), `token_fields_of_an_unexpected_type_count_as_missing_and_keep_the_row_parsed` (OpenCode).
- AC: no usage shows no token/cached part — `a_turn_without_usage_shows_neither_tokens_nor_cached`, `a_turn_without_any_usage_has_no_token_figures`, `rows_without_tokens_leave_the_token_figures_unknown`.
- AC: type no longer `Option<u64>`, reasoning not added to a total — `the_footer_total_is_input_plus_output_plus_cache_writes`; only OpenCode adapter adds reasoning to `output` as specified.
- AC: snapshot changes limited to new footer snapshots; no existing `.snap` changed.
- AC: budgets — `cargo test --release --test budgets -- --ignored budgets --nocapture` passes (worst full parse 383 ms, frames 3.4 ms, memory 0.83x).
- UI: footer with cached @ 80 / 120 — `footer_with_cached_at_80`, `footer_with_cached_at_120`
- UI: footer with cached and cost @ 80 / 120 — `footer_with_cached_and_cost_at_80`, `footer_with_cached_and_cost_at_120`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites, 0 failed; lib 2166 passed)

Note: `cargo test --release -- --ignored budgets --nocapture` as literally written also tries other targets with 0 matches; run with `--test budgets` to see the output.
