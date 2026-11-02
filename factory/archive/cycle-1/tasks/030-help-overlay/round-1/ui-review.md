VERDICT: PASS

## Summary
All six required `App::view` snapshots are present and match the `UI` section and the planner notes. They show a centred `╭─ Keybindings ─…╮` overlay with a `ring` border on `surface`, bold `foreground` headings at column 3, `help_key` keys at column 5 padded to the widest label + 6, `help_text` descriptions, and muted ` ↑ more ` / ` ↓ more ` indicators with one `─` before the corner. I rendered the other sizes, contexts, themes, remaps and resizes in a scratch crate, and they behave correctly. The only issue is cosmetic: on terminals narrower than 76 columns, descriptions are cut off mid-word against the right border.

## States checked
- Browser, top of list, `↓ more` @ 120x40: snapshots `the_browser_help_120x40_top_text` and `_top_styled`. ok. The styled run shows these colours:
  - border `ring` Rgb(207,207,207) on `surface` Rgb(18,18,18);
  - title `Keybindings` in `foreground` (250);
  - headings in bold `foreground`;
  - keys in `help_key`/`primary` (229);
  - descriptions in `help_text`/`muted_foreground` (161);
  - ` ↓ more ` in `muted_foreground` with ring-coloured `─╯`.
- Browser, scrolled to end, `↑ more` + Shift note @ 120x40: snapshot `the_browser_help_120x40_end_text`. ok. There is no `↓ more`, and the quit row reads `q / Ctrl-q / Ctrl-w / Ctrl-c`.
- Browser @ 80x24: snapshot `the_browser_help_80x24_text`. ok. The overlay is 72x22, centred at (4,1), with `↓ more`.
- Browser @ 200x60, everything fits: snapshot `the_browser_help_200x60_text`. ok. There are no indicators, and it is centred at (64,4).
- Selector @ 120x40: snapshot `the_selector_help_120x40_text`. ok. It shows Session selector (only the `1` harness row) and Application, without `Ctrl-o`. The selector frame, panes and help line stay visible around it.
- `copy = "c"` @ 120x40: snapshot `a_remapped_copy_key_shows_in_the_drawn_overlay_120x40_text`. ok. The copy row shows `c`, and so does the browser help line.
- Component, scrolled middle, styled: snapshot `the_overlay_snapshot_at_a_scrolled_position_is_styled`. ok. Both indicators are shown.
- Browser top/end @ 60x20 (minimum size): rendered in scratch crate. Layout ok; descriptions are clipped (see nit 1).
- Browser top/end @ 70x24: rendered in scratch crate. Layout ok; `Fold or unfold current sec` is clipped (nit 1).
- Browser top/middle/end @ 80x24, scrolled with `PgDn`, `j`, `↑`: rendered in scratch crate. ok. Scrolling clamps at the end, and the indicators are correct at each position.
- Browser @ 200x60 with `mouse = false`: rendered in scratch crate. ok. There is no Shift note; the last row is the Application group's trailing blank row.
- Selector, all four harnesses registered @ 120x40: rendered in scratch crate. ok. Rows `1`–`4` appear in order.
- Selector @ 60x20 through `App::view`: rendered in scratch crate. ok apart from nit 1. The overlay covers the selector frame and leaves 2 columns and 1 row on each side.
- Selector, all harnesses, opened at 60x20 then resized to 120x40: rendered in scratch crate. ok. The overlay grows to fit, the indicators disappear, and the interior is refreshed via `resized` → `push_viewports` → `push_help_interior`.
- Resized to 59x19 while open: rendered in scratch crate. ok. Only "Terminal too small" is shown, and the overlay is hidden.
- Transparent `default` theme @ 80x30: rendered in scratch crate. ok. It uses `Clear` only, so the background is `Reset` and none of the filler shows through. The `ring` border and the text colours are unchanged.
- `terminal` built-in theme @ 80x30: rendered in scratch crate. ok. The border is White on Black and the descriptions are Gray.
- Long remap, `copy = c / ctrl-y / f5 / shift-tab / "z z"` @ 100x40 and 60x20: rendered in scratch crate. ok. The key column widens to the longest label + 6 and the overlay widens to match. At 60x20 the descriptions are clipped (nit 1).
- Remapped `[keys.global] help = "f1"` and `[keys.overlay] down = "n"`: checked by calling the component's `on()` from the scratch crate. ok. `F1` closes the overlay and `?` no longer does. `n` scrolls and `j` no longer does.
- Test gate:
  - `cargo fmt --check`: pass.
  - `cargo clippy --all-targets -- -D warnings`: pass.
  - `INSTA_UPDATE=no cargo test`: all pass.

## Findings
1. [nit] HelpOverlay, any context, terminals 60–75 columns wide (e.g. browser and selector @ 60x20, browser @ 70x24). When `w − 4` is less than the desired width, the `Paragraph` cuts each row hard at the interior edge. Descriptions then stop mid-word right against the right border, with no padding and no ellipsis. Examples are `Scroll half a pa│`, `Clear search, cl│`, `Switch to OpenCo│` and `Fold or unfold current sec│`. With long remapped keys at 60x20 it gets worse: `Previous se│`. The planner notes pin the width formula and the keys + 6 gap, so the clipping itself is expected. Only how a row ends is open.

   Change: when a description does not fit in the interior, shorten it with the existing `truncate_with_ellipsis` so it ends in `…`. Optionally keep one column of right padding before `│`. Keep the key column and the gap unchanged.

   See `ui/selector-60x20.txt` (`App::view`), `ui/browser-60x20-component.txt`, `ui/browser-70x24-component.txt` and `ui/browser-longkeys-60x20-component.txt`. The last three are component renders over a `.` filler, placed with `help_overlay_rect`.
