VERDICT: FAIL

## Summary
Round 2 fixes the round-1 must-fix: the Subagent `Result:` is now cut at `truncate_output_lines` source lines before markdown layout, and the cut feeds the usual `… N more lines` stop, `Submit` expansion and fold persistence. The round-1 nits are fixed too: Write/Edit show their failed or pending result, and the Read fixes are in. The gate is green: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` exit 0 with no failures, and the working tree is unchanged. One spec gap remains. The new Write/Edit status `Result:` is always shown in full, which bypasses the planner's general truncation rule, the same rule that made the round-1 Subagent finding must-fix.

## Acceptance criteria
- [x] Each `ToolKind` renders the body from the planner-notes table, with one test per kind for labels, columns 4/7 and tokens.
  - Implemented in `renderers/tool/{shell,read,write,edit,search,web,other,subagent}.rs`, with enum dispatch through `body_renderer`.
  - Covered by the per-kind tests from round 1 (all passing) and by `tool::tests::dispatch_picks_a_renderer_for_every_kind`.
  - The new failed/pending Write/Edit `Result:` rows are covered by `write::tests::a_failed_write_shows_its_error_and_a_pending_one_shows_pending` and `edit::tests::{a_failed_edit_shows_its_error_after_the_diff,a_pending_edit_shows_pending}`. These check the `error_output` / `muted_foreground` colours, and that a successful Write has no `Result:`.
- [x] Shell command and result are char-wrapped. Implemented through `text_block` → `char_wrap`. Covered by `shell::tests::a_long_path_is_char_wrapped_at_the_body_width` and the shell snapshots at 100 columns.
- [x] Read parses Claude numbering into the gutter once, and numbers plain output from the offset.
  - Covered by `read::tests::{claude_numbering_is_parsed_into_the_gutter_once,tab_separated_numbering_is_parsed_too,plain_output_is_numbered_from_the_input_offset,numbering_is_detected_past_a_leading_unnumbered_line}`.
  - Overflow is covered by `huge_offsets_and_limits_do_not_overflow`.
- [x] Write content is highlighted by extension. Covered by `write::tests::rust_content_is_highlighted_by_extension` and the styled snapshot.
- [x] Edit diff colours and `@@` headers. Covered by `edit::tests::a_diff_has_headers_and_coloured_plus_and_minus_lines` and the styled snapshot. The new styled snapshot `failed_edit_shows_its_error_below_the_diff_at_100_styled` pins the failed state.
- [x] Subagent fold, unfold and the remapped hint.
  - Covered by `subagent::tests::*`, `turn_block::tests::the_open_transcript_hint_follows_the_remapped_open_subagent_key` and `transcript::tests::truncation::the_transcript_passes_the_open_subagent_key_to_subagent_sections`.
  - Truncation now applies here as well:
    - `subagent::tests::a_long_result_is_cut_and_expanding_shows_all_of_it` checks `remaining == Some(16)`, and that expansion shows `r10`.
    - `the_more_row_is_the_last_row_after_the_hint` checks the row order.
    - `transcript::tests::truncation::a_long_subagent_result_is_truncated_with_a_stop_that_expands_it` checks the stops `[tool, more]` and the `… 49 more lines` row, and that `Submit` removes the row and shows `line 30`.
- [x] 1 434 lines at a limit of 200 show 200 lines plus `… 1 234 more lines`. `Cmd::Move` / `Cmd::Submit` work, and the state survives fold/unfold. Covered by the `transcript::tests::truncation::*` tests from round 1, all passing.
- [x] A folded call never calls `ToolOutput::text()`; unfolding calls it. Covered by `truncation::a_folded_call_never_materialises_its_output_and_unfolding_does`. `read::tests::an_unreadable_output_is_read_once_and_reported` now also asserts a single `text()` call on the error path.
- [x] Layout-cache heights match rendering for all new bodies. `property::cached_heights_equal_rendered_heights` covers all ten kinds, with Ok/Error status, a random limit of 1..8 and random expansion, so it exercises subagent truncation and the failed Write/Edit `Result:` block. Pending is not generated (see nit 2).
- [x] Every UI state has a snapshot: states 1–9 from round 1, plus the new failed-Edit styled snapshot.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. `INSTA_UPDATE=no cargo test` exits 0, with no `FAILED` or `panicked` lines and no `.snap.new` files.

## Findings
1. [must-fix] A failed Write/Edit `Result:` ignores `truncate_output_lines`.
   - The problem is in `with_status_result` in `src/components/turn_block/renderers/tool/mod.rs`. It always lays out the result with `SectionCtx { expanded: true, .. }`, so a failed Write or Edit whose error text has more than N lines is drawn in full, with no `… N more lines` stop.
   - This `Result:` block exists because the planner's general rules apply to every body ("Results use … `error_output` when the call failed", "A pending call shows `(pending)`"). The truncation rule ("When a body's result has more than `truncate_output_lines` lines…") is one of those general rules, and nothing exempts this block. It is the same basis on which round-1 finding 1 (the Subagent result) was must-fix.
   - The builder's reason, keeping a single more row, holds only when the content or diff has already been cut. It does not justify skipping truncation in the common case, where the content is short and the error is long (for example a multi-line hook denial or an OpenCode stack trace).
   - Change: in `with_status_result`, when `body.remaining.is_none()`, call `result_block("Result:", call, ctx)` with the normal `ctx` and set `body.remaining` to the returned cut. Keep the forced full result only when `body.remaining` is already `Some`, and say so in the doc comment.
   - Add a unit test in `write.rs` or `edit.rs`: a failed call with short content, a 10-line error and `truncate: 4` gives 4 error lines and `remaining == Some(6)`, and `expanded: true` shows all 10. The property test will then check the heights, since it already generates failed Write/Edit calls.
2. [nit] The property test never generates a pending call, so the pending Write/Edit `Result: (pending)` rows are never height-checked.
   - In `src/components/transcript/tests/property.rs`, `build_item` passes only `ToolStatus::Ok` or `ToolStatus::Error`. `fixtures::tool_of` forwards the status unchanged, so a `None` output does not turn into `Pending`.
   - The round-2 build notes say "the property test generates failed/pending Edit/Write calls", but it does not.
   - Change: choose the status three ways, with `None` output plus `ToolStatus::Pending` as the third option.
3. [nit] The `… N more lines` row is no longer directly below the content it continues, in two cases:
   - **Subagent** (`subagent.rs`): the row sits below the blank row and the `a to open transcript` hint. Round-1 finding 1 suggested putting the more row directly under the result. I accept the builder's order instead: the planner calls the hint "a final row", and also requires the stop to come "directly after its tool section". Both cannot hold at once, and the builder documented the choice on `SubagentBody`.
   - **Edit or Write with cut content and a failed status**: the more row lands below the error `Result:`. It reads as though the error was truncated, but `Submit` expands the diff or content. Change, optionally: in that case, say what is cut in the row text, or note the situation in the `with_status_result` doc comment. No code change is required for the Subagent case.
4. [nit] `range_note` in `src/components/turn_block/renderers/tool/read.rs` now omits the note for `offset: 1` without a `limit`, following a round-1 UI nit. The planner note reads "plus ` (lines a–b)` when the input has an offset/limit". The output is the same in practice, since the read starts at line 1 either way, but it departs from the literal wording. It is recorded here so the deviation is on file. No change needed.
