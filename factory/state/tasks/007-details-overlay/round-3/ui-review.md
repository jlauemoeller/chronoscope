VERDICT: FAIL

## Summary
The round-2 must-fix is fixed: `details` chords that use an overlay key (`z j`, `z k`, `j i` and the others) now open and close the overlay with the same keys, and the carriage-return nit is gone. One must-fix is new: a value with emoji-presentation graphemes such as `⚠️` is not wrapped and its end is never drawn, at any screen size. Round 3's `widest_row` change turned this from an 80x24 problem into one at every size. `cargo fmt --check`, clippy, `cargo test` (2774 passed) and the release budgets all pass with `INSTA_UPDATE=no`.

## States checked
- User prompt, assistant text, thinking, tool call, `model → …` note, stats footer, compaction, system, notice @ 80x24 and 120x40, `default` and `terminal` — snapshots `<kind>_<size>_<theme>` (36) — ok, reviewed in round 2, none changed since
- Content taller than the screen @ 80x24, top and end, both themes — snapshots `tall_80x24_{top,end}_{default,terminal}` — ok
- Long value wrapped to the value column @ 80x24 — snapshot `wrapped_value_80x24_default` — ok
- Unavailable group, one muted row @ 80x24 — snapshot `unavailable_80x24_default` — ok
- Overlay over the browser @ 80x24 and 120x40 — snapshots `the_details_over_the_browser_{80x24,120x40}_text` — ok
- Snapshot changes this round (`git diff cb674fb HEAD -- '*.snap'`) — none, and no `.snap.new` files — ok
- Heading, label, value, muted row, `ring` border and indicator styles @ 60x24, `default` — rendered in scratch crate with styles — ok
- Chords ending in an overlay key, `z j`, `z k`, `z down`, `z up`, `z pgdn`, `z pgup`, `z esc` — scratch crate: the transcript resolver gives `Pending`, `Action(Details)`; the overlay gives `None`, `CloseDetails` with the offset unchanged — ok
- Chords starting with an overlay key, `j i`, `k i`, `down i`, `up i`, `pgdn i`, `pgup i` — scratch crate: the first key scrolls (or does nothing at the top), then `i` closes — ok
- Other chords, `z i`, `z z`, `i i`, `i j`, `j k`, `j j`, `down down` — scratch crate — ok. With `j j` the first `j` scrolls and the second closes, which follows from the rework note
- Single keys: `details = ["j"]`, `["k"]`, `["down"]` and `["pgdn"]` act as overlay keys, `["esc"]` closes, `["x"]` closes; `?`, `q` and `t` are rejected with a warning and the default is kept — scratch crate — ok, the planner's precedence
- `details = ["z j"]`, `["z k"]`, `["j i"]`: open and close with the same chord @ 80x24 — real binary in tmux — ok
- `details = ["z j"]`: after closing, `j` moves the selection in the transcript and the overlay stays closed, so the focus returned — real binary in tmux — ok
- `details = ["j i"]`: a lone `i` leaves it open, `j` then `Esc` closes, a lone `i` in the transcript does nothing — real binary in tmux — ok
- `details = ["z j"]`: help overlay shows `z j  Show section details` @ 120x40 — real binary in tmux — ok
- Default keys: `i` opens; `q`, `t`, `?`, `Ctrl-o`, `y`, `Y`, `w`, `W` leave it open; `i` and `Esc` close; `Ctrl-c` quits @ 80x24 — real binary in tmux — ok
- Wanted size against what is drawn, one code path on opening: screen widths 60–170, value lengths 0–259 (plain, with spaces, trailing spaces, wide characters), 5624 cases: all text drawn, no indicator, one blank row at the bottom, width within limits — scratch crate — ok
- Resize while open: 80x24 → 160x40 → 80x40 → 60x20 → 50x15 → 120x40 — scratch crate — ok
- Scrolled to the end, then resized: 80x24 → 100x60 → 60x20 → 80x24 — scratch crate — ok
- Opening below the minimum size: 50x15, 59x20 and 60x19 stay closed; 60x20 opens — scratch crate — ok
- Transparent theme @ 80x24 — scratch crate — ok
- Value with many carriage returns @ 120x20 — scratch crate — ok, now 41 wide (round-2 finding 2 fixed)
- Space at a cut, trailing spaces, CRLF, empty value, newline-only value @ 60x24, 80x24 and 120x24 — scratch crate — ok, except the all-spaces case in finding 2
- Values of `😀`, flags and CJK — scratch crate — ok
- Values of `⚠️` and keycaps @ 80x24, 120x40 and 200x30 — scratch crate and real binary in tmux — issue, finding 1
- Values of `👩‍💻` (ZWJ) @ 80x24, 120x40 and 200x30 — scratch crate — issue, finding 1
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `INSTA_UPDATE=no cargo test` (2774 passed), `cargo test --release -- --ignored budgets --nocapture` (worst first content 2.5 ms, full parse 304.0 ms, frame 3.26 ms, peak memory 0.58x) — ok

## Findings
1. [must-fix] `DetailsOverlay`, any section, 80x24, 120x40 and 200x30, a value with graphemes whose drawn width differs from the sum of their characters' widths — the value is measured wrongly, so it is not wrapped and its end is lost. This contradicts the `UI` section ("A long value wraps inside the overlay, aligned to the value column") and spec/SPEC.md §5.5 (paths and long tokens are never hidden).
   - `columns` and `char_wrap` in `src/components/details_overlay/content.rs` add up per-character widths, while ratatui draws whole graphemes. `⚠️` and `1️⃣` count as 1 column and are drawn as 2. `👩‍💻` counts as 4 and is drawn as 2.
   - With `"⚠\u{fe0f}".repeat(40) + "END"` the overlay is 58 wide at every size and draws 22 signs on one row. The other 18 signs and `END` are never drawn, and there is no wrapped row or indicator.
   - With ZWJ graphemes nothing is lost, but the overlay is about twice as wide as its text (178 wide at 200x30 for 83 drawn columns) and the text wraps early.
   - This is a round-3 regression at wide sizes. At cb674fb `widest_row` used `UnicodeWidthStr::width`, so `END` was drawn at 120x40 and 200x30 and lost only at 80x24. Now it is lost at all three.
   - It can happen today: `turn.model` and `turn.effort` are free strings from the session file. An assistant turn with this `message.model` shows the clipped row in the real binary at 120x40. Tasks 008 and 009 will add more raw strings, such as `cwd`, `gitBranch` and `error`.

   Change, in `content.rs`, measure and cut by grapheme, the way the buffer draws. Add `use unicode_segmentation::UnicodeSegmentation;` (already a dependency), remove the `UnicodeWidthChar` import, and use:

   ```rust
   /// The graphemes of `text` that are drawn, with the columns each takes: the
   /// buffer drops those with a control character and draws the rest whole.
   fn drawn_graphemes(text: &str) -> impl Iterator<Item = (&str, usize)> {
       text.graphemes(true)
           .filter(|g| !g.contains(char::is_control))
           .map(|g| (g, g.width()))
   }

   /// The columns `text` takes, counted as [`char_wrap`] counts them.
   fn columns(text: &str) -> usize {
       drawn_graphemes(text).map(|(_, cols)| cols).sum()
   }
   ```

   In `char_wrap`, loop with `for (grapheme, cols) in drawn_graphemes(text)`, test `grapheme == " "` at the cut, and use `current.push_str(grapheme)`. The control filter is needed because `UnicodeWidthStr::width` counts a control character as a column, which would break `what_is_not_drawn_does_not_widen_the_overlay`.

   Test to add, next to `wide_characters_wrap_by_columns`:

   ```rust
   #[test]
   fn a_grapheme_is_measured_as_it_is_drawn() {
       let sign = "\u{26a0}\u{fe0f}";
       let value = format!("{}END", sign.repeat(40));
       let wanted = |value: &str| {
           let rows = vec![row("k", value)];
           DetailsContent::new(vec![DetailsGroup::new("G", rows)]).desired_size(u16::MAX)
       };
       // 83 columns are wanted, as 83 "x" want.
       assert_eq!(wanted(&value), wanted(&"x".repeat(83)));
       let pieces = char_wrap(&value, 10);
       assert_eq!(pieces[0], sign.repeat(5));
       assert_eq!(pieces.last().unwrap(), "END");
       let zwj = "\u{1f469}\u{200d}\u{1f4bb}";
       assert_eq!(wanted(&zwj.repeat(40)), wanted(&"x".repeat(80)));
   }
   ```

   Checked on a patched copy of HEAD outside the repo: with this change and test, all 111 tests matching `details` pass with no snapshot change, and `cargo fmt --check` and clippy pass. The full `cargo test` was not run on the copy.

   See `ui/details-vs16-value-clipped.txt`.

2. [nit] `DetailsOverlay`, a value of only spaces longer than the value's width, for example 89 spaces @ 60x24 — `char_wrap` drops spaces only in the last piece, so a piece between two cuts is kept and the row takes two rows at 60x24 but one at 80x24. It is harmless, because nothing is hidden, and such a value is unlikely. Change: in `char_wrap`, push a piece at a cut only if it is the first piece or is not all spaces. See `ui/details-all-spaces-value-60x24.txt`.
