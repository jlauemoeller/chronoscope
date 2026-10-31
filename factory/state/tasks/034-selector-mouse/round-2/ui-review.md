VERDICT: PASS

## Summary
The task's `## UI` section is `UI: none`. Round 2 adds no drawing code and no new or changed `insta` snapshots. Its changes are to mouse handling order, routing refresh and a doc comment, plus `push_viewports`, which now reads the selector frame from the refreshed routing layout instead of calling `layout()` a second time; the list viewport sizes stay the same. `INSTA_UPDATE=no cargo test` passes and leaves the working tree clean.

## States checked
- UI section — `UI: none` — ok (no states or sizes required)
- Snapshots — `git diff main...HEAD -- '*.snap'` is empty; no snapshot added or changed in either round — ok
- Round-2 diff (`git diff HEAD~1 HEAD`) — `list_pane.rs` changes only a doc comment; `mod.rs` drops a redundant `refresh_routing` in `resized` and rewords a comment — ok
- Viewport rows after a resize — `push_viewports` calls `refresh_routing()` first, then takes `Id::SelectorFrame` from `self.mouse.routing`. This is the same pure `layout(&layout_input(), area)` result as before, so `VISIBLE_ROWS_ATTR` (and therefore the drawn list height and scroll) is unchanged — ok (code inspection)
- Click on a wheel-scrolled list — the row is now selected before Sessions gets focus, so the list no longer jumps to the newest session and the row drawn under the pointer stays where it was — ok (code inspection; `a_double_click_in_a_list_scrolled_by_the_wheel_opens_the_row_under_the_pointer`)
- Test gate — `INSTA_UPDATE=no cargo test`: all suites pass, 0 failed; `git status` clean afterwards — ok

## Findings
None.
