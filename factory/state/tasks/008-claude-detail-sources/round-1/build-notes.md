STATUS: DONE

## Changes
- `src/model/sources.rs` (new): `SourceRef` (`ByteSource` + byte range), `TurnSources` (the side table on a turn), `SectionRefs`/`SectionSource`/`SourceKind`, and the adapter's answer types `SourceGroup`/`SourceRow`/`SourceValue` (typed values: text, time, tokens, count, duration, so the App formats them). The table keeps one `ByteSource` per turn and byte ranges only: per message its latest line and latest line with a non-null `stop_reason`; per item a message number, one line, or a tool call's call line and result line; the prompt / compaction boundary line; the footer's last `turn_duration` line. No field values are stored.
- `src/model/mod.rs`: `UserTurn`, `AssistantTurn`, `CompactionTurn` gain `sources: TurnSources`; `Turn::sources()`.
- `src/harness/claude/assemble.rs`: records the references above (tool call line + result line, message lines, prompt line incl. `queued_command`, compaction boundary or summary line, first held entry's line for system sections, last `turn_duration` line). The turn is marked dirty whenever a reference changes, so a later line of a message that adds nothing visible (only `stop_reason`/`usage`) still updates the live transcript. `wire::Entry` and the assembler read no detail field.
- `src/harness/claude/details.rs` (new): the reader. Typed heads separate from `wire::Entry`, hand-written lenient readers (`pick`, `pick_object`, `pick_text`): a field of an unexpected JSON type counts as missing, `message.content` is never read, `toolUseResult` is an object-or-ignored head read by a visitor that never builds a `Value` (and does not even buffer it, as an `untagged` enum would). Only a JSON object is a line. Field locations as the planner notes (confirmed on a real transcript: `permissionDecision`/`sandboxed` at the top of the result line, `isAsync`/`resolvedModel` in `toolUseResult`, `diagnostics` under `message`). `permissionDecision` and `cache_miss_reason` are small objects in real transcripts, shown as `key=value, ...` of their scalar members (a plain string is shown as is).
- `src/harness/mod.rs`: `Harness::section_details(&self, &SectionSource) -> Vec<SourceGroup>` with a default that provides nothing (documented as what Codex/Pi/OpenCode inherit). `ClaudeHarness` implements it.
- `src/components/details_overlay/source.rs` (new): `DetailsRequest` (model rows + `Option<SectionSource>`), `section_source` (section id to references and kind), and `compose` (the model's rows and the adapter's, in the order of §5.3; Tool call is `start`, model `duration`/`status`, adapter's rest; the adapter's Prompt `timestamp` replaces the model's; one muted `unavailable` row last in a group whose source failed).
- `Msg::OpenDetails` now carries a `DetailsRequest`; the transcript builds it; `App::open_details` (src/app/details.rs) asks the open session's harness on the UI thread, after the busy/too-small guards, and composes with `self.timestamps`.
- Tests: `tests/claude_details.rs` (adapter), unit tests in `source.rs`, `details.rs`, `harness/mod.rs`, `src/app/browser_tests/details_source.rs` (App level + 4 snapshots). `tests/budgets.rs` prints the details-read reference. `tests/claude_injected.rs`: one test compared turns assembled over two different `ByteSource`s; both now use the same bytes (the turns carry the bytes their references name).
- `docs/performance.md`: new section with the figures.

## Self-check
- AC: adapter tests per group, missing fields, unreadable source, `toolUseResult` string — `the_message_group_lists_the_fields_of_the_message_in_order`, `a_tool_calls_start_is_from_its_call_and_the_rest_from_its_result`, `the_prompt_group_lists_its_fields`, `the_footer_reads_its_turn_duration_line_for_the_environment_only`, `a_compaction_and_a_system_section_read_their_first_line_for_the_environment` (Turn group is the model's: App level `a_text_section_shows_the_message_and_environment_rows_of_its_source` runs a Claude transcript with effort, thinking and cache tokens through all five groups, in order, with no `cost`), `missing_fields_are_left_out_and_a_group_with_no_rows_is_empty`, `a_field_of_an_unexpected_type_counts_as_missing_and_does_not_fail_the_line`, `a_tool_use_result_that_is_a_string_gives_the_top_level_fields_only`, `a_file_that_is_gone_...`, `a_range_past_the_end_of_the_file_is_unavailable`, `a_line_that_does_not_parse_is_unavailable` (all in `tests/claude_details.rs`).
- AC: assembling does not parse detail fields; budgets hold — `assembling_a_line_with_detail_fields_of_unexpected_types_and_sizes_is_cheap` (2 MB fields of wrong types, not counted unparsed); `cargo test --release -- --ignored budgets --nocapture` passes (figures below).
- AC: `Harness` method with a default, stub harness shows model rows only, no `unavailable` — `a_harness_provides_no_section_details_by_default` (`src/harness/mod.rs`), `a_harness_that_provides_nothing_leaves_the_models_rows_and_no_unavailable_row` (App level, `details_source.rs`).
- AC: `wire::Entry`/assembler gain no detail fields; `details.rs` reads `toolUseResult` through a typed head, with a very large and a string value — diff of `wire.rs` is empty; `tool_use_result_is_a_typed_head_and_not_a_value`, `a_big_tool_use_result_and_message_content_do_not_get_in_the_way`, `a_very_large_tool_use_result_is_skipped_and_its_fields_are_read`, `a_tool_use_result_that_is_a_string_...`.
- AC: text block on the first line of a multi-line message gets a later line's `stop_reason` — `a_text_block_on_the_first_line_of_a_message_gets_the_stop_reason_of_a_later_line`, `a_later_line_of_a_message_updates_the_turn_even_when_nothing_visible_changes`, `a_message_without_a_stop_reason_reads_its_latest_line`.
- AC: tool call permission/sandbox/isAsync from the result line, `start` from the call's — `a_tool_calls_start_is_from_its_call_and_the_rest_from_its_result`, `the_tool_call_fields_are_read_from_the_top_of_the_result_line_first`, App level `a_tool_call_shows_its_start_then_the_models_rows_then_the_rest_of_its_result`.
- AC: subagent reads its own file — `a_subagent_transcript_reads_its_own_file`, `a_loaded_subagent_names_its_own_file`.
- AC: file gone / range past end / unparseable line, one muted `unavailable` row per affected group, no panic, overlay stays open — adapter tests above, plus App level `a_file_that_is_gone_leaves_an_unavailable_row_per_group_and_the_overlay_open`, `a_range_past_the_end_of_the_file_leaves_an_unavailable_row_per_group`, `a_line_that_does_not_parse_leaves_an_unavailable_row_per_group`, `a_prompt_whose_source_is_gone_keeps_the_models_timestamp_then_unavailable`; muted style: `an_unavailable_group_ends_with_one_muted_row_after_what_the_model_knows` and, through the App on the real buffer, `unavailable_is_muted` (called by `a_file_that_is_gone_...`: every `unavailable` cell is `muted_foreground`).
- AC: tool call with no result yet, no `unavailable` — `a_call_without_a_result_yet_is_not_unavailable`, `a_call_without_a_result_yet_opens_with_no_unavailable_row`.
- AC: missing fields / empty group left out, no `cost` row on Claude — `missing_fields_are_left_out_and_a_group_with_no_rows_is_empty`, `a_group_the_harness_read_nothing_of_is_left_out`, `claude_records_no_cost_so_none_is_shown`, App level `a_text_section_shows_the_message_and_environment_rows_of_its_source`.
- AC: App::update test on a Claude fixture shows Message and Environment rows — `a_text_section_shows_the_message_and_environment_rows_of_its_source`.
- AC: fmt/clippy/test pass — see below.
- AC: snapshot changes limited to App-level details snapshots — only 4 new `.snap` files under `src/app/browser_tests/snapshots/`; no existing `.snap` changed (`git diff --stat main -- '*.snap'` shows only additions).
- UI: Claude tool call @ 80x24 — `the_details_of_a_claude_tool_call_80x24_text`
- UI: Claude tool call @ 120x40 — `the_details_of_a_claude_tool_call_120x40_text`
- UI: section with an unreadable source @ 80x24 — `the_details_of_a_section_whose_source_is_gone_80x24_text`
- UI: section with an unreadable source @ 120x40 — `the_details_of_a_section_whose_source_is_gone_120x40_text`

## Notes for the reviewers
- `unavailable` row and the label column: the task-007 component draws the row with the word `unavailable` in the label column, so a group with that row widens the label column of the whole overlay (visible in the new unreadable-source snapshots, and in the task-007 snapshot `unavailable_80x24_default`). The UI section says "no label column". I did not change it: excluding the row from the label width would alter the accepted task-007 snapshot, and the AC limits snapshot changes to new App-level ones. If wanted, it is a small change in `DetailsContent::label_width` plus that one snapshot.
- Every Claude section with a source gets an Environment group, so a text section whose line records no `gitBranch`/`cwd`/`version`/`entrypoint` has none; `0` counts (`web searches 0`, `5m cache writes 0`) are shown as recorded.
- The Message group of a section is read from the message's latest line with a non-null `stop_reason`, else its latest line, so its `timestamp` is that line's, not the block's own line (as the planner notes say). `start` of a tool call is the call's line.
- The adapter's `Duration` value variant is unused by Claude (the model owns durations); it is there for formatting with the shared format when OpenCode needs it.
- Assembly stays linear in a turn's items: items of a message are linked only when new (checked against the builder's index) and a new tool call is added to the table without a lookup (`TurnSources::add_call`).
- Duplicate keys inside a line fail the line (serde's derive), which shows `unavailable`.

## Budgets (`cargo test --release -- --ignored budgets --nocapture`, passes)
Same two 100 MiB fixtures, same machine, `main` vs this branch (the fixtures on this machine are the generator's current ones, so the older figures in `docs/performance.md` do not apply: heap there is 0.21x/0.83x; I measured `main` again):

| | main mixed | now mixed | main dense | now dense |
|---|---|---|---|---|
| first content (worst of 5) | 2.1 ms | 2.6 ms | 1.8 ms | 1.5 ms |
| full parse (worst of 5) | 140 ms | 141 ms | 403 ms | 297 ms |
| slowest frame | 4.8 ms | 3.2 ms | 1.7 ms | 0.95 ms |
| peak heap | 22.2 MiB (0.22x) | 22.4 MiB (0.22x) | 58.0 MiB (0.58x) | 62.7 MiB (0.63x) |
| heap after load | 9.2 MiB | 9.4 MiB | 55.6 MiB | 60.0 MiB |

The references add about 4 MiB (8%) to the dense session. Reference line for opening: reading the details of the largest tool result (2 033 269 bytes, mixed fixture) took 1.2 ms; the dense session's largest (3 084 bytes) took 0.1 ms.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2832 passed, 0 failed)
