VERDICT: FAIL

## Summary
Everything the round-3 rework note asked for is done: the seven tests and the changed tests match it, and all twelve temporary edits in its "Done when" fail exactly the named test, re-run on a private copy of HEAD. The behaviour matches the spec everywhere I traced or probed it, and the gates and budgets pass. It fails only on test coverage: five rules the spec or planner notes bind work today but can be broken with the full suite green, and none was in an earlier round's list. No production code needs to change: the fix is the five tests below, and each one passes on HEAD and fails with its edit.

## Acceptance criteria
- [x] Adapter tests cover each Claude group of §5.3, a line missing the fields, an unreadable source, and a `toolUseResult` string — `tests/claude_details.rs`: `the_message_group_lists_the_fields_of_the_message_in_order`, `a_tool_calls_start_is_from_its_call_and_the_rest_from_its_result`, `the_prompt_group_lists_its_fields`, `the_footer_reads_its_turn_duration_line_for_the_environment_only`, `missing_fields_are_left_out_and_a_group_with_no_rows_is_empty`, `a_file_that_is_gone_makes_every_group_of_the_section_unavailable`, `a_tool_use_result_that_is_a_string_gives_the_top_level_fields_only`. The Turn group is the model's and is covered through the App by `a_text_section_shows_the_message_and_environment_rows_of_its_source`. A prompt from a queued command has no test: finding 4.
- [x] Assembling does not parse the detail fields, and the budgets hold — `assembling_a_line_with_detail_fields_of_unexpected_types_and_sizes_is_cheap`. I re-ran the budget command and it passes: dense full parse worst 316.7 ms, peak heap 0.63x, one endless turn 681.7 ms, details of the largest result 1.2 ms.
- [x] `Harness` has the new method with a default, and a stub harness shows the model's rows only — `Harness::section_details` in `src/harness/mod.rs`, `a_harness_provides_no_section_details_by_default`, `a_harness_that_provides_nothing_leaves_the_models_rows_and_no_unavailable_row`.
- [x] `wire::Entry` and the assembler gain no detail fields, and `details.rs` reads `toolUseResult` through a typed head, tested with a very large and a string value — `wire.rs` is not in the diff; `ResultHead` read by `ObjectVisitor`; `a_very_large_tool_use_result_is_skipped_and_its_fields_are_read`, `a_tool_use_result_that_is_a_string_gives_the_top_level_fields_only`.
- [x] A text block on the first line of a multi-line message gets the `stop_reason` of a later line — `a_text_block_on_the_first_line_of_a_message_gets_the_stop_reason_of_a_later_line`, and `a_line_after_the_stopped_line_leaves_the_message_group_on_the_stopped_line`. The rule for two stopped lines has no test: finding 2.
- [x] A tool call's permission, sandbox and `isAsync` rows come from its result's line and its `start` from its call's line — `tool_group` in `details.rs`; `a_tool_calls_start_is_from_its_call_and_the_rest_from_its_result`.
- [x] A subagent transcript's section reads its own file — `a_subagent_transcript_reads_its_own_file`, `a_loaded_subagent_names_its_own_file`.
- [ ] File gone, range past the end and an unparseable line give one muted `unavailable` row in each affected group, no panic, overlay open — met for a text section and for a tool call whose result line fails (`a_file_that_is_gone_leaves_an_unavailable_row_per_group_and_the_overlay_open`, `a_range_past_the_end_of_the_file_leaves_an_unavailable_row_per_group`, `a_line_that_does_not_parse_leaves_an_unavailable_row_per_group`, `a_result_line_that_fails_leaves_start_and_marks_the_tool_call_unavailable`). The code is also right for a pending call whose file is gone and for bytes after a line's object, but no effective test covers them: findings 1 and 3.
- [x] A tool call with no result yet opens with no `unavailable` row — `a_call_without_a_result_yet_opens_with_no_unavailable_row`, `a_call_without_a_result_yet_is_not_unavailable`.
- [x] Missing fields and empty groups are left out, and Claude shows no `cost` row — `missing_fields_are_left_out_and_a_group_with_no_rows_is_empty`, `a_group_the_harness_read_nothing_of_is_left_out`, `claude_records_no_cost_so_none_is_shown`.
- [x] A test through `App::update` opens the overlay on a Claude fixture and shows Message and Environment rows — `a_text_section_shows_the_message_and_environment_rows_of_its_source`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — re-run: clean, clean, 2847 passed, 0 failed, 2 ignored.
- [x] Snapshot changes are limited to App-level details overlay snapshots — four App-level snapshots are added, no transcript snapshot changes, and no `.snap` file changes in this round. The one component-level change, `unavailable_80x24_default`, was accepted by the Approver in the round-3 note.

## Findings
All five must-fix findings are missing or ineffective tests, and the code is correct today in each case. Put the tests in `tests/claude_details.rs`. I ran the exact test code below: each test passes on HEAD and is the only test in `tests/claude_details.rs` to fail with the edit its finding names. Without these tests, each of those edits also leaves the full `cargo test` green.

1. [must-fix] Nothing tests that a tool call's group is `unavailable` when its own call line cannot be read. The planner notes bind this ("each group whose rows depend on a source that could not be read"), and the criterion says "each affected group". The edit: `src/harness/claude/details.rs:162` becomes `unavailable: matches!(result, Some(None)),`. A pending call whose file is gone then shows a Tool call group with no `start` and no `unavailable` row. The existing file-gone test uses a call that has a result, so the result line's failure sets the flag anyway. Change: add this test after `a_call_without_a_result_yet_is_not_unavailable`:
   ```rust
   #[test]
   fn a_call_without_a_result_whose_file_is_gone_is_unavailable() {
       let t = transcript(&[prompt_line(), first_line(), call_line()]);
       let source = t.item("c1", SourceKind::ToolCall);
       fs::remove_file(&t.path).unwrap();
       assert_eq!(
           unavailable(&read(&source)),
           [SourceGroupKind::ToolCall, SourceGroupKind::Environment]
       );
   }
   ```

2. [must-fix] Nothing tests that the *latest* of two lines with a non-null `stop_reason` wins. §4.2 says "Use the last non-null value per message id", and the planner notes say "the message's latest line with a non-null `stop_reason`". The edit: `src/model/sources.rs:127` becomes `if stopped && lines.stopped.is_none() {`, so the first stopped line wins. Every test has at most one stopped line per message, including the table's own `a_message_item_gets_the_latest_stopped_line_else_the_latest`. Change: add this test after `a_line_after_the_stopped_line_leaves_the_message_group_on_the_stopped_line`:
   ```rust
   #[test]
   fn the_latest_of_two_stopped_lines_gives_the_message_group() {
       let mut second = call_line();
       second["uuid"] = json!("a3");
       second["timestamp"] = json!("2026-01-01T00:00:03Z");
       second["gitBranch"] = json!("second-stop");
       second["message"]["stop_reason"] = json!("end_turn");
       second["message"]["content"] = json!([]);
       let t = transcript(&[prompt_line(), first_line(), call_line(), second]);
       let groups = read(&t.item("m1:0", SourceKind::Message));
       let message = group(&groups, SourceGroupKind::Message).unwrap();
       assert!(
           message.rows.contains(&"stop_reason=end_turn".to_owned()),
           "{message:?}"
       );
       assert_eq!(
           group(&groups, SourceGroupKind::Environment).unwrap().rows,
           environment("second-stop")
       );
   }
   ```

3. [must-fix] The test for bytes after a line's object is ineffective. The planner notes make a line that does not parse as JSON `unavailable`. `only_a_json_object_is_a_line` in `src/harness/claude/details.rs` checks `{} {}` through the test module's `parse`, a copy of `read_line`, so the production check is not exercised. The edit: remove `json.end().ok()?;` at `details.rs:59`. Change: add this test after `a_line_that_does_not_parse_is_unavailable`:
   ```rust
   #[test]
   fn a_line_with_bytes_after_its_object_is_unavailable() {
       let line = SectionSource {
           kind: SourceKind::Prompt,
           refs: chronoscope::model::SectionRefs {
               line: SourceRef {
                   bytes: ByteSource::Memory(Arc::from(&b"{} {}"[..])),
                   range: 0..5,
               },
               result: None,
           },
       };
       assert_eq!(
           unavailable(&read(&line)),
           [SourceGroupKind::Prompt, SourceGroupKind::Environment]
       );
   }
   ```

4. [must-fix] A prompt that starts from a human `queued_command` attachment has no reference test. The planner notes bind "a prompt its entry's line", and this is the other caller of `start_user_turn`. The edit: `&(0..0)` instead of `range` at `src/harness/claude/assemble.rs:541`, which makes every queued prompt's groups `unavailable`. Change: add this test after `a_summary_without_a_boundary_reads_the_environment_of_its_own_line`:
   ```rust
   #[test]
   fn a_prompt_from_a_queued_command_reads_its_own_line() {
       let queued = json!({
           "type": "attachment", "uuid": "q1", "timestamp": "2026-01-01T00:02:00Z",
           "permissionMode": "plan", "gitBranch": "queued", "cwd": CWD,
           "attachment": {"type": "queued_command", "prompt": "more", "origin": {"kind": "human"}}
       });
       let t = transcript(&[prompt_line(), first_line(), queued]);
       let turn = t.turns.len() - 1;
       assert!(matches!(&*t.turns[turn], Turn::User(_)));
       let id = SectionId {
           turn: t.turns[turn].id().clone(),
           key: SectionKey::Prompt,
       };
       let source = section_source(&t.turns[turn], &id).expect("a source for the prompt");
       assert_eq!(source.kind, SourceKind::Prompt);
       let groups = read(&source);
       assert_eq!(
           group(&groups, SourceGroupKind::Prompt).unwrap().rows,
           ["timestamp=2026-01-01T00:02:00+00:00", "permissionMode=plan"]
       );
       assert_eq!(environment_branch(&source), "gitBranch=queued");
   }
   ```

5. [must-fix] Nothing tests that a later `turn_duration` line updates the turn when the stats stay the same. The planner notes bind "the footer references its last `turn_duration` line", and a live session goes through this path (spec/SPEC.md §7.1). `a_later_line_of_a_message_updates_the_turn_even_when_nothing_visible_changes` already pins the same rule for message lines. The edit: ignore the result of `set_footer` at `src/harness/claude/assemble.rs:1040` and start from `let mut changed = false;`. The turn is then not re-sent, so the overlay reads the previous footer line. Change: add this test after `a_later_line_of_a_message_updates_the_turn_even_when_nothing_visible_changes`:
   ```rust
   #[test]
   fn a_later_footer_line_updates_the_turn_even_when_the_stats_do_not_change() {
       let mut last = duration_line();
       last["uuid"] = json!("d2");
       last["gitBranch"] = json!("last-footer");
       let first = format!("{}\n{}\n{}\n", prompt_line(), first_line(), duration_line());
       let second = format!("{last}\n");
       let bytes: Arc<[u8]> = Arc::from(format!("{first}{second}").as_bytes());
       let mut asm = Assembler::new(ByteSource::Memory(bytes));
       asm.push_chunk(first.as_bytes());
       let updates = asm.push_chunk(second.as_bytes());
       assert!(
           updates
               .iter()
               .any(|u| matches!(u, chronoscope::model::ModelUpdate::Updated(t) if t.len() == 1)),
           "{updates:?}"
       );
   }
   ```

6. [nit] When a tool call's line fails but its result line reads, the result's rows are kept next to the `unavailable` row, and no test shows it. Finding 1 pins the `unavailable` row, and the spec does not say whether those rows stay. Change: if they are meant to stay, add a test that breaks the first byte of the call line in `full()` and asserts a non-empty `rows` with `unavailable` set.

7. [nit] `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` in `tests/index_stub.rs` failed once in about 30 full runs on a loaded machine. It passed on the next run of the same code. That file is not in this branch's diff and the test has nothing to do with this task, so it does not block. Change: open a follow-up to check that test for a timing dependency.
