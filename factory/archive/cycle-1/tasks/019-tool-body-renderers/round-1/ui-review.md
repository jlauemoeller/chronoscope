VERDICT: FAIL

## Summary
All nine UI states listed in the task have snapshots, and they match the `UI` section. The task-017 geometry holds throughout: `▼ description`, blank row, label at column 4, blank row, content at column 7. Tokens are right, and the `▒` marker covers only the more row when that row is selected. `fmt`, `clippy -D warnings` and `INSTA_UPDATE=no cargo test` all pass.

My scratch-crate renders found two gaps in behaviour the planner notes require:
- Write and Edit bodies never show a failed call's error, or `(pending)`.
- Subagent results are never truncated.

There are also four nits.

## States checked
- 1. Shell unfolded, long char-wrapped command + multi-line result @ 100 — snapshots `shell_unfolded_with_a_long_command_at_100_text` / `_styled` — ok. The command is in `command` (foreground) and the result in `output` (muted_foreground). The 200-char `/tmp/...` path breaks at the body width with a 3-cell right margin, as in the sketch.
- 2. Read unfolded with numbered content @ 100 — `read_unfolded_with_numbered_content_at_100_text` (+ `_styled`) — ok. Claude `N→` prefixes are parsed once into a right-aligned gutter, and `(lines 8–11)` is shown.
- 3. Write unfolded with `.rs` content @ 100 — `write_unfolded_with_rust_content_at_100_styled` — ok. `fn` is in `syntax.keyword`, and strings and comments get their own colours.
- 4. Edit unfolded with a diff @ 100 — `edit_unfolded_with_a_diff_at_100_text` / `_styled` — ok. `@@` is muted, `-` lines are in `diff_removed` (destructive), `+` lines in `diff_added` (primary) and context lines in `output`.
- 5. Grep / Glob / WebFetch / WebSearch unfolded @ 100 — `grep_unfolded_at_100_text`, `glob_unfolded_at_100_text`, `web_fetch_unfolded_at_100_text`, `web_search_unfolded_at_100_text` — ok.
- 6. Other (`mcp__github__get_issue`) unfolded, pretty JSON @ 100 — `other_mcp_tool_unfolded_with_pretty_json_at_100_text` — ok (2-space indent).
- 7. Subagent folded / unfolded @ 100 — `subagent_folded_at_100_text`, `subagent_unfolded_at_100_text` — ok. Shows `(Explore)`, markdown prompt and result, and `a to open transcript` at column 4.
- 8. Truncated result with the more row selected @ 80x24 in a `Transcript` — `truncated_result_with_the_more_row_selected_at_80x24_text` / `_styled` — ok. `… 1 234 more lines` sits at column 7, `▒` is on that row only, and the row is muted plus BOLD for the selection. After expansion: `expanded_result_after_submitting_the_more_row_at_80x24_text` — ok (row removed, tool section selected, viewport kept).
- 9. Failed Shell call @ 100 — `failed_shell_call_at_100_styled` — ok (result in `error_output`).
- Shell @ 20/40/60/80, multi-line heredoc command, empty output `(no output)`, pending `(pending)` in muted, missing `command` field, blank description → `Shell` — rendered in scratch crate — ok.
- Read: plain output from `offset` 998 (gutter widens to 4 digits, wrapped text stays under the text column), OpenCode `00001<TAB>` numbering plus an unnumbered trailer, Claude numbering plus a `<system-reminder>` trailer, failed read, truncated at 3 with the more row selected (styled), expanded @ 40 — rendered in scratch crate — ok.
- Write: long `.rs` line char-wrapped with highlight spans kept across the wrap, unknown extension (plain), empty content `(no content)`, truncated at 4 — scratch crate — ok. Failed and pending — issue (finding 1).
- Edit: MultiEdit (two hunks in turn), empty `old_string`, long-line wrap (styled), identical strings `(no changes)`, two hunks, diff truncated at 5, OpenCode camelCase keys — scratch crate — ok. Failed and pending — issue (finding 1).
- Grep with glob+type filters, without a path, with an empty result; Glob without a path; WebFetch with a long URL and no prompt; WebSearch pending; Other with `{}` input and a blank description; TodoWrite JSON wrap @ 50 — scratch crate — ok. OpenCode `include` — nit (finding 3).
- Subagent: folded @ 40 with a long description (agent type kept, description cut), unfolded with a word-wrapped description, remapped key `ctrl+o` (styled), pending OpenCode `agent`, no agent type, JSON-looking result — scratch crate — ok. 300-line result — issue (finding 2).
- Transcript @ 80x24, 1 434-line result:
  - `Move` Down reaches the more row after 9 steps, Down on it stays, and Up returns to the tool section.
  - `Submit` expands the output and selects the tool section without moving the viewport.
  - Scrolling to the end shows `line 1434`.
  - Fold then unfold keeps the expansion.
  - Rendered in scratch crate — ok.
- Control characters (ESC, BEL, BS, CR, C1 CSI) in a Shell command and result @ 60 — scratch crate — no control character reaches any buffer cell (checked per cell). Escape remnants stay visible (finding 4).
- Widths 9–16 (text column collapses) — scratch crate — degenerate, but the App shows its too-small screen below 60 columns, so not reportable.

## Findings
1. [must-fix] Write and Edit bodies, failed and pending states, any width (seen @ 50). `WriteBody` and `EditBody` ignore `call.status` and `call.result`, so:
   - A failed Edit (`String to replace not found in file.`) draws a clean diff with no sign that it failed. A failed Write (`Permission denied`) draws its content the same way. The error text is never drawn. See `ui/edit-failed-50.txt` and `ui/write-failed-50.txt`.
   - A pending Write or Edit shows no `(pending)`. Read, by contrast, shows it. See `ui/write-edit-read-pending-50.txt`.

   This contradicts the binding planner rules "Results use `output`, or `error_output` when the call failed" and "A pending call shows `(pending)` in `muted_foreground` as its result". It is also a regression from task 017's generic body, which showed `Result:` for every kind.

   Change: in both renderers, after the `Content:` / `Diff:` block, add:
   - when `status == Error`: a `Result:` block with the result text in `error_output`, using `result_block`;
   - when `status == Pending`: a `Result:` block with `(pending)` in `muted_foreground`.

   A successful call may keep omitting the `Result:` block. Add unit tests for both states in `write.rs` and `edit.rs`.
2. [must-fix] Subagent body, unfolded, a result longer than `truncate_output_lines` (seen with 300 lines, limit 3 @ 60). The whole result is rendered, with no `… N more lines` row and no expansion stop. See `ui/subagent-300-lines-truncate-3-60.txt`. The planner's truncation rule covers "a body's result" with no exception, and the subagent body has a `Result:`. Subagent final reports (Explore runs, say) are regularly long.

   Change:
   - Cut the result source at `limit(ctx)` logical lines before markdown rendering (`truncate_lines`), and set `Body::remaining` to the number of lines cut.
   - Make the `… N more lines` row the same separate selection stop with the same `Submit` expansion as other kinds.
   - Keep the `<key> to open transcript` hint. Order the hint and the more row so that the more row stays the stop directly after the tool section (that is, the last row, as `ToolCallSection::render` pops it).
   - Add a unit test and extend the property test's subagent case.
3. [nit] Grep body, OpenCode input @ 60. OpenCode's grep tool passes its file filter as `include` (`{"pattern","path","include"}`), and it is not shown (`x in /p`). See `ui/grep-opencode-include-60.txt`. No repo fixture carries `include` yet, so this is a nit. Change: in `search.rs`'s `pattern_line`, treat `include` like `glob`, giving `(glob: *.ts)`.
4. [nit] Shell/any result containing ANSI escapes @ 60. Ratatui drops the ESC byte, so this is safe: no control character reaches a cell. The parameters stay visible, though, as `[31mred[0m`. See `ui/shell-ansi-escapes-60.txt`. Change (optional, spec-silent): strip CSI/OSC sequences (`ESC [ … final-byte`, `ESC ] … BEL/ST`) from tool output and commands before wrapping.
5. [nit] Read body, input with `offset: 1` and no `limit`. The path reads `(from line 1)`, which adds nothing. Change: omit the note when the offset is 1 and there is no limit.
6. [nit] Write/Edit snapshots @ 100. Neither snapshot set has a failed or pending case. Once finding 1 is fixed, add a styled snapshot of a failed Edit (`Result:` in `error_output` below the diff), so the new state is pinned visually.
