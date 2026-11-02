STATUS: DONE

## Changes
Round 2 addresses the round-1 reviews (no human/approver note).
- `src/app/mouse.rs`: `session_clicked` now selects the row before focusing Sessions, so the §4.1 newest-session rule no longer resets the scroll of a list scrolled with the wheel; the double-click on that row now opens it. Wheel over Projects is ignored while the selector is filtered (`takes_mouse` now takes the event kind). A left press that is not on a row (tab, empty space, elsewhere, nothing) clears the pending double-click (`MouseState::row_pressed`). `refresh_routing` doc says what it really does.
- `src/app/mod.rs`, `src/app/selector.rs`: dropped the redundant `refresh_routing` in `resized` and the second `layout()` call in `push_viewports` (reads the refreshed routing layout). `refresh_routing` still runs after every `update`; the comments now say so.
- `src/components/list_pane.rs`: `ListCursor::set` doc reworded (new rows undo a wheel scroll made with nothing selected).

## Self-check
- Review (code-review 1, must-fix): click makes a wheel-scrolled list jump — `a_double_click_in_a_list_scrolled_by_the_wheel_opens_the_row_under_the_pointer` (80x22, fails with the old order, verified).
- Review (code-review 2 / feature-review 1, nit): wheel over Projects in the filtered selector — ignored; `the_wheel_over_projects_leaves_the_filtered_selector_alone`.
- Review (code-review 3 / feature-review 2, nit): comment fixed, redundant layout work removed; no extra test needed.
- Review (code-review 4, nit): `a_press_that_is_not_on_a_row_ends_a_double_click_under_way`.
- Review (code-review 5, nit): doc comment reworded.
- All round-1 acceptance-criteria tests still pass; UI: none.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2328 passed, 0 failed)
