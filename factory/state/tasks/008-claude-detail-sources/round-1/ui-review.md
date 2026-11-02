VERDICT: FAIL

## Summary
The overlay's frame, layout and keys are unchanged from task 007: the only snapshot changes are the 4 new App-level ones, and `details_overlay/mod.rs` and `rows.rs` gain only a module, a re-export and a constant. Message, Tool call, Prompt and Environment rows appear in §5.3 order for every section kind, and `unavailable` is one muted row at the row column, both in the Builder's snapshots and in renders of the real App path (real `ClaudeHarness`, real file load) at 60x20, 80x24 and 120x40. One must-fix remains, which the Builder flagged for reviewers: the `unavailable` word is measured as a label, although the UI section says the row has no label column, so it moves the values of other rows. Two nits are also listed.

## States checked
Scratch renders ran the public `App` with a real `ClaudeHarness::open_file` and `load`, keys through a port, on a synthetic transcript covering every section kind. They were also run on a copy of a real transcript at 80x24 for its first 6 sections (no copy is kept).

- Claude tool call @ 80x24 — snapshot `the_details_of_a_claude_tool_call_80x24_text` — ok (Environment is below the fold with ` ↓ more `, as the 007 frame does)
- Claude tool call @ 120x40 — snapshot `the_details_of_a_claude_tool_call_120x40_text` — ok (`start`, `duration`, `status`, `permissionDecision`, `sandboxed`, `isAsync`; see finding 3 on `resolvedModel`)
- Claude tool call, scrolled to the end @ 80x24 — rendered in scratch crate — ok (` ↑ more `, all of Environment, wrapped `permissionDecision` aligned to the value column)
- Tool call with all four result fields @ 120x40 — rendered in scratch crate — ok (`resolvedModel` between `sandboxed` and `isAsync`)
- Section with an unreadable source (text, file gone) @ 80x24 — snapshot `the_details_of_a_section_whose_source_is_gone_80x24_text` — ok
- Section with an unreadable source (text, file gone) @ 120x40 — snapshot `the_details_of_a_section_whose_source_is_gone_120x40_text` — ok
- `unavailable` colour @ 80x24 — styled scratch render — ok (`Rgb(161,161,161)` on `surface`), and `unavailable_is_muted` asserts `muted_foreground` through the App at 120x40
- File gone: prompt, thinking, text, tool call, model note, notice, system, footer, compaction @ 120x40 — rendered in scratch crate — ok (one row per affected group, after the model's rows; overlay stays open)
- File gone: prompt, compaction @ 120x40 — rendered in scratch crate — issue, finding 1
- Result line unparseable, call line fine @ 120x40 — rendered in scratch crate — ok (`start`, `duration`, `status`, `unavailable`; Environment still read from the call's line)
- Call line unparseable, result line fine @ 120x40 — rendered in scratch crate — ok (`duration`, `status`, the result's rows, `unavailable`; Environment `unavailable`)
- Tool call with no result yet @ 120x40 — rendered in scratch crate — ok (`start`, `status pending`, no `unavailable`)
- Transcript shrunk while open in the browser @ 120x40 — rendered in scratch crate — ok (reloads; the new turns carry new references)
- Prompt @ 120x40 — rendered in scratch crate — ok (one `timestamp`, the line's; `permissionMode`, `promptSource`; no Turn group)
- Thinking, text, notice @ 120x40 — rendered in scratch crate — ok (Turn, Message, Environment; `stop_reason` from the later line; no `cost`)
- Model note, footer @ 120x40 — rendered in scratch crate — ok (Turn, Environment, as the 007 `model_note` snapshot shows)
- System, compaction @ 120x40 — rendered in scratch crate — ok (`timestamp` first under Environment, then what the source provides)
- All Message rows, including `diagnostics.cache_miss_reason` @ 60x20 (the smallest size), top and end — rendered in scratch crate — ok (the 29-column label leaves 17 columns; values wrap aligned, nothing overflows the border)
- 59x20 — rendered in scratch crate — ok (`Terminal too small`, overlay does not open)
- Long `cwd`, 200-character `requestId`, 300-character `permissionDecision` member, CJK `entrypoint` @ 80x24 — rendered in scratch crate — ok (wrap at the value column, overlay capped)
- Control characters in values (ESC in `gitBranch` and `permissionDecision`, BEL and CR in `stop_reason`) @ 80x24 — rendered in scratch crate — ok (dropped by the 007 layout, so no terminal sequence reaches the screen)
- Wrong JSON types, string `toolUseResult`, line without the fields @ 80x24 — rendered in scratch crate — ok (rows left out, group left out, no `unavailable`)
- Empty-string field @ 80x24 — rendered in scratch crate — issue, finding 2
- Harness that provides nothing — test `a_harness_that_provides_nothing_leaves_the_models_rows_and_no_unavailable_row` — ok

Gates, run with `INSTA_UPDATE=no`: `cargo test` 2832 passed, 0 failed; `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass. The working tree stayed clean and no existing snapshot changed.

## Findings
1. [must-fix] Details overlay, a prompt or compaction section with its source gone (or any section whose other labels are all shorter than 11 columns), 120x40 and 80x24 — the UI section says `unavailable` is "one row with no label column", but `DetailsRow::unavailable()` stores the word as a label and `DetailsContent::label_width` measures it. The word itself is drawn correctly (at the row column, muted, no value), but the values of the other rows sit at offset 13 from the row column instead of 11, and a prompt's overlay is 41 columns wide instead of 40. The two unreadable-source snapshots cannot show this, because they use a text section, where `output tokens` (13 columns) is already wider than the word. Task 007's UI section does not specify this row, so its snapshot `unavailable_80x24_default` records an unpinned choice, and this task's specific line takes precedence over its general "layout unchanged from task 007". Change: (a) leave label-only muted rows out of `DetailsContent::label_width`, which `widest_row` and the overlay width also use, so `timestamp` values start at offset 11 beside `unavailable`; (b) accept the one intended change to `unavailable_80x24_default`, where the `model` value moves to offset 7 and the overlay stays at the 40-column minimum; (c) add an App-level check with a prompt whose file is gone, asserting the `timestamp` value's column. Note for the Approver: (b) goes beyond the acceptance criterion that limits snapshot changes to App-level ones. SPEC §7 allows changes to the rows a task intends to change, and this one is intended. See `ui/unavailable-widens-label-column-120x40.txt`.
2. [nit] Details overlay, a line with an empty-string field, 80x24 — `"version": ""` draws a `version` label with a blank value, where a missing field draws no row. No `gitBranch` in the local transcripts is empty, so this is not seen in practice. Change: treat an empty string as missing in `Rows::text` in `src/harness/claude/details.rs`, as `turn_group` already does for `effort`. See `ui/empty-string-field-80x24.txt`.
3. [nit] Claude tool call snapshots, 80x24 and 120x40 — `result_line()` in `src/app/browser_tests/details_source.rs` records no `resolvedModel`, so the App-level snapshots show three of the four rows after `status`. Its position is covered only by adapter tests and this review's render. Change: add `"resolvedModel": "claude-haiku-5-5"` to `toolUseResult` in `result_line()` and accept the two snapshots in that round.
