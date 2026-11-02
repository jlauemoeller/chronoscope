VERDICT: REWORK

## Assessment

Round 3 did everything the round-2 note asked, and neither the reviewers nor I found wrong behaviour. What blocks is test coverage again: required behaviours can still break with the whole suite green, and one of them is on an acceptance criterion the Feature Reviewer left unticked. Each gap has one fix, almost all of it tests, so this is a rework and not a human choice.

**Code review: FAIL**, two must-fixes and four nits.
- Finding 1 (must-fix), nothing tests the `call.is_none()` half of `unavailable`: to be fixed. A pending call whose file is gone would lose its `unavailable` row. The criterion asks for one row "in each affected group", and a live session's transcript can be deleted.
- Finding 2 (must-fix), a queued prompt's reference has no test: to be fixed. The planner notes bind "a prompt its entry's line", and a regression would show `unavailable` on a readable file.
- Finding 3 (nit), `only_a_json_object_is_a_line` tests a copy of `read_line`: to be fixed, with feature finding 3. See Conflicts.
- Finding 4 (nit), an empty string at the top of a line hides a value in the other location: ignored. No such data is known, and the effect is one missing row in a rarely read view. The fix adds a filter at three production sites for input nobody has seen. Round 2's empty member was different, because it drew a visible `decision=`.
- Finding 5 (nit), the fallback for an unparseable timestamp has no test: to be fixed, because the round is happening anyway. It is one small test of what the `Rows::time` comment already documents.
- Finding 6 (nit), the `changed` result of `set_footer` has no test: to be fixed, as feature finding 5.
- The note on `a_stale_result_after_a_revert_to_the_sent_key_is_dropped`: ignored for this task. That file is not in the branch's diff.

**Feature review: FAIL**, five must-fixes and two nits.
- Finding 1 (must-fix): the same gap as code finding 1. To be fixed, with the code reviewer's test.
- Finding 2 (must-fix), nothing tests that the latest of two stopped lines wins: to be fixed. §4.2 says "the last non-null value per message id", and the existing tests never have two such lines.
- Finding 3 (must-fix), the `json.end()` check has no effective test: to be fixed. See Conflicts.
- Finding 4 (must-fix): the same gap as code finding 2. To be fixed, with the code reviewer's test.
- Finding 5 (must-fix), a later footer line with unchanged stats does not re-send the turn if `changed` is ignored: to be fixed. The case is rare, because it needs two entries giving the same duration. It would not justify a round alone, but the test is short and the live path is otherwise unpinned.
- Finding 6 (nit), whether the result's rows stay when the call's line fails: closed by the second half of the code reviewer's test. It needs no human. The planner notes put `unavailable` after the rows that are known, the comments on `section_details` and `tool_group` say the group holds what could be read, and the UI review checked this state and found it right.
- Finding 7 (nit), the `index_stub` flake: ignored for this task, and listed below.

**UI review: PASS**, no findings.

**Why the gaps are not minor.** `factory.md` lists "a missing or ineffective test for required behaviour" as a must-fix, and I do not lower these. Each guards a rule from §4.2, §5.3 or the planner notes, and a regression would be silent. A rework budget remains, and the 006 round-3 verdict refused to call the same kind of gap minor in the same position.

**Why this is not an A/B choice.** Each gap has one fix, and none offers the human a functional choice.

**A correction to round 2.** My round-2 verdict said the note was "not the start of an open-ended chase", and that claim was wrong: the reviewers found five more gaps. So I ran my own sweep this round, listed below, and the note also closes the eight further gaps it found. The sweep was hand-picked because `cargo-mutants` is not installed, so it lowers the chance of another gap and does not prove there is none.

**Independent checks**, on a private export of `HEAD` (22745cb) with its own target directory and `INSTA_UPDATE=no`:
- `cargo fmt --check` passes. `cargo test` gives 2847 passed, 0 failed, 2 ignored, and no `.snap.new` file is written. The merge base equals `main`, and no `.snap` file changed in this round.
- With all 14 temporary edits of the note applied together, the full suite on `HEAD` stays green at 2847 passed. So every gap the note closes is real.

With the rework note applied to that export:
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass. `cargo test` gives 2852 passed, 0 failed, 2 ignored. No `.snap` file changes.
- Each of the 14 edits, run alone on the full suite, fails exactly the test the note names.
- `cargo test --release -- --ignored budgets --nocapture` passes. Mixed: full parse worst 97.7 ms, peak heap 0.22x, worst frame 3.19 ms, details of the largest tool result 1.0 ms. Dense: 304.3 ms, 0.63x, 0.99 ms. One endless turn: 675.5 ms.

My sweep, with the note applied:
- **28 edits to the reference links, the reader's field locations and the composition:** all caught. These ran on the library and `tests/claude_details.rs` only, which is enough for an edit that is caught.
- **38 edits, each deleting one `visit_*` method of the four lenient visitors in `details.rs`:** 24 were caught by the tests as they stood. Eight survived and guard the planner rule that a field of an unexpected type "never fails the line" (`visit_f64` on the scalar reader; `visit_i64` and `visit_f64` on the object and text readers; `visit_i64`, `visit_f64` and `visit_unit` on `Member`). The note closes them by adding values to three existing tests. The other six are `visit_none` and `visit_some` on three visitors. They are equivalent: `serde_json`'s `deserialize_any` calls `visit_unit` for `null` and never reaches them, so they get no test.

**Not blocking, but these still need a home.** A search of `backlog.md` and the `task.md` of tasks 009 to 014 finds none of them:
- **The real load path on one long turn.** Round 2's feature review measured `ClaudeHarness::load` on the 70 MB single turn at 3.4 s on this branch and 4.0 s on `main`, over the 2 s budget on both. Those were single runs, the cause was not profiled, and I have not measured it myself.
- **The control-character filter in `display_lines`** (`src/components/details_overlay/content.rs`), recommended by the 007 round-4 verdict. The UI review's control-character render found no escape byte reaching the screen this round, and the input that matters is narrow.
- **The possible flake in `a_stale_result_after_a_revert_to_the_sent_key_is_dropped`** (`tests/index_stub.rs`). Two reviewers each report one failure in about 30 full runs on a loaded machine. Its message was not captured, and I did not reproduce it.

## Conflicts

No reviews make opposing demands about behaviour. The UI review raises nothing. Three smaller points, of which the first two use the priority order:

1. **How to pin `json.end()`.** The code reviewer rates this a nit and moves the parsing into `parse_line`, so the existing unit test runs production code. The feature reviewer rates it a must-fix and adds an adapter test. The gap is real, and the note takes the code reviewer's fix by the priority order. It also removes a test-only copy of production code, which the feature reviewer's test would leave in place. The adapter test is dropped because it would then add no coverage. The cost is one small production move with no change in behaviour.
2. **Which test for the two shared gaps.** Both reviews offer a test for the call's line and for the queued prompt. The note uses the code reviewer's, by the priority order. For the call's line it also covers the case in feature finding 6, and for the prompt the feature reviewer's `kind` assertion adds nothing, because reading the Prompt group already requires that kind.
3. **The severity of the footer gap.** The code reviewer rates it a nit and the feature reviewer a must-fix. Both want the same kind of test, so the note uses the feature reviewer's exact one and the difference changes nothing.

## Rework budget

Earlier REWORK verdicts: 1 of 2 (round 2). Round 1 has no `verdict.md`, and `round-3/approver-review.md` is the copied note, not a verdict. This verdict is the second and last REWORK, so the next round can only be approved or escalated.

## Rework note

The behaviour is right. This round adds five tests, adds values to three existing tests, and moves the parsing of a line into its own function so a unit test exercises it. No behaviour changes. All the code below has been tried on the current branch.

**1. New tests in `tests/claude_details.rs`.**

After `the_prompt_group_lists_its_fields`:

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

After `a_line_after_the_stopped_line_leaves_the_message_group_on_the_stopped_line`:

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

Before `an_empty_string_is_no_value`:

```rust
#[test]
fn a_timestamp_that_does_not_parse_is_shown_as_written() {
    let mut line = prompt_line();
    line["timestamp"] = json!("yesterday");
    let t = transcript(&[line]);
    let groups = read(&t.prompt());
    let prompt = group(&groups, SourceGroupKind::Prompt).unwrap();
    assert_eq!(prompt.rows[0], "timestamp=yesterday");
}
```

After `a_result_line_that_fails_leaves_start_and_marks_the_tool_call_unavailable`:

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

After `a_later_line_of_a_message_updates_the_turn_even_when_nothing_visible_changes`:

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

**2. Values added to existing tests.** A field of an unexpected JSON type must never fail the line, and no test feeds these types yet.

In `a_field_of_an_unexpected_type_counts_as_missing_and_does_not_fail_the_line` in `tests/claude_details.rs`, add `"promptSource": 2.5` to the top of the line:

```rust
"version": true, "entrypoint": null, "diagnostics": "no", "promptSource": 2.5,
```

In the test module of `src/harness/claude/details.rs`, in `tool_use_result_of_another_type_than_an_object_counts_as_missing`, use these values:

```rust
for raw in [
    r#""Error: boom""#,
    "[true]",
    "true",
    "7",
    "-7",
    "2.5",
    "null",
    "{}",
] {
```

In `a_permission_decision_is_text_or_its_scalar_members`, add this assertion after the one that expects `decision=accept, n=2, on=true`:

```rust
assert_eq!(
    text(r#"{"below":-2,"ratio":2.5,"none":null}"#).as_deref(),
    Some("below=-2, ratio=2.5")
);
```

Then add `"-5"` and `"2.5"` to the values in that test's `for` loop, after `"5"`.

**3. Parse a line in one function, in `src/harness/claude/details.rs`.** The test module's `parse` is a copy of `read_line`, so `only_a_json_object_is_a_line` does not exercise the production check for bytes after the object. Replace `read_line` with:

```rust
/// Reads and parses a line; `None` when it cannot be done.
fn read_line(source: &SourceRef) -> Option<Line> {
    parse_line(&source.read().ok()?)
}

/// Parses a line; `None` unless it is one JSON object and nothing after it.
fn parse_line(bytes: &[u8]) -> Option<Line> {
    let mut json = serde_json::Deserializer::from_slice(bytes);
    // Only an object is a transcript line (a derived struct would also be
    // read from an array).
    let line = pick_object::<_, Line>(&mut json).ok()??;
    json.end().ok()?;
    Some(line)
}
```

Replace the body of the test module's helper, so its tests run the production code:

```rust
fn parse(json: &str) -> Option<Line> {
    parse_line(json.as_bytes())
}
```

Do not add the adapter test for `{} {}` that the feature review proposes. With this change the unit test covers it.

**Leave as is.**
- An empty string at the top of a line that hides a value in the other location. No such data is known, so do not add a filter for it.
- `visit_none` and `visit_some` on the visitors in `details.rs`. `serde_json` does not reach them, so do not add a test for them, and do not remove them in this round.
- `has_result` in `src/model/sources.rs` and its use in `assemble.rs`. Do not remove it and do not add a test for it.
- `unavailable_80x24_default`. Its change from round 2 is accepted, so do not revert it.
- The load path on a single long turn. It is over budget on `main` too, and it is not part of this task.
- All other production code. The only production change in this round is section 3.

**Done when.**
- Each of these temporary edits fails exactly the named test and no other. Make each edit on its own, run `INSTA_UPDATE=no cargo test --no-fail-fast` with no other build running, restore the code, and say so in the build notes. `a_stale_result_after_a_revert_to_the_sent_key_is_dropped` in `tests/index_stub.rs` is a possible flake outside this branch: if it fails in a run, rerun that edit and report it in the build notes.

  - `details.rs`, `tool_group`: `unavailable: matches!(result, Some(None)),` fails `a_call_line_that_fails_marks_the_tool_call_unavailable`.
  - `assemble.rs`, `on_attachment`: passing `&(0..0)` as the last argument of `start_user_turn` fails `a_queued_prompt_reads_the_prompt_group_of_its_own_line`.
  - `sources.rs`, `note_message_line`: `if stopped && lines.stopped.is_none() {` fails `the_latest_of_two_stopped_lines_gives_the_message_group`.
  - `details.rs`, `parse_line`: removing `json.end().ok()?;` fails `only_a_json_object_is_a_line`.
  - `assemble.rs`, `on_turn_duration`: `let _ = turn.sources.set_footer(&self.source, range);` followed by `let mut changed = false;` fails `a_later_footer_line_updates_the_turn_even_when_the_stats_do_not_change`.
  - `details.rs`, `Rows::time`: `Err(_) => SourceValue::Text(String::new()),` fails `a_timestamp_that_does_not_parse_is_shown_as_written`.
  - `details.rs`, `PickVisitor`: deleting `visit_f64` fails `a_field_of_an_unexpected_type_counts_as_missing_and_does_not_fail_the_line`.
  - `details.rs`, `ObjectVisitor`: deleting `visit_i64`, and separately deleting `visit_f64`, each fails `tool_use_result_of_another_type_than_an_object_counts_as_missing`.
  - `details.rs`, `TextVisitor`: deleting `visit_i64`, and separately deleting `visit_f64`, each fails `a_permission_decision_is_text_or_its_scalar_members`.
  - `details.rs`, the visitor inside `Member`'s `Deserialize`: deleting `visit_i64`, separately deleting `visit_f64`, and separately deleting `visit_unit`, each fails `a_permission_decision_is_text_or_its_scalar_members`.

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass with the code restored.
- `INSTA_UPDATE=no cargo test` passes with 2852 tests passing, 0 failing and 2 ignored (2847 plus the 5 new ones).
- `INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture` stays within budget.
- No `.snap` file changes in this round, and no `.snap.new` file is left behind. A changed snapshot is a failure, so do not accept one.
