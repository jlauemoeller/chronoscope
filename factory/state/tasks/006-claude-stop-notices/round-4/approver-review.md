## Rework note

The code behaves correctly. Do not change any behaviour. This round adds tests to `tests/claude_notices.rs` and re-wraps one comment in `src/harness/claude/assemble.rs`. Nothing else in `src/` changes.

**1. Test that a removal leaves calls of earlier turns alone.** Nothing fails today if `c.turn == b.idx &&` is deleted from `shift_after`. Add this test to the "positions after a removal" section:

```rust
#[test]
fn a_removal_in_a_later_turn_leaves_a_call_of_an_earlier_turn_attachable() {
    let next_prompt = r#"{"type":"user","uuid":"u2","origin":{"kind":"human"},"timestamp":"2026-01-01T00:03:00Z","message":{"role":"user","content":"again"}}"#;
    let lines = [
        line(1, "a", "null", &text("one")),
        line(2, "b", "null", &text("two")),
        line(3, "c", "null", &tool_use("old")),
        next_prompt.to_owned(),
        line(4, "m", r#""max_tokens""#, &text("x")),
        line(5, "m", r#""end_turn""#, ""),
        result_line(6, "old"),
    ];
    let items = items_of(&assemble(&lines));
    assert_eq!(call_status(&items, "old"), Some(ToolStatus::Ok));
}
```

**2. Assert the order of merged entries.** At the end of `injected_items_a_removal_leaves_side_by_side_collapse`, add:

```rust
    let Item::Injected { entries, .. } = &items[1] else {
        panic!("{items:?}");
    };
    let order: Vec<&str> = entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(order, ["meta 2", "meta 4"]);
```

**3. Test that a merged item's id is dropped.** Add this test to the same section:

```rust
#[test]
fn a_merged_injected_items_id_is_free_again() {
    // `meta_line(4)` sent again shows a second copy. No spec rule asks for
    // that copy; this test is about the id: it must not point at another item.
    let lines = [
        line(1, "z", "null", &text("z")),
        meta_line(2),
        line(3, "m", r#""max_tokens""#, ""),
        meta_line(4),
        line(5, "m", r#""end_turn""#, ""),
        line(6, "t", "null", &text("t")),
        meta_line(4),
        line(7, "u", "null", &text("u")),
    ];
    let items = items_of(&assemble(&lines));
    assert_eq!(
        shape(&items),
        ["text z", "injected 2", "text t", "injected 1", "text u"]
    );
}
```

**4. Test a flagged message with only whitespace.** Add this next to `a_flagged_message_without_text_has_no_notice`:

```rust
#[test]
fn a_flagged_message_with_only_whitespace_has_no_notice() {
    let items = items_of(&assemble(&[api_error_line(1, "msg_e", "null", "   ")]));
    assert!(items.is_empty(), "{items:?}");
}
```

**5. Test invalid UTF-8 in the two `message` fields.** Add these next to `invalid_utf8_in_a_top_level_flag_keeps_the_line_parsed`:

```rust
#[test]
fn invalid_utf8_in_the_flag_inside_the_message_keeps_the_line_parsed() {
    let mut bytes = PROMPT.as_bytes().to_vec();
    bytes.extend_from_slice(
        b"\n{\"type\":\"assistant\",\"uuid\":\"a1\",\"timestamp\":\"2026-01-01T00:00:01Z\",\"message\":{\"id\":\"msg_1\",\"model\":\"m\",\"isApiErrorMessage\":\"x\xffy\",\"stop_reason\":null,\"content\":[{\"type\":\"text\",\"text\":\"Fine.\"}]}}\n",
    );
    let asm = Assembler::from_memory(Arc::from(bytes));
    assert_eq!(asm.unparsed_count(), 0);
    let items = items_of(&asm);
    assert!(notices(&items).is_empty());
    assert_eq!(texts(&items), ["Fine."]);
}

#[test]
fn invalid_utf8_in_the_stop_reason_keeps_the_line_parsed() {
    let mut bytes = PROMPT.as_bytes().to_vec();
    bytes.extend_from_slice(
        b"\n{\"type\":\"assistant\",\"uuid\":\"a1\",\"timestamp\":\"2026-01-01T00:00:01Z\",\"message\":{\"id\":\"msg_1\",\"model\":\"m\",\"stop_reason\":\"max\xff\",\"content\":[{\"type\":\"text\",\"text\":\"Fine.\"}]}}\n",
    );
    let asm = Assembler::from_memory(Arc::from(bytes));
    assert_eq!(asm.unparsed_count(), 0);
    let items = items_of(&asm);
    assert_eq!(texts(&items), ["Fine."]);
    let notices = notices(&items);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].2, "stopped: max\u{FFFD}");
}
```

**6. Re-wrap the doc comment on `error_block_key`.** It runs past 80 columns. Comment only:

```rust
/// The number of an error text block. With `apiBlockIndex` it is that index
/// plus the block's position. Without it, a text that equals (first) or
/// extends a block the message already has (not one from this line) is a
/// re-send and replaces it; any other text gets the next free number.
```

**Dropped, do not do these.**
- Do not use `saturating_add` in `error_block_key` or `block_id`. With an absurd `apiBlockIndex`, it gives two blocks the same key, and one text is lost. The release build keeps both today, and the panic is debug-only on a value no transcript has.
- Do not use the feature reviewer's alternative sequence for step 3. The code review's test takes priority, and the comment in it records that the second copy is not a requirement.

**Leave as is.** `remove`, `shift_after`, `error_block_key`'s logic, `take_message_text`, the per-turn `stop`, the empty `stop_reason`, `wire.rs`, the views and all snapshots.

**Done when.**
- All the tests above pass with the exact expected values given, and every existing test passes.
- Each of these temporary edits fails the named test. Make each edit, check it, restore the code, and say so in the build notes:
  - Delete `c.turn == b.idx &&` in `shift_after`: `a_removal_in_a_later_turn_leaves_a_call_of_an_earlier_turn_attachable`.
  - Replace `first.extend(entries);` in `remove` with `let old = std::mem::replace(first, entries); first.extend(old);`: `injected_items_a_removal_leaves_side_by_side_collapse`.
  - Delete `b.index.remove(&second);` in `remove`: `a_merged_injected_items_id_is_free_again`.
  - Replace `text.trim().is_empty()` with `text.is_empty()` in `message_notice`: `a_flagged_message_with_only_whitespace_has_no_notice`.
  - Add `stop_reason: Option<String>,` to `MessageWithoutUsage`: `invalid_utf8_in_the_stop_reason_keeps_the_line_parsed`.
  - Add `#[serde(rename = "isApiErrorMessage")] flag: Option<String>,` to `MessageWithoutUsage`: `invalid_utf8_in_the_flag_inside_the_message_keeps_the_line_parsed`.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass, with the 5 new tests and every existing test passing (2664 expected: 2659 plus the 5 new ones).
- `INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture` stays within budget.
- Outside `factory/`, this round's diff touches only `tests/claude_notices.rs` and that one comment in `src/harness/claude/assemble.rs`. No `.snap` file changes, and no `.snap.new` file is left behind.
