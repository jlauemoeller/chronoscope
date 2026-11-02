VERDICT: FAIL

## Summary
Every round-2 finding the verdict asked for is fixed, and each of the rework note's 12 temporary edits fails the test it names, checked in a private export of HEAD (78ff001). The production code behaved correctly everywhere I probed. The review fails only because two more required behaviours have no test that fails if they break: `unavailable` when a tool call's own line cannot be read, and the source reference of a queued prompt. The tests proposed below were tried on that export: each passes on HEAD and with `cargo fmt --check` and clippy, and with the named edit each is the only test that fails.

Note on method: all results here come from that private export, with its own build target. An earlier run in the shared `scratchpad/copy` collided with another process editing the same tree and was discarded. My writes and restores there may in turn have disturbed that process, so scratch results from `copy` between about 18:44 and 18:56 may be unreliable.

Possible flake, not against this task and not in this branch's diff: `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` (`tests/index_stub.rs:307`) failed once in about 30 full suite runs on a busy machine, then passed on a rerun with the same edit and in 3 more runs of that test binary. I did not capture its message or find the cause, so it may be worth a backlog note.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2847 passed, 0 failed, 2 ignored; no `.snap.new` file, working tree unchanged)
- cargo test --release -- --ignored budgets --nocapture (also required by §7): pass. Mixed: full parse 139.7 ms, peak heap 0.22x, worst frame 4.67 ms, details of the largest tool result 1.2 ms. Dense: full parse 294.6 ms, peak heap 0.63x, worst frame 0.96 ms. One endless turn (70 MB): 672.2 ms.

## Findings
1. [must-fix] `src/harness/claude/details.rs:162` — nothing tests the `call.is_none()` half of `unavailable`. Removing it leaves the whole suite passing, and a pending call whose file is gone then shows its Tool call group with no `unavailable` row. §5.3 and the planner notes require one in each group whose source could not be read, and "the file is gone" with a pending call is reachable when a live session's transcript is deleted. The existing tests miss it because `a_file_that_is_gone_makes_every_group_of_the_section_unavailable` uses a call with a result, whose failed result line sets the flag anyway, and `a_result_line_that_fails_leaves_start_and_marks_the_tool_call_unavailable` breaks only the result line. Change: add this test to `tests/claude_details.rs`, after `a_result_line_that_fails_leaves_start_and_marks_the_tool_call_unavailable`:
   ```rust
   #[test]
   fn a_call_line_that_fails_marks_the_tool_call_unavailable() {
       // A call with no result yet, whose file is gone.
       let t = transcript(&[prompt_line(), first_line(), call_line()]);
       let source = t.item("c1", SourceKind::ToolCall);
       fs::remove_file(&t.path).unwrap();
       assert_eq!(
           unavailable(&read(&source)),
           [SourceGroupKind::ToolCall, SourceGroupKind::Environment]
       );
       // Only the call's line is broken: the result's rows stay.
       let t = full();
       let source = t.item("c1", SourceKind::ToolCall);
       let mut bytes = fs::read(&t.path).unwrap();
       bytes[usize::try_from(source.refs.line.range.start).unwrap()] = b'(';
       fs::write(&t.path, bytes).unwrap();
       let groups = read(&source);
       let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
       assert!(call.lead.is_empty(), "{call:?}");
       assert!(call.rows.contains(&"isAsync=true".to_owned()), "{call:?}");
       assert!(call.unavailable);
   }
   ```
   Check: with `unavailable: matches!(result, Some(None)),` at `details.rs:162`, this is the only test that fails.

2. [must-fix] `src/harness/claude/assemble.rs:541` — a human `queued_command` starts a User turn through `on_attachment`, the second of the two paths that create a prompt, and nothing tests its reference. Passing `&(0..0)` instead of `range` there leaves the whole suite passing, and the prompt then shows `unavailable` in its Prompt and Environment groups on a readable file. The planner notes require "a prompt its entry's line", and only the `user` entry path has a test. This is the same kind of gap as round 2's finding 3. Change: add this test to `tests/claude_details.rs`, after `the_prompt_group_lists_its_fields`:
   ```rust
   #[test]
   fn a_queued_prompt_reads_the_prompt_group_of_its_own_line() {
       let queued = json!({
           "type": "attachment", "uuid": "q1", "timestamp": "2026-01-01T00:00:05Z",
           "permissionMode": "plan", "gitBranch": "queued", "cwd": CWD,
           "attachment": {"type": "queued_command", "prompt": "Also this.", "origin": {"kind": "human"}}
       });
       let t = transcript(&[prompt_line(), first_line(), queued]);
       let turn = t.turns.last().unwrap();
       assert!(matches!(&**turn, Turn::User(_)));
       let id = SectionId {
           turn: turn.id().clone(),
           key: SectionKey::Prompt,
       };
       let source = section_source(turn, &id).expect("a source for the prompt");
       let groups = read(&source);
       let prompt = group(&groups, SourceGroupKind::Prompt).unwrap();
       assert_eq!(
           prompt.rows,
           ["timestamp=2026-01-01T00:00:05+00:00", "permissionMode=plan"]
       );
       assert_eq!(environment_branch(&source), "gitBranch=queued");
   }
   ```
   Check: with `&(0..0)` as the last argument at `assemble.rs:541`, this is the only test that fails.

3. [nit] `src/harness/claude/details.rs:53-61`, `:621-626` — the unit test `only_a_json_object_is_a_line` calls a test-only `parse` helper that copies the body of `read_line`, so it does not exercise the production code. Removing `json.end().ok()?;` from `read_line` leaves the suite passing, and a line with trailing content would then read as valid. Other tests cover unparseable lines and non-objects through the harness, so only the trailing-content case is open. Change: move the parsing into `fn parse_line(bytes: &[u8]) -> Option<Line>`, call it from `read_line`, and have the tests call it instead of `parse`.

4. [nit] `src/harness/claude/details.rs:142-146`, `:150-153`, `:86-95` — an empty string at the top of the line hides a real value in the other location. With `"permissionDecision": ""` and `"resolvedModel": ""` at the top and both set in `toolUseResult`, neither row appears (tried on the export). `diagnostics.cache_miss_reason` has the same shape. `Rows::text` treats an empty string as no value, but the `.or_else` has already chosen it. No such data is known. Change: filter empty strings before the `.or_else`, for example with a small `fn non_empty(value: Option<&str>) -> Option<&str>`.

5. [nit] `src/harness/claude/details.rs:229` — the fallback that shows an unparseable timestamp as written has no test. Changing it to an empty `Text` leaves the suite passing. The spec does not pin this fallback. Change: add `line["timestamp"] = json!("yesterday");` to a small adapter test and assert `timestamp=yesterday`.

6. [nit] `src/harness/claude/assemble.rs:1040` — the `changed` result of `set_footer` has no test. Ignoring it leaves the suite passing. A later `turn_duration` line that leaves the stats unchanged would then not reach the transcript, so the footer's Environment stays on the earlier line until the next update of that turn. That needs two entries giving the same duration, so it is rare. Change: add a live test like `a_later_line_of_a_message_updates_the_turn_even_when_nothing_visible_changes`, with two `turn_duration` lines of equal timestamp in separate `push_chunk` calls, and assert the second call returns an `Updated`.
