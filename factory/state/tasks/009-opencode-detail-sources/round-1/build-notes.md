STATUS: DONE

## Changes
- `src/model/sources.rs`: the references gain an OpenCode row form. `SectionRefs` is now an enum, `Lines(LineRefs)` (the old struct, Claude) or `Rows(RowRefs)`. `RowRefs` holds the section's `RowRef` (row id, plus the part id for a tool call: one reference for the call and its result) and the turn's first assistant row. `TurnSources` gets `set_own_row`, `set_turn_row`, `add_row_item`, `remove_item`. `SourceGroupKind::Turn` is new, and `SourceRow` gets `replaces` (a prompt's `time.created` stands in for the model's `timestamp`).
- `src/harness/opencode/assemble.rs`: records the row of each item when `upsert` really inserts it (a row replaced in place adds nothing; a notice that goes loses its entry), the user row of a prompt, the first assistant row of a turn, and the first row of injected entries. The text of an error is now `error_text`, which the notice and the details `error` row share. `RowError` and `lenient_error` are reused by the details reader. `AssistantData` and `UserData` are unchanged.
- `src/harness/opencode/details.rs` (new): reads one `session_message` row with `Db::select_one` (read-only) when the overlay opens and parses `data` with its own tolerant heads (every field `lenient`/`lenient_object`; `content` is only read for a tool part and `state` is skipped). Gives Turn (`provider`, `agent`), Message, Tool call (`start`), Prompt (`time.created`) and Environment (`snapshot.start`, `snapshot.end`). Database gone, row deleted or `data` not an object: each affected group is `unavailable`.
- `src/harness/opencode/discovery.rs`: `OpenCodeHarness::section_details` calls the reader.
- `src/components/details_overlay/source.rs`: `compose` reads the Turn group from the harness too (its rows follow the model's `cost`) and drops a model row that a read row replaces.
- `src/harness/claude/details.rs`, `src/harness/mod.rs`, `tests/claude_details.rs`, `src/components/mod.rs`: mechanical follow-ups of the enum (Claude gives nothing for a row reference; tests use `lines()`); `section_details` is re-exported for tests.
- `tests/fixtures/opencode/fixture.sql`: `msg_main_04` gains `rawFinish` and `snapshot` (the fixture had no snapshot, so no Environment rows could be shown). No other snapshot moved.
- Tests: `src/harness/opencode/details_tests.rs` (adapter), `src/app/browser_tests/opencode_details.rs` (App + 2 snapshots), additions in `sources.rs` and `details_overlay/source.rs`.

## Self-check
- AC: each OpenCode group, a row missing the fields, an unreadable source — `details_tests`: `a_text_section_has_the_turn_message_and_environment_groups`, `thinking_and_notice_sections_have_the_message_group_of_their_row`, `a_tool_call_has_start_then_duration_then_status`, `a_prompt_shows_the_time_created_of_its_row_once_in_place_of_the_timestamp`, `a_prompt_has_the_environment_of_its_row`, `a_system_section_shows_its_timestamp_then_the_snapshot_of_its_first_row`, `a_row_without_the_fields_leaves_them_out_and_is_not_unavailable`, `an_empty_object_is_a_row_without_the_fields`, `a_database_that_is_gone_makes_each_group_unavailable`, `a_row_that_was_deleted_makes_its_groups_unavailable`, `a_data_that_does_not_parse_makes_its_groups_unavailable`, `a_tool_part_that_is_gone_makes_only_the_tool_group_unavailable`, `no_database_connection_means_every_group_is_unavailable`
- AC: Turn has `cost`, `provider`, `agent`; a turn without them leaves them out — `the_turn_group_has_cost_then_provider_then_agent`, `a_turn_without_cost_provider_and_agent_leaves_them_out`, `provider_and_agent_come_from_the_first_assistant_row_of_the_turn`
- AC: `error` row matches the notice — `the_error_row_says_what_the_notice_of_the_same_row_says`, `an_aborted_error_is_the_warning_text_and_an_absent_error_has_no_row`
- AC: Tool call `start`, `duration`, `status`; call and result share one reference — `a_tool_call_has_start_then_duration_then_status`, `a_tool_call_and_its_result_share_one_reference`, App: `a_tool_call_shows_its_start_duration_and_status_in_this_order`
- AC: unexpected JSON type left out, not `unavailable` — `a_value_of_an_unexpected_json_type_is_left_out_without_unavailable`, `a_time_that_is_out_of_range_is_left_out`
- AC: database untouched — `reading_details_leaves_the_database_untouched` (read-only `Db`, file bytes and directory listing unchanged)
- AC: App::update on the fixture shows Message and Environment — `a_text_section_shows_the_message_and_environment_rows_of_its_row`, `a_row_that_is_gone_leaves_an_unavailable_row_and_the_overlay_open`
- AC: assembler parse unchanged — `AssistantData`/`UserData` not edited; `a_row_replaced_in_place_keeps_the_references_of_a_fresh_assembly`, `a_notice_that_goes_loses_its_reference`; all earlier assembler/notice/live tests pass untouched
- AC: gate and budgets — below
- AC: snapshot changes limited to details overlay snapshots at App level — only the two new `.snap` files were added; no existing `.snap` changed (`INSTA_UPDATE=no` run is green)
- UI: OpenCode assistant text section @ 80x24 default — `the_details_of_an_opencode_text_section_80x24_text`
- UI: OpenCode assistant text section @ 120x40 default — `the_details_of_an_opencode_text_section_120x40_text`
- Replacement/ordering in compose: `a_row_that_stands_in_for_the_models_replaces_it_whatever_its_label`, `the_turn_groups_read_rows_follow_the_models`

## Decisions
- The footer gets the Turn group (provider, agent) via the turn's first row, and nothing else; the footer has no row of its own.
- A tool part is found by its `id`, or by `<row id>:<index>` when it has none (as the assembler names it).
- A `content` part that is not an object makes only the Tool call group unavailable.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2893 passed, 0 failed; `INSTA_UPDATE=no`)
- cargo test --release -- --ignored budgets --nocapture: pass (mixed full parse worst 145 ms, peak heap 0.22x; dense 463 ms, 0.63x)
