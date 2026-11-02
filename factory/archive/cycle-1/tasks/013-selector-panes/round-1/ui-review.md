VERDICT: PASS

## Summary
The selector panes match the `UI` section and the planner-note geometry at every size checked. Tabs, underline, panes, the Working Directory box and the help line sit at the prescribed offsets, and the §8.2 row styles are right: the band covers columns 4 … W−4 with the marker outside it, the unfocused selection is bold `secondary_foreground` with no band, and the ring/border swap with focus. Every required snapshot exists and matches its description, and the test gate passes with `INSTA_UPDATE=no` (697 tests; fmt and clippy clean). My scratch-crate renders of states the snapshots don't cover found no defects.

## States checked
- 1 Startup @ 120x40 — snapshots `startup_120x40_text` / `startup_120x40_styled` — ok
  - Content width 112, Projects 36, gap 1, Sessions 75, panes 24 rows.
  - Tabs at overlay row 2 col 5; `━` under exactly the 10-cell label.
  - Projects band at cols 8..36 with `>` in `primary`; no `>` in Sessions.
- 1 Startup @ 80x24 — snapshot `startup_80x24_text` — ok (Projects 24, Sessions 47, 5 visible rows, stats correctly omitted)
- 1 Startup @ 200x50 — snapshot `startup_200x50_text` — ok (160 wide, centred)
- 2 Sessions focused @ 120x40 — snapshots `sessions_focused_120x40_text` / `_styled` — ok
  - The ring has moved to Sessions; Sessions band at cols 45..112, stats inside the band.
  - The Projects row is bold `secondary_foreground` with `>` not bold and no band.
- 2 Sessions focused @ 80x24 — snapshot `sessions_focused_80x24_text` — ok
- 3 Long titles @ 80x24 — snapshot `long_titles_80x24_text` — ok
  - `…` truncation; stats omitted below 10 cells; wide glyphs not split.
  - `long_titles_with_stats_120x40_text` shows the stats being kept where there is room.
- 4 Active `●` in an unselected row and inside the band @ 120x40 styled — snapshots `the_active_marker_in_an_unselected_row_120x40_styled` / `the_active_marker_inside_the_band_120x40_styled` — ok (`●` is `primary` outside the band and takes the band style inside it)
- 5 Scrolled list, 40 sessions @ 80x24 — snapshot `a_scrolled_list_80x24_text` — ok
- 6 Long working directory @ 80x24 — snapshot `a_working_directory_longer_than_the_box_is_cut_from_the_left_80x24_text` — ok (cut from the left with a leading `…`)
- 7 Per-theme state 2 @ 120x40 for default, terminal, catppuccin, gruvbox, nord, tokyo-night and solarized — snapshots `sessions_focused_theme_*` — ok (all seven distinct; in the terminal theme the band is `Reset` + REVERSED)
- Component snapshots — `harness_tabs::one_selected_and_one_muted_tab_snapshot` (width 60), `project_list::{focused,unfocused}_list_snapshot`, `session_list::{rows_with_stats,rows_without_stats}_snapshot` (60x10), `working_dir::box_with_a_directory_snapshot` — ok
- Re-accepted older screen snapshots (`selector_help_line_*`, `selector_with_toast_*`, `startup_frame_*`, transparent theme) — git diff against main — ok
  - Only the overlay interior changed (tabs, empty panes, WD box).
  - The help-line row and the toast are unchanged, and the toast still draws on top.
  - The transparent theme paints only `Reset` backgrounds.
- Minimum size 60x20, Sessions focused, stepped to s4 with only one visible row — rendered in scratch crate (keys sent through a Port into `App::tick`) — ok
  - Projects 20, Sessions 31; the list scrolls and the band stays within W−4.
  - Moving to another project clears the session selection.
  - See `ui/minimum-60x20-sessions-scrolled-styled.txt`.
- Mirror focus (Sessions, then back to Projects) @ 120x40, distinct-colour theme — rendered in scratch crate — ok
  - The Sessions row is bold `secondary_foreground`, its `>` is `secondary_foreground` and not bold, and there is no band.
  - Stats stay `muted_foreground`, as the planner note requires outside a band; the ring is back on Projects.
  - See `ui/sessions-unfocused-selection-120x40-distinct-styled.txt`.
- State 2 and startup with the distinct-colour theme @ 120x40 — rendered in scratch crate — ok (every role lands on its own colour: border/ring, title `foreground`, band `foreground`+REVERSED, marker `primary`, unselected rows `muted_foreground`)
- Before the first snapshot, an unavailable harness, zero projects, and a project with zero sessions (then Sessions focused) @ 80x24 — rendered in scratch crate — ok
  - Tabs are muted with no underline, the panes are empty with title `Sessions`, nothing panics, and no `>` appears.
  - The "No sessions for this harness" empty state belongs to task 014.
- Long project name in the Projects row and the `Sessions in …` title, plus a cwd with wide characters cut from the left @ 80x24 and 81x24 — rendered in scratch crate — ok (`…` never lands on half a glyph; the right margin is kept)
- Scrolling — rendered in scratch crate — ok
  - Sessions to the last row, with no wrap-around.
  - A resize from 80x24 to 120x40 and back keeps the selection visible with no blank tail.
  - Projects scrolled to Nayra (6th of 5 visible) and back.
  - A live snapshot that shrinks the list under a scrolled selection clamps it and keeps it visible.
- Wide-character session titles inside and outside the band @ 80x24 and 120x40 — rendered in scratch crate — ok (the band stays full width; stats stay right-aligned at W−4)
- 300x60 — rendered in scratch crate — ok (160 columns, centred)
- A `Tick` 5 minutes later @ 120x40 — rendered in scratch crate — ok (the `●` clears)

## Findings
1. [nit] SessionList, selected row while Projects has focus, any size — no snapshot or cell-level test covers the unfocused selection in the **Sessions** pane; `the_unfocused_selection_is_bold_secondary_without_a_band` only checks Projects. The rendering is correct (same `RowStyles` path as Projects). Change: add a styled snapshot or a `distinct_theme` cell check at 120x40 for focus Sessions, select s2, then focus Projects. It should assert:
   - the s2 row's `>` is `secondary_foreground` and not bold;
   - the timestamp and title are bold `secondary_foreground` with no REVERSED;
   - the stats stay `muted_foreground`.

   See `ui/sessions-unfocused-selection-120x40-distinct-styled.txt`.
2. [nit] State 4 snapshots @ 120x40 — two of them duplicate existing snapshots byte for byte:
   - `the_active_marker_in_an_unselected_row_120x40_styled` matches `startup_120x40_styled`;
   - `the_active_marker_inside_the_band_120x40_styled` matches `sessions_focused_120x40_styled` (and `sessions_focused_theme_default`).

   They are valid for the state but add no coverage. Change (optional): render the unselected-row case with Sessions focused and s2 selected, so the `primary` `●` on s1 sits directly above a band.
