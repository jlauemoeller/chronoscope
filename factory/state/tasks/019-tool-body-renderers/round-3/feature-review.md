VERDICT: PASS

## Summary
Round 3 does what the round-2 rework note asked for. A failed or pending Write/Edit now has one truncatable block, and it is always the last one. If the content or diff is cut at `truncate_output_lines`, the status `Result:` goes right after `Path:` and is shown in full. Otherwise the `Result:` comes last and is truncated like any other result. The order is computed from `ctx.truncate`, not from the current limit, so it does not change when the section is expanded. The extra items are also done: the `Body` doc comments, the subagent `Err` arm, the reversed-range guard in `range_note`, and Pending in the proptest. The gate is green: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean, `INSTA_UPDATE=no cargo test` has no failures, there are no `.snap.new` files, and the tree is unchanged.

## Acceptance criteria
- [x] Each `ToolKind` renders the body from the planner-notes table, with one test per kind for labels, columns 4/7 and tokens.
  - Implemented in `renderers/tool/{shell,read,write,edit,search,web,other,subagent}.rs`, with enum dispatch. These tests are unchanged from rounds 1 and 2 and pass.
  - The status `Result:` of a failed or pending Write/Edit now follows the general result rules (`error_output`, `(pending)`, truncation). See `with_status_result` in `tool/mod.rs`, called from `write.rs:56` and `edit.rs:126` with `content_cut`, which is `truncate_lines(content, ctx.truncate).1 > 0` for Write and `rows.len() > ctx.truncate` for Edit.
- [x] Shell command and result are char-wrapped. Covered by `shell::tests::a_long_path_is_char_wrapped_at_the_body_width` and the Shell snapshots.
- [x] Read parses Claude numbering into the gutter once, and numbers plain output from the offset.
  - Covered by the existing `read::tests::*`.
  - `range_note` now drops an end below the start (`read.rs:43`). Covered by `range_note_never_gives_a_reversed_range`, which checks disagreeing Claude numbers, `u64::MAX` offset and limit, and offset only. `huge_offsets_and_limits_do_not_overflow` was updated to expect `(from line …)`.
- [x] Write content is highlighted by extension. Covered by `write::tests::rust_content_is_highlighted_by_extension` and the styled snapshot.
- [x] An Edit diff has coloured `+`/`-` lines and `@@` headers. Covered by `edit::tests::a_diff_has_headers_and_coloured_plus_and_minus_lines` and the styled snapshots. `failed_edit_shows_its_error_below_the_diff_at_100_styled` is unchanged: its diff is short, so the order stays Path, Diff, Result.
- [x] Subagent folded and unfolded views, and the remapped hint. The existing tests pass. The `Err` arm now uses `unavailable_block`.
- [x] 1 434 lines at a limit of 200 show 200 lines plus `… 1 234 more lines`. `Cmd::Move`, `Cmd::Submit` and fold persistence all work. The `transcript::tests::truncation::*` tests are unchanged and pass.
  - The new Write/Edit cases check each body at `expanded: false` and at `expanded: true`, and the block order is the same in both states:
    - Short content or diff with a 10-line error: Path, content/diff, Result. Shows 4 then 10 error lines; `remaining` is `Some(6)` then `None`.
    - Long content or diff with a short error: Path, Result, content/diff; `remaining` is the content/diff cut.
    - Both long: the Result is shown in full; `remaining` is the content/diff cut only.
    - Pending with long content or diff: Path, `(pending)`, content/diff.
  - These are in `write::tests::{failed_short_content_long_error_puts_the_truncated_result_last, failed_long_content_short_error_puts_the_result_before_the_content, failed_long_content_and_long_error_show_the_error_in_full, pending_long_content_puts_pending_before_the_content, a_successful_long_write_has_no_result_block}` and the matching `edit::tests::*`.
- [x] A folded tool call never calls `ToolOutput::text()`; unfolding calls it. `truncation::a_folded_call_never_materialises_its_output_and_unfolding_does` is unchanged and passes.
- [x] Layout-cache heights match rendering for all new bodies.
  - `property::cached_heights_equal_rendered_heights` now picks the status three ways: Ok, Error, or Pending with `None` output.
  - `fixtures::tool_of` passes the status through as given, and `result_block` treats `Pending` as `(pending)`.
  - `input_of` puts the up-to-300-character generated text into `content`/`old_string`, and the random limit is 1..8. So pending and failed Write/Edit bodies are height-checked in both block orders.
- [x] Every UI state has a snapshot: states 1–9 from earlier rounds, plus the new `failed_edit_with_a_truncated_diff_shows_its_error_first_at_100_text`. It shows Path, Result, Diff, with `… 6 more lines` directly under the last diff row.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I ran all three. Every `test result:` line reports `ok`, and there are no `FAILED` or `panicked` lines.

## Findings
None.
