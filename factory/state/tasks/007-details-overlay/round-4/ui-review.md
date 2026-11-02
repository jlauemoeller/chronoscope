VERDICT: FAIL

## Summary
The round-3 must-fix is fixed for values as written: `⚠️`, keycaps, ZWJ emoji and halfwidth sound marks now wrap and draw to their end at every size checked, and no snapshot changed. One must-fix remains of the same kind: when a control character sits directly between a sign and its variation selector, the value is still measured too narrow and its end is lost at every size. All gates pass with `INSTA_UPDATE=no`, and a one-expression change fixes the finding on a patched copy.

## States checked
- User prompt, assistant text, thinking, tool call, `model → …` note, stats footer, compaction, system, notice @ 80x24 and 120x40, `default` and `terminal` — snapshots `<kind>_<size>_<theme>` (36) — ok, reviewed in earlier rounds, none changed since
- Content taller than the screen @ 80x24, top and end, both themes — snapshots `tall_80x24_{top,end}_{default,terminal}` — ok, `tall_80x24_end_default` reread this round
- Long value wrapped to the value column @ 80x24 — snapshot `wrapped_value_80x24_default` — ok, reread this round
- Unavailable group, one muted row @ 80x24 — snapshot `unavailable_80x24_default` — ok
- Overlay over the browser @ 80x24 and 120x40 — snapshots `the_details_over_the_browser_{80x24,120x40}_text` — ok, 80x24 reread this round
- Snapshot changes this round (`git diff 5b18587 HEAD -- '*.snap'`) — none, and no `.snap.new` files — ok
- Values of `⚠️`, keycaps, ZWJ emoji, halfwidth katakana with a sound mark, flags, CJK and combining marks @ 80x24, 120x40, 200x30 and 60x20, opened through `App::update` — scratch crate — ok, all drawn to `END` (cut across two rows in five of the 60x20 cases, as char-wrapping does), aligned to the value column, right padding and border intact
- Round-3 finding 1 in the real binary: `model` of `⚠️` x 40 + `END` @ 80x24 and 120x40, `j` then `i` opens, `Esc` closes — tmux 3.8 — ok for wrapping and for `END`; drift issue at 80x24, finding 2
- Random mix of graphemes as written (`clean` palette), 4000 values at five widths — scratch crate — ok, every value drawn whole, padding and border intact, no indicator
- Base x separator x mark sweep, 10080 values at widths 60, 80, 120 and 200 — scratch crate — issue, 120 bad, finding 1
- Random mix with bare bases, marks and control characters, 6000 values — scratch crate — issue, 233 bad, finding 1
- Wanted size against what is drawn, screen widths 60–170, value lengths 0–259 (plain, with spaces, trailing spaces, wide characters), 5624 cases — scratch crate — ok
- Tall content @ 60x20, top and end, and tool call @ 60x20 and 100x30 — scratch crate — ok
- Heading, label, value, muted row, `ring` border and indicator styles @ 60x24, `default` — scratch crate with styles — ok
- All-spaces value (round-3 nit) — unchanged, and the approver's round-4 note leaves it as is on purpose, so it is not raised again
- Mutations on a copy outside the repo, tests matching `details`: unmodified 112 pass; per-`char` measuring fails `a_grapheme_is_measured_as_it_is_drawn` and `a_value_is_drawn_to_its_end_however_its_graphemes_are_measured`; `g.width()` fails the second; `desired_size(u16::MAX)` in `open_details` fails `a_wider_screen_gives_the_overlay_the_width_its_rows_want` — ok, the new tests are effective
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `INSTA_UPDATE=no cargo test` (2776 passed, 0 failed), `INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture` (worst first content 2.5 ms, full parse 291.7 ms, frame 3.22 ms, peak memory 0.58x) — ok

## Findings
1. [must-fix] `DetailsOverlay`, any section, 80x24, 120x40 and 200x30, a value with a control character directly between a base and its U+FE0F, for example `"⚠\u{7}\u{fe0f}".repeat(40) + "END"` — the value is measured too narrow, so it does not wrap and its end is never drawn. This contradicts the `UI` section ("A long value wraps inside the overlay, aligned to the value column") and spec/SPEC.md §5.5 (paths and long tokens are never hidden).
   - `drawn_graphemes` in `src/components/details_overlay/content.rs` segments the text with the control character still in it, so `⚠` counts 1 column and U+FE0F counts 0. `char_wrap` then drops the control character, so the piece holds `⚠` + U+FE0F, which the buffer segments again and draws as one 2-column grapheme. Its doc comment ("as the buffer does it") is not true for this case.
   - The overlay is 58 wide at all three sizes and draws 22 signs on one row. The other 18 signs and `END` are never drawn, and there is no wrapped row or indicator. The real binary shows the same at 120x40, 71 wide, for an assistant turn whose `message.model` is this value.
   - Reach: it needs a raw C0 or C1 control character directly between a base and U+FE0F, so it is unlikely in real data. `turn.model` and `turn.effort` are free strings from the session file today, and tasks 008 and 009 add more raw strings. In the sweep, every failing case has a control character as the separator and U+FE0F as the mark; other separators (U+200B, U+2060, U+00AD, U+200D, U+2028, U+FEFF, U+180E) never fail. Rows that overflow by less than the two padding columns still draw `END`, so a value needs several such graphemes before text is lost.

   Change, in `display_lines` in `content.rs`, remove every control character before segmenting, after the tab becomes a space. Keep `\n`, which splits the lines:

   ```rust
   let value = value
       .replace('\t', " ")
       .replace(|c: char| c.is_control() && c != '\n', "");
   ```

   Update its doc comment to say a tab is a space and other control characters are dropped.

   Tests to add:
   - In `a_value_is_drawn_to_its_end_however_its_graphemes_are_measured`, add the value `format!("{}END", "\u{26a0}\u{7}\u{fe0f}".repeat(40))`.
   - In `a_grapheme_is_measured_as_it_is_drawn`, add:

   ```rust
   let split = format!("{}END", "\u{26a0}\u{7}\u{fe0f}".repeat(40));
   assert_eq!(wanted(&split), wanted(&"x".repeat(83)));
   ```

   Checked on a patched copy of HEAD outside the repo:
   - Both additions fail without the change and pass with it.
   - With the change, `cargo fmt --check` and clippy pass, the full `INSTA_UPDATE=no cargo test` passes (2776 passed, 0 failed), and no snapshot changes.
   - The sweep and the random mix give 0 bad of 10080 and 0 bad of 6000.
   - The release budgets were not run on the copy.

   See `ui/details-control-split-vs16-clipped.txt`.

2. [nit] Any view, a VS16 emoji drawn over non-blank cells, in a terminal that advances 2 columns for it (tmux 3.8) — pre-existing and not caused by this task, so no change is needed here. With `model` of `⚠️` x 40 + `END` @ 80x24 in the real binary, the second value row drifts past the overlay's right border and its overflow lands on the next row. `TestBackend` renders it correctly. The cause is the trailing-cell workaround in the ratatui-core 0.1.2 diff together with the crossterm backend's skipped `MoveTo`. The transcript shows the same drift when scrolling with no overlay open. Change: none in this task; consider a separate task if VS16 emoji in the transcript matter. See `ui/details-vs16-tmux-drift-80x24.txt`.
