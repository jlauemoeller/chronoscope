VERDICT: PASS

## Summary
Round 1's must-fix is closed: going back from a subagent now draws the pane as it was, with the same first list row and the marker on the same pane row, at every size I tried and through nested subagents. The selected, unfocused row matches the `UI` section in the snapshots and in every built-in theme. Only nits remain, mostly test comments and one resize test that does not pin what its name says.

## States checked
- Selection on a User row @ 120x40 `default` — snapshots `loaded_browser_120x40_text`, `_styled` — ok (`>` in column 3, `secondary_foreground`, not bold; columns 4..27 bold `secondary_foreground`; not inverted; `border` border)
- Selection on an Assistant row @ 120x40 `default` — snapshots `the_selection_on_an_assistant_row_120x40_text`, `_styled` — ok (the duration takes the row's bold style)
- Selection on a Compaction row @ 120x40 `default` — snapshots `the_selection_on_a_compaction_row_120x40_text`, `_styled` — ok (`>══ Compaction`, the `>` replaces the first `═` in column 3)
- Long list scrolled to keep the selected row visible @ 120x40 `default` — snapshots `a_long_list_scrolled_to_keep_the_selected_row_visible_120x40_text`, `_styled` (new) — ok (marker on the last list row, same styles as above)
- Subagent open and back, list longer than the pane @ 120x40 — snapshot `going_back_from_a_subagent_shows_the_pane_as_it_was_with_a_list_longer_than_the_pane` (new), and rendered in a scratch copy of the repo through `App::view` — ok (marker on list row 26 before and after, whole screen equal)
- Same round trip, marker away from the last list row where the size allows, @ 90x20, 90x21, 90x24, 120x26, 90x40, 200x60 — scratch copy — ok (whole screen equal at each)
- Nested subagents, each way back @ 120x24 and 120x40 — scratch copy and `nested_subagents_bring_back_each_pane_as_it_was` — ok
- Pane shrinks by one row while covered, 120x40 → 120x39, marker mid-pane — scratch copy — ok (marker and first list row stay; see finding 1 on its test)
- Pane shrinks while covered, 120x40 → 120x24 — `going_back_scrolls_the_least_needed_when_the_pane_shrank_while_it_was_covered` — ok (marker on the last list row)
- Pane grows while covered, 120x24 → 120x60 — scratch copy — ok (the list fills the pane, `End` shown, no blank rows below it)
- Sidebar hidden while covered, 120x40 → 80x24 → 120x40 — scratch copy — ok (absent at 80x24; on widening the pane equals the one before opening)
- Subagent opened while the sidebar is hidden, 80x24, then 120x24 — scratch copy — ok (subagent's row, then the parent's row visible)
- Covered parent gets new turns and its following selection moves, by 2 turns @ 120x40 and by 60 turns @ 120x30 — scratch copy — ok (the new row is selected and visible)
- Covered parent's following selection moves onto a turn without an entry @ 120x40 — scratch copy — ok (the row selected before opening, same marker row; it does not clear or jump)
- Covered parent reset to a shorter list, and to nothing and then 60 turns, @ 120x30 — scratch copy — ok (first entry selected, list from its top)
- Restored list shorter than the saved offset, selection kept, pane 30x17 — `TurnSidebar` in a throwaway crate — ok (offset clamped, the list fills the pane)
- Subagent with no turns, `EmptySession` shown, @ 120x40 — scratch copy — ok (pane not drawn; after going back it is as it was)
- Selected row with long durations (`72h 0m`, `999h 59m`), panes 30x17, 30x8, 30x6, 30x5, 30x4, 30x3, 30x1, 12x8 — throwaway crate — ok (right margin kept; no list rows and no panic in the smallest)
- Empty list through a cover round trip, pane 30x10 — throwaway crate — ok (`Start` and `End` only, no selection)
- Below the minimum size, 120x8 to 120x16, covered and back — scratch copy — ok (`Terminal too small`, and the selection stays visible on regrowing to 120x40)
- Selected row in all seven built-in themes @ 120x40 — scratch copy, cell by cell — ok (`>` and text equal `secondary_foreground`, `>` not bold, text bold through column 26, column 27 plain, nothing reversed, border equals `border`)
- All changed snapshots against the merge base (script over text and style rows) — 44 modified, each differing only by `>` in column 3 and the pane's styles left of column 30; 7 new, all browser screens at 120x40. Round 2 modified none and added two — ok

Gates, run with `INSTA_UPDATE=no` and my own target directory: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (3049 passed, 0 failed) and the release `budgets` test (frames worst 4.78 ms, sidebar frames worst 0.10 ms) all pass.

Mutations in the scratch copy: restoring nothing on going back fails four tests, and restoring the selection but not the offset fails three, including the whole-screen test. So the fix is guarded.

## Findings
1. [nit] `going_back_scrolls_the_least_needed_when_the_pane_shrank_while_it_was_covered` (`src/app/browser_tests/sidebar_follow.rs:674`) — this test does not show the scroll starts from the restored offset. It still passes when the offset is not restored, because revealing from offset 0 also puts the marker on the last list row. The same holds for the shrink step of `a_restored_selection_scrolls_the_least_needed_and_covers_nest`. The behaviour itself is right (see the 120x39 state above). This is a nit because round 1's required whole-screen round trip is effective, failing when the offset is not restored, and the shrink path runs through the same `restore` code. Change: start with the marker mid-pane, as `subagent_rig_after(long_turns(20), 120, 30)` then a draw at 120x40 does, shrink to 120x39 while covered, and assert the marker stays on list row 26. Without the restored offset it moves to row 27.
2. [nit] `src/app/browser_tests/sidebar_follow.rs:651` — the comment says "29 list rows against 90 rows", and `round-2/build-notes.md` says the same. The list has 101 rows (49 entries, `Start`, `End` and the rows between them). Correct both to 101.
3. [nit] `src/components/turn_sidebar/tests.rs`, `a_restored_selection_scrolls_the_least_needed_and_covers_nest` — the comment says "three rows fewer are visible", but the visible rows go from 9 to 5, which is four. Correct the comment.
4. [nit] Snapshot `going_back_from_a_subagent_shows_the_pane_as_it_was_with_a_list_longer_than_the_pane` — it takes its name from the test, with no size or kind, while its neighbours end in `_120x40_text`. Name it `the_pane_as_it_was_after_a_subagent_with_a_long_list_120x40_text` through `insta::assert_snapshot!(name, value)`.
5. [nit] Browser, selection on a system-only turn that then gains an entry through `Updated`, 120x40 — carried over from round 1 and not changed. The sidebar stays on the row above until the selection next moves. This follows the planner's binding "only on a change" note, so it needs a planning decision, not a change here.
6. [nit] Browser, selected turn loses its entry through `Updated`, 120x40 — carried over from round 1 and not changed. `TurnSidebar::sync_from` clears the selection, where the planner's note for a turn without an entry says it "does not clear and does not jump". It is rare, because a turn must lose all its non-system sections. If wanted, move the selection to the nearest entry instead.
