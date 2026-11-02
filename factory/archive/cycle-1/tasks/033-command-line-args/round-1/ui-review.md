VERDICT: FAIL

## Summary
All three snapshots the `UI` section asks for exist. They match the description: the title is ` Sessions matching "85a8" `, rows use the task-015 result format, the first row has the focused band, Projects is unfocused, Esc returns to the startup screen, and the direct open at 80x24 is correct. The styled cell runs are identical to the task-015 search-results snapshot. The test gate passes (fmt OK; `INSTA_UPDATE=no cargo test` passes 2239 tests; clippy was not run here, the build notes report it passing).

One must-fix remains. When the first match belongs to a project other than the startup preselection, the filtered selector moves the Projects selection and the Working Directory box to that project, so the Projects pane no longer keeps its normal startup look. The committed snapshot misses this only because its first match happens to be in the preselected project (Agentmon).

## States checked
- Filtered selector, three matches @ 120x40 (text): snapshot `the_filtered_selector_120x40_text`, also rendered in the scratch crate. ok
- Filtered selector, three matches @ 120x40 (styled, with stats): snapshot `the_filtered_selector_120x40_styled`. ok. Sessions border uses the focus colour, Projects border is muted, the selected row is a REVERSED band with the `>` marker, other rows are muted. Each style run is the same as in `search_tests__query_brio_120x40_styled`.
- After Esc from the filtered selector @ 120x40 (text): snapshot `after_esc_from_the_filtered_selector_120x40_text`, asserted equal to the normal startup screen, also rendered in the scratch crate. ok
- Direct open of a session id, browser after loading @ 80x24 (text): snapshot `a_direct_open_80x24_text`. ok
- Filtered @ 80x24, 60x20 and 40x12 (too small): rendered in the scratch crate. ok (truncation with `…`; the "Terminal too small" screen at 40x12)
- Filtered with 43 matches, long and CJK project names @ 80x24, after 30×Down: rendered in the scratch crate. ok (scrolls, the selection stays visible)
- Filtered with an active match @ 120x40 (styled): rendered in the scratch crate. ok (`●` in the dot slot)
- Filtered across Claude and OpenCode @ 120x40, then `2`, then Esc: rendered in the scratch crate. ok (starts on the Claude tab; the OpenCode tab lists its own matches under the same title; Esc returns to the startup state)
- Filtered with the first match in BRIO (not the preselected project) @ 120x40: rendered in the scratch crate. **issue** (finding 1)
- Direct open by id and by a `.jsonl` path outside the data dir @ 80x24 and 120x40, first frame and after loading: rendered in the scratch crate with a real `ClaudeHarness`. ok (browser screen from the first frame, header shows the alias "Loose Alias")
- `--project nope` warning toast @ 80x24 and `--theme nosuchtheme` warning toast @ 80x24: rendered in the scratch crate. ok (no `Config: ` prefix; the message names the option)

## Findings
1. [must-fix] Filtered selector, first match in a project other than the startup preselection, 120x40 (any size). `App::show_filtered` sets `selector.project` to the project of the first match. In my render the first match is in BRIO, so the Projects pane draws `> BRIO` and the Working Directory box shows `/home/jacob/Development/projects/BRIO`. With the same index and working directory, the normal startup draws `> Agentmon` and the Agentmon cwd. The `UI` section says the Projects pane "keeps its normal list, with no focus", so its selection should not change. The project choice also does not follow the selection: after `Down` onto an Agentmon row, Projects still says BRIO (see `ui/filtered-first-match-in-brio-down1-120x40.txt`).
   Change: in `show_filtered`, leave `selector.project` (and therefore the Projects marker and the Working Directory box) at the startup preselection, the result of `preselection()` that `leave_filtered` uses, as task-015 search results do. Then the filtered screen differs from the startup screen only in the Sessions pane and focus. Add an update test or snapshot whose first match is not in the preselected project, and assert that the Projects selection equals the normal startup one.
   See `ui/filtered-first-match-in-brio-120x40.txt`, `ui/filtered-first-match-in-brio-down1-120x40.txt` and `ui/normal-startup-same-index-120x40.txt`.
2. [nit] Filtered selector, all sizes: the Working Directory box never follows the selected match. After `Down` onto the BRIO row it still shows the Agentmon cwd (`ui/filtered-down1-120x40.txt`). Search results (task 015) behave the same way, so this is consistent with the existing behaviour. Showing the selected match's cwd would arguably be clearer, but neither spec pins it.
3. [nit] Filtered selector, 120x40: the help line still reads `ESC to Cancel`, but in filtered mode Esc leaves the filter (the Builder notes this too). A filtered-mode hint such as `ESC to show all sessions` would be more accurate. The spec does not pin the hint text.
