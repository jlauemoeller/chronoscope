VERDICT: REWORK

## Assessment

Round 3 did everything the round-2 note asked, and nobody found wrong behaviour. What blocks is one acceptance criterion: "A test shows an unexpected JSON type in any detail field is left out, without `unavailable`". Its test does not show that for eight attributes. The fix is one test, already written and verified, so this is a rework and not a human choice.

**Code review: PASS**, one nit.
- Finding 1 (nit), with `finish: "error"`, the shapes of `error` that give no row are not pinned, and the comment at `details.rs:162-163` reads as if every `error` notice has a row: to be fixed, because the round is happening anyway. It is one short test and a comment.

**Feature review: FAIL**, one must-fix and two nits.
- Finding 1 (must-fix), the test for criterion 5 is ineffective for eight attributes: to be fixed. See below.
- Finding 2 (nit): the same as code finding 1. To be fixed.
- Finding 3 (nit), no test deletes one row of a turn and reads a group from the other: to be fixed. An edit that makes a missing first row fail every group passes the whole `opencode` test set today, and §5.3 limits `unavailable` to the affected group.

**UI review: PASS**, one nit.
- Finding 1 (nit), the WAL test would pass on a database the overlay cannot read: to be fixed. It is one assertion. The round-2 note specified this test, and its check was too weak: it holds when every group is `unavailable`, so the bytes would then be unchanged trivially.

**Why the must-fix is not minor.** The impact today is nil, and I want to be plain about that. The code is correct, since the new test passes on it unchanged, and this adds no user-visible change. I still do not ignore it:
- **The criterion asks for the test.** Correct code does not meet "a test shows". The Feature Reviewer left this box unticked, and I reproduced why below.
- **Three of the eight are the exact failure the criterion forbids.** Without `lenient_object` on `model`, or `lenient` or `lenient_object` on a tool part's `id` or `time`, a wrong type gives `unavailable`, with the suite green. The existing test's `"model":[1]` does not exercise this, because serde reads a struct from an array.
- **The other five would hide valid values.** One bad `time.created` would hide a valid `time.completed`, because the test sets every odd value in one row.
- **§5.2 expects this input.** These fields "vary across Claude Code versions", and the module's head says the same of OpenCode, so the lenient heads are this reader's main defence.
- **The precedent fits.** 008 round 3 reworked the same kind of gap, surviving lenient visitors under the rule that a wrong type "never fails the line". 006 rounds 2 and 3 also reworked test gaps in correct code while budget remained.

**A ruling, so that it is not reopened.** With `finish: "error"`, an `error` that is missing, `null`, `7` or `[1]` gives the notice `error` and no `error` row. That meets the criterion on the `error` row: a row that exists has the notice's text, and §5.3 and the planner notes leave out a field that is missing or of an unexpected type. `{}` and `""` have the right type and say nothing, so they give the row `error`. The note pins this and does not change it.

**Independent checks**, on a private export of `HEAD` (507c955) with its own target directory and `INSTA_UPDATE=no`. The repo was not touched.

On unchanged code, `cargo test --lib opencode` gives 158 passed. All eight removals below still give 158 passed, so the finding reproduces.

With the rework note applied:
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass. `cargo test` gives 2905 passed, 0 failed, 2 ignored, and writes no `.snap.new` file.
- Each edit under "Done when" fails exactly what the note says, under `cargo test --lib opencode`.
- Removing `lenient` from `ModelHead::provider_id` or `PartHead::kind` still passes every test. Both are equivalent, so they get no test and need not be raised again: `model` is read with `lenient_object`, so a bad `providerID` gives no provider either way, and a part that fails to parse is dropped just as the `Some("tool")` filter drops it.
- I did not rerun the budgets. The Code and Feature Reviewers both report them within budget on this code, nobody disputes that, and the note changes only tests and one comment. The builder reruns them as part of the gate.

## Conflicts

No demands oppose each other. The Code Reviewer's PASS does not contradict the Feature Reviewer's FAIL: the Code Reviewer did not test this criterion's strength, and the finding reproduces. Two differences in form are settled here:
- **The `error` shapes.** The reviewers name `7` and `12`, which behave the same. The note uses missing, `7`, `null` and `[1]`.
- **The comment.** Both reviewers ask for a rewording without giving it. The note gives the text.

## Rework budget

Earlier REWORK verdicts: 1 of 2 (round 2). Round 3's `approver-review.md` is that verdict's note and does not count. This verdict is the second, and none remain.

## Rework note

Add tests to `src/harness/opencode/details_tests.rs` and reword one comment in `src/harness/opencode/details.rs`. The code behaves correctly today, so no production behaviour changes: not `assemble.rs`, not `AssistantData` or `UserData`, and no `.snap` file. All the code below passes on the current code, and fmt and clippy are clean with it.

**1. Show that a wrong type leaves out that field alone.** The existing `a_value_of_an_unexpected_json_type_is_left_out_without_unavailable` sets every odd value in one row, and its `"model":[1]` is read as a struct from an array. So it still passes when a wrong type in `model`, or in a tool part's `time` or `id`, gives `unavailable`, or when a wrong type in a `time.*` or `snapshot.*` field hides the valid fields beside it. Leave that test as it is, and add this one directly after it:

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

**2. Pin the `error` that gives no row.** With `finish: "error"`, an `error` that is missing or neither an object nor a string gives the notice `error` and no `error` row. This behaviour is right and stays. Add this test directly before the comment `// -- which part a tool call is read from`:

```rust
#[test]
fn an_error_missing_or_of_the_wrong_type_is_no_row_though_the_notice_says_error() {
    for error in ["", r#""error":7,"#, r#""error":null,"#, r#""error":[1],"#] {
        let data =
            format!(r#"{{"finish":"error",{error}"content":[{{"type":"text","text":"x"}}]}}"#);
        let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", &data)]);
        let turns = fx.turns();
        let Turn::Assistant(turn) = &*turns[1] else {
            panic!()
        };
        assert!(
            turn.items.iter().any(
                |i| matches!(i, Item::Notice { level: NoticeLevel::Error, text, .. } if text == "error")
            ),
            "{data}"
        );
        let content = fx.item(1, "a1:0");
        assert_eq!(labels(&content, "Message"), ["row id", "finish"], "{data}");
        assert!(!has_unavailable(&content, "Message"), "{data}");
    }
}
```

In `message_group` in `details.rs`, replace the two comment lines above `let ended_in_error` with:

```rust
        // The text of the notice for the same row. An `error` that says
        // nothing is a row only where the notice says `error` for it. One that
        // is missing, or not an object or string, is left out even then.
```

**3. Pin groups read from different rows.** Add this test directly before `a_data_that_does_not_parse_makes_its_groups_unavailable`:

```rust
#[test]
fn a_deleted_row_makes_only_the_groups_read_from_it_unavailable() {
    let first = r#"{"agent":"build","model":{"id":"m","providerID":"first"},"content":[{"type":"text","text":"one"}]}"#;
    let second =
        r#"{"finish":"stop","snapshot":{"start":"s"},"content":[{"type":"text","text":"two"}]}"#;
    let fx = || {
        Fx::new(&[
            ("u1", "user", USER),
            ("a1", "assistant", first),
            ("a2", "assistant", second),
        ])
    };
    // The turn's first row is gone: Turn only.
    let gone = fx();
    let turns = gone.turns();
    gone.delete("a1");
    let content = gone.content_of(&turns, 1, SectionKey::Item(ItemId::from("a2:0")));
    assert!(has_unavailable(&content, "Turn"));
    assert_eq!(
        rows(&content, "Message").unwrap(),
        [("row id", "a2"), ("finish", "stop")]
    );
    assert_eq!(
        rows(&content, "Environment").unwrap(),
        [("snapshot.start", "s")]
    );
    // The section's own row is gone: Message and Environment only.
    let gone = fx();
    let turns = gone.turns();
    gone.delete("a2");
    let content = gone.content_of(&turns, 1, SectionKey::Item(ItemId::from("a2:0")));
    assert!(!has_unavailable(&content, "Turn"));
    assert!(
        rows(&content, "Turn")
            .unwrap()
            .contains(&("provider", "first"))
    );
    assert_unavailable(&content, &["Message", "Environment"]);
}
```

**4. Make the WAL test show that the database was read.** In `reading_details_in_wal_mode_leaves_the_database_file_untouched`, replace the line `assert!(!section_details(&fx.db.path, &source).is_empty());` with:

```rust
    let groups = section_details(&fx.db.path, &source);
    assert!(!groups.is_empty());
    assert!(groups.iter().all(|g| !g.unavailable), "{groups:?}");
```

**Dropped, and why:**
- Tests for removing `lenient` from `ModelHead::provider_id` or `PartHead::kind`: neither removal has any visible effect, so no test can tell them apart.
- A change to `reading_details_leaves_the_database_untouched`: other tests pin the values read in that journal mode.

**Done when:**
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `INSTA_UPDATE=no cargo test` and `cargo test --release -- --ignored budgets --nocapture` all pass, with 2905 tests passing and no `.snap` file changed.
- Each of these temporary edits in `details.rs`, made alone and undone afterwards, gives this result under `INSTA_UPDATE=no cargo test --lib opencode`:
  - Change the attribute to `#[serde(default)]` on each of `Head::model`, `TimeHead::created`, `TimeHead::streamed`, `TimeHead::completed`, `SnapshotHead::start`, `SnapshotHead::end`, `PartHead::id` and `PartHead::time`, one at a time: each fails only `a_value_of_an_unexpected_json_type_leaves_out_that_field_alone`.
  - Replace the `let error = ...;` statement in `message_group` with the following: fails only `an_error_missing_or_of_the_wrong_type_is_no_row_though_the_notice_says_error`.
    ```rust
    let error = ended_in_error
        .then(|| error_text(head.error.as_ref()).1)
        .or_else(|| {
            let error = head.error.as_ref().filter(|error| error.has_text());
            error.map(|error| error_text(Some(error)).1)
        });
    ```
  - Replace the `let read = ...;` statement in `read_groups` with the following: fails only `a_deleted_row_makes_only_the_groups_read_from_it_unavailable`.
    ```rust
    let read = |row: &str| {
        db.and_then(|db| {
            if let Some(turn) = &refs.turn {
                read_row(db, turn)?;
            }
            read_row(db, row)
        })
    };
    ```
  - In `section_details`, replace `read_groups(db.as_ref(), source)` with `read_groups(None, source)`: fails many tests, and `reading_details_in_wal_mode_leaves_the_database_file_untouched` must be among them.
