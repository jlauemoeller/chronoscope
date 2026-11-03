STATUS: DONE

## Changes
- `src/components/turn_sidebar/mod.rs`: the pane handles `UserEvent::Mouse` (`RoutedMouse`) in `on`. The wheel steps the selection like `Up`/`Down` (shared `moved`, no wrap, first entry when nothing is selected) and answers `Msg::TurnSelected`; a left press on an entry row answers the new `Msg::TurnClicked(TurnId)`. `turn_at(column, row, size)` is the sidebar's own `row_at` (border, two blank rows, list from its scroll offset, `entry_at_row`); it returns `None` on Start, End, connectors, blank rows, the border columns, the rows above the list, and below the last row. Raw `Event::Mouse` still answers nothing. Module doc updated.
- `src/app/mod.rs`: `Msg::TurnClicked(TurnId)`, handled by `turn_clicked`.
- `src/app/mouse.rs`: `takes_mouse` gives `Id::TurnSidebar` mouse events unless a prompt has the input; `ClickTarget::Turn(TurnId)`; `App::turn_clicked` presses via `MouseState::press` (so a press elsewhere ends a double-click), focuses the sidebar (`focus_turns`), selects the turn (`SELECTED_TURN_ATTR`), and calls `go_to_turn(id, double)` (the `]`/`[` path; `Enter` behaviour on a double-click). Clicking the transcript needed no new code.
- Tests: new `src/app/browser_tests/sidebar_mouse.rs` (app level), new `mouse` module in `src/components/turn_sidebar/tests.rs` (component level), `hidden`/`focus_sidebar`/`select_down` made `pub(super)` in `sidebar_keys.rs`. Two tests in `browser_tests/sidebar.rs` that asserted the sidebar ignores clicks/wheel were narrowed to the border/gap (the pane now takes both). No snapshot files changed.

Interpretation note: "the margin" is read as the gap column after the pane (`SidebarGap`, never hit-tested) and the border columns, as in the selector's lists; a click on the blank cells inside the border of an entry's row selects that entry, as in the selector.

## Self-check
- AC: mouse tests cover click, double-click, connector click, wheel — `a_click_on_a_row_focuses_the_sidebar_selects_it_and_scrolls_like_j`, `a_double_click_does_what_enter_does`, `presses_on_no_entry_change_neither_the_selection_nor_the_focus`, `the_wheel_selects_the_next_and_previous_entry_and_scrolls_like_the_keys`
- AC: click focuses, selects, scrolls like `j`/`k` — `a_click_on_a_row_focuses_the_sidebar_selects_it_and_scrolls_like_j`, `a_click_moves_the_selection_up_too_and_leaves_the_pane_focused`
- AC: double-click leaves transcript focus, selection on first selectable section — `a_double_click_does_what_enter_does`, `a_double_click_on_a_row_the_transcript_cannot_select_in_leaves_its_selection`
- AC: presses on Start/End/connector/blank/border/margin change nothing, and end a double-click — `presses_on_no_entry_change_neither_the_selection_nor_the_focus`, `presses_on_no_entry_do_not_focus_the_sidebar_from_the_transcript`, `a_press_on_no_entry_between_two_clicks_on_a_row_ends_the_double_click`, `a_press_elsewhere_on_the_screen_between_two_clicks_ends_the_double_click_too`, `end_is_found_at_the_end_of_a_scrolled_list_and_is_not_selectable`, component `a_press_elsewhere_in_the_pane_answers_nothing`
- AC: different rows / more than DOUBLE_CLICK apart not a double-click — `two_clicks_on_different_rows_are_not_a_double_click`, `two_clicks_further_apart_than_the_double_click_time_are_not_a_double_click` (400/401/1000 ms)
- AC: row found when scrolled — `a_row_is_found_when_the_list_is_scrolled`, `a_connector_row_is_found_when_the_list_is_scrolled`, component `a_row_is_found_by_the_scroll_offset`
- AC: wheel moves selection, scrolls, keeps focus, no wrap — `the_wheel_selects_the_next_and_previous_entry_and_scrolls_like_the_keys`, `the_wheel_does_not_change_the_focus`, `the_wheel_does_not_wrap_around_at_either_end`, component `the_wheel_*`
- AC: wheel moves past entry with nothing selectable, transcript focused — `the_wheel_moves_past_an_entry_with_nothing_selectable_while_the_transcript_has_the_focus`
- AC: clicking transcript focuses it — `a_click_on_the_transcript_gives_it_the_focus_while_the_sidebar_has_it`, `a_click_on_the_transcript_after_the_wheel_moved_the_sidebar_on_syncs_it`
- AC: prompt / overlay / `mouse = false` give no mouse — `with_a_prompt_open_the_sidebar_gets_no_mouse`, `with_an_overlay_open_the_sidebar_gets_no_mouse` (help, themes, selector), `a_press_on_the_sidebar_under_an_overlay_that_covers_it_does_not_reach_it`, `with_mouse_off_the_sidebar_gets_no_mouse`, `the_sidebar_gets_no_mouse_when_it_is_hidden`
- AC: raw `Event::Mouse` to the focused sidebar does nothing — `the_other_keys_do_nothing_and_neither_does_a_raw_mouse_event`
- AC: no snapshot changes — no `.snap` touched; UI: none.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (3181 passed, 0 failed)
