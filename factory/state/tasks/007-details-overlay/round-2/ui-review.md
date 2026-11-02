VERDICT: FAIL

## Summary
All round-1 UI findings are fixed: the overlay follows a resize, `g i` and `z i` chords close it, wrapped rows start at the value column, and `i` does nothing on a too-small screen. One must-fix remains: a `details` chord that uses an overlay key (`z j`, `z down`, `j i`) opens the overlay, but the same chord does not close it. `cargo fmt --check`, clippy, `cargo test` and the release budgets all pass with `INSTA_UPDATE=no`.

## States checked
- User prompt, assistant text, thinking, tool call, `model → …` note, stats footer, compaction, system, notice @ 80x24 and 120x40, `default` and `terminal` — snapshots `<kind>_<size>_<theme>` (36) — ok
- Content taller than the screen @ 80x24, top and end, both themes — snapshots `tall_80x24_{top,end}_{default,terminal}` — ok
- Long value wrapped to the value column @ 80x24 — snapshot `wrapped_value_80x24_default` — ok
- Unavailable group, one muted row @ 80x24 — snapshot `unavailable_80x24_default` — ok
- Overlay over the browser @ 80x24 and 120x40 — snapshots `the_details_over_the_browser_{80x24,120x40}_text` — ok
- Snapshot changes this round (`git diff 7ef6af0 HEAD`) — only `notice_120x40_{default,terminal}`, the continuation row moved one column left to the value column — ok
- Open with `i`, close with `i` and with `Esc` @ 80x24 — real binary in tmux — ok
- `details = ["g i"]`: opens, a lone `i` and `g j` leave it open, `g i` closes @ 80x24 — real binary in tmux — ok
- `details = ["g i"]`: help overlay shows `g i  Show section details` @ 120x40 — real binary in tmux — ok
- `details = ["z j"]`, `"z down"`, `"z pgdn"`, `"z k"` — real binary in tmux (`z j`) and scratch crate — issue, finding 1
- `details = ["j i"]`, `"down i"`, `"pgdn i"` — scratch crate — issue, finding 1
- Resize while open with a long `cwd`: 80x24 → 160x40 (one row, no indicator) → 80x40 (wrapped, as high as its rows, no indicator) → 60x20 (` ↓ more `) — scratch crate — ok
- Resize while open: 80x24 → 120x40 → 70x22 → 50x15 → 80x24, rect recentred each time — real binary in tmux — ok
- Scrolled to the end, then resized: 80x24 → 100x60 (all rows, no indicator) → 60x20 (` ↓ more ` only), scrolled to the end, then → 80x24 (last rows drawn, no gap) — scratch crate — ok
- Opening below the minimum size: 50x15, 59x20 and 60x19 stay closed; 60x20 opens; growing afterwards shows nothing open; `q` still quits at 50x15 — scratch crate and real binary in tmux — ok
- Shrunk to too small while open: the "Terminal too small" screen draws, `q` and a left press do nothing, the overlay is intact after growing — real binary in tmux and scratch crate — ok, the help overlay behaves the same way
- Wrapping @ 60x30, 80x30 and 140x30: space at a cut, two spaces at a cut, tabs, CRLF, trailing newlines, wide characters, combining characters and emoji — scratch crate — ok
- ESC and BEL in a value do not reach the buffer, so no escape sequence is emitted @ 60x30 — scratch crate — ok
- Value with many carriage returns @ 120x20 — scratch crate — issue, finding 2
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `INSTA_UPDATE=no cargo test`, `cargo test --release -- --ignored budgets --nocapture` (worst first content 2.1 ms, full parse 463.6 ms, frame 4.68 ms, peak memory 0.58x) — ok, no `.snap.new` files and no tracked files changed

## Findings
1. [must-fix] `DetailsOverlay`, open, any size, `details` bound to a chord that uses an overlay key — the chord opens the overlay, but pressing it again does not close it, against SPEC §5.1 ("The `details` key also closes it") and the criterion "A remapped `details` key opens and closes the overlay". Only `Esc` closes it.
   - `details = ["z j"]` loads with no warning. In the overlay, `z` is pending, then `j` resolves to the overlay's `Down` and scrolls. `z down` and `z pgdn` scroll the same way, and `z k` answers nothing at the top.
   - `details = ["j i"]` loads with a warning that `j` is removed from `next`. `j` scrolls and is not kept as a prefix, so `i` answers nothing. `down i` and `pgdn i` behave the same way.
   - The cause is in `DetailsOverlay::on`: the frame resolves the key first, and any overlay key calls `resolver.reset()`, so these chords can never complete.

   Change, in `DetailsOverlay::on`:
   - Feed every key to the resolver exactly once, before the frame, remembering whether a prefix was pending.
   - If a prefix was pending and the result is `Resolved::Action(Action::Details)`, answer `Msg::CloseDetails`.
   - Otherwise call `frame.on_key`. On `FrameKey::Unbound`, answer `Msg::CloseDetails` if the earlier result was `Action(Details)`. Do not feed the key again.
   - Remove the `reset()` call, because `feed` already drops a prefix that the next key does not complete.
   - A single overlay key keeps the planner's precedence: with `details = ["j"]`, `j` still scrolls.

   Tests to add:
   - Component, `"x j"`: `x` gives `None`, then `j` gives `Some(Msg::CloseDetails)`, and a `j` without the prefix still scrolls.
   - Component, `"j i"`: `j` scrolls, then `i` gives `Some(Msg::CloseDetails)`, and an `i` without the prefix gives `None`.
   - App, `"z j"`: the chord opens and closes the overlay, and the focus returns to the transcript.
   - `an_overlay_key_drops_a_pending_prefix` must still pass.

   See `ui/details-chord-with-an-overlay-key-80x24.txt`.

2. [nit] `DetailsContent::widest_row`, value with carriage returns, for example 120x20 — the wanted width is measured on the raw `row.value.split('\n')`, while drawing uses `display_lines`, which drops `\r`. Each `\r` therefore adds a column that is never drawn: `"x\r"` repeated 30 times gives a 71-wide overlay where 41 is wanted. A CRLF value adds one column. This is not reachable with this task's model rows, but tasks 008 and 009 will pass raw harness strings. Change: measure `display_lines(&row.value)` in `widest_row`, and add a test that a CRLF value wants the same width as its LF form. See `ui/details-wanted-width-counts-cr-120x20.txt`.
