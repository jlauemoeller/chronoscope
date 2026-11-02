VERDICT: FAIL

## Summary
Both round-1 must-fixes are fixed: `TurnSidebar::view` paints only its own area, the gap column is its own `BackgroundFill` component under `Id::SidebarGap`, and the transparent-background guards are now tested. One thing must change: the new `BackgroundFill` lacks the insta snapshot and the `on()` check that base spec §9.6 requires of every component. The gate and the release budgets pass, the round-1 nits meant for this task are done, and those left to later tasks remain noted for them.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

Also run: `cargo test --release -- --ignored budgets --nocapture` passes. Mixed session: loaded in 284 ms, worst sidebar frame 3.11 ms. Dense session: loaded in 447 ms, worst sidebar frame 0.10 ms. The budget is 16 ms a frame.

The gate and the budgets changed no tracked file, and no `.snap.new` or `.pending-snap` file is committed.

Snapshot scope:
- Round 2 changes 8 snapshots, all `widest_duration*`, and no screen snapshot.
- Against `main`, the 43 modified snapshots are all browser screens at 90 columns or wider. The other 51 are new.
- At 90x24 the selected `9999h 59m` band runs over columns 4-26 in `REVERSED`, with `>` outside it.

Mutation checks on an exported copy (each change made alone; every one fails tests):
- The sidebar's transparent guard removed: `a_transparent_theme_leaves_the_pane_unpainted` and `a_transparent_theme_leaves_the_browser_unpainted` fail. This is the gap round-1 finding 2 reported.
- The fill's transparent guard removed: `background_fill::tests::a_transparent_theme_paints_nothing` and `a_transparent_theme_leaves_the_browser_unpainted` fail.
- The gap left out of `layout()`: `every_cell_of_the_browser_is_painted_with_the_background` and the styled screen snapshots fail.
- `hit_test` no longer skipping the gap: both hit-testing tests in `src/app/mouse_tests.rs` fail.
- `Id::SidebarGap` removed from `THEMED`: `the_previewed_screen_is_the_screen_of_an_app_started_with_that_theme` and `the_themes_after_three_j_120x40_gruvbox_styled` fail.
- `Id::SidebarGap` removed from `BROWSER_IDS`: 8 tests that open another session fail.

Round-1 findings:
- Fixed: 1, 2 (must-fix); 3, 6-13 (nits).
- Left for later tasks, as they asked: 4 and 5. See finding 4 below.

## Findings
1. [must-fix] `src/components/background_fill.rs:70-113` — `BackgroundFill` is a new `pub` component with its own `Id`, and its tests have no insta snapshot and never call `on()`. Base spec §9.6, which the base-rules table in `SPEC.md` says binds every task and every review, requires both: "every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked". Every other component in `src/components/` that draws and implements `AppComponent` has both, including ones whose `on()` always returns `None`, such as `WorkingDir` and `EmptyState`. The gap column does appear in the browser's styled screen snapshots, but that does not test the component on its own. Change, in `mod tests`:
   - Add `insta::assert_snapshot!(buffer_to_styled(&buffer))` for the fill in `builtin::default_theme()`, and another for `builtin::default_theme().with_transparent_background(true)`. Use the existing `draw`, which fills a sub-area of a larger frame, so each snapshot shows painted and unpainted cells.
   - Add a test that `on()` returns `None` for `Event::Tick`, a key event and a mouse event, and that `perform(Cmd::None)` returns `CmdResult::Invalid(Cmd::None)` and `state()` returns `State::None`. `it_ignores_events_and_a_tiny_area_does_not_panic` in `src/components/working_dir.rs` is the pattern to follow.
   - Review and commit the new `.snap` files, with no `.snap.new` file.
2. [nit] `src/app/mouse.rs:79-82` — the round-2 sentence left the doc comment of `hit_test` with a short line (`/// Components under the topmost overlay are not`). Change: rewrap the paragraph.
3. [nit] `src/components/turn_sidebar/rows.rs:238` — `TurnRows::entry_at_row` is `pub`, but only `src/components/turn_sidebar/tests.rs` calls it. The compiler cannot report it because the type is exported. The same is true of `Row::selectable` (`rows.rs:96`) and `TurnSidebar::offset` (`mod.rs:149`), though the row tests for "`Start` and `End` not selectable" do need `selectable`. Change: none needed if the mouse task uses `entry_at_row` as expected. If it does not, remove it in that task.
4. [nit] Still open for later tasks, as the build notes record, with no change in this task:
   - `src/components/turn_sidebar/mod.rs:178-188`: `sync` runs on every `apply_update`, and `ListCursor::set` resets the offset to 0 when nothing is selected. Task 013 must keep a wheel scroll across live updates.
   - `src/app/selector.rs:1136-1143`: focus moves only when the sidebar holds it at that moment and the width hides it. Task 012 must also handle a sidebar waiting in the focus stack under an overlay, and one hidden by `EmptySession`.
   - The UI review's question of whether the first and last entries reveal `Start` and `End` is for the task that adds the keys.
