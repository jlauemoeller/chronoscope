VERDICT: PASS

## Summary
The task's UI section is `UI: none`. This task adds mouse input to the transcript, the help overlay and the theme selector. It does not change any `view` code, add or change `insta` snapshots, or change anything drawn on screen. Mouse events only change state through the existing paths (scroll, fold, cursor, close), so the existing snapshots still cover the rendering. `INSTA_UPDATE=no cargo test` passes: 2131 lib tests, and every other test binary passes.

## States checked
- UI: none — no views or snapshots in `git diff main...HEAD` (only `on()`/mouse handlers and tests changed) — ok
- Existing snapshot suite — `INSTA_UPDATE=no cargo test` passes and no snapshot changed — ok

## Findings
None.
