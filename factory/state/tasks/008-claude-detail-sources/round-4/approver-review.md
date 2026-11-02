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
