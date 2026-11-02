VERDICT: PASS

## Summary
Every state and size in the task's `UI` section has a screen snapshot through `App::view`, and each one matches the description. That covers the header layout and roles, the transcript rows, the blank separator row, the help line text and roles, the right-aligned `Loading… 34%`, the centred `Session file is empty`, the parse-warning note ending at column `w − 3`, the selector overlay with the browser visible in row 0, row `h − 1` and the side columns, and all seven themes. I also rendered the views in a scratch crate at the minimum size, with long names and paths, through the loading lifecycle, with the selector over the browser at several sizes, and with a transparent theme. None of these renders showed a UI defect. `INSTA_UPDATE=no cargo test` passes.

## States checked
- Loaded browser @ 120x40 (text): snapshot `loaded_browser_120x40_text`. ok. Header rows 0–4, first block at row 5, row 38 blank, help line at row 39 from column 1, reading exactly `/ to search | ENTER to fold or unfold sections | y to copy current | ? for keybindings`.
- Loaded browser @ 120x40 (styled): snapshot `loaded_browser_120x40_styled`. ok. Border in `border`, ` Project ` in `muted_foreground`, `BRIO` bold `foreground`, session id in `timestamp`, cwd (cols 2–25) in `foreground`, keys in `help_key` (primary) and text in `help_text` (muted). Every cell is on `background`.
- Loaded browser @ 80x24 (text): snapshot `loaded_browser_80x24_text`. ok. The help line drops whole trailing hints to fit, as the task-011 help line does.
- Loading @ 80x24 (text): snapshot `loading_browser_80x24_text`. ok. `Loading… 34%` is right-aligned, ending at column `w − 2`, over a partial transcript.
- Loading @ 120x40 (styled), 60x20, 0% with no turns @ 80x24 and 60x20, 0.999 → `Loading… 100%`, then `Loaded` clears the status: rendered in the scratch crate. ok. No "Session file is empty" shows before `Loaded`.
- Empty session @ 80x24 (text): snapshot `empty_session_80x24_text`. ok. The text is centred in the transcript area (row 13, column 30).
- Empty session @ 60x20 and 120x40, and `Ctrl-o` over an empty session: rendered in the scratch crate. ok. See `ui/empty-session-60x20.txt`.
- Parse warnings (3) @ 120x40 (text): snapshot `parse_warnings_120x40_text`. ok. The note ends at column 117 (`w − 3`). The header's own styled snapshot puts the note in `muted_foreground`.
- Parse warnings (1) @ 80x24 (styled): rendered in the scratch crate. ok. `1 entry could not be parsed` in `muted_foreground`.
- Long project name, long cwd and a 1234-entry note @ 60x20, 80x24 and 120x40: rendered in the scratch crate. ok. The name is cut with `…` before the session id, the cwd is cut from the left keeping 2 cells before the note, and the note stays right-aligned.
- Minimum size 60x20, loaded: rendered in the scratch crate. ok. 59 columns shows only "Terminal too small", even with the selector open over the browser, and returns to the overlay when the terminal grows again.
- Selector over the browser @ 120x40 and 200x50 (text): snapshots `selector_over_the_browser_120x40_text` and `selector_over_the_browser_200x50_text`. ok. The browser shows in row 0, row `h − 1` and, at 200x50, columns 0–19 and 180–199. The open session is selected.
- Selector over the browser @ 120x40 (styled), 80x24 and 60x20: rendered in the scratch crate. ok. Sessions has the `ring` border, Projects has `border`, the open session's row is in reverse video, the overlay is on `surface`, and the browser rows outside it keep `background`. Focus is on `SessionList`.
- Selector over the browser with a transparent theme @ 200x50 (styled): rendered in the scratch crate. ok. The overlay is cleared, so no transcript text bleeds through, and the browser columns stay unpainted.
- `Esc` from the selector over the browser @ 80x24: rendered in the scratch crate. ok. The browser comes back unchanged, with focus on `Transcript`.
- Per-theme @ 120x40 (styled), for default, terminal, catppuccin, gruvbox, nord, tokyo-night and solarized: snapshots `loaded_browser_theme_*`. ok. Each theme uses its own palette. Only `terminal` (transparent) leaves the background as `Reset`.
- Browser state across `Ctrl-o` then `Esc` @ 80x24, with the transcript scrolled and the `▒` selection mid-transcript (`j` ×8): rendered in the scratch crate. ok. The renders before and after are identical, so neither scroll nor selection is reset.
- Updates arriving while the overlay is open @ 80x24 (`Appended` of the rest, `ParseWarnings(2)` and `Loaded` sent after `Ctrl-o`): rendered in the scratch crate. ok. `Loading… 34%` leaves the browser help line under the overlay. After `Esc`, all 12 turns are present (`G` reaches the last one), the header shows `2 entries could not be parsed`, and no empty state appears.
- `Ctrl-o` after switching sessions @ 120x40, from BRIO to Agentmon `c3` and from BRIO to the newer BRIO session: rendered in the scratch crate. ok. The header shows the new project, cwd and session id. The overlay selects the new project and the new session's row, with Sessions focused.
- Transcript navigation inside the browser viewport (`j` ×30, `G`) @ 80x24: rendered in the scratch crate. ok. The transcript scrolls inside rows 5 … `h − 3` and never draws over the header or the help line.

## Findings
1. [nit] Browser help line, empty-session state, all sizes. Once `Loaded` arrives with zero turns, the help line still offers `ENTER to fold or unfold sections | y to copy current`, but there is nothing to fold or copy (see `ui/empty-session-60x20.txt`). The spec does not pin this down. Change, if wanted: when the session is empty, show only `<browser.search> to search | <global.help> for keybindings`, or leave it as is.
