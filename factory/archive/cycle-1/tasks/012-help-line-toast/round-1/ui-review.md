VERDICT: PASS

## Summary
Every snapshot the `UI` section asks for is there, and each one matches it. The help line sits on `overlay.y + overlay.height − 3` from overlay column 5, and with default keys it reads exactly as the sketch, with keys in `help_key` (primary) and text in `help_text` (muted_foreground). It drops whole hints at narrow widths. The toast is a 3-row rounded box in the `toast` style, one column in from the right edge with its bottom border on `h − 2`, truncated with `…` and drawn on top. I re-rendered the views in a scratch crate at extra sizes, with extra themes and edge-case text, and found no must-fix issues. There are four nits.

## States checked
- Selector with help line @ 80x24 (trailing `TAB …` and `ESC …` hints dropped whole): snapshot `selector_help_line_80x24_text` (and `startup_frame_80x24_text`). ok
- Selector with help line @ 120x40 (all four hints, row 36, column 5): snapshot `selector_help_line_120x40_text`. ok
- Selector with help line @ 120x40, styled: snapshot `selector_help_line_120x40_styled`. Keys are Rgb(229,229,229) = `primary`, text and ` | ` are Rgb(161,161,161) = `muted_foreground`, on `surface`. ok
- HelpLine component @ width 40 with `3/17` (one hint, status right-aligned): snapshot `narrow_width_40_with_status_keeps_one_hint`. ok
- HelpLine component @ width 60 with `3/17` (two hints): snapshot `width_60_with_status_keeps_two_hints`. ok
- HelpLine full line, styled: snapshot `full_line_text_and_styles`. ok
- HelpLine sweep over widths 0–120 with no status, `3/17`, and three status items: rendered in scratch crate. It never cuts mid-hint, status is right-aligned whenever it fits, and it keeps a 2-column gap. ok (see nit 2 for status wider than the area)
- HelpLine with `fill = Background`: rendered in scratch crate. Paints `background` (Rgb 10,10,10). ok
- Selector with startup toast @ 80x24 and 120x40, text: snapshots `selector_with_toast_80x24_text`, `selector_with_toast_120x40_text`. Width is text+4 = 27, the right border is on column w−2, the bottom border on row h−2, drawn over the overlay. ok
- Selector with startup toast @ 120x40, styled: snapshot `selector_with_toast_120x40_styled`. fg Rgb(250,250,250) = `foreground` on Rgb(18,18,18) = `surface`. ok
- Toast with long text @ 80x24, truncated with `…`, width 60: snapshot `long_text_is_truncated_with_an_ellipsis_at_80x24`. ok
- Selector with toast @ 60x20, 61x21, 100x30: rendered in scratch crate. Geometry is per the planner notes. ok (see nit 3)
- Selector with three long warnings @ 60x20 and 80x24: rendered in scratch crate. Width is capped at min(60, w−4) (56 at 60x20, 60 at 80x24). ok (see nit 1)
- Too-small screen 59x24 with a startup warning: rendered in scratch crate. Only "Terminal too small" is drawn, with no toast. ok
- Toast with `terminal` theme and with a transparent `default` theme @ 80x24, styled: rendered in scratch crate. Terminal uses Reset on Black. Transparent paints no `surface` (bg Reset), and neither does the help line. ok
- Toast with CJK text, empty text, and widths 3 and 5: rendered in scratch crate. Truncation respects double-width characters, and tiny boxes don't panic. ok
- Frame after the toast expires @ 80x24 (Tick-driven `ToastExpired(2)`, then `update`): rendered in scratch crate. The toast is unmounted, and the frame is identical to `selector_help_line_80x24_text` (`ENTER to open` and the bottom border are fully back). ok
- Test gate with `INSTA_UPDATE=no`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` all pass. ok

## Findings
1. [nit] Toast, startup with several warnings whose first warning is long, @ 80x24 and 60x20. The whole text `Config: <first> (+N more, see log)` is truncated from the end, so the `(+2 more, see log)` suffix disappears. The user can't tell that more warnings exist or that they are in the log. The planner notes only say "longer text is truncated with `…`", so this is allowed. Change (optional): in `startup_toast_text` or in the toast's fitting, truncate only the first warning with `…` and always keep ` (+N more, see log)`, e.g. `Config: [keys.selector] unknown acti… (+2 more, see log)`. See `ui/selector-toast-three-long-warnings-80x24.txt`.
2. [nit] HelpLine, status items wider than the area (for example three status items at widths 10–25). The status text is drawn left-aligned and cut on the right (`3/17  ↓ 12 new  Loa`) with no `…`. The spec doesn't cover this. It can't happen in the selector today (the help line is at least 50 columns), but the browser (task 021) can hit it. Change (when the browser reuses it): when the status is wider than the row, drop the leftmost status items, or keep the right end so it stays right-aligned. See `ui/help-line-status-overflow.txt`.
3. [nit] Selector with toast, all sizes. Because the toast's top row is `h − 4`, it overlaps the help line row and the overlay's bottom border. It covers hint text (`… | ENTE╭` at 60x20, `… | ES╭` at 120x40, `/ to search se╭` with a 60-wide toast at 80x24) and joins the overlay border (`╰───╰──╯╯`). This follows the binding planner geometry and "drawn on top of everything", so the builder needs to change nothing. It is flagged for the planner: if the help line should stay readable while a toast shows, move the toast up (e.g. bottom border on `overlay.bottom − 4`), or let a later task decide. See `ui/selector-toast-60x20.txt`.
4. [nit] Toast component, test `short_text_is_padded_by_one_space_in_a_rounded_box` (`src/components/toast.rs`). It renders into a hard-coded 28-wide area, but the layout gives this text a 27-wide box (text+4). The snapshot therefore shows two trailing spaces that never appear in the app, and it doesn't pin the one-space padding. Change: size the area as `text.width() + 4` (27), so the snapshot reads `│ Config: unknown key "x" │`.
