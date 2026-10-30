VERDICT: PASS

## Summary
The task's `UI` section is `UI: none`. The diff adds the worker-side live watch and a new `ModelUpdate::Activity` variant. The only UI-adjacent changes are no-op match arms in `src/app/browser.rs` and `src/components/transcript/updates.rs`. No views, layouts or `insta` snapshots were added or changed, so there is nothing to render.

## States checked
- No UI states are specified (`UI: none`). Checked that `git diff main...HEAD` touches no `*.snap` files and changes no rendering code. ok
- Browser handling of `ModelUpdate::Activity` (`App::session_update`) and transcript handling (`Transcript` updates): both ignore it, so header and transcript rendering are unchanged. ok
- Reload path seen from the browser: `Reset` goes to the transcript, `Progress` after the first `Loaded` is ignored because `browser.loaded` stays true, so no loading indicator flashes again, and the closing `Loaded` refreshes the empty-session state. ok (code review, nothing to render)

## Findings
1. [nit] Browser header, any state: `Activity` is received but dropped, so the `● live` marker from SPEC §7.2 does not appear yet. That is consistent with this task's scope ("so the browser can show `● live`"), but the code comment "not shown yet" should name the task that will draw it, so the gap is tracked. Change: no UI change in this task. Make sure a later task (with its own `UI` section and snapshots) stores `last_modified`/`idle` and renders the marker against the 60 s window.
