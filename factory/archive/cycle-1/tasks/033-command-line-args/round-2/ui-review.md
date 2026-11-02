VERDICT: PASS

## Summary
The round-1 must-fix is fixed. The filtered selector now keeps the startup Projects selection and Working Directory, whichever project the first match is in. The left side of the screen is the same as at a normal startup, before and after `Down`. All three snapshots the `UI` section asks for exist and match the description. The only snapshot change in this round is the filtered help line, which now reads `ESC to show all sessions`. `--harness` keeps restricting the filtered list after a tab switch: the other tab shows `No matching sessions`. The test gate passes: `cargo fmt --check` OK, `cargo clippy --all-targets -- -D warnings` OK, and `INSTA_UPDATE=no cargo test` passes 2249 tests with 0 failures. There are no `.snap.new` files.

## States checked
- Filtered selector, three matches @ 120x40 (text): snapshot `the_filtered_selector_120x40_text`, also rendered in the scratch crate — ok. The render is identical to round 1 except for the hint text.
- Filtered selector, three matches @ 120x40 (styled): snapshot `the_filtered_selector_120x40_styled` — ok. The only change is the hint text run on row 36, which now ends at column 114. The Sessions border has the focus colour, the selected row has the REVERSED band, and the Projects border is muted.
- After Esc from the filtered selector @ 120x40 (text): snapshot `after_esc_from_the_filtered_selector_120x40_text`, also rendered in the scratch crate — ok. The help line goes back to `ESC to Cancel`.
- Direct open of a session id, browser after loading @ 80x24 (text): snapshot `a_direct_open_80x24_text`. I also rendered direct opens by id and by a `.jsonl` path outside the data dir at 80x24, 120x40 and 50x16 — ok.
- Filtered, first match in BRIO (not the startup project) @ 120x40, before and after `Down`, and after `Esc`: rendered in the scratch crate — ok. Projects shows `> Agentmon` and the Working Directory box shows the Agentmon cwd. Columns 1–42 are identical to the normal startup with the same index (round-1 finding 1 is fixed).
- Filtered with `--project calira` @ 120x40: rendered in the scratch crate — ok. The Projects pane shows `> Calira` with the Calira cwd, the same as the startup preselection with `--project`.
- Filtered with `$PWD` under Nayra @ 120x40, then `Esc`: rendered in the scratch crate — ok. Projects shows `> Nayra`, and after `Esc` the screen shows `Sessions in Nayra`.
- `--harness opencode 85a8` @ 120x40, then `1`, then `Esc`: rendered in the scratch crate — ok. The selector starts on OpenCode with its two matches. On the Claude tab the list is empty and reads `No matching sessions` under the title ` Sessions matching "85a8" `. `Esc` returns to the normal startup on the OpenCode tab.
- `--harness claude 85a8` @ 120x40, then `2`: rendered in the scratch crate — ok. The OpenCode tab shows `No matching sessions`.
- Filtered across both harnesses without `--harness` @ 120x40, then `2`, then `Esc`: rendered in the scratch crate — ok. Each tab lists its own matches.
- Filtered, then `/`, then `Esc`, and filtered, then `Tab` @ 120x40: rendered in the scratch crate — ok. The filter ends and the help line returns to `ESC to Cancel`.
- Filtered @ 80x24, 60x20 and 40x12: rendered in the scratch crate — ok, with `…` truncation. At 40x12 the "Terminal too small" screen is shown. See nit 1 for the help line.
- Filtered with 43 matches and long CJK names @ 80x24, after 30×Down, and with an active match (styled) @ 120x40: rendered in the scratch crate — ok, the same as round 1.
- `--project nope` and `--theme nosuchtheme` startup toasts @ 80x24 (with `COMMAND_LINE_KEY`): rendered in the scratch crate — ok. Neither toast has a `Config: ` prefix.

## Findings
1. [nit] Filtered selector, 80x24 and narrower. The help line drops whole hints that do not fit, so it shows only `/ to search sessions | ENTER to open`. The new `ESC to show all sessions` hint is never visible at the most common terminal size. This is the existing priority-dropping behaviour of the selector help line, and the spec does not pin it. Possible change: in `filtered_selector_hints`, order the Esc hint before the `TAB to switch between projects and sessions` hint, so it survives at 80 columns. See `ui/filtered-80x24.txt`.
2. [nit] Filtered selector, all sizes. The Working Directory box stays on the startup project while the selection moves over matches in other projects. After `Down` onto the BRIO row, it still shows the Agentmon cwd. This is consistent with the Projects pane, with task-015 search results, and with the `UI` section's "Projects pane keeps its normal list", so no change is required. It is noted only because the cwd box does not describe the selected session. See `ui/filtered-down1-120x40.txt`.
