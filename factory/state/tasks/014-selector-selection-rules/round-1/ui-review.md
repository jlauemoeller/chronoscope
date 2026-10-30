VERDICT: PASS

## Summary
Every snapshot the `UI` section asks for exists and matches its description. That covers the empty state at 80x24 and 120x40, text and styled, plus the no-harness variant; the 9th project preselected and scrolled into view at 80x24, text and styled; and the next project selected with no session marker at 120x40. My scratch-crate renders at other sizes, themes and data shapes also showed nothing that contradicts the `UI` section or §8.2. The test gate passes: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `INSTA_UPDATE=no cargo test` with 0 failures. Three cosmetic or edge-case nits remain.

## States checked
- Empty state @ 80x24: snapshot `empty_state_80x24_text`. ok. Projects shows border and title only. Sessions is titled ` Sessions ` and shows `No sessions for this harness` centred (row 3 of 6 interior rows, 8/9 cells of side padding). The Working Directory box is empty, and the help line matches 013's.
- Empty state @ 120x40: snapshot `empty_state_120x40_text`. ok. Centred vertically (row 11 of 22) and horizontally (22/23 cells).
- Empty state @ 120x40 styled: snapshot `empty_state_120x40_styled`. ok.
  - The message is `muted_foreground` (161,161,161) on the overlay surface, because `without_background` keeps the surface.
  - The Projects border is `ring` (focused); the Sessions border is `border`.
  - The help-line style runs on row 36 are identical to `selector_help_line_120x40_styled` from 013.
- No available harness @ 120x40: snapshots `no_available_harness_120x40_text` and `no_available_harness_120x40_styled`. ok. Both registered tabs are drawn in `muted_foreground` and none is underlined. The panes are the same as in the empty state.
- Startup with the 9th project (Quill) preselected @ 80x24: snapshots `startup_with_the_9th_project_preselected_80x24_text` and `startup_with_the_9th_project_preselected_80x24_styled`. ok.
  - The list is scrolled so `> Quill` is the last visible row.
  - The `>` marker is `primary`, not inverted. The band is `foreground` + REVERSED, not bold, and ends two cells before the pane border.
  - The session row shows no `>` and is `muted_foreground`.
- After moving to the next project (Relay) @ 120x40: snapshot `after_moving_to_the_next_project_no_session_has_a_marker_120x40_text`. ok. `> Relay` is selected, the title is `Sessions in Relay`, no session has a marker, and the Working Directory box is updated.
- Components: `an_empty_list_draws_only_its_border_and_title` and `a_list_of_an_empty_harness_shows_the_empty_state_in_muted_foreground`. ok.
- Empty state @ 60x20 (minimum size), rendered in the scratch crate: ok, but the message is flush against the left border (nit 2). See `ui/empty-state-60x20.txt`.
- Empty state @ 200x50, scratch crate: ok. The overlay is 160 wide and centred, and the message is centred in the Sessions pane.
- Empty state with Sessions focused @ 80x24, styled, scratch crate: ok. The Sessions border becomes `ring`, Projects becomes `border`, and the message is unchanged.
- Empty state in the `terminal` and `catppuccin` themes @ 80x24, styled, scratch crate: ok. The message uses each theme's `muted_foreground` (`Gray`, Rgb(166,173,200)) on the theme's surface.
- No available harness @ 60x20, scratch crate: ok.
- Preselection, scratch crate: ok in each case below. The selection is always visible and no session has a marker.
  - Quill @ 60x20 (one visible row, `> Quill`).
  - Quill @ 80x24.
  - Quill @ 120x40 (all 12 projects visible, no scroll).
  - Tessera, the 12th project, matched through `/p/Tessera/a/b` @ 60x20 and 80x24.
  - Tessera preselected at 120x40 and then shrunk to 60x20: it stays in view.
- Preselection in harness 2 (OpenCode owns the deepest match) @ 80x24, styled, scratch crate: ok. The OpenCode tab is `primary` and underlined, Claude is muted, and `> Parsley` is scrolled into view.
- Moving Quill → Relay → Sundial @ 80x24, scratch crate: ok.
  - The list scrolls one row each time, the session marker stays cleared, and the title and Working Directory follow the project.
  - Focusing Sessions then puts `>` on the newest Sundial session.
  - Returning to Projects and moving to Tessera clears it again.
- Live snapshot with a newer session while nothing is selected @ 80x24, scratch crate: ok. The new newest row is at the top and no `>` is drawn.
- Claude available but empty while OpenCode has sessions, no cwd match @ 80x24, scratch crate: shows the Claude empty state, as the binding planner note requires (nit 3). See `ui/empty-first-harness-other-has-sessions-80x24.txt`. Switching to `2` and back renders correctly.
- A harness whose projects all have zero sessions @ 80x24, scratch crate: mixed state (nit 1). See `ui/projects-without-sessions-80x24.txt`.

## Findings
1. [nit] Selector screen, harness whose projects all have zero sessions, 80x24.
   - Problem: `harness_empty` is true, so the Sessions pane shows `No sessions for this harness`. But the Projects pane still lists the projects with `> Agentmon` selected, the Sessions title is `Sessions in Agentmon` instead of ` Sessions `, and the Working Directory box shows `/p/Agentmon`. That contradicts the empty state described in the `UI` section.
   - Reachability: not reachable today, because Claude's `build_index` only creates projects from sessions. The OpenCode adapter, whose projects come from the `project` table, could produce it.
   - Change: either (a) when the harness has no sessions in any project, draw the full empty state: no project rows, title ` Sessions `, empty Working Directory box; or (b) set `harness_empty` only when the harness has no projects, and let project-without-sessions rows render as an ordinary empty list. Raise this with the OpenCode adapter task if it is not fixed now.
   - See `ui/projects-without-sessions-80x24.txt`.
2. [nit] Selector screen, empty state, 60x20 (the minimum size).
   - Problem: the Sessions interior is 29 cells and the 28-cell message is centred with 0 cells on the left. `No sessions for this harness` touches the left border (`│No sessions for this harness │`), while list rows keep a 2-cell margin.
   - Change: centre the message inside the same inner area the rows use, i.e. the interior minus the 2-cell row margin on each side. Below that width, truncate it with an ellipsis rather than letting it touch the border.
   - See `ui/empty-state-60x20.txt`.
3. [nit] Startup, no cwd match, first available harness empty while a later harness has sessions, 80x24.
   - Problem: startup lands on Claude's `No sessions for this harness` even though OpenCode has projects. This follows the binding planner note ("first available harness (by number) and its first project"), so it is not a builder fault.
   - Change (for the planner, if wanted): in the fallback, skip available harnesses with no projects, and use the empty state only when every available harness is empty.
   - See `ui/empty-first-harness-other-has-sessions-80x24.txt`.
