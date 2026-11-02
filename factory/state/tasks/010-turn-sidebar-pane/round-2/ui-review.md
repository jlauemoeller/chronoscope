VERDICT: PASS

## Summary
The `TurnSidebar` pane still matches the task's `UI` section and the wireframe cell for cell, and the three round-1 UI nits this round addressed are fixed: the widest durations are on screen at 90x24, the band covers the widest value, and the gap column has its own component. Rendering of the rows is unchanged since round 1 (`line.rs`, `rows.rs` and `list_pane.rs` have no diff); only the 8 `widest_*` snapshots changed in this round. Two nits remain, both round-1 scrolling questions for the later key and mouse tasks, and neither can be seen in this task.

Gate, run on an export of `HEAD` in my scratch directory: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` all pass (3013 passed, 0 failed, 2 ignored, no `.snap.new`). I did not run the release budgets; `build-notes.md` reports them as passing.

Snapshot scope: across the task, 43 existing snapshots changed (120x40, 200x50, 200x60 and the 120x40 theme screens) and 51 are new. No existing snapshot below 90 columns changed; the one below 90 in the diff, `loaded_browser_89x30_text`, is new. In round 2, `git diff 426ea41..HEAD` touches only `widest_durations_*` and `widest_duration_selected_*`: I read four of the eight directly, and compared all eight by script (the text is equal across themes at each size, the widest value is the first Assistant row, and only the four selected snapshots have a band, on row 7).

## States checked
Component snapshots, each in `default` and `terminal` at 120x40 (pane 30x33) and 90x24 (pane 30x17). All 48 exist and pass; the 12 states are unchanged from round 1 apart from the two widest ones.
- Focused selection on User, Assistant, Compaction — snapshots `focused_{user,assistant,compaction}_*` — ok. Sampled `focused_compaction_90x24_terminal`: `>` in `primary` on column 3, band `foreground` + `REVERSED`, not bold, on columns 4-26, columns 27-28 unstyled, border `ring`.
- Unfocused selection on User, Assistant, Compaction — snapshots `unfocused_{user,assistant,compaction}_*` — ok.
- No selection — snapshots `no_selection_*` — ok.
- Long list at the top, at the end, selection in the middle — snapshots `long_top_*`, `long_end_*`, `long_middle_*` — ok.
- Widest durations, no selection — snapshots `widest_durations_*` — ok. `9999h 59m`, `100h 30m` and `26h 30m` are visible at 90x24, and all six values down to `0s` at 120x40, each ending on column 26.
- Widest duration, focused selection — snapshots `widest_duration_selected_*` — ok. The selected row reads `>   Assistant  9999h 59m`; the band runs over columns 4-26, so the widest duration takes the band's style.

Browser screens.
- Browser with the sidebar @ 120x40, each built-in theme, and @ 90x24; without it @ 89x30 — snapshots `loaded_browser_*` — ok.
- `EmptySession` @ 120x40 — snapshot `empty_session_120x40_text` — ok.
- Subagent, overlays over the pane @ 120x40 and 200x50 — snapshots — ok, unchanged in this round.

Rendered in the scratch crate (`TestBackend`, default theme).
- Wireframe turns @ 30x20 — ok. Identical to the `UI` section's drawing.
- Empty list @ 30x17 and 30x13 — ok. `Start`, `│`, `End`.
- Durations of 3 to 11 cells, focused selection — ok. They end on column 26 and the band covers columns 4-26 up to `99999h 59m` (10 cells). `999999h 59m` (11 cells, over 100 years) is left out, and the row still draws correctly.
- Pane rendered into a 32-column frame — ok. Column 29 is painted and columns 30-31 stay `Reset`, so the pane now paints only its own area.
- `transparent_background` theme, focused selection on a Compaction — ok. No cell in the frame has a background other than `Reset`.
- 200 turns, selection moved to entries 0, 50, 49, the last and 0 again @ 30x17 — ok. Least scrolling each way; see finding 1.
- Compaction as first and last entry, two adjacent Compactions, unfocused @ 30x24 — ok. One blank row between neighbours; `>` in `secondary_foreground` not bold, the rest bold, not inverted.
- A running Assistant, selected, gaining its duration, then `Reset` @ 30x12 — ok. The duration appears in place; `Reset` leaves `Start` and `End`.
- Degenerate panes (30x1 to 30x5, 12x8, 11x8, 3x3, 1x1) and a pane at a non-zero origin — ok. No panic and no text outside the area.

Rendered through the App, with a temporary test in a scratch copy of the repo.
- Browser @ 120x40, 91x25, 90x24, 90x20, 89x30, 89x20 and 200x60 — ok. The pane is as tall as the transcript from 90 columns on and absent below; header and help line are full width.
- Gap column 30, rows 5-37 @ 120x40, `default` — ok. Blank, painted `background`.
- Theme preview through `terminal`, `catppuccin` and `gruvbox`, then cancelled, @ 120x40 — ok. Gap and pane take each theme's `background` and return to `default`.
- Theme preview with a subagent open, then back to the parent, @ 120x40 — ok. Gap and pane follow the theme.
- `transparent_background` theme @ 120x40 — ok. All 1023 cells of pane and gap keep `Reset`.
- `EmptySession` @ 120x40, painted and transparent — ok. No pane, the message is centred on the full width, and column 30 is painted by `EmptySession` (`Reset` when transparent).
- Resize 120 -> 89 -> 120 — ok. The buffer at 120 is identical to the one before.
- Subagent and back @ 90x24 — ok. The pane shows `Start`, `User`, `Assistant`, `End`, then the parent's turns.
- Theme selector, help overlay and search prompt @ 90x24 — ok. Overlays draw over the pane; the prompt takes the full-width help line.

## Findings
1. [nit] Long list, focused selection on the first and last entries, 90x24 (open from round 1, deferred by the Builder) — selecting the last entry leaves `End` hidden, and returning to the first entry after scrolling leaves `Start` hidden (offset 2). This matches "only as far as needed" in the spec and the `UI` section, and nothing moves the selection in the running app yet, so no change is needed in this task. Change: the task that adds the keys should decide whether the first and last entries also reveal `Start` and `End`, because keys alone cannot show those rows on a long list. See `factory/state/tasks/010-turn-sidebar-pane/round-1/ui/long-list-selection-ends-90x24.txt`.
2. [nit] `TurnSidebar::sync`, no selection (open from round 1, deferred by the Builder) — `ListCursor::set` resets the offset to 0 on every update when nothing is selected. It cannot be seen in this task because nothing scrolls the pane in the running app. Change, in the task that lets the wheel scroll the pane: in `sync`, keep the offset when `apply_update` changes the rows, and reset it only when `set_rows` replaces them.
