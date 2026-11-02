STATUS: DONE

## Changes
- `tool/mod.rs` `with_status_result(body, call, ctx, content_cut)`: one truncatable block per body, always the last. If the Write content / Edit diff exceeds `ctx.truncate` (computed in `write.rs` as `truncate_lines(content, ctx.truncate).1 > 0` and in `edit.rs` as `rows.len() > ctx.truncate`, independent of `expanded`), the status `Result:` is inserted right after `Path:` and shown in full; `remaining` stays the content/diff cut. Otherwise the order is `Path:`, content/diff, `Result:`, the result is laid out with the normal `ctx` and `body.remaining` is its cut (`(pending)` is never cut). `(no content)` / `(no changes)` pass `false`. Doc comment states the rule. A successful call is unchanged.
- `Body::remaining` / `Body::hint` doc comments reworded.
- `subagent.rs`: the `Err` arm uses `unavailable_block("Result:", ...)`.
- `read.rs` `range_note`: an end below the start is ignored (gives `(from line N)`). `huge_offsets_and_limits_do_not_overflow` now expects `(from line ...)`.
- `property.rs`: tool status is chosen three ways (Ok, Error, Pending with no output), so pending Write/Edit are height-checked in both block orders.
- `write::tests::long_lines_wrap_and_long_content_truncates` now uses a finished call (a pending one gets the `Result:` block in front of the content).

## Self-check
- Failed, short content/diff, 10-line error, `truncate: 4` (order Path, content, Result; 4 error lines; `remaining == Some(6)`; expanded 10 lines, `None`) — `write::tests::failed_short_content_long_error_puts_the_truncated_result_last`, `edit::tests::failed_short_diff_long_error_puts_the_truncated_result_last`.
- Failed, long content/diff, short error (Path, Result, content; `remaining` is the content cut; same order expanded) — `write::tests::failed_long_content_short_error_puts_the_result_before_the_content`, `edit::tests::failed_long_diff_short_error_puts_the_result_before_the_diff`.
- Failed, both long (error in full, `remaining` is the content cut only) — `write::tests::failed_long_content_and_long_error_show_the_error_in_full`, `edit::tests::failed_long_diff_and_long_error_show_the_error_in_full`.
- Pending, long content/diff (Path, `(pending)`, content) — `write::tests::pending_long_content_puts_pending_before_the_content`, `edit::tests::pending_long_diff_puts_pending_before_the_diff`.
- Successful Write unchanged — `write::tests::a_successful_long_write_has_no_result_block`.
- Truncated failed Edit snapshot (width 100, truncate 5, `Result:` before `Diff:`, `… 6 more lines` directly under the last diff row) — `failed_edit_with_a_truncated_diff_shows_its_error_first_at_100_text` (new). `failed_edit_shows_its_error_below_the_diff_at_100_styled` unchanged; no other snapshot changed.
- `range_note` reversed case — `read::tests::range_note_never_gives_a_reversed_range`.
- Proptest with Pending — `property::cached_heights_equal_rendered_heights`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (1189 passed, 0 failed)
