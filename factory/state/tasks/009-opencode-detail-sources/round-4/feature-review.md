VERDICT: PASS

## Summary
All four items of the Approver's round-4 note are done as written, and the round changes only `details_tests.rs` and one comment in `details.rs`. The must-fix from round 3 is closed: every mutation the note lists now fails exactly the tests it names, so every acceptance criterion is met and covered by an effective test. All results come from a private export of `HEAD` (7ce4bc7) with its own target directory and `INSTA_UPDATE=no`.

## Acceptance criteria
- [x] Adapter tests cover each OpenCode group of §5.3, a row missing the fields, and an unreadable source — `src/harness/opencode/details.rs`; `details_tests.rs`: `a_text_section_has_the_turn_message_and_environment_groups`, `a_tool_call_has_start_then_duration_then_status`, `a_prompt_shows_the_time_created_of_its_row_once_in_place_of_the_timestamp`, `a_prompt_has_the_environment_of_its_row`, `a_row_without_the_fields_leaves_them_out_and_is_not_unavailable`, `a_database_that_is_gone_makes_each_group_unavailable`, `a_row_that_was_deleted_makes_its_groups_unavailable`, `a_deleted_row_makes_only_the_groups_read_from_it_unavailable` (new), `a_data_that_does_not_parse_makes_its_groups_unavailable`.
- [x] Turn rows include `cost`, `provider` and `agent`, and a turn without them leaves them out — `turn_group` in `details.rs`, `merge` in `details_overlay/source.rs`; `the_turn_group_has_cost_then_provider_then_agent`, `a_turn_without_cost_provider_and_agent_leaves_them_out`.
- [x] The `error` row matches the notice's text for the same row — shared `error_text` in `assemble.rs`; `the_error_row_says_what_the_notice_of_the_same_row_says` compares the row with the assembled notice. `an_error_missing_or_of_the_wrong_type_is_no_row_though_the_notice_says_error` (new) pins the case the Approver ruled on in round 3.
- [x] A tool call's group has `start`, `duration`, `status` in that order, and a call and its result share one reference — `tool_group`, `RowRef`; `a_tool_call_has_start_then_duration_then_status`, `a_tool_call_and_its_result_share_one_reference`.
- [x] An unexpected JSON type in any detail field is left out, without `unavailable` — `lenient` and `lenient_object` on the heads in `details.rs`; `a_value_of_an_unexpected_json_type_is_left_out_without_unavailable`, and `a_value_of_an_unexpected_json_type_leaves_out_that_field_alone` (new). This was the open box in round 3. The new test fails for each of the eight attributes (see below).
- [x] Reading details leaves the database untouched — `Db::open` and `select_one` in `details.rs`; `reading_details_leaves_the_database_untouched` checks the read-only mode, the file's bytes and the directory listing. `reading_details_in_wal_mode_leaves_the_database_file_untouched` now also shows the database was really read.
- [x] A test through `App::update` opens the overlay on the OpenCode fixture and shows Message and Environment rows — `src/app/browser_tests/opencode_details.rs`: `a_text_section_shows_the_message_and_environment_rows_of_its_row`.
- [x] The assembler's parse is unchanged — the diff against `main` adds no field to `AssistantData` or `UserData`, and round 4 does not touch `assemble.rs`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — re-run: both clean; 2905 passed, 0 failed, 2 ignored; no `.snap.new` file.
- [x] `cargo test --release -- --ignored budgets --nocapture` stays within budget — re-run: `budgets` passes. Mixed session: first content worst 2.5 ms, full parse worst 155.0 ms, peak heap 0.22x, worst frame 5.47 ms. Dense session: first content worst 2.1 ms, full parse worst 609.5 ms, peak heap 0.63x, worst frame 3.09 ms. Endless turn: full parse 1079.5 ms.
- [x] Snapshot changes are limited to details overlay snapshots at App level — the only `.snap` changes against `main` are the two new `opencode_details` files, and round 4 changes no `.snap` file.

## Approver's rework
Items 1 to 3 match the note's code, in the places it names, and item 4 replaces the line it names. The new comment in `message_group` is the note's wording, and it describes what the code does. I applied each temporary edit from the note alone to the export, and ran `INSTA_UPDATE=no cargo test --lib opencode`:

| Edit | Result |
|---|---|
| `#[serde(default)]` on `Head::model`, `TimeHead::created`, `TimeHead::streamed`, `TimeHead::completed`, `SnapshotHead::start`, `SnapshotHead::end`, `PartHead::id`, `PartHead::time`, one at a time | each fails only `a_value_of_an_unexpected_json_type_leaves_out_that_field_alone` |
| The note's `let error = ...;` | fails only `an_error_missing_or_of_the_wrong_type_is_no_row_though_the_notice_says_error` |
| The note's `let read = ...;` | fails only `a_deleted_row_makes_only_the_groups_read_from_it_unavailable` |
| `read_groups(None, source)` | fails 36 tests, among them `reading_details_in_wal_mode_leaves_the_database_file_untouched` |

## Findings
1. [nit] An OpenCode footer shows only the Turn group, while §5.3 gives Environment to "every section with a source". `RowRefs` documents that a footer has no row of its own, and `the_footer_has_the_turn_group_and_nothing_read_from_a_row` (`src/harness/opencode/details_tests.rs`) pins it. The reading is defensible, because a footer sums several rows and the task's planner notes give the turn's row only to the Turn group's `provider` and `agent`, but no spec text or planner note states it. Change: no code change. If the spec is revised, say in §5.3 that an OpenCode footer has no Environment group.
