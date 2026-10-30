VERDICT: FAIL

## Summary
The per-kind body renderers, the subagent section, the truncation row as a selection stop, the expansion state and lazy materialisation are well structured and thoroughly tested. Every UI state has an accepted snapshot, and the layout-cache proptest covers all ten kinds with random truncation and expansion. Three issues block: a failed or pending Edit/Write looks exactly like a successful one, the Subagent `Result:` ignores `truncate_output_lines`, and Read does unchecked arithmetic on input numbers taken from the session file.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1165 passed, 0 failed; no `.snap.new` files, working tree clean)

## Findings
1. [must-fix] `src/components/turn_block/renderers/tool/edit.rs` (`EditBody::body`) and `src/components/turn_block/renderers/tool/write.rs` (`WriteBody::body`) — neither body calls `result_block`.
   - Nothing else in `turn_block/` reads `ToolStatus` or `is_error`: not the title, not the emphasis, not the description.
   - So a rejected Edit (Claude's "String to replace not found …" `tool_result` with `is_error`) shows its diff exactly like an applied one, and the error text is never shown.
   - A pending Edit/Write likewise shows no `(pending)`.
   - This breaks the planner's general rules: "Results use `output`, or `error_output` when the call failed" and "A pending call shows `(pending)` in `muted_foreground` as its result".
   - Change: in both bodies, when `call.status` is `ToolStatus::Error` or `ToolStatus::Pending`, append `result_block("Result:", call, ctx)` after the `Content:`/`Diff:` block. On success, leave the body as the table specifies.
   - Add unit tests for a failed Edit (result text in `error_output`) and a pending Write (`(pending)` in `muted_foreground`). The property test already generates failed and pending Edit/Write calls through `kind_of`, so it will cover the new rows.
2. [must-fix] `src/components/turn_block/renderers/tool/subagent.rs` (`SubagentBody::body`) — the `Result:` is never truncated (`remaining: None`, documented as "The result is not truncated").
   - The binding planner note reads: "When **a body's** result has more than `truncate_output_lines` lines, only the first N lines are shown, followed by a row `… <remaining> more lines`". SPEC §5.3 says the same. Nothing excludes subagents.
   - A long final report from a subagent therefore unfolds in full, with no `… N more lines` stop.
   - Change: apply `truncate_lines(&text, limit(ctx))` to the result text before `render_markdown`, and set `Body::remaining` to the cut count. The existing more-row, `Cmd::Submit` expansion and fold persistence then work unchanged.
   - Keep the hint row after the result, before the more row, or document the order you choose. Add a unit test like `read::tests::only_the_first_lines_are_numbered_when_truncated` for a subagent.
3. [must-fix] `src/components/turn_block/renderers/tool/read.rs` — `range_note` computes `a + n - 1`, and `numbered` computes `offset + i`, on numbers taken unchecked from the tool input JSON in the session file.
   - An input such as `{"offset": 18446744073709551615, "limit": 2}` panics with an arithmetic overflow in debug/test builds. `Cargo.toml` has no `overflow-checks`, so release builds silently wrap to nonsense line numbers.
   - The render path must not panic on harness data (SPEC §9.5 robustness; best practice is no panics on external input).
   - Change: use `a.saturating_add(n).saturating_sub(1)` in `range_note` and `offset.saturating_add(i)` in `numbered`.
   - Add a test that renders a Read with `offset = u64::MAX` and `limit = u64::MAX` and asserts that it does not panic.
4. [nit] `src/components/turn_block/renderers/tool/read.rs` (`ReadBody::body`) — when the output cannot be read (`text().ok()` gives `None`) or is blank, `result_block` calls `ToolOutput::text()` a second time. A lazy output is then re-read from its source twice. Change: match on `text()` once, and build the error, `(no output)` or numbered block from that single result.
5. [nit] `src/components/turn_block/renderers/tool/mod.rs` (`result_block`) and `subagent.rs` — `log::warn!` for an unavailable output runs whenever the section is laid out, so it repeats on every relayout: width change, setting change, live update. Consider logging once per call id, for example by de-duplicating at the caller, or document that this is accepted.
6. [nit] `src/components/turn_block/renderers/mod.rs` and `tool/mod.rs` — public API hygiene:
   - `Rendered` and `ToolRendered` are identical structs. Return `Rendered` from `ToolCallSection::render` and drop `ToolRendered`.
   - `renderers/mod.rs` re-exports every `*Body` type, `body_renderer`, `ToolRendered` and `group_thousands` as `pub`, though nothing outside `tool/` uses them.
   - `ToolBodyRenderer` is re-exported, but the `Body` and `Block` types it returns are not, so they cannot be named outside the module.
   - Either re-export `Body`/`Block` with the trait, or narrow the unused re-exports to `pub(crate)` or remove them.
7. [nit] `src/components/turn_block/widget.rs` (`TurnBlock::layout`) — now 52 lines, at the SPEC §1.3 "about 50 lines" limit. Consider moving the more-row push (the `if let (Some(more), Some(id))` block) into a small helper.
8. [nit] `src/components/turn_block/renderers/tool/subagent.rs` and `tool/mod.rs` — the `<key> to open transcript` hint and the `… N more lines` row are single unwrapped lines. At very narrow widths, or with a long remapped key label, they are clipped at the padding instead of wrapping. This is harmless for heights, since each stays one row. Consider `ellipsize` so the clipping is visible.
9. [nit] `src/components/turn_block/renderers/tool/read.rs` (`range_note`) — the note shows the requested range (`offset`…`offset+limit-1`), not the lines actually returned. A 3-line file read with `limit: 2000` shows `(lines 1–2000)`. Consider taking the end from the last number shown in the gutter when it is smaller.
10. [nit] `src/components/turn_block/renderers/tool/edit.rs` (`diff_rows`) — `TextDiff::from_lines` runs without a deadline on every layout of an unfolded Edit. Consider `TextDiff::configure().timeout(...)` so that a pathological MultiEdit cannot stall layout.
