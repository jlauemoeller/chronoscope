VERDICT: PASS

## Summary
The `TurnSidebar` pane matches the task's `UI` section and the wireframe cell for cell: title, border roles, columns 3 / 7 / 18 / 26, connector and blank rows, both selection states, scrolling, hiding below 90 columns and with `EmptySession`, and a full-width header and help line. All 48 component snapshots and the browser screen snapshots exist and pass with `INSTA_UPDATE=no`, and my own renders of states the snapshots skip showed no defect. The five nits are about snapshot coverage, one column painted outside the pane, and scrolling questions for the next tasks.

Gate, run on an export of `HEAD` in my scratch directory: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` all pass. I did not run the release budgets; `build-notes.md` reports them as passing.

Snapshot scope: 43 existing snapshots changed, all at 120 or 200 columns, and a script comparing `main` and `HEAD` found rows 0-4 and the last two rows unchanged in every one. The other 51 are new files. No 80x24 or 60x20 snapshot changed.

## States checked
Component snapshots, each in `default` and `terminal` at 120x40 (pane 30x33) and 90x24 (pane 30x17). I read the text and styles of a sample and rely on the passing test for the rest.
- Focused selection on User, Assistant, Compaction — snapshots `focused_{user,assistant,compaction}_*` — ok. `>` in `primary` on column 3, band `foreground` + `REVERSED`, not bold, on columns 4-26, columns 27-28 unstyled, border `ring`.
- Unfocused selection on User, Assistant, Compaction — snapshots `unfocused_{user,assistant,compaction}_*` — ok. `>` in `secondary_foreground`, not bold; rest of the row bold `secondary_foreground`, not inverted; border `border`.
- No selection — snapshots `no_selection_*` — ok. Title in `foreground`; rows, `●`, `│`, `═` in `muted_foreground`.
- Long list at the top and at the end, and selection in the middle — snapshots `long_top_*`, `long_end_*`, `long_middle_*` — ok. `Start` is on the first list row at the top and `End` on the last at the end.
- Widest durations — snapshots `widest_durations_*`, `widest_duration_selected_*` — ok at 120x40, issue at 90x24 (findings 1 and 2).

Browser screens.
- Browser with the sidebar @ 120x40 — snapshots `loaded_browser_120x40_{text,styled}`, `loaded_browser_theme_*` — ok. Gap column 30 is blank with the theme background.
- Browser @ 90x24 — snapshot `loaded_browser_90x24_text` — ok. The transcript is 59 columns.
- Browser without the sidebar @ 89x30 — snapshot `loaded_browser_89x30_text` — ok.
- `EmptySession` @ 120x40 — snapshot `empty_session_120x40_text` — ok. No pane, message centred on the full width.
- Subagent @ 120x40 — snapshot `subagent__the_subagent_120x40_*` — ok. The pane lists the subagent's turns.
- Overlays over the pane @ 120x40 and 200x50 (help, details, selector, toast) — snapshots — ok. Overlays draw on top and the pane is intact around them.

Rendered in the scratch crate (`TestBackend`, default theme unless noted).
- Empty list, unfocused @ 30x33, 30x17, 30x13 and focused @ 30x13 — ok. `Start`, `│`, `End`.
- Wireframe turns @ 30x20 — ok. Identical to the `UI` section's drawing.
- Wireframe turns @ 30x13 (the 90x20 minimum) and 30x53 (200x60) — ok.
- 200 turns, selection moved forward, back, to the last and to the first entry @ 30x17 — ok. Least scrolling each way, no scroll on a move inside the view; see finding 4.
- Compaction as first and last entry, focused and unfocused — ok. Blank rows on both sides, `>══ Compaction ═…` with the band or bold.
- Two adjacent Compactions — ok. One blank row between them.
- `transparent_background` theme, focused selection — ok. Every cell keeps `Reset` as background; the band is still `REVERSED`.
- Durations of 3 to 11 cells, plain and selected — ok. They end on column 26 up to `99999h 59m` (10 cells). Eleven cells and more are left out, which takes over 11 years.
- Reset, and a turn gaining its entry and duration while another row is selected — ok. The rows and the selection stay correct.
- Degenerate panes (30x1 to 30x5, 12x8, 11x8, 3x3, 1x1) and a pane at a non-zero origin — ok. No panic, and no text outside the area.

Rendered through the App, with a temporary test in the scratch copy.
- Browser @ 90x20 — ok. The pane is as tall as the transcript.
- Session opened, nothing delivered yet @ 120x40 — ok. The pane shows `Start` and `End` beside a blank transcript until the load ends.
- Resize 120 -> 89 -> 120 — ok. The pane goes and returns, the viewport follows, and the screen at 120 is identical to the one before.
- 40 live appended turns @ 90x24 — ok. The pane stays on its first rows with no selection.

## Findings
1. [nit] `widest_durations` snapshots, no selection, 90x24, both themes — the pane shows 13 list rows, so the widest duration in these snapshots is `59m 59s`; `100h 30m` and `9999h 0m` are below the fold. `100h 30m` does appear at 90x24 in `widest_duration_selected`, and the pane is 30 columns at every size, so the width is covered, but the snapshot does not show what its name says. Change: in `the_widest_durations` (`src/components/turn_sidebar/snapshots.rs`), pass `scroll_to_end = true`, or put the widest values first in `widest_durations()`, so that they are on screen at 90x24. See `ui/widest-durations-90x24.txt`.
2. [nit] `widest_duration_selected` snapshots, focused, both sizes and themes — `with_selected(Some(9))` selects `26h 30m` (7 cells), not the widest entry, so no snapshot shows a band over the widest duration. Also, the `widest_durations()` doc says "the widest durations the format gives", but its widest are 8 cells, and `9999h 59m` (9 cells) and `99999h 59m` (10 cells) still fit. Change: select entry 11, make its duration `9_999 * 3600 + 59 * 60` so that it reads `9999h 59m`, and correct the doc comment. See `ui/widest-durations-90x24.txt`.
3. [nit] `TurnSidebar::paint_background`, every size from 90 columns — the component paints the gap column, which is outside the 30-column area it is given, and base spec §1.3 says a component draws only itself. It draws correctly, is documented in the build notes and is covered by `every_cell_of_the_browser_is_painted_with_the_background`. The gap also has to stay out of `hit_test`, so this is not a mis-drawing. Change, if wanted: give the gap to a component that owns it, for example a small background filler in `layout()` that `hit_test` skips. Otherwise, name the exception in the doc comment on `view`.
4. [nit] Long list, focused selection on the first and last entries, 90x24 — following "only as far as needed" exactly, selecting the last entry leaves `End` hidden, and returning to the first entry after scrolling leaves `Start` hidden (offset 2). This matches the spec and the `UI` section, so no change is needed in this task. The task that adds the keys should decide whether the first and last entries reveal `Start` and `End`, because keys alone cannot show those rows on a long list. See `ui/long-list-selection-ends-90x24.txt`.
5. [nit] `TurnSidebar::sync`, no selection — `ListCursor::set` resets the offset to 0 on every update when nothing is selected. That cannot be seen in this task because nothing scrolls the pane in the running app. Once the wheel can scroll the pane, a live update would jump an unselected list back to the top. Change, in that task: in `sync`, keep the offset when the rows are updated by `apply_update`, and reset it only when `set_rows` replaces them.
