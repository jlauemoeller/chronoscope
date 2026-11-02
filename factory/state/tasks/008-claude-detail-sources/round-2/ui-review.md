VERDICT: PASS

## Summary
The round-1 must-fix is resolved: the `unavailable` word no longer counts towards the label column, so `timestamp` values sit at offset 11 beside it and a prompt or compaction whose source is gone is 40 columns wide. Both round-1 nits are fixed too: an empty-string field draws no row, and the App-level tool call snapshots show `resolvedModel`. All required snapshots exist and match the `UI` section, and renders of the real App path at 60x20, 80x24 and 120x40 found no must-fix issue. Two nits remain.

## States checked
Scratch renders ran the public `App` with a real `ClaudeHarness::open_file` and `load`, keys through a port, on synthetic transcripts covering every section kind. No real transcript was used this round.

- Claude tool call @ 80x24 — snapshot `the_details_of_a_claude_tool_call_80x24_text` — ok (`start`, `duration`, `status`, `permissionDecision`, `sandboxed`, `resolvedModel`, `isAsync`; Environment below the fold with ` ↓ more `)
- Claude tool call @ 120x40 — snapshot `the_details_of_a_claude_tool_call_120x40_text` — ok (all groups in §5.3 order)
- Section with an unreadable source (text, file gone) @ 80x24 — snapshot `the_details_of_a_section_whose_source_is_gone_80x24_text` — ok
- Section with an unreadable source (text, file gone) @ 120x40 — snapshot `the_details_of_a_section_whose_source_is_gone_120x40_text` — ok
- Component-level `unavailable` @ 80x24 — snapshot `unavailable_80x24_default` — ok (the diff changes only the `model` row and its style line: the value moves from offset 13 to 7, and the overlay stays 40 columns)
- Every section kind, source intact @ 120x40 — rendered in scratch crate — ok (Prompt, Turn, Message, Tool call and Environment rows in §5.3 order; `stop_reason` from the later line; no `cost`; compaction shows `timestamp` first under Environment, then what the source provides)
- File gone: prompt @ 60x20, 80x24 and 120x40 — rendered in scratch crate — ok (round-1 issue fixed: `timestamp` value at offset 11, overlay 40 wide, one row per affected group)
- File gone: compaction @ 120x40 — rendered in scratch crate — ok (round-1 issue fixed: `timestamp` value at offset 11, overlay 40 wide)
- File gone: thinking, text, tool call, model note, notice, system, footer @ 120x40 — rendered in scratch crate — ok (the model's rows, then one `unavailable` per affected group; overlay stays open)
- File gone: text section @ 60x20 — rendered in scratch crate — ok (overlay capped, ` ↓ more `)
- `unavailable` colour, prompt with file gone @ 80x24 — styled scratch render — ok (`Rgb(161,161,161)` on `surface`), and `unavailable_is_muted` asserts `muted_foreground` through the App at 120x40
- Result line unparseable, call line fine @ 120x40 — rendered in scratch crate — ok (`start`, `duration`, `status`, `unavailable`; Environment from the call's line)
- Result line unparseable, scrolled to the end @ 60x20 — rendered in scratch crate — ok (` ↑ more `, `unavailable` at the row column)
- Call line unparseable, result line fine @ 120x40 — rendered in scratch crate — ok (`duration`, `status`, the result's rows, `unavailable`; Environment `unavailable`)
- Tool call with no result yet @ 120x40 — rendered in scratch crate — ok (`start`, `status pending`, no `unavailable`)
- Tool call with no result, then the result appended live @ 120x40 — rendered in scratch crate — ok (reopening shows the result's rows)
- Transcript truncated while open in the browser @ 120x40 — rendered in scratch crate — ok (reloads to the empty state; no overlay, no panic)
- A tool call id repeated in a later turn @ 120x40 — rendered in scratch crate — ok (the later call now reads its own `start` and Environment, and stays `pending`; the second result still goes to the first call, as before this task, so the first call shows `resolvedModel m-r2` from the later turn's result line)
- All Message rows, including `diagnostics.cache_miss_reason` @ 60x20, scrolled to the end — rendered in scratch crate — ok (values wrap aligned, nothing overflows the border)
- Tool call with a long `permissionDecision` @ 60x20, scrolled to the end — rendered in scratch crate — ok (wrapped at the value column)
- 59x20 — rendered in scratch crate — ok (`Terminal too small`, overlay does not open)
- Long `cwd`, 200-character `requestId`, 300-character `permissionDecision` member, CJK `entrypoint` @ 80x24 — rendered in scratch crate — ok (wrap at the value column, overlay capped)
- Control characters in values (ESC in `gitBranch` and `permissionDecision`, BEL and CR in `stop_reason`) @ 80x24 — rendered in scratch crate — ok (dropped by the layout, so no terminal sequence reaches the screen)
- Empty-string field (`"version": ""`) @ 80x24 — rendered in scratch crate — ok (round-1 nit fixed: no row)
- Wrong JSON types, string `toolUseResult`, line without the fields @ 80x24 — rendered in scratch crate — ok (rows left out, group left out, no `unavailable`)
- `permissionDecision` and `cache_miss_reason` objects with no scalar members @ 80x24 — rendered in scratch crate — ok (no row)
- `permissionDecision` with an empty-string member @ 80x24 — rendered in scratch crate — issue, finding 1
- Harness that provides nothing — test `a_harness_that_provides_nothing_leaves_the_models_rows_and_no_unavailable_row` — ok
- Values start at the widest real label — test `unavailable_has_no_label_column_so_values_start_at_the_widest_real_label` — ok

Gates, run with `INSTA_UPDATE=no`: `cargo test` 2840 passed, 0 failed; `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass. No `.snap.new` file was left, and the working tree stayed clean before this review was written.

Note for the Approver: `unavailable_80x24_default` is a component-level snapshot, so its change goes beyond the acceptance criterion that limits snapshot changes to App-level ones. SPEC §7 allows changes to the rows a task intends to change, this one is intended, and the round-1 UI review asked for it. No other existing snapshot changed outside the task's own four App-level ones.

## Findings
1. [nit] Details overlay, tool call whose `permissionDecision` has an empty-string member, 80x24 — `{"decision": ""}` draws `permissionDecision  decision=`, a member with a blank value, although `Rows::text` now treats an empty string as no value. No such line is known from Claude Code, so this is only about consistency. Change: in `Member::visit_str` in `src/harness/claude/details.rs`, return `Member(None)` for an empty string, so the member is left out and an object with no other members draws no row. See `ui/empty-member-80x24.txt`.
2. [nit] `factory/state/tasks/008-claude-detail-sources/round-2/build-notes.md`, "`unavailable` and the label column" — it says the `model` value of `unavailable_80x24_default` moves "from offset 13 to 9". The snapshot's style line shows the label at column 25 and the value at column 32, so the new offset is 7. The snapshot itself is right. Change: none in the code; read 7 when reviewing the snapshot, and correct the sentence if the notes are edited again.
