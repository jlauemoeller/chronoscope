## Rework note
Fix the status `Result:` block of failed and pending Write/Edit calls (`with_status_result` in `src/components/turn_block/renderers/tool/mod.rs`, called from `write.rs` and `edit.rs`). After the fix, the result is truncated like every other result, and the single `… N more lines` row always sits directly under the block whose lines it counts.

**Rule: one truncatable block per body, and it is always the last block.** It applies when the status is `Error` or `Pending`. A successful Write or Edit is unchanged and gets no `Result:` block.

1. **The content/diff would be cut at `ctx.truncate`.** That means the Write content has more than `ctx.truncate` lines, or the Edit diff has more than `ctx.truncate` rows. In that case:
   - Insert the `Result:` block directly after the `Path:` block, so the order is `Path:`, `Result:`, `Content:`/`Diff:`.
   - Show the `Result:` in full.
   - `remaining` is the content/diff cut, as today.
   - The more row then follows the last content or diff row.
2. **Otherwise:**
   - Keep the order `Path:`, `Content:`/`Diff:`, `Result:`.
   - Lay out the `Result:` with the normal `ctx`: `result_block("Result:", call, ctx)`, without the forced `expanded: true`.
   - Set `body.remaining` to the cut it returns. A `(pending)` result is never cut.

**Decide the order from the cut at `ctx.truncate`, not from `limit(ctx)` or `body.remaining`.** When the section is expanded, `limit(ctx)` is `usize::MAX` and `body.remaining` is `None`. Keying the order off them would make the blocks jump when `Submit` expands the section. Have `write.rs` and `edit.rs` compute whether the content/diff exceeds `ctx.truncate` regardless of `ctx.expanded`, and pass that into `with_status_result` (a `bool` or the would-be cut count).
- Derive this flag from the same computation that already produces `remaining`, with the limit swapped for `ctx.truncate`:
  - Write: `truncate_lines(content, ctx.truncate).1 > 0`.
  - Edit: `rows.len() > ctx.truncate`.
- Do not use a separate line count such as `content.lines().count()`. Trailing-newline edge cases could make the flag and `remaining` disagree.
- The early-return `(no content)` and `(no changes)` paths pass "not cut". Update the doc comment of `with_status_result` to state the rule, including that the result is shown in full only when the content/diff is the cut block.

This drops the UI review's request to also cut the result and sum the counts when both blocks are long. A summed row under `Content:`/`Diff:` would also count hidden `Result:` lines higher up, which is the same ambiguity this rework removes, and the Feature review, which outranks UI, accepts the full result in that case. It also drops the Code review's alternative of not truncating the content when a status result is present, because that would draw unbounded file content.

**Also, in the same round:**
- **`Body` doc comments** (`tool/mod.rs`):
  - Reword `Body::remaining` to say it counts the lines cut from the body's last block, which is always the truncatable one.
  - Reword `Body::hint` to say it is a row after the blocks, before the more row if there is one.
- **Subagent `Err` arm** (`subagent.rs`, `SubagentBody::body`): replace its own `log::warn!` and `(output unavailable: …)` block with `unavailable_block("Result:", call, &err, ctx)`.
- **`range_note`** (`read.rs`): ignore a computed end that is below the start. Examples are Claude numbers that disagree with `offset`, and `offset = limit = u64::MAX`. This gives `(from line N)` or no range rather than a reversed `(lines 98–3)`.
- **Proptest** (`src/components/transcript/tests/property.rs`, `build_item`): choose the status three ways, adding `ToolStatus::Pending` with `None` output. Make sure `fixtures::tool_of` really yields a pending call, so that pending Write/Edit bodies in both block orders are height-checked.

**Done when:**
- **Unit tests in `write.rs` and `edit.rs`** cover each case and assert the block labels in order and `remaining`, both at `expanded: false` and at `expanded: true`. The block order must be identical in both states for every case:
  - Failed, short content, 10-line error, `truncate: 4`: order Path, Content/Diff, Result. The result shows 4 error lines and `remaining == Some(6)`. Expanded, all 10 lines show and `remaining == None`.
  - Failed, content/diff longer than the limit, short error: order Path, Result, Content/Diff. `remaining` is the content/diff cut.
  - Failed, both longer than the limit: order Path, Result (full), Content/Diff. `remaining` is the content/diff cut only.
  - Pending, content/diff longer than the limit: order Path, `Result: (pending)`, Content/Diff.
- **A new text snapshot** of a failed Edit whose diff is truncated (width 100, a small `truncate_output_lines`). It shows `Result:` before `Diff:`, and `… N more lines` directly after the last diff row.
- **`failed_edit_shows_its_error_below_the_diff_at_100_styled`** is updated only if its order actually changes. Its diff is short, so it should not change.
- **No other snapshot changes**, except intentional ones that you list in the build notes.
- **A test for `range_note`** covers the reversed case.
- **The gate passes:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` all pass, with no `.snap.new` files.

**Out of scope, leave as is:**
- The Read numbering-detection heuristic.
- A blank row between the subagent hint and the more row.
- ANSI remnants.
- Relayout logging.
