VERDICT: PASS

## Summary
The `TurnSidebar` pane still matches the task's `UI` section and the wireframe cell for cell. Round 3 changed no production rendering: `git diff 8fbdca1..HEAD -- src tests` shows only tests, one rewrapped doc comment and two new `BackgroundFill` snapshots, and no existing snapshot changed. Two nits remain, both scrolling questions for the later key and mouse tasks; neither can be seen in this task.

Gate, run on an export of `HEAD` in my scratch directory: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test --no-fail-fast` all pass (3016 passed, 0 failed, 2 ignored, no `.snap.new`). I did not run the release budgets; `build-notes.md` reports them as passing, and no production code changed in this round.

Snapshot scope: across the task, 43 existing snapshots changed, and a script found every one at least 90 columns wide. The only snapshot below 90 columns in the diff, `loaded_browser_89x30_text`, is new.

## States checked
Component snapshots, each in `default` and `terminal` at 120x40 (pane 30x33) and 90x24 (pane 30x17). All 48 exist and pass. A script checked all 48 for the pane size, the title text and style, the two blank rows, the border role, one `>` only where a selection is expected, the band or bold on columns 4-26 only, the marker style, blank columns 27-28, and the `═` rule. It found no problems. I also read a sample directly.
- Focused selection on User, Assistant, Compaction — snapshots `focused_{user,assistant,compaction}_*` — ok. `>` in `primary`, band `foreground` + `REVERSED`, not bold, border `ring`.
- Unfocused selection on User, Assistant, Compaction — snapshots `unfocused_{user,assistant,compaction}_*` — ok. `>` not bold, the rest of the row bold in the same `secondary_foreground`, not inverted, border `border`.
- No selection — snapshots `no_selection_*` — ok.
- Long list at the top, at the end, selection in the middle — snapshots `long_top_*`, `long_end_*`, `long_middle_*` — ok.
- Widest durations, no selection and focused selection — snapshots `widest_durations_*`, `widest_duration_selected_*` — ok. `9999h 59m` ends on column 26 and is inside the band.

`BackgroundFill` (new in this round's snapshots).
- Default theme — snapshot `the_fill_in_the_default_theme` — ok. Background on `1..2` of rows 1 and 2 only; rows 0 and 3 are all `Reset`.
- Transparent theme — snapshot `the_fill_in_a_transparent_theme` — ok. Every row is `0..5 Reset/Reset/NONE`.

Browser screens.
- Browser with the sidebar @ 120x40 in each built-in theme, and @ 90x24; without it @ 89x30 — snapshots `loaded_browser_*` — ok.
- `EmptySession` @ 120x40 — snapshot `empty_session_120x40_text` — ok.
- Subagent and overlays over the pane @ 120x40, 200x50 and 200x60 — snapshots — ok, unchanged in this round.

Rendered in the scratch crate (`TestBackend`, default theme unless noted).
- Wireframe turns @ 30x20 — ok. Identical to the `UI` section's drawing.
- Empty list @ 30x13, also focused with a selection pushed — ok. `Start`, `│`, `End`, and nothing selected.
- Focused and unfocused selection on each kind @ 30x20, with styles — ok.
- A selection past the last entry @ 30x20 — ok. Nothing is selected.
- Compaction as first and last entry, and two adjacent Compactions @ 30x18 — ok. One blank row between neighbours.
- Durations from `0s` to `99999h 59m`, focused selection @ 30x10 — ok. They end on column 26 inside the band. An 11-cell value (over 100 years) is left out and the row still draws correctly.
- 200 turns, selection moved forward, back, to the last and to the first entry @ 30x17 — ok. Least scrolling each way; see finding 1.
- The same cursor drawn in a shorter (30x10) and a taller (30x33) pane — ok. The selection stays visible.
- A running Assistant, selected, gaining its duration, then an `Appended` with its id, then `Reset` @ 30x12 — ok. The duration appears in place, no row is added, and `Reset` leaves `Start` and `End`.
- A turn without an entry gaining its first section, with a later turn selected @ 30x14 — ok. The new row takes its transcript position and the selection stays on the same turn.
- `transparent_background` theme, focused selection on a Compaction — ok. No cell has a background.
- `terminal` theme set through the attribute — ok.
- Pane at a non-zero origin in a larger frame — ok. No cell outside the area is touched.
- Degenerate panes (30x0 to 30x5, 12x8, 11x8, 3x3, 1x1, 0x0) — ok. No panic.

Rendered through the App, with a temporary test in a scratch copy of the repo.
- Browser @ 120x40, 91x25, 90x24, 90x20 and 200x60 — ok. The pane is 30 columns and as tall as the transcript, the gap is 1 column, header and help line are full width, and the viewport matches the transcript's area (59 columns at 90).
- Browser @ 89x30, 89x20 and 60x20 — ok. No pane and no gap; the transcript has the full width.
- Gap column 30, rows 5-37 @ 120x40, `default` — ok. Blank, painted `background`.
- `transparent_background` theme @ 120x40 — ok. No cell of the pane or gap has a background.
- `EmptySession` @ 120x24 and 90x24 — ok. No pane, message centred on the full width.
- Turns arriving after `EmptySession` @ 120x24 — ok. The pane appears.
- A turn appended while hidden at 89 columns, then widened to 120 — ok. The rows include it.
- Resize 120 -> 89 -> 120 — ok. The buffer at 120 is identical to the one before.
- Subagent and back @ 90x24 — ok. The pane shows the subagent's turns, then the parent's.
- Help overlay, theme selector, search prompt and selector overlay @ 90x24 — ok. Overlays draw over the pane; the prompt takes the full-width help line.

## Findings
1. [nit] Long list, focused selection on the first and last entries, 90x24 (open since round 1; the approver left it out of this round) — selecting the last entry leaves `End` hidden, and returning to the first entry after scrolling leaves `Start` hidden (offset 2). This matches "only as far as needed" in the spec and the `UI` section, and nothing moves the selection in the running app yet, so no change is needed in this task. Change: the task that adds the keys should decide whether the first and last entries also reveal `Start` and `End`, because keys alone cannot show those rows on a long list. See `factory/state/tasks/010-turn-sidebar-pane/round-1/ui/long-list-selection-ends-90x24.txt`.
2. [nit] `TurnSidebar::sync`, no selection (open since round 1; the approver left it out of this round) — `ListCursor::set` resets the offset to 0 on every update when nothing is selected. It cannot be seen in this task because nothing scrolls the pane in the running app. Change, in the task that lets the wheel scroll the pane: in `sync`, keep the offset when `apply_update` changes the rows, and reset it only when `set_rows` replaces them.
