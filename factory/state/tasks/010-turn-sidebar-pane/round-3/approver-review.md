## Rework note

The sidebar works and no production behaviour changes in this round. Make these four test and comment changes, and nothing else.

1. **`BackgroundFill` tests, the required change (base spec §9.6).** In `mod tests` of `src/components/background_fill.rs`:
   - Add a test `the_fill_in_the_default_theme` with `insta::assert_snapshot!(buffer_to_styled(&draw(builtin::default_theme())));`.
   - Add a test `the_fill_in_a_transparent_theme` doing the same with `builtin::default_theme().with_transparent_background(true)`. Import `buffer_to_styled` from `crate::test_support`.
   - Keep the existing `draw` helper, so each snapshot shows painted and unpainted cells. Review both new files. In the default theme, rows 1 and 2 of the styles have the background on `1..2` only, and rows 0 and 3 are all `Reset`. In the transparent theme, every row is `0..5 Reset/Reset/NONE`.
   - Add a test `it_ignores_events_and_commands`: `fill.on(&event)` is `None` for `Event::Tick`, a key event and a raw `Event::Mouse` event; `fill.perform(Cmd::None)` is `CmdResult::Invalid(Cmd::None)`; `fill.state()` is `State::None`. `src/components/working_dir.rs` shows the pattern, and `src/components/harness_tabs.rs` shows how to build the raw mouse event. Keep `Invalid` in the production code, as `TurnSidebar` does: the assertion follows the code, even though `WorkingDir` returns `NoChange`.
   - The existing cell-by-cell tests stay. The feature review's "the snapshot is optional" is dropped, because the code review outranks it and §9.6 binds.

2. **Deselection tests.** In `src/components/turn_sidebar/tests.rs`:
   - In `a_selected_turn_that_loses_its_entry_is_deselected`, after the existing assert, send `update(updated(vec![assistant("a1", 9)]))` through `MODEL_UPDATE_ATTR` and assert `selected_entry()` is still `None`.
   - In `a_selected_turn_that_is_not_in_new_rows_is_deselected`, after the first assert, push `rows_of(&wireframe())` through `TURN_ROWS_ATTR` and assert `selected_entry()` is still `None`.

3. **Test list.** In `src/app/browser_tests.rs`, make the test constant `BROWSER_IDS` `[Id; 5]` by adding `Id::TurnSidebar` and `Id::SidebarGap`. Do not add `Id::EmptySession`.

4. **Doc comment.** In `src/app/mouse.rs`, rewrap the last sentence of the doc comment on `hit_test` so its two lines read:

   ```
   /// Components under the topmost overlay are not candidates, even where the
   /// overlay does not cover them.
   ```

Leave everything else as it is. The review notes for tasks 012 and 013 and the UI question about revealing `Start` and `End` are not for this round.

Done when:
- `git diff 8fbdca1..HEAD -- src tests` shows only the test changes above, the one doc comment, and exactly two new `.snap` files under `src/components/snapshots/`. No existing snapshot changes, and no `.snap.new` or `.pending-snap` file is committed.
- With `INSTA_UPDATE=no`, `cargo test --lib background_fill` passes with six tests.
- Deleting `if row.is_none() { self.selected = None; }` from `TurnSidebar::sync` makes `cargo test --lib turn_sidebar` fail exactly `a_selected_turn_that_loses_its_entry_is_deselected` and `a_selected_turn_that_is_not_in_new_rows_is_deselected`. Restore the lines afterwards.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `cargo test --release -- --ignored budgets --nocapture` pass.
