VERDICT: PASS

## Summary
The task's `## UI` section is `UI: none`, and this task adds no new visual state: mouse events only drive the selection, focus and scroll state that existing views already draw. The diff changes no drawing code and adds or changes no `insta` snapshots. `INSTA_UPDATE=no cargo test` passes and leaves the working tree clean.

## States checked
- UI section — `UI: none` — ok (no states or sizes required)
- Snapshots — `git diff main...HEAD -- '**/snapshots/*'` is empty; no snapshot added or changed — ok
- Drawing code — `list_pane.rs` (list drawing at line ~490), `harness_tabs.rs` `view`, `layout.rs`: unchanged apart from `#[derive(Default)]` on `ScreenLayout` and a doc comment — ok
- Click on a row matches the drawn row — `PaneState::row_at` and the list drawing both use `ListCursor::offset_for(visible_rows(height))` and `ROWS_ABOVE_LIST`, so a click lands on the row drawn under the pointer, including in a scrolled list — ok (code inspection; covered by `a_click_on_a_session_row_of_a_scrolled_list_hits_the_row_drawn_there`)
- Wheel scroll with no selection — `ListCursor::scroll` changes `offset`, which `offset_for` (used by the drawing code) keeps when nothing is selected, so the list draws scrolled — ok (code inspection; `a_scroll_survives_a_selection_that_stays_none`)
- Clicks on the harness tabs — `HarnessTabs::tab_at` uses the same `label()` widths and `SEPARATOR` as the drawn tab line, so clickable spans match the drawn labels — ok (code inspection; component tests)
- Test gate — `INSTA_UPDATE=no cargo test`: all suites pass, 0 failed — ok

## Findings
None.
