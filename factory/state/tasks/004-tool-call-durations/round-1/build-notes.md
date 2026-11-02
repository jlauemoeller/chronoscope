STATUS: DONE

## Changes
- `src/components/turn_block/renderers/tool/timing.rs` (new): `format_tool_duration` (`340ms` / `1.2s`, truncating, delegating to `crate::duration::format_duration` from 60 s), `duration_suffix` (` · 1.2s`; none for a pending call or an unknown duration; an `Error` call with a duration shows it) and `mute_tail` (re-styles the last words of word-wrapped rows as muted).
- `src/components/turn_block/renderers/tool/mod.rs`: `ToolCallSection::description` now adds the duration. Folded: the description is cut with `…` first, the agent type and duration stay; when they leave no room (`tail + 1 >= width`) the whole row is cut, with the `…` in the style of the text it replaces. Unfolded: the description, agent type and duration are word-wrapped and the duration is muted afterwards, so it stays muted across a wrap. New `description_with_duration` (full export summary) and `drawn_description` (what search sees). `plain_parts`/`plain_lines` come from the same rows, so as-displayed copy, save, full copy and as-folded export follow without their own code.
- `src/components/turn_block/export.rs`: the `<details><summary>` of the full export uses `description_with_duration()`.
- `src/transcript_search.rs`: `tool_text` starts with `drawn_description()` (description, agent type, duration as drawn), so a drawn description row ends where its source line ends (`SectionView::place`) and `1.2s` is searchable.
- `src/harness/claude/wire.rs`: `Entry.tool_use_result: Option<ToolUseResult>`, an untagged object-or-`IgnoredAny` head. The object arm is a hand-written map-only `Deserialize` that reads `isAsync` only (a derived struct would also read `[true]` from an array). A non-boolean `isAsync` makes the object arm fail and the value falls to `IgnoredAny`. `Entry::is_async_launch`.
- `src/harness/claude/assemble.rs`: `attach_result` takes `is_async` and leaves `duration` empty for an asynchronous launch.
- `src/bench_support/generate.rs`, `mod.rs`: generated tool-result lines carry `toolUseResult` (an object repeating the output in the shape of the tool; a string for failed calls); `GENERATOR_VERSION` 1 -> 2. The generated file now holds fewer entries per byte, so the two tests that load a 5 MiB fixture (`bench_support/tests.rs`) load 12 MiB to keep their "many batches / long stretch" assumptions.
- Tests and snapshots: `src/components/turn_block/durations.rs` (new, 12 snapshots), plus tests in `tests/claude_assembly.rs`, `src/harness/claude/wire.rs`, `src/components/transcript/tests/search.rs`, `src/components/transcript/search.rs`, `src/transcript_search/tests.rs`, `src/app/browser_tests/save.rs`, `turn_block/fixtures.rs` (`with_duration`).
- One existing snapshot changed: `an_opencode_session_in_the_browser_with_a_shell_tool_unfolded_120x40_text`. The OpenCode adapter already produces real durations (1.0s, 2.0s, 4.0s, 1.0s), so the four tool rows gained ` · Ns`. No other row moved; all other snapshots use `duration: None`.

## Self-check
- AC: fixtures with `duration: Some(..)`, snapshots folded/unfolded, subagent suffix, narrow row — `durations.rs` snapshots: `folded_call_with_a_duration_at_80/120`, `unfolded_call_with_a_duration_at_80/120`, `folded_subagent_with_a_duration_at_80/120`, `unfolded_subagent_with_a_duration_at_80/120`, `folded_call_too_narrow_for_its_description_at_40`, `folded_subagent_too_narrow_for_its_description_at_40`, `unfolded_call_wrapped_at_40_keeps_the_duration_muted_styled`, `selected_folded_call_keeps_the_duration_muted_styled`
- AC: format boundaries at 1 s and 60 s — `timing::tests::{under_a_second_shows_milliseconds, from_a_second_to_a_minute_shows_tenths, from_a_minute_on_it_is_the_shared_format}` (999ms, 1.0s, 59.9s, 1m 0s)
- AC: `isAsync: true` gives no duration; string `toolUseResult` still parses — `claude_assembly::{an_asynchronous_launch_has_no_duration, a_string_tool_use_result_still_parses_the_line_and_its_result, a_synchronous_call_has_a_duration_whatever_its_tool_use_result_is}`
- AC: search match in a description highlights the correct cells — `transcript::tests::search::a_hit_in_a_description_with_a_duration_lands_on_its_row_with_the_right_cells`; `transcript::search::tests::{a_description_row_ending_in_the_duration_is_found_as_the_source_line, a_bare_description_does_not_end_where_its_row_does}` exercise `SectionView::place` directly
- AC: pending / unknown duration show no duration and no separator — `durations::{a_pending_call_shows_no_duration_and_no_separator, a_call_without_a_known_duration_shows_no_duration_and_no_separator}`, `transcript_search::tests::a_pending_call_or_one_without_a_duration_has_no_duration_text`
- AC: `Error` status shows its duration — `durations::a_failed_call_with_a_duration_shows_it`
- AC: `isAsync` of a wrong type still parses line and result, and gets a duration — `claude_assembly::an_is_async_of_the_wrong_type_still_parses_the_line_and_its_result`, `wire::tests::only_a_boolean_true_is_an_asynchronous_launch`
- AC: `toolUseResult` is not a `serde_json::Value` — `wire::tests::tool_use_result_is_the_head_and_not_a_value` (type-checks the field as `Option<ToolUseResult>`), `claude_assembly::a_large_tool_use_result_is_skipped_and_the_result_is_read_from_the_message`
- AC: duration in as-displayed copy (folded, unfolded), full copy, saved file, as-folded export, full export summary — `durations::{the_displayed_copy_of_a_folded_call_has_the_duration, the_displayed_copy_of_a_subagent_has_the_duration_after_the_agent_type, the_displayed_copy_of_an_unfolded_call_has_the_duration, the_full_copy_has_the_duration, the_folded_export_has_the_duration_folded_and_unfolded, the_full_export_summary_has_the_duration, a_call_without_a_duration_exports_as_it_did}`, `browser_tests::save::w_saves_the_duration_with_the_description_and_capital_w_with_the_full_content`
- AC: search for `1.2s` finds and highlights it — `transcript::tests::search::{a_search_for_the_duration_finds_it_and_highlights_its_cells, a_search_for_a_duration_of_minutes_finds_it, a_search_for_the_duration_of_a_subagent_finds_it_after_the_agent_type}`, `transcript_search::tests::the_duration_is_searched_as_it_is_drawn_after_the_description` (these fail with the old bare-description `tool_text`)
- AC: narrow row shows `…` and the intact duration — `durations::{the_narrow_row_cuts_the_description_and_keeps_the_duration, the_narrow_subagent_row_cuts_the_description_and_keeps_both_suffixes, a_row_that_leaves_no_room_for_the_description_is_cut_as_a_whole}` and the 40-column snapshots
- AC: generated fixtures contain `toolUseResult`, `GENERATOR_VERSION` bumped, budgets within budget — `src/bench_support/generate.rs`, `GENERATOR_VERSION = 2`; budget run below
- AC: snapshot changes limited to calls with a duration — only the OpenCode snapshot above changed (its calls have real durations)
- UI: folded @ 80 / 120 — `folded_call_with_a_duration_at_80`, `..._at_120`
- UI: unfolded @ 80 / 120 — `unfolded_call_with_a_duration_at_80`, `..._at_120`
- UI: subagent @ 80 / 120 (folded and unfolded) — `folded_subagent_with_a_duration_at_80/120`, `unfolded_subagent_with_a_duration_at_80/120`
- UI: folded row narrow @ 40 — `folded_call_too_narrow_for_its_description_at_40`, `folded_subagent_too_narrow_for_its_description_at_40`
- UI: muted duration, description keeps its token, selected/wrapped — `the_duration_is_muted_and_the_description_keeps_its_token` (distinct theme, folded and unfolded, 100 and 40), `unfolded_call_wrapped_at_40_keeps_the_duration_muted_styled`, `selected_folded_call_keeps_the_duration_muted_styled`

## Budgets
`cargo test --release -- --ignored budgets --nocapture` (regenerated v2 fixtures, 100 MiB, worst of five):
- mixed (5% large): first content 2.1 ms, full parse 108 ms, slowest frame 3.44 ms, peak heap 0.22x (22.2 MiB)
- dense: first content 1.6 ms, full parse 281 ms, slowest frame 0.97 ms, peak heap 0.58x (58.0 MiB)
All within the budgets (300 ms / 2 s / 16 ms / 3x). The v2 fixtures hold fewer lines than v1 for the same bytes (each result is now written twice), so figures are not line-for-line comparable with `docs/performance.md`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2542 passed, 0 failed)
