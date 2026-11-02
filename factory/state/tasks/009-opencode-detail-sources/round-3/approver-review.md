## Rework note

Change `src/harness/opencode/details.rs` and add tests to `src/harness/opencode/details_tests.rs`. Nothing else changes: not `assemble.rs`, not `AssistantData` or `UserData`, and no `.snap` file.

**1. Make `find_part` read the part the assembler uses.** Today it takes the first match of any type. The assembler makes tool calls only from `type: "tool"` parts (`part_item`), and the last part with an id wins (`upsert`). So `start` can come from another part than `duration`, which breaks the planner note that they have the same source.
- Add this as the first field of `PartHead`:
  ```rust
  #[serde(default, rename = "type", deserialize_with = "lenient")]
  kind: Option<String>,
  ```
- In `find_part`, add `.rev()` directly after `.enumerate()`, so that the index stays the original position.
- After the `.filter_map(...)`, add `.filter(|(_, p)| p.kind.as_deref() == Some("tool"))`. This is an exact, untrimmed comparison, as in `part_item`. Keep the `.find(...)` as it is.
- Replace the doc comment with:
  ```rust
  /// The tool part of the row that the item `part` shows, the way the assembler
  /// reads it: only a part of type `tool` is a tool call, it is named by its own
  /// `id`, or `<row id>:<index>` when it has none, and of several parts with one
  /// id the last wins.
  ```

**2. Trim text values in `Rows::text`.** Add `.map(str::trim)` before the existing `.filter(|v| !v.is_empty())`, and change its doc comment to `A text field, trimmed; a blank string is no value.` The notice and the `error` row already trim, and today `finish: " error "` starts one column right of the other values.

**3. Pin one more `error` shape.** In `the_error_row_says_what_the_notice_of_the_same_row_says`, add `r#""error":{"type":5,"message":true}"#,` to the `errors` list.

**4. Add these tests at the end of `details_tests.rs`:**

```rust
// -- which part a tool call is read from

const ONE_SECOND: (i64, i64) = (1_700_000_001_000, 1_700_000_002_000);
const FIVE_SECONDS: (i64, i64) = (1_700_000_500_000, 1_700_000_505_000);

/// A completed tool part; `id` is the JSON member, with its comma, or empty.
fn tool_part(id: &str, (created, completed): (i64, i64)) -> String {
    format!(
        r#"{{"type":"tool",{id}"name":"shell","state":{{"status":"completed","input":{{}},"content":[]}},"time":{{"created":{created},"completed":{completed}}}}}"#
    )
}

#[test]
fn a_tool_part_sent_twice_in_one_row_is_read_from_the_last() {
    let id = r#""id":"c1","#;
    let data = format!(
        r#"{{"content":[{},{}]}}"#,
        tool_part(id, ONE_SECOND),
        tool_part(id, FIVE_SECONDS)
    );
    let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", &data)]);
    assert_eq!(
        rows(&fx.item(1, "c1"), "Tool call").unwrap(),
        [
            ("start", "2023-11-14 22:21:40Z"),
            ("duration", "5s"),
            ("status", "ok")
        ]
    );
}

#[test]
fn a_part_of_another_type_with_the_id_of_a_tool_part_is_not_read() {
    let tool = tool_part(r#""id":"c1","#, ONE_SECOND);
    for other in [
        r#"{"type":"reasoning","id":"c1","text":"r","time":{"created":1600000000000}}"#,
        r#"{"type":"text","id":"c1","text":"t","time":{"created":1600000000000}}"#,
        r#"{"id":"c1","time":{"created":1600000000000}}"#,
        r#"{"type":5,"id":"c1","time":{"created":1600000000000}}"#,
    ] {
        for data in [
            format!(r#"{{"content":[{other},{tool}]}}"#),
            format!(r#"{{"content":[{tool},{other}]}}"#),
        ] {
            let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", &data)]);
            assert_eq!(
                rows(&fx.item(1, "c1"), "Tool call").unwrap(),
                [
                    ("start", "2023-11-14 22:13:21Z"),
                    ("duration", "1s"),
                    ("status", "ok")
                ],
                "{data}"
            );
        }
    }
}

#[test]
fn a_position_that_holds_another_type_now_makes_the_tool_group_unavailable() {
    let data = format!(r#"{{"content":[{}]}}"#, tool_part("", ONE_SECOND));
    let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", &data)]);
    let turns = fx.turns();
    fx.set_data(
        "a1",
        r#"{"content":[{"type":"reasoning","text":"r","time":{"created":1600000000000}}]}"#,
    );
    let content = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("a1:0")));
    assert_eq!(
        labels(&content, "Tool call"),
        ["duration", "status", "unavailable"]
    );
}

// -- text with blanks around it

#[test]
fn a_text_value_is_trimmed_and_a_blank_one_is_left_out() {
    let data = r#"{"agent":"  ","model":{"providerID":" acme "},"finish":" stop ","rawFinish":"  ","snapshot":{"start":" aaa ","end":"\t"},"content":[{"type":"text","text":"x"}]}"#;
    let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", data)]);
    let content = fx.item(1, "a1:0");
    assert_eq!(
        rows(&content, "Message").unwrap(),
        [("row id", "a1"), ("finish", "stop")]
    );
    assert_eq!(
        rows(&content, "Environment").unwrap(),
        [("snapshot.start", "aaa")]
    );
    let turn = rows(&content, "Turn").unwrap();
    assert!(turn.contains(&("provider", "acme")));
    assert!(!labels(&content, "Turn").contains(&"agent"));
    for title in ["Turn", "Message", "Environment"] {
        assert!(!has_unavailable(&content, title), "{title}");
    }
}

// -- the real database's journal mode

#[test]
fn reading_details_in_wal_mode_leaves_the_database_file_untouched() {
    let fx = full();
    fx.db.exec("PRAGMA journal_mode = WAL;");
    let turns = fx.turns();
    let before = std::fs::read(&fx.db.path).unwrap();
    let id = SectionId {
        turn: turns[1].id().clone(),
        key: SectionKey::Item(ItemId::from("call-1")),
    };
    let source = section_source(&turns[1], &id).unwrap();
    assert!(!section_details(&fx.db.path, &source).is_empty());
    assert_eq!(std::fs::read(&fx.db.path).unwrap(), before);
}
```

The WAL test asserts only the database file's bytes, not the directory listing: `Db::open` can create `-wal` and `-shm` files beside a WAL database, which is the shared `Db`'s behaviour and not part of this task.

**Dropped, and why:**
- The Feature Reviewer's `.filter(...)` and `.last()` form of item 1: it gives the same result as the Code Reviewer's form used here.
- `SourceRow.replaces` as an enum: a design preference with no effect on behaviour, in a public model type.
- An 80x24 snapshot scrolled to the end, and a styled App snapshot: the `UI` section requires only the two snapshots that exist, and they must stay unchanged.

**Done when:**
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `INSTA_UPDATE=no cargo test` and `cargo test --release -- --ignored budgets --nocapture` all pass, with no `.snap` file changed.
- Each of these temporary edits fails the tests named, and only those. Undo each one afterwards:
  - Delete `.rev()` from `find_part`: `a_tool_part_sent_twice_in_one_row_is_read_from_the_last`.
  - Delete the `Some("tool")` filter: `a_part_of_another_type_with_the_id_of_a_tool_part_is_not_read` and `a_position_that_holds_another_type_now_makes_the_tool_group_unavailable`.
  - Delete `.map(str::trim)` from `Rows::text`: `a_text_value_is_trimmed_and_a_blank_one_is_left_out`.
