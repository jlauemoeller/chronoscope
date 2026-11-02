VERDICT: FAIL

## Summary
All four items of the Approver's rework are done and work, and all gates and budgets pass. The implementation meets every criterion, as far as I traced and probed it. One must-fix remains, in a test only: the test for "an unexpected JSON type in any detail field is left out" sets every odd value in one row, so it would still pass if a wrong type in `model` or in a tool part's `time` or `id` gave `unavailable`, or if a wrong type in a `time.*` or `snapshot.*` field hid the valid fields beside it. All results below come from a fresh export of `HEAD` (2490755) in its own directory with its own target directory, and `INSTA_UPDATE=no`.

## Acceptance criteria
- [x] Adapter tests cover each OpenCode group of §5.3, a row missing the fields, and an unreadable source — `src/harness/opencode/details.rs`; `details_tests.rs`: `a_text_section_has_the_turn_message_and_environment_groups`, `a_tool_call_has_start_then_duration_then_status`, `a_prompt_shows_the_time_created_of_its_row_once_in_place_of_the_timestamp`, `a_prompt_has_the_environment_of_its_row`, `a_row_without_the_fields_leaves_them_out_and_is_not_unavailable`, `a_database_that_is_gone_makes_each_group_unavailable`, `a_row_that_was_deleted_makes_its_groups_unavailable`, `a_data_that_does_not_parse_makes_its_groups_unavailable`. See finding 3.
- [x] Turn rows include `cost`, `provider` and `agent`, and a turn without them leaves them out — `turn_group` in `details.rs`, `merge` in `details_overlay/source.rs`; `the_turn_group_has_cost_then_provider_then_agent`, `a_turn_without_cost_provider_and_agent_leaves_them_out`. Moving `agent` ahead of the other Turn rows fails `the_turn_group_has_cost_then_provider_then_agent`.
- [x] The `error` row matches the notice's text for the same row — shared `error_text` in `assemble.rs`; `the_error_row_says_what_the_notice_of_the_same_row_says` compares the row with the assembled notice, and now includes `{"type":5,"message":true}`. See finding 2.
- [x] A tool call's group has `start`, `duration`, `status` in that order, and a call and its result share one reference — `tool_group`, `RowRef`; `a_tool_call_has_start_then_duration_then_status`, `a_tool_call_and_its_result_share_one_reference`. `start` now comes from the same part as `duration`: `a_tool_part_sent_twice_in_one_row_is_read_from_the_last`, `a_part_of_another_type_with_the_id_of_a_tool_part_is_not_read`.
- [ ] A test shows an unexpected JSON type in any detail field is left out, without `unavailable` — the behaviour is right, but the test does not show it for eight of the attributes that give it. See finding 1.
- [x] Reading details leaves the database untouched — `Db::open` and `select_one` in `details.rs`; `reading_details_leaves_the_database_untouched` checks the mode, the file's bytes and the directory listing. `reading_details_in_wal_mode_leaves_the_database_file_untouched` is new, and a throwaway check confirmed that the file really is in WAL mode at that point (header bytes 18 and 19 are both 2).
- [x] A test through `App::update` opens the overlay on the OpenCode fixture and shows Message and Environment rows — `src/app/browser_tests/opencode_details.rs`: `a_text_section_shows_the_message_and_environment_rows_of_its_row`.
- [x] The assembler's parse is unchanged — the diff against `main` adds no field to `AssistantData` or `UserData`, and round 3 does not touch `assemble.rs`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — re-run: 2902 passed, 0 failed, 2 ignored, and no `.snap.new` file.
- [x] `cargo test --release -- --ignored budgets --nocapture` stays within budget — re-run: `budgets` passes (mixed session: first content worst 2.5 ms, full parse worst 109.5 ms, peak heap 0.22x, worst frame 3.36 ms; dense session: first content worst 1.7 ms, full parse worst 313.0 ms, peak heap 0.63x, worst frame 1.04 ms; endless turn: full parse 714.0 ms).
- [x] Snapshot changes are limited to details overlay snapshots at App level — the only `.snap` changes against `main` are the two new `opencode_details` files, unchanged in round 3.

## Approver's rework
- Item 1 (`find_part`) is done as written. Deleting `.rev()` fails only `a_tool_part_sent_twice_in_one_row_is_read_from_the_last`. Deleting the `Some("tool")` filter fails only `a_part_of_another_type_with_the_id_of_a_tool_part_is_not_read` and `a_position_that_holds_another_type_now_makes_the_tool_group_unavailable`.
- Item 2 (`Rows::text`) is done. Deleting `.map(str::trim)` fails only `a_text_value_is_trimmed_and_a_blank_one_is_left_out`.
- Items 3 and 4 (the extra `error` shape and the tests) are done, and the round's code changes are limited to `details.rs` and `details_tests.rs`.
- Throwaway tests found no further difference from the assembler: a time that is a float, `1e12` or above the `i64` range gives neither `start` nor `duration`; an `id` that is `5`, `""` or `null` falls back to the position; an `id` with spaces around it matches exactly; and a part without an id is found at its own position when an earlier part also has no id.

## Findings
1. [must-fix] The test for acceptance criterion 5 is ineffective for eight attributes. I removed each `deserialize_with` in `details.rs` in turn and ran the `opencode` tests; with any of these eight removed, all of them still pass. The code itself is correct today, so this is a missing test, not a bug. `a_value_of_an_unexpected_json_type_is_left_out_without_unavailable` (`src/harness/opencode/details_tests.rs:478-508`) misses them for two reasons:
   - **Three would give `unavailable`, which the criterion forbids.** The test's "whole objects of the wrong type" block uses `"model":[1]`, which serde can read as a struct from an array, so it does not exercise the wrong type it means to. Without `lenient_object` on `model` (`details.rs:289`), a row with `"model":"x"` fails to parse and its groups show `unavailable`. No test has a tool part whose `time` is not an object or whose `id` is not a string (`details.rs:338-341`). Without `lenient`, such a part fails to parse and is dropped, and the Tool call group shows `unavailable`.
   - **Five would hide valid fields.** The test sets all the odd values in one row. If `time.created`, `time.streamed`, `time.completed` (`details.rs:303-308`), `snapshot.start` or `snapshot.end` (`details.rs:319-322`) loses its `lenient`, its whole object falls back to `None`, which looks the same as every field being left out on its own. One bad `time.created` would then hide a valid `time.completed`, and one bad `snapshot.end` a valid `snapshot.start`, although §5.3 leaves out only the fields that are missing.

   Changing `"end":null` to `"end":7` alone is not enough, because `"start":false` beside it masks the result (tried).

   Change: add this test after `a_value_of_an_unexpected_json_type_is_left_out_without_unavailable`. On unchanged code it passes, fmt and clippy are clean, and it fails for each of those eight removals. Removing `lenient` from `provider_id` or the part's `kind` stays undetected and has no visible effect: `ModelHead` has no other field to lose, and a part that fails to parse is dropped just as the `Some("tool")` filter would drop it. The existing test catches the remaining six removals.
   ```rust
   #[test]
   fn a_value_of_an_unexpected_json_type_leaves_out_that_field_alone() {
       const ROW_TIME: [&str; 3] = ["time.created", "time.streamed", "time.completed"];
       let message = |missing: &str| -> Vec<&str> {
           let mut all = vec!["row id"];
           all.extend(ROW_TIME);
           all.extend(["finish", "rawFinish", "error"]);
           all.into_iter().filter(|l| *l != missing).collect()
       };
       for (from, to, missing) in [
           (
               r#""created":1700000001000,"streamed""#,
               r#""created":"soon","streamed""#,
               "time.created",
           ),
           (
               r#""streamed":1700000003000"#,
               r#""streamed":1.5"#,
               "time.streamed",
           ),
           (
               r#""completed":1700000004000"#,
               r#""completed":[1]"#,
               "time.completed",
           ),
       ] {
           let fx = full();
           let turns = fx.turns();
           fx.set_data("a1", &FULL.replace(from, to));
           let content = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("a1:0")));
           assert_eq!(labels(&content, "Message"), message(missing), "{to}");
       }
       for (from, to, left) in [
           (r#""start":"aaa111""#, r#""start":false"#, "snapshot.end"),
           (r#""end":"bbb222""#, r#""end":7"#, "snapshot.start"),
       ] {
           let fx = full();
           let turns = fx.turns();
           fx.set_data("a1", &FULL.replace(from, to));
           let content = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("a1:0")));
           assert_eq!(labels(&content, "Environment"), [left], "{to}");
       }
       // A `model` that is not an object.
       let fx = full();
       let turns = fx.turns();
       fx.set_data(
           "a1",
           &FULL.replace(
               r#""model":{"id":"acme/coder-1","providerID":"acme","variant":"high"}"#,
               r#""model":"x""#,
           ),
       );
       let content = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("a1:0")));
       assert!(!labels(&content, "Turn").contains(&"provider"));
       assert!(labels(&content, "Turn").contains(&"agent"));
       assert!(!has_unavailable(&content, "Turn"));
       // A tool part's `time` that is not an object leaves out `start` alone.
       let fx = full();
       let turns = fx.turns();
       fx.set_data(
           "a1",
           &FULL.replace(
               r#""time":{"created":1700000001500,"ran":1700000001600,"completed":1700000002500}"#,
               r#""time":"x""#,
           ),
       );
       let content = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("call-1")));
       assert_eq!(labels(&content, "Tool call"), ["duration", "status"]);
       // A tool part's `id` of another type: named by its position, as assembled.
       let data = FULL.replace(r#""id":"call-1""#, r#""id":5"#);
       let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", &data)]);
       assert_eq!(
           labels(&fx.item(1, "a1:2"), "Tool call"),
           ["start", "duration", "status"]
       );
   }
   ```
2. [nit] With `finish: "error"`, an `error` that is missing, `null` or `12` gives the notice `error` and no `error` row (confirmed by a throwaway test), and no test pins this. The behaviour follows §5.3, since a missing field is left out. The comment at `src/harness/opencode/details.rs:162-163` reads as if every `error` notice has a row. The Code Reviewer reports the same. Change: add a test that `finish: "error"` with `error` missing or `12` has a notice and no `error` row, and reword the comment to say that a missing or wrongly typed `error` is left out even when the notice says `error`.
3. [nit] No test deletes a turn's first row and reads a section from another row, or the reverse. `a_row_that_was_deleted_makes_its_groups_unavailable` uses one row for both. A throwaway test confirmed the behaviour is right: with `a1` gone, section `a2:0` shows `unavailable` in Turn only; with `a2` gone, it shows `unavailable` in Message and Environment only, and Turn keeps `provider`. Change: add that case to `details_tests.rs`, so that "each affected group" is pinned for groups read from different rows.
