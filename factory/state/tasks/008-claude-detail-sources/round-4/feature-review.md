VERDICT: FAIL

## Summary
The round-4 note is done as written, all five round-3 must-fix findings are closed, and the gates and budgets pass. One new behaviour bug fails the review: a non-object `toolUseResult` that the assembler accepts, but the details reader cannot decode, marks the Tool call group `unavailable` and loses the line's other rows. No real transcript here holds such a value, and the fix below is about ten lines of production code and has been tried on a private copy.

## Rework note and round-3 findings
- Round-3 finding 1 (a call's own line fails): closed by `a_call_line_that_fails_marks_the_tool_call_unavailable`. It also closes round-3 nit 6, because it asserts that the result's rows stay.
- Round-3 finding 2 (latest of two stopped lines): closed by `the_latest_of_two_stopped_lines_gives_the_message_group`.
- Round-3 finding 3 (bytes after the object): closed by `parse_line` in `src/harness/claude/details.rs`, which `only_a_json_object_is_a_line` now runs through the test helper `parse`. No adapter test was added, as the note ruled.
- Round-3 finding 4 (queued prompt): closed by `a_queued_prompt_reads_the_prompt_group_of_its_own_line`.
- Round-3 finding 5 (later footer line): closed by `a_later_footer_line_updates_the_turn_even_when_the_stats_do_not_change`.
- `a_timestamp_that_does_not_parse_is_shown_as_written` and the added values (`"promptSource": 2.5`, `-7`, `2.5`, `below=-2, ratio=2.5`, `-5`) are present.
- The note's "Leave as is" items are untouched. `git diff 2c7c470 HEAD -- src tests` shows only `details.rs` and `tests/claude_details.rs`.

## Independent checks
All of these ran on a fresh private export of HEAD (3375e03) with its own target directory and `INSTA_UPDATE=no`.
- **Gates:** `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean. `cargo test` gives 2852 passed, 0 failed, 2 ignored, both before the edits and with the code restored after them.
- **Temporary edits:** each of the note's 14 edits, run alone on the full suite, fails exactly its named test. The `Member` `visit_unit` edit also failed `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` on its first run. On its rerun it failed only the named test (see finding 3). No `.snap.new` file was written.
- **Budgets:** `cargo test --release -- --ignored budgets --nocapture` passes.

  | Fixture | Worst first content | Worst full parse | Peak heap | Worst frame | Details of largest result |
  |---|---|---|---|---|---|
  | Mixed | 2.5 ms | 122.9 ms | 0.22x | 3.26 ms | 1.2 ms (2 MB) |
  | Dense | 1.7 ms | 309.0 ms | 0.63x | 2.70 ms | 0.1 ms |
  | One endless turn | not measured | 1060.6 ms | not measured | not measured | not measured |

- **Real transcripts (read-only):** the reader ran over all 912 transcripts under `~/.claude/projects`. All 29,920 sections have a source and no group is `unavailable`. Every field of the Message, Tool call, Prompt and Environment groups appears at least once.
- **Live path (scratch test through `ClaudeHarness::load`):**
  - A line appended in two writes, cut mid-line, gives the right references, and the text section moves to the stopped line.
  - A result that arrives later adds its rows to the same call.
  - After the file is replaced by a shorter one, old references read `unavailable` and the reloaded turns read the new file.

## Acceptance criteria
- [x] Adapter tests cover each Claude group of §5.3, a line missing the fields, an unreadable source, and a `toolUseResult` string — `tests/claude_details.rs`: `the_message_group_lists_the_fields_of_the_message_in_order`, `a_tool_calls_start_is_from_its_call_and_the_rest_from_its_result`, `the_prompt_group_lists_its_fields`, `a_queued_prompt_reads_the_prompt_group_of_its_own_line`, `the_footer_reads_its_turn_duration_line_for_the_environment_only`, `missing_fields_are_left_out_and_a_group_with_no_rows_is_empty`, `a_file_that_is_gone_makes_every_group_of_the_section_unavailable`, `a_tool_use_result_that_is_a_string_gives_the_top_level_fields_only`. The Turn group is the model's and is covered through the App by `a_text_section_shows_the_message_and_environment_rows_of_its_source`.
- [x] Assembling does not parse the detail fields, and the budgets hold — `assembling_a_line_with_detail_fields_of_unexpected_types_and_sizes_is_cheap`; budget figures above.
- [x] `Harness` has the new method with a default, and a stub harness shows the model's rows only — `Harness::section_details` in `src/harness/mod.rs`; `a_harness_provides_no_section_details_by_default`, `a_harness_that_provides_nothing_leaves_the_models_rows_and_no_unavailable_row`.
- [x] `wire::Entry` and the assembler gain no detail fields, and `details.rs` reads `toolUseResult` through a typed head, tested with a very large and a string value — `wire.rs` is not in the diff, and the assembler gains only byte ranges; `ResultHead` is read by `ObjectVisitor`; `a_very_large_tool_use_result_is_skipped_and_its_fields_are_read`, `a_tool_use_result_that_is_a_string_gives_the_top_level_fields_only`. The criterion as written is met, but a string the head cannot decode fails the line: finding 1.
- [x] A text block on the first line of a multi-line message gets the `stop_reason` of a later line — `note_message_line` and `TurnSources::get` in `src/model/sources.rs`; `a_text_block_on_the_first_line_of_a_message_gets_the_stop_reason_of_a_later_line`, `the_latest_of_two_stopped_lines_gives_the_message_group`.
- [x] A tool call's permission, sandbox and `isAsync` rows come from its result's line and its `start` from its call's line — `tool_group` in `details.rs`; `a_tool_calls_start_is_from_its_call_and_the_rest_from_its_result`, and through the App `a_tool_call_shows_its_start_then_the_models_rows_then_the_rest_of_its_result`.
- [x] A subagent transcript's section reads its own file — `a_subagent_transcript_reads_its_own_file`, `a_loaded_subagent_names_its_own_file`.
- [x] File gone, range past the end and an unparseable line give one muted `unavailable` row in each affected group, no panic, overlay open — `a_file_that_is_gone_leaves_an_unavailable_row_per_group_and_the_overlay_open`, `a_range_past_the_end_of_the_file_leaves_an_unavailable_row_per_group`, `a_line_that_does_not_parse_leaves_an_unavailable_row_per_group`, `a_result_line_that_fails_leaves_start_and_marks_the_tool_call_unavailable`, `a_call_line_that_fails_marks_the_tool_call_unavailable`, `only_a_json_object_is_a_line`.
- [x] A tool call with no result yet opens with no `unavailable` row — `a_call_without_a_result_yet_opens_with_no_unavailable_row`, `a_call_without_a_result_yet_is_not_unavailable`.
- [x] Missing fields and empty groups are left out, and Claude shows no `cost` row — `missing_fields_are_left_out_and_a_group_with_no_rows_is_empty`, `a_group_the_harness_read_nothing_of_is_left_out`, `claude_records_no_cost_so_none_is_shown`, and the `cost` assertion in `a_text_section_shows_the_message_and_environment_rows_of_its_source`.
- [x] A test through `App::update` opens the overlay on a Claude fixture and shows Message and Environment rows — `a_text_section_shows_the_message_and_environment_rows_of_its_source`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — re-run, figures above.
- [x] Snapshot changes are limited to App-level details overlay snapshots — four App-level snapshots are added and no transcript snapshot changes. No `.snap` file changes in this round. The one component-level change, `unavailable_80x24_default`, was accepted by the Approver in the round-3 and round-4 notes.

## Findings
1. [must-fix] A non-object `toolUseResult` that the reader only throws away can still fail the line. `ObjectVisitor` reads the field with `deserialize_any` (`src/harness/claude/details.rs:502`), and `serde_json` checks the value before the visitor discards it. Three values trigger this:
   - a string holding a lone surrogate escape (`"cut \ud83d here"`), which a truncated error string that splits a surrogate pair could produce;
   - a string holding invalid UTF-8;
   - a bare `1e999`.

   In a scratch test the assembler read each of these lines with `unparsed=0` and showed the result. The Tool call group then lost its top-level `permissionDecision` and `sandboxed` rows and showed `unavailable`. This breaks the planner note "a field of an unexpected JSON type counts as missing, and it never fails the line", and the line matches none of the note's causes for `unavailable`. `ToolUseResult` and `lenient_entry` in `wire.rs` already guard the assembler against this same input class. The impact is narrow: the overlay stays open, nothing panics, and none of the 143,114 lines under `~/.claude/projects` holds a lone surrogate.

   Change, in `src/harness/claude/details.rs`, in three parts:

   In `parse_line`, read the field in a second parse, so that a failure there only makes it missing:
   ```rust
   let mut line = pick_object::<_, Line>(&mut json).ok()??;
   json.end().ok()?;
   // Skipped above, so that a value it cannot read does not fail the line.
   line.tool_use_result = serde_json::from_slice::<ResultField>(bytes)
       .ok()
       .and_then(|field| field.head);
   Some(line)
   ```
   On `Line`, replace the attribute of `tool_use_result` with `#[serde(skip)]`, and add after `Line`:
   ```rust
   /// A line's `toolUseResult`, read on its own after the line (see
   /// [`parse_line`]).
   #[derive(Deserialize)]
   struct ResultField {
       #[serde(default, rename = "toolUseResult", deserialize_with = "pick_object")]
       head: Option<ResultHead>,
   }
   ```
   In the module doc, add that a key appearing twice inside `toolUseResult` now makes that field missing instead of failing the line.

   Add this test to `tests/claude_details.rs`, before `a_very_large_tool_use_result_is_skipped_and_its_fields_are_read`:
   ```rust
   #[test]
   fn a_tool_use_result_that_is_not_read_never_fails_the_line() {
       // The assembler reads these lines, and the values are only thrown away.
       let cases: [&[u8]; 3] = [br#""cut \ud83d here""#, b"\"bad \xff byte\"", b"1e999"];
       for raw in cases {
           let dir = tempfile::tempdir().unwrap();
           let path = dir.path().join("session.jsonl");
           let line = result_line(json!("RAW")).to_string();
           let (before, after) = line.split_once(r#""RAW""#).unwrap();
           let mut text = format!("{}\n{}\n{}\n", prompt_line(), first_line(), call_line());
           text.push_str(before);
           let mut bytes = text.into_bytes();
           bytes.extend_from_slice(raw);
           bytes.extend_from_slice(after.as_bytes());
           bytes.push(b'\n');
           fs::write(&path, bytes).unwrap();
           let t = Transcript {
               turns: Assembler::from_file(&path).unwrap().into_turns(),
               _dir: dir,
               path,
           };
           let source = t.item("c1", SourceKind::ToolCall);
           assert!(source.refs.result.is_some());
           let groups = read(&source);
           let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
           assert!(!call.unavailable, "{call:?}");
           assert_eq!(
               call.rows,
               [
                   "permissionDecision=decision=accept, reasonType=classifier, source=config",
                   "sandboxed=false",
               ]
           );
       }
   }
   ```
   Tried on the private export (the module-doc sentence is not code and was not part of the run):
   - With only the test added, it is the one failure in the full suite (2852 passed, 1 failed).
   - With the fix, `cargo test` gives 2853 passed, 0 failed, 2 ignored, with no `.snap.new` file. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean.
   - The budgets pass. The largest result's details take 2.1 ms instead of 1.2 ms, because that line is parsed twice. Mixed full parse is 133.3 ms, dense 298.9 ms, endless turn 671.8 ms. Peak heap is unchanged at 0.22x and 0.63x.
2. [nit] A lone surrogate in `diagnostics.cache_miss_reason` at the top of a line also shows `unavailable`, and the fix above does not cover it. That field has the expected type and the value is one the reader actually shows, so the planner note's type rule does not apply, and I saw no such value in the real transcripts. Change: none needed. If it ever appears in real data, read that field in a second parse in the same way.
3. [nit] `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` in `tests/index_stub.rs` failed in 1 of my 19 full runs and passed when that edit was rerun. I did not capture its message. Its file is not in this branch's diff, it has nothing to do with this task, and the round-3 verdict already lists it as needing a home. Change: open a follow-up to check that test for a timing dependency.
