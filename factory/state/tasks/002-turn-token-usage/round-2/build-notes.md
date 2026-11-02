STATUS: DONE

## Changes
- `Cargo.toml`: `serde_json` gets the `raw_value` feature (no new crate).
- `src/harness/shared.rs`: `lenient` and `lenient_object` now read the value as raw JSON text (`Box<RawValue>`) and parse it into the target type, instead of going through `serde_json::Value`. An out-of-range number (`1e400`) in a new or unknown field of `usage` / `tokens` no longer fails the line or row. Both helpers are `pub(crate)`.
- `src/components/turn_block/renderers/stats.rs`: `StatsSection` keeps the separator at the end of a wrapped row (no-break space glue, restored to a plain space afterwards). `format_stats` is unchanged, so search is unaffected.
- `src/harness/opencode/assemble.rs`: the token tests moved to their own `token_tests` module.
- Tests and one new styled snapshot (see below).

## Self-check
- Review (feature, must-fix 1): out-of-range number regresses parsing — fixed by the `RawValue` helpers; `an_out_of_range_number_in_usage_keeps_the_line_parsed` (tests/claude_assembly.rs: new field, unknown nested field, `thinking_tokens`) and `an_out_of_range_number_in_tokens_keeps_the_row_parsed` (OpenCode: `cache.read`, unknown field, `input`/`reasoning`) assert `unparsed_count() == 0`, the text items kept and the field counted as zero.
- Review (feature nit 2): absent key — `a_turn_without_any_usage_has_no_token_figures` and `rows_without_tokens_leave_the_token_figures_unknown` now also omit the key.
- Review (code nit 2): moved to `token_tests`. Code nit 3: helpers `pub(crate)`.
- Review (code nit 1): `{}` / unreadable usage still shows `0 tokens`; left as is (planner note: a missing number counts as zero; `main` does the same for `{}`).
- Review (UI nit 1): `a_wrapped_footer_row_never_starts_with_the_separator` (block widths 64 and 20 from the review, plus every text width from 10 to 70, rows rejoin to the footer text).
- Review (UI nit 2): `footer_with_cached_and_cost_at_80_styled` (new snapshot; the whole footer row is `muted_foreground`).
- No existing snapshot changed.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (0 failed; lib 2169 passed)
- release budgets (`cargo test --release --test budgets -- --ignored budgets --nocapture`): pass (worst full parse 380 ms, frame 3.3 ms)
