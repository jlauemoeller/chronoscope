VERDICT: PASS

## Summary
The task's `UI` section is `UI: none`, so there are no UI states or sizes to check. Round 2 adds tests and moves a timestamp parse in `on_assistant`; across the whole branch, no rendering code and no snapshot changed, so no scratch crate was needed.

## States checked
- None required (`UI: none`).
- Diff scope: outside `factory/` and `tests/claude_assembly.rs`, `git diff main...HEAD` touches only `src/harness/claude/assemble.rs`, `src/harness/claude/wire.rs` and a doc comment in `src/model/mod.rs`. It changes no `.snap` file and nothing under `src/components` or `src/app` — ok.
- Count of 0 (a turn of only `<synthetic>` messages): the unchanged `format_stats` in `src/components/turn_block/renderers/stats.rs` only adds the message part when `message_count > 0` — ok, read in code, not rendered.
- Snapshots: `INSTA_UPDATE=no cargo test` reports 0 failed in every suite, leaves the working tree clean and produces no `.snap.new` files — ok.

## Findings
None.
