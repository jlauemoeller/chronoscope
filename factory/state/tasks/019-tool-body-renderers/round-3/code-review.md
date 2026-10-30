VERDICT: PASS

## Summary
The Write/Edit status-result rule matches the rework note:
- `content_cut` is derived from `ctx.truncate`, so the block order stays the same when the section is expanded.
- A cut content/diff puts the full `Result:` right after `Path:`. Otherwise `Result:` comes last, truncated, and its cut becomes `remaining`.

The side items are done, each with a test: the `Body` docs, the subagent `Err` arm, the `range_note` guard and the three-way proptest status. The gate is green, the tree is clean, and the only new snapshot is the requested truncated failed Edit.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (976 lib tests passed, 1 ignored and not from this task, plus all integration tests passing; no `.snap.new` files; `git status` clean)

## Findings
1. [nit] `src/components/turn_block/renderers/tool/mod.rs`, doc comment of `Body::remaining`: the sentence "The `… N more lines` row goes directly under it" is false for the Subagent body. There the order is the cut `Result:`, a blank row, the hint, and then the more row. The sentence also contradicts the reworded `Body::hint` doc just below it, which says the hint comes before the more row. Change: drop that sentence, or reword it to "The `… N more lines` row follows it (after the hint, if any)."
2. [nit] `src/components/turn_block/renderers/tool/write.rs` test `failed_long_content_and_long_error_show_the_error_in_full`: the content and the error both have 10 lines at `truncate: 4`, so the content cut and the would-be result cut are both 6. `assert_eq!(body.remaining, (!expanded).then_some(6))` therefore cannot tell "remaining is the content cut only" from "remaining is the result cut". The Edit twin avoids this, because its 11 diff rows give a cut of 7 against 6 for the error. Change: give the Write test a different content length, for example 12 content lines and `remaining == Some(8)`, so the assertion pins the content cut.
3. [nit] `src/components/turn_block/renderers/tool/write.rs` and `edit.rs` test modules: the `labels`, `at` and `ten_lines` helpers are duplicated word for word. Change: move them into `tool/testing.rs` next to the other shared test helpers.
4. [nit] `src/components/transcript/tests/property.rs` (`ItemSpec::Tool::status`): the status is a `u8` with magic values 0/1/2, explained only by a doc comment. Change (optional): store a `ToolStatus` and generate it with `prop_oneof![Just(ToolStatus::Ok), Just(ToolStatus::Error), Just(ToolStatus::Pending)]`, then match on the enum in `build_item`.
