VERDICT: FAIL

## Summary
Both round-1 must-fixes are fixed as asked:
- A failed or pending Write/Edit now shows a `Result:` block, with the error in `error_output` or `(pending)` in muted.
- Subagent results are cut at `truncate_output_lines`, with a working `… N more lines` stop, `Submit` expansion and fold persistence.

The new `failed_edit_shows_its_error_below_the_diff_at_100_styled` snapshot pins the failed-Edit state. The gate is green: `fmt`, `clippy -D warnings` and `INSTA_UPDATE=no cargo test` all pass, and no snapshots changed.

Two issues block, both in the new Write/Edit `Result:` block:
- When the content or diff is truncated, the single more row is drawn under the `Result:` text, so it counts lines that belong to another block.
- The `Result:` itself is never truncated.

## States checked
- 1. Shell unfolded, long command + multi-line result @ 100 — `shell_unfolded_with_a_long_command_at_100_text` / `_styled` — ok (unchanged since round 1).
- 2. Read numbered @ 100 — `read_unfolded_with_numbered_content_at_100_text` / `_styled` — ok (unchanged).
- 3. Write `.rs` @ 100 — `write_unfolded_with_rust_content_at_100_styled` — ok (unchanged).
- 4. Edit diff @ 100 — `edit_unfolded_with_a_diff_at_100_text` / `_styled` — ok (unchanged).
- 5. Grep / Glob / WebFetch / WebSearch @ 100 — the four `*_unfolded_at_100_text` snapshots — ok (unchanged).
- 6. Other `mcp__` with pretty JSON @ 100 — `other_mcp_tool_unfolded_with_pretty_json_at_100_text` — ok (unchanged).
- 7. Subagent folded / unfolded @ 100 — `subagent_folded_at_100_text`, `subagent_unfolded_at_100_text` — ok (unchanged).
- 8. Truncated result, more row selected @ 80x24 in a `Transcript`, and expanded — `truncated_result_with_the_more_row_selected_at_80x24_text` / `_styled`, `expanded_result_after_submitting_the_more_row_at_80x24_text` — ok (unchanged).
- 9. Failed Shell @ 100 — `failed_shell_call_at_100_styled` — ok.
- Failed Edit, `Result:` below the diff @ 100 (new) — `failed_edit_shows_its_error_below_the_diff_at_100_styled` — ok. The error run is `Rgb(255,100,103)` (destructive), and the label is at column 4 with content at column 7.
- Failed Write and failed Edit @ 50, styled — scratch crate — ok. `Result:` follows `Content:`/`Diff:`, with the error in destructive.
- Pending Write and pending Edit @ 50, styled — scratch crate — ok. `(pending)` is in muted.
- Failed or pending Write/Edit whose content/diff is truncated (Write at limit 4, Edit at limit 5) @ 50 — scratch crate — **issue** (finding 1).
- Failed Edit with a 30-line error at limit 3 @ 50 — scratch crate — **issue** (finding 2).
- Subagent, 300-line result at limit 3 @ 60 — scratch crate — ok. The result is cut, and `… 297 more lines` is the last row. With the more row selected, `▒` is on that row only. With the tool selected, `▒` covers the section down to the hint. Expanded shows `row 300`. See nit 3 on the row's placement.
- Subagent with the result cut inside a code fence (limit 4) @ 60 — scratch crate — ok. The unclosed fence renders as code.
- Failed subagent with a 300-line result at limit 2 @ 60, styled — scratch crate — ok. The result is in destructive; the hint and more row are in muted.
- Subagent truncated @ 20 with the key `ctrl+shift+o` — scratch crate — ok. The hint and more row are ellipsized (`ctrl+shift+…`, `… 297 mo…`).
- Subagent with a 1 434-line result in a `Transcript` @ 80x24 — scratch crate — ok:
  - `Move` reaches the more row in two steps, and `… 1 234 more lines` gets `▒` on that row only.
  - `Submit` expands the result and selects the tool section without moving the viewport.
  - Fold then unfold keeps the expansion, and the end shows `row 1434` followed by the hint.
- Read range notes @ 60 — scratch crate — ok:
  - `offset: 1` → no note.
  - `limit: 2000` with 2 lines returned → `(lines 1–2)`.
  - `offset: 50, limit: 10` with 2 lines → `(lines 50–51)`.
  - `offset: 50` with plain output → `(lines 50–51)`.
  - Pending `offset: 50` → `(from line 50)`.
  - Empty output with `offset: 5, limit: 3` → `(lines 5–7)` and `(no output)`.
- Read with a leading blank line before Claude numbering @ 60 — scratch crate — ok. The numbers are parsed once.
- Read with `offset = limit = u64::MAX` @ 60 — scratch crate — no panic, but see nit 4.
- Grep with OpenCode `include` @ 60 — scratch crate — ok (`x in /p (glob: *.ts)`).

## Findings
1. [must-fix] Write and Edit bodies, failed or pending, with content/diff longer than `truncate_output_lines` (seen @ 50 with limits 4 and 5). `with_status_result` appends the `Result:` block after the truncated `Content:`/`Diff:` block. `ToolCallSection::render` then puts the `… N more lines` row last, directly under the `Result:` text, with no blank row between them. The row therefore reads as the rest of the error, or of `(pending)`, while the hidden lines are content or diff lines:
   - `Permission denied / second error line / … 6 more lines` — see `ui/write-failed-truncated-4-50.txt`.
   - `String to replace not found in file. / … 36 more lines` — see `ui/edit-failed-truncated-5-50.txt`.
   - `(pending) / … 6 more lines`. A pending result has no lines at all — see `ui/write-pending-truncated-4-50.txt`.

   The planner rule says the first N lines are shown "followed by" the more row. The build notes say the single more row "stays unambiguous", which these renders contradict.

   Change:
   - In `with_status_result` (`tool/mod.rs`), for `ToolStatus::Error | ToolStatus::Pending`, insert the `Result:` block directly after the `Path:` block, before `Content:`/`Diff:`. The truncated block is then the last block, and the more row sits directly under the lines it counts.
   - Update `failed_edit_shows_its_error_below_the_diff_at_100_styled` and the `write.rs`/`edit.rs` unit tests to the new order.
   - Add a unit test and a text snapshot of a failed Edit whose diff is truncated, asserting that the `… N more lines` row comes directly after the last diff row.
2. [must-fix] Write and Edit bodies, failed, with an error result longer than `truncate_output_lines` (seen @ 50 with a 30-line error at limit 3). `with_status_result` forces `expanded: true`, so the `Result:` is never truncated. See `ui/edit-long-error-truncate-3-50.txt`. SPEC §5.3 says "Results longer than N lines … are truncated", and the planner note says "When a body's result has more than `truncate_output_lines` lines, only the first N lines are shown". This is the rule that made the round-1 subagent finding a must-fix. Tool errors can be long, for example when they echo the input or include a trace.

   Change:
   - Cut the failed/pending `Result:` with the normal `result_block` limit (drop the forced `expanded: true`).
   - Keep the invariant from finding 1: the single more row must sit directly below a block that has hidden lines. Using finding 1's order (Path, Result, Content/Diff):
     - When only the content/diff is cut, the row follows it as in finding 1.
     - When only the `Result:` is cut, put the `Result:` block last instead.
     - When both are cut, keep Result first and set `remaining` to the sum of the hidden result and content/diff lines. `Submit` already expands the whole section.
   - Decide the order from the cut state at `truncate_output_lines`, not from `ctx.expanded`, so that expanding does not reorder the blocks.
   - Add unit tests for the result-only-cut and both-cut cases. The layout proptest already generates failed/pending Write/Edit calls with small limits, so it will check the heights.
3. [nit] Subagent body, unfolded, truncated result (@ 60). The more row (column 7) follows the hint `a to open transcript` (column 4) directly, with no blank row between them. The rule "the more row is the last row" was agreed in round 1, and it is documented. Visually, though, the row reads as attached to the hint. See `ui/subagent-more-after-hint-60.txt`. Change, optional: push a blank row between the hint and the more row in `ToolCallSection::render` when both are present. The span bookkeeping already keeps the more row as its own stop.
4. [nit] Read body, `offset = limit = u64::MAX` (@ 60). The fix stops the panic, but the note reads `(lines 18446744073709551615–18446744073709551614)`, a reversed range, and every gutter number is the same saturated value. See `ui/read-max-offset-limit-60.txt`. This only happens with pathological input. Change, optional: drop the range note when the computed end is below the start, for example by `filter(|end| *end >= start)`.
5. [nit] Carried from round 1, nit 4: ANSI escape remnants (`[31m…[0m`) in tool output are still visible. The builder accepted this, and it is outside the planner notes. It is mentioned here for the Approver only.
