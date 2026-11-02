STATUS: DONE

## Changes
- `src/duration.rs` (new, registered in `src/lib.rs`): shared `pub fn format_duration(Duration) -> String` (`45s`, `4m 3s`, `3h 4m`; truncating, no zero padding) with its unit tests.
- `src/components/session_list.rs`, `src/components/turn_block/renderers/stats.rs`: removed their own `format_duration`; both `format_stats` call the shared one. Re-exports removed from `src/components/mod.rs` and `src/components/turn_block/renderers/mod.rs`. No layout code (`row_parts`, `MIN_TITLE_WIDTH`) changed; `SessionStats`/stats cache untouched.
- Tests updated for the new format: `src/components/turn_block/tests.rs`, `src/components/turn_block/plain.rs`, `src/app/selector_tests.rs`, `src/components/session_list.rs`. New: `seconds_under_an_hour_can_push_the_stats_out`, `the_stats_use_the_shared_duration_format`, `stats_that_no_longer_fit_snapshot`.
- Snapshots: 65 existing `.snap` files updated, 1 new. Every diff was checked by script: each changed line is only a footer/selector-row duration, or a title/footer text that re-fit because of the width change (e.g. help overlay now shows "messages" in full, long selector titles gain/lose 1-2 cells). Styled snapshots differ only in the span ranges of those same rows.

## Self-check
- AC: format cases 0s, 59.9s, 60s, 5m 2s, 59m 59.9s, 1h, 1h 5m, 100h 30m — `duration::tests::*` (under_a_minute..., under_an_hour..., an_hour_or_more...)
- AC: truncation and no zero padding (1h 5m 59s -> `1h 5m`) — `duration::tests::values_are_truncated_not_rounded`, `numbers_are_not_zero_padded`
- AC: selector and footer assert the new format — `session_list::tests::stats_read_turns_and_duration`, `the_stats_use_the_shared_duration_format`; `turn_block::tests::footer_is_muted_and_indented_by_two_spaces`, `stats_footer_formats_the_spec_example`, `stats_footer_adds_cost_and_omits_unknown_parts`; `app::selector_tests::the_focused_selection_is_one_inverted_band_with_a_primary_marker`
- AC: row whose stats fit in old format (`12 turns · 59m` at text width 47) but drop in new — `session_list::tests::seconds_under_an_hour_can_push_the_stats_out`
- AC: one `fn format_duration` in `src/` — only `src/duration.rs` (grep)
- AC: SessionStats/cache unchanged — no edits to `src/index/cache.rs`/model; cache tests pass unedited
- AC: no layout change — `row_parts` untouched
- AC: budgets — `cargo test --release -- --ignored budgets --nocapture` passes (worst frame 3.6 ms, parse 350 ms)
- UI: stats that no longer fit @ 55-col pane (47 text columns) — `components::session_list::tests::stats_that_no_longer_fit_snapshot`
- UI: existing selector row/footer snapshots at their existing sizes — all updated and reviewed (rows_with_stats_snapshot, selector_tests::*, browser_tests::*, transcript/turn_block snapshots, export golden files)

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2149 passed in lib, 0 failed, plus integration tests all passing)
