VERDICT: FAIL

## Summary
All five UI states in the task's `UI` section have snapshots, and each one matches its description, but there is one must-fix. Blocks are stacked with no gaps. Blocks are clipped at the top and bottom edges, the background is painted in `background`, the `▒` marker is in `primary`, and the selected section is `foreground` bold while unselected sections are `muted_foreground`. With `dim_unselected = false`, sections keep their token colours. I also rendered the transcript in a scratch crate at small, inset and unannounced sizes, with empty and short content, long text, scrolling to the bottom, resizes, updates, Reset and both `show_system` values. The must-fix: when the selected section disappears (through `show_system` or an `Updated` turn), the selection moves to the right section, but the view does not scroll to it. The new selection can be off-screen, so the frame has no `▒`. `INSTA_UPDATE=no cargo test` passes.

## States checked
- Top of transcript, first section selected @ 80x24 (text): snapshot `top_of_the_transcript_at_80x24_text`. ok (block 2 is clipped at the bottom edge, `▒` on both prompt rows)
- Top of transcript @ 120x40 (text): snapshot `top_of_the_transcript_at_120x40_text`. ok
- Top of transcript @ 120x40 (styled): snapshot `top_of_the_transcript_at_120x40_styled`. ok (`▒` = primary 229,229,229; selected section = foreground 250,250,250 BOLD; unselected text/tool rows = muted_foreground 161,161,161; borders = border 94,94,94; bg = background 10,10,10 on every cell)
- Scrolled to the middle, clipped at top and bottom @ 80x24 (text): snapshot `scrolled_to_the_middle_with_both_edges_clipped_at_80x24_text`. ok (top row inside block 2, bottom row inside block 4)
- Selection in the 5th block after `Cmd::Move` @ 80x24 (text): snapshot `selection_in_the_fifth_block_after_moves_at_80x24_text`. ok (compaction section selected, scrolled minimally so its bottom border is on the last row)
- Resize 120 to 80 columns, re-wrapped @ 80x24 (text): snapshot `after_a_resize_from_120_to_80_columns_text_is_wrapped_again_at_80x24_text`. ok (the test also asserts the output equals a fresh 80-column render)
- `dim_unselected = false` @ 120x40 (styled): snapshot `dim_unselected_false_at_120x40_styled`. ok (the diff against the dimmed snapshot touches only unselected text/tool rows, which change to foreground non-bold; thinking, model-change and stats keep their muted token colours)
- Empty transcript @ 40x6 (styled): rendered in scratch crate. ok (background only; Scroll/Move/Submit return `NoChange`)
- Content shorter than the viewport @ 30x8 (styled): rendered in scratch crate. ok (rows below the last block are background; Scroll Down returns `NoChange`)
- Small terminals @ 20x5, 12x4, 6x3, 3x2, 1x1, 80x1, 80x2: rendered in scratch crate. ok (no panics, clipping is correct; wrapping and truncation inside the block come from the task-017 renderers)
- Inset area 30x8 at (10,3) in a 50x14 frame: rendered in scratch crate. ok (nothing drawn outside the area)
- View without an announced viewport, and an announced 80x24 drawn into 40x10: rendered in scratch crate. ok (matches a prepared render at that size)
- Transparent background theme @ 40x4 (styled): rendered in scratch crate. ok (bg stays `Reset`)
- Long text (1000-char paragraph, unbroken 280-char word, 60-line tool output unfolded) @ 40x30 / 50x10: rendered in scratch crate. ok (a section taller than the viewport is paged through 10 rows at a time, in both directions)
- Scroll to the bottom @ 60x12, then grow to 60x40: rendered in scratch crate. ok (the last row sits on the viewport bottom; the offset clamps from 51 to 23 when the viewport grows)
- Resize mid-scroll 120x20 to 30x20 to 30x5: rendered in scratch crate. ok (the top block stays at the top, at the same row within it)
- Appended / Updated (growing) / Reset @ 50x12: rendered in scratch crate. ok (the offset is kept, the updated block is re-wrapped, and Reset clears to background with no selection)
- `show_system` true/false @ 50x60: rendered in scratch crate. ok, with nit 2 below
- First turn fully hidden (`show_system = false`, empty text + injected only) @ 50x10: rendered in scratch crate. ok (the initial selection is the first visible section, `t1:Prompt`, and `▒` is drawn; `show_system` back on keeps it)
- Selection navigation around the hidden section @ 50x10: rendered in scratch crate. ok (Up/Down skip the empty t6 shell: t7:c, then t5:Summary, then t4:Footer, and back)
- Selected section hidden by `show_system = false` @ 50x10: rendered in scratch crate. issue (finding 1, `ui/selection-offscreen-after-show-system-50x10.txt`)
- Selected section removed by an `Updated` turn @ 50x10: rendered in scratch crate. issue (finding 1, `ui/selection-offscreen-after-update-50x10.txt`)

## Findings
1. [must-fix] Transcript, after its selected section disappears, at any size (shown at 50x10). This covers two paths: `show_system` switching to `false` while an injected section is selected, and an `Updated` turn that no longer contains the selected section. `restore_selection` (`updates.rs`) moves the selection to the nearest following section, as the planner note requires. But `set_show_system` and `change_turns` then only call `refresh()`, which clamps the offset and never scrolls the new selection into view. When the new section sits outside the viewport, the frame shows no `▒` at all.
   - In the `show_system` case, the selection moves from `t6:i1` to `t7:c` ("Done.", transcript row 61), and the offset stays at 51 (rows 51..60).
   - In the `Updated` case, the selection moves from `t2:c1` (last visible row) to `t2:Footer` below the viewport, and the offset stays at 0.
   - SPEC §6.3 says "Selecting a section scrolls it into view". The selected section is the target of `Submit` now, and of `y`/copy and the other actions in task 020, so the user would act on a section they cannot see.
   - Change: in `restore_selection` (or right after it in `set_show_system` and `change_turns`), when the selected id changed, scroll it into view with the same minimal reveal `select()` uses (`nav.rs`), before `refresh()`. When the selection did not change, keep the offset as it is today. Add a component test for each path: assert that after the change, the newly selected section's rows overlap `offset..offset + height` and a `▒` appears in the rendered frame.
   - See `ui/selection-offscreen-after-show-system-50x10.txt` and `ui/selection-offscreen-after-update-50x10.txt`.
2. [nit] Transcript with `show_system = false`, any size, a user turn whose text is empty and whose only items are injected/system items. The block is still drawn as an empty 3-row shell: title row, one blank row, bottom border. It has no selectable section. See `ui/empty-user-block-show-system-false-50x60.txt`. The spec does not pin this down, and the shell comes from `TurnBlock` (task 017), not from the viewport code. Change (optional, could be a later task): give such a block height 0 and skip it in the layout cache and in navigation, or keep it and document that an empty block is intentional.
3. [nit] Resize snapshot, 80x24. Only the narrow (80-column) side is snapshotted. The 120-column render before the resize is used only in an assertion (more `▒` rows at 80 than at 120). A second text snapshot of the 120x24 state before the resize would show the re-wrap in the snapshot history, not just in the test logic.
