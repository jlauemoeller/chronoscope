VERDICT: APPROVE

## Assessment
I re-ran the gates on this branch with `INSTA_UPDATE=no`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` all pass, and the lib tests give 2762 passed, 0 failed. `git diff main...HEAD --name-only` lists no `.snap` file. The reviews commit `6b8390d` touches only review files, so these results apply to the reviewed code.

### Code review: PASS (8 nits, all ignored)
1. The `("bottom border", 10, 29)` dead spot presses off the screen. Ignored. `App::mouse` clears `last_press` the same way for any press that is not on a row, and the other in-pane dead spots exercise that at app level. `a_press_elsewhere_in_the_pane_answers_nothing` covers the bottom border at component level, so the criterion holds.
2. "The margin" (judgment call). Ignored, see "The margin" below. The docs at `src/components/turn_sidebar/mod.rs:42` and `:210` are wrong on either reading, because `turn_at` never sees the gap column and does accept the margin cells. This is a doc follow-up and changes no behaviour. `list_pane.rs:264` has the same inaccuracy already.
3. `turn_at` is `pub` with one caller. Ignored: visibility tidy-up only.
4. The doc of `Msg::TurnSelected` does not mention the wheel. Ignored: wording only.
5. The module doc of `mouse.rs` does not mention the sidebar, and a line is short in the `takes_mouse` doc. Ignored: wording only.
6. `a_click_on_the_transcript_after_the_wheel_moved_the_sidebar_on_syncs_it` uses a click, not the wheel. Ignored: the test is misnamed, but what it does exercise is valid. See feature finding 3 for the path it does not reach.
7. `assert!(area.width > 0)` proves nothing. Ignored: harmless, and the test's real assertions stand without it.
8. A misleading comment at `src/components/turn_sidebar/tests.rs:1467`. Ignored: comment only. `a_row_is_found_by_the_scroll_offset` and the app-level scrolled-list tests cover the behaviour.

### Feature review: PASS (5 nits, all ignored)
1. "The margin". Ignored, see "The margin" below.
2. The same off-screen bottom-border spot as code finding 1. Ignored for the same reason.
3. After the wheel moves onto an entry with nothing selectable, a transcript click that leaves its selection unchanged does not re-sync the sidebar. Ignored. Reading `sync_sidebar_selection` confirms the behaviour. The planner explicitly decided that the wheel leaves the sidebar on such an entry, and no criterion covers this path. It needs a turn that renders no rows, plus a click that does not move the selection, and the next change of selection heals it. Nothing is lost: the transcript is unaffected and the sidebar row is only briefly out of step.
4. `a_double_click_does_what_enter_does` lands on a turn with one section. Ignored. The test compares the whole `Presentation` with `j` x3 and `Enter`, and both go through `go_to_turn`, so a multi-section turn would not exercise a different path.
5. `with_an_overlay_open_the_sidebar_gets_no_mouse` does not open `Id::DetailsOverlay`, and it has the pointless `area` assertion. Ignored. `hit_test` treats every entry of `OVERLAYS` the same way, so the details overlay gets the same protection as the three that were tested.

### UI review: PASS
No findings. The task is `UI: none`, and the diff changes no drawing code or snapshots.

### The margin
Both reviewers passed this but flagged it for me. The task's planner note and acceptance criterion say a press on "the margin" does nothing. `turn_at` excludes only the border columns, so columns 1-2 and 27-28 of an entry's row select it, and `a_click_selects_the_row_wherever_in_the_row_it_lands` asserts this. Read literally, with "margin" meaning the two cells §6.2 calls the right margin, the criterion is not met.

I judge the build correct, and the difference minor, for these reasons:
- SPEC §6.2 names what cannot be clicked: connector and blank rows. It does not name margin cells. Base spec §6.9 says a click selects a list row, and §6.3 says selection behaves like the selector's lists.
- The same planner note tells the builder to write the sidebar's own version of `list_pane::row_at`, changing only the rows above the list and using its scroll offset. The note's list of dead spots follows the wording of `row_at`'s doc (`list_pane.rs:264`), which also says "the margin". Yet `row_at` excludes only the border columns, and `row_at_maps_a_cell_to_the_row_drawn_there` hits column 18 of a 20-wide pane, which is a margin cell by the geometry in `ROW_CHROME_COLUMNS`. So the word carries over an inaccurate doc and does not describe a separate requirement.
- The literal reading would make the sidebar differ from the selector, which the task's goal names as the model. A press in those cells would also do nothing and end a double-click under way, just past the visible end of the row's band.
- The effect is limited to four blank cells of a row. Neither behaviour changes what the feature does, so this is not an A/B choice for the human.

The other dead spots in that criterion (`Start`, `End`, connector, blank row, border, and the gap after the pane) are tested and hold.

## Conflicts
None. All three reviews pass, and the two reviewers who raised the margin reached the same view.

## Rework budget
Earlier REWORK verdicts: 0 of 2.
