VERDICT: FAIL

## Summary
The `TurnSidebar` component, its rows, the layout, the live updates and the subagent rebuild behave as the spec and wireframe require, and the gate and budgets pass. Two things must change. `TurnSidebar::view` paints outside the area it is given, which breaches base spec §1.3, and no test covers its transparent-background guard: removing the guard leaves all 2589 lib tests passing.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

Also run: `cargo test --release -- --ignored budgets --nocapture` passes. Worst sidebar frame is 0.10 ms; load with rows is 142 ms (mixed) and 296 ms (dense).

Snapshot scope also checked against `main`: every changed snapshot is at 120x40, 200x50 or 200x60. Header rows and help-line rows are unchanged in all of them, in both text and styles.

## Findings
1. [must-fix] `src/components/turn_sidebar/mod.rs:206-214`, `:50` — `paint_background` writes the column at `area.right()`, outside the `area` passed to `view`, and imports `SIDEBAR_GAP` from `crate::app::layout` to know how far to paint. No other non-test component code imports from `app::layout`. Base spec §1.3 (a binding rule, quoted in the task) says "A component draws only itself" and that components render on their own. The sidebar therefore changes a cell that is not its own in any frame wider than its area, as `the_background_is_painted_over_the_pane_and_the_gap_unless_transparent` asserts. Change:
   - `TurnSidebar::view` paints only `area`, and the `SIDEBAR_GAP` import goes.
   - `browser_areas` exposes the gap's rect, and `layout()` adds it as its own area under a new `Id`, drawn before the transcript, only when the sidebar is drawn.
   - A tiny component paints that area with `Role::Background`, or nothing when the theme is transparent. Add it to `BROWSER_IDS` and `THEMED`.
   - `hit_test` skips that `Id`, so the gap still finds nothing.
   - Add the new `Id` to the layout tests that list exact ids. `every_cell_of_the_browser_is_painted_with_the_background` and `hit_testing_finds_the_sidebar_in_the_pane_nothing_in_the_gap_and_the_transcript_beyond` must stay green unchanged.
2. [must-fix] `src/components/turn_sidebar/tests.rs:737-745` — `the_background_is_painted_over_the_pane_and_the_gap_unless_transparent` only tests an opaque theme, so the "unless transparent" half of its name is untested. Base spec §8.1 says a transparent theme's `background` is not painted. I removed the guard at `mod.rs:207-209` in an exported copy, and all 2589 lib tests still passed. The existing `a_transparent_theme_leaves_the_browser_unpainted` samples no sidebar or gap cell. Change: draw with `builtin::default_theme().with_transparent_background(true)` and assert `bg == Color::Reset` at a pane cell. Assert the same for the gap column wherever its painting ends up after finding 1, for example by adding `(10, 10)` and `(30, 10)` to the points in `a_transparent_theme_leaves_the_browser_unpainted`.
3. [nit] `src/components/turn_sidebar/snapshots.rs:141-161`, `fixtures.rs:121-134` — the 90x24 "widest durations" snapshots are misleading, though the criterion is met: `widest_duration_selected_90x24_{default,terminal}` do show `100h 30m`, which is 8 cells and the widest width.
   - `widest_durations_90x24_*` stop at `59m 59s`, because the fixture lists the widest values last.
   - `the_widest_duration_selected_in_the_focused_pane` selects `26h 30m` (7 cells), so no snapshot shows an 8-cell duration inside the band.

   Change: list the widest values first in `widest_durations()`, select `Some(1)`, update the expected order at `tests.rs:561` and review the regenerated snapshots.
4. [nit] `src/components/turn_sidebar/mod.rs:181-191` — note for task 013. `sync()` runs on every `apply_update` and calls `ListCursor::set`, which resets the offset to 0 without a selection and re-reveals the selected row with one. A wheel scroll of the pane would therefore be undone by any live update. Change in that task: only re-reveal when the selection or the row count changes, as the wheel requires.
5. [nit] `src/app/selector.rs:1137-1145` — note for task 012. Focus only moves when the sidebar holds it at that moment and the width hides it. A sidebar waiting in the focus stack under an overlay, and a sidebar hidden by `EmptySession`, are not handled. Neither can happen until the sidebar can take focus. Change in that task: handle both and test them.
6. [nit] `src/bench_support/mod.rs:324` — `SidebarScenario::draw_once` is never called. Change: remove it.
7. [nit] `src/components/mod.rs:72-75` — `Look` is re-exported but not used outside `turn_sidebar`. `list_rows` also sits next to `visible_rows` with different chrome, which invites mixing them up. Change: drop `Look` from the re-exports and name the function something like `sidebar_list_rows`.
8. [nit] `src/components/turn_sidebar/mod.rs:159-168` — public `select` only forwards to private `select_entry`. Change: keep one of them.
9. [nit] `src/components/turn_sidebar/tests.rs:936-950` — `thinking_alone_is_not_a_section` asserts that a readable-thinking turn has an entry, but `with_stats` adds a footer section, so it passes whatever the thinking does, and its name contradicts its assertion. Change: drop the stats, assert redacted thinking gives `None` and readable thinking gives `Some`, and rename it.
10. [nit] `src/components/turn_sidebar/tests.rs:767-776` — `the_selection_stays_on_its_turn_when_turns_are_inserted_before_it` has a stale comment naming `a0` (the turn is `x0`) and two steps that do nothing for the test: the `Updated` of `u1` and `rows.apply(&ModelUpdate::Loaded)`. Change: remove them and fix the comment.
11. [nit] `src/components/turn_sidebar/tests.rs:879` — `contains('>') || contains("Compaction")` is a weak check, because the last entry of `long_session(40)` is a Compaction and it is selected. Change: assert the exact row, `│  >══ Compaction ═════════  │`.
12. [nit] `src/app/browser_tests/sidebar.rs:338` — `the_rows_of_the_transcript_and_the_sidebar_agree_after_a_live_session` sends no live updates after the load. Change: send `Appended` and `Updated` first, or rename it.
13. [nit] `src/app/browser_tests/overlay_mouse.rs` — the doc comment on `summary_point` has broken line wrapping and does not say the helper now pushes an `Updated` turn. Change: rewrap it and mention the update.
