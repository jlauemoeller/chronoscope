VERDICT: PASS

## Summary
The task's `UI` section is `UI: none`, so there are no UI states or sizes to check. The diff changes only the values the Claude adapter puts in `TurnStats`: no rendering code and no snapshot changed, and no scratch crate was needed.

## States checked
- None required (`UI: none`).
- Diff scope: `git diff main...HEAD` touches `src/harness/claude/assemble.rs`, `src/harness/claude/wire.rs`, a doc comment in `src/model/mod.rs`, `tests/claude_assembly.rs` and the build notes. It changes no `.snap` file and nothing under `src/components` or `src/app` — ok.
- Count of 0 (a turn of only `<synthetic>` messages): the unchanged footer renderer, `src/components/turn_block/renderers/stats.rs:28`, only adds the message part when `message_count > 0` — ok, read in code, not rendered.
- Snapshots: `INSTA_UPDATE=no cargo test` reports 0 failed in every suite and leaves the working tree clean — ok.

## Findings
None.
