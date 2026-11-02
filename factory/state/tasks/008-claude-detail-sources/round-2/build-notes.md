STATUS: DONE

## Changes
- Quadratic assembly (code-review 1, feature-review 1): the reference table is now push-only while assembling. `TurnSources::link*` (which scanned the table with `rposition`) are gone; `add_message_item`, `add_line` and `add_call` push without a lookup (their callers know the item is new, as the `b.index` guards and the new `Injected` items establish). `get` keeps its `rfind` and runs only when the overlay opens. The earlier claim in the round-1 notes ("linear") was wrong for text, thinking, notice, model-change and injected items; it is true now.
- Regression checks: `tests/budgets.rs` gains `long_turn_failures` (one endless turn, 100 000 rounds of text, tool call and result, 70 MB: full parse 0.66 s against the 2 s budget; it failed by tens of seconds with the scan), and `tests/claude_details.rs::assembling_a_long_turn_takes_time_in_proportion_to_its_items` compares 3 000 and 24 000 items (re-adding a scan to `add_message_item` makes it fail: 35x against a limit of 24x).
- Untested links (code-review 2, feature-review 2): new adapter tests, each checked by removing the link it covers (six tests fail): `a_notice_reads_the_message_group_and_environment_of_the_line_that_ended_the_message` (`max_tokens` on a second line), `an_api_error_notice_reads_the_message_group_and_environment`, `a_model_note_reads_the_environment_of_its_message_and_no_message_group`, `a_system_section_in_an_assistant_turn_reads_the_environment_of_its_first_line` (flush arm), `held_entries_that_open_an_assistant_turn_read_the_environment_of_their_first_line` (boundary, summary, hook, assistant). They go through `section_source`, so the section kind is checked as well.
- `unavailable` and the label column (ui-review 1, code-review 3, feature-review 3): `DetailsContent::label_width` leaves the `unavailable` row out, so values start at the widest real label (`timestamp` values at offset 11 beside `unavailable`). This changes the accepted task-007 snapshot `unavailable_80x24_default` (the `model` value moves from offset 13 to 9; the overlay stays at 40 columns). That is the one intended change outside the App-level snapshots, as the UI reviewer proposed; the Approver should note it. New App-level check: `unavailable_has_no_label_column_so_values_start_at_the_widest_real_label`.
- Empty strings (ui-review 2, code-review 8): `Rows::text` and `Rows::time` treat an empty string as missing; test `an_empty_string_is_no_value`.
- `resolvedModel` in the App fixture (ui-review 3): `result_line()` records it; the two tool-call snapshots are re-accepted (they now show `resolvedModel` between `sandboxed` and `isAsync`), and the order test lists it.
- Code nits: a tool call whose id was seen in an earlier turn gets its own entry (`known.turn == b.idx`, review 4); removed `link_call`, `is_empty`, `SourceValue::Duration` and the unused `bool` returns of `set_own`/`set_result`/`add_*`, the test now uses `add_call` (5); one `get` for `b.messages` (6); `_value`/`_source` parameter names and `as_deref` for `cache_miss_reason` (7); module doc says a duplicated key fails the line (feature 4); the comment in the cheap-assembly test no longer claims a size check (feature 6).

## Self-check
- Review: code-review 1 / feature-review 1 — push-only table, `assembling_a_long_turn_takes_time_in_proportion_to_its_items`, `long_turn_failures` in `budgets`, round-1 claim corrected here and in `docs/performance.md`.
- Review: code-review 2 / feature-review 2 — the five tests above.
- Review: ui-review 1 — `label_width`, snapshot `unavailable_80x24_default` re-accepted, App-level column test.
- Review: ui-review 2, code-review 8 — `an_empty_string_is_no_value`.
- Review: ui-review 3 — `result_line()` and snapshots `the_details_of_a_claude_tool_call_{80x24,120x40}_text`.
- Review: code-review 4–7, feature-review 4–6 — as listed under Changes.
- All round-1 acceptance criteria are still covered by the tests named in the round-1 notes (renamed or removed: none; `a_call_has_two_lines_and_links_are_not_replaced` became `a_call_has_two_lines_and_the_latest_entry_of_an_item_wins`).

## Budgets (`cargo test --release -- --ignored budgets --nocapture`, passes)
Mixed: full parse worst 141 ms, peak heap 22.4 MiB (0.22x), slowest frame 5.2 ms (noise: 3.2-5.2 ms over runs, `main` 4.8 ms). Dense: full parse worst 304 ms, peak heap 62.7 MiB (0.63x), slowest frame 1.25 ms. One endless turn (70 MB): 665 ms. Details of the largest tool result: 1.2 ms.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2840 passed, 0 failed)
