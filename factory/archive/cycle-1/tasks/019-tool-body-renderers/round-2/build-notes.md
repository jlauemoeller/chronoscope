STATUS: DONE

## Changes
- `tool/write.rs`, `tool/edit.rs`, `tool/mod.rs` (`with_status_result`): a failed or pending Write/Edit now appends a `Result:` block (error text in `error_output`, or `(pending)` in `muted_foreground`) after the `Content:`/`Diff:` block; a successful call keeps the table body. The result is shown in full (only the content/diff is truncated, so the single more-row stays unambiguous).
- `tool/subagent.rs`: the result is cut at `truncate_output_lines` source lines before the markdown is laid out and `Body::remaining` carries the cut count, so the `… N more lines` stop, `Submit` expansion and fold persistence work as for the other kinds. Row order: result, blank, `<key> to open transcript`, then the more row. The more row has to be the last row because it is the stop directly after the tool section, so it sits below the hint (documented on `SubagentBody`).
- `tool/read.rs`: no unchecked arithmetic on input numbers (`saturating_*`); the output is read with a single `text()` call and an error or empty output is rendered from that result (`unavailable_block` shared with `result_block`); numbering is detected on the first non-blank line; the range note is capped at the last line returned and omitted for a plain read from line 1 (`(from line N)` only when the result is unknown).
- `tool/search.rs`: `include` (OpenCode) is shown as a glob filter.
- `tool/edit.rs`: the diff has a 250 ms deadline.
- API hygiene: `ToolRendered` removed (`Rendered` is returned), `renderers/mod.rs` re-exports only `ToolCallSection`, `ToolBodyRenderer`, `Body`, `Block`. `TurnBlock::layout` shortened with `section_ctx` / `push_more` helpers. The hint and more row are `ellipsize`d to their width.
- Not done (nits, accepted): the unavailable-output warning is still logged on every layout of an unfolded section (documented on `unavailable_block`); no ANSI/OpenCode `|` gutter parsing (outside the planner notes).

## Self-check
- Review code 1 / feature 2 / UI 1 (Write, Edit failed + pending) — `write::tests::a_failed_write_shows_its_error_and_a_pending_one_shows_pending`, `edit::tests::{a_failed_edit_shows_its_error_after_the_diff,a_pending_edit_shows_pending}`, snapshot `failed_edit_shows_its_error_below_the_diff_at_100_styled` (UI 6); the property test generates failed/pending Edit/Write calls.
- Review code 2 / feature 1 / UI 2 (subagent truncation) — `subagent::tests::{a_long_result_is_cut_and_expanding_shows_all_of_it,the_more_row_is_the_last_row_after_the_hint}`, `transcript::tests::truncation::a_long_subagent_result_is_truncated_with_a_stop_that_expands_it` (stops, row, Submit expansion); heights by the extended proptest.
- Review code 3 (overflow) — `read::tests::huge_offsets_and_limits_do_not_overflow`.
- Review code 4 / feature 5 (double read) — `read::tests::an_unreadable_output_is_read_once_and_reported`.
- Review code 6 (API), 7 (layout length), 8 (ellipsize), 10 (timeout) — as above.
- Review code 9 / feature 6 / UI 5 (range note) — `read::tests::{the_range_does_not_run_past_the_lines_returned,a_read_from_line_one_without_a_limit_has_no_note}`.
- Review feature 3 (leading unnumbered line) — `read::tests::numbering_is_detected_past_a_leading_unnumbered_line`.
- Review UI 3 (OpenCode `include`) — `search::tests::opencode_include_is_shown_like_a_glob`.
- Review code 5, UI 4, feature 4 — accepted as above.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1178 passed, 0 failed)
