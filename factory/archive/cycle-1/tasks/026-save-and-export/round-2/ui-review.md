VERDICT: PASS

## Summary
Round 2 does not change any of the task's UI snapshots. All four required states, plus the extra styled and `Saved to …` ones, are still present and still match the `UI` section. The round-1 UI nit 1 is fixed: a long path in a success toast is now cut at its start, so the file name stays visible at 64 columns and wider. One small nit remains for that fix at the narrowest terminals (60–63 columns). The round-1 nit about the prompt scroll marker is unchanged, as the build notes say. The test gate passes with `INSTA_UPDATE=no`.

## States checked
- Save path prompt, default path @ 80x24: snapshots `save_prompt_80x24_text` and `save_prompt_80x24_styled` (unchanged since round 1). ok. Row 23 reads ` Save to: ./brio-85a86657-shell.txt`. The label is fg 229 (`help_key`), the path is fg 250 (`foreground`), and the cursor cell at col 35 has bg 115 (`cursor`), at the end of the text.
- Export path prompt @ 120x40: snapshot `export_prompt_120x40_text`. ok. It reads ` Export to: ./brio-20261021-0835-85a86657.md`.
- Overwrite confirmation @ 80x24: snapshots `overwrite_confirmation_80x24_text` and `_styled`. ok. It reads ` Overwrite ./brio-85a86657-shell.txt? (y/n)`. `Overwrite ` and `? (y/n)` are fg 161 (`help_text`), the path is fg 250 (`foreground`), and there is no cursor cell.
- `Exported to …` toast @ 80x24: snapshot `exported_toast_80x24_text`. ok. The box is bottom-right in the task-012 style, and the help line is back to its key hints. `saved_toast_80x24_text` (extra) is ok too.
- Success toast with a long absolute path @ 120, 80 and 64 columns (export and save): rendered in the scratch crate with the text built as `WriteKind::done` builds it and the `toast_rect` geometry. ok. For example, `Exported to …long-project-name-20261021-0835-85a86657.md` keeps the whole file name, with fg 250 on bg 18 (`toast`).
- Success toast with a long path of wide (CJK) characters @ 80 columns: rendered in the scratch crate. ok. `Saved to …ト/とても長い名前のファイル-85a86657-shell.txt` fits exactly.
- Success toast with a long path @ 60–63 columns: rendered in the scratch crate. Issue, see nit 1 and `ui/success-toast-long-path-60x24.txt`.
- Short default-path toasts @ 60 columns, plus `Refusing to write into harness data` and `Export failed: Permission denied (os error 13)` @ 60 columns: rendered in the scratch crate. ok. They fit without cutting.
- The dangling-symlink refusal (new in round 2) shows the existing `Refusing to write into harness data` toast and mounts no prompt (`a_dangling_symlink_into_harness_data_is_refused_without_asking`). No new UI state.
- Test gate: `cargo fmt --check` passes, `cargo clippy --all-targets -- -D warnings` passes, and `INSTA_UPDATE=no cargo test` passes. There are no `.snap.new` files, and the working tree is clean after the run.

## Findings
1. [nit] Success toast (`Saved to …` / `Exported to …`) with a long path, at terminal widths 60–63 (for example 60x24).
   - **Problem:** `WriteKind::done` sizes the start cut for `MAX_TOAST_WIDTH - TOAST_CHROME` (56 columns). But `toast_rect` makes the box only `min(60, screen width - 4)` wide. Below 64 columns the `Toast` then cuts the text at its end as well. The result is cut at both ends, and the file name is lost: `Exported to …long-project-name-20261021-0835-85a866…` at 60 columns.
   - **Change:** cut the path for the box width actually used, `MAX_TOAST_WIDTH.min(screen_width - 4) - TOAST_CHROME`, minus the prefix. Either compute this in `done` from the current screen width, or let the `Toast` cut the path part at its start when it renders. At 60 columns the toast would then read `Exported to …-project-name-20261021-0835-85a86657.md`.
   - See `ui/success-toast-long-path-60x24.txt`.
2. [nit] Carried over from round 1, unchanged: when the path prompt (`Save to: ` / `Export to: `) text is wider than the row, the shared `Prompt` scrolls horizontally and drops the start of the path with no `…` marker. This behaviour predates this task. Change (optional, could be a follow-up): when the visible input does not start at its first column, draw `…` in the first text cell.
