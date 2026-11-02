VERDICT: PASS

## Summary
The task's `UI` section is `UI: none`. The diff changes only the OpenCode harness (`src/harness/opencode/{assemble,discovery,follow,load,mod,watch}.rs`) and adds `tests/opencode_live.rs`. It touches no view, component, theme or `insta` snapshot. The browser's `● live`, follow mode and `↓ N new` (task 023) and the selector's `●` marker are reused unchanged. They are driven by the `ModelUpdate`s (`Appended`/`Updated`/`Activity`) that the new worker emits, so this task has no UI to review.

## States checked
- No UI states are listed (`UI: none`). The diff adds or changes no views or snapshots, so nothing was rendered in a scratch crate.

## Findings
None.
