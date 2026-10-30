VERDICT: PASS

## Summary
Round 3 changed only `soft_joined`, the function that joins the rows of a folded prompt back into logical lines, plus some doc comments. The toast UI and its four snapshots are unchanged since round 2, and they still match the `UI` section. Both parts of round-2 finding 1 are fixed:
- A row is joined with the whitespace the source has at the break, and with nothing where the screen broke inside a word.
- A wrapped quote is copied as one line with its `│ ` bar once.

I rendered the cases from the round-2 finding and many new edge cases at 60x20, 80x24 and 120x40. Each copy matched the screen rows. The gate passes with `INSTA_UPDATE=no`, and there are no `.snap.new` files.

## States checked
- Toast `Copied 42 lines` over the browser @ 80x24: snapshot `copy__copied_toast_80x24_text`. ok, unchanged in this round.
- Toast `Copied 42 lines` @ 120x40: snapshots `copy__copied_toast_120x40_text` and `copy__copied_toast_120x40_styled`. ok, unchanged.
- Toast `Copy failed: no clipboard available` @ 80x24: snapshot `copy__copy_failed_toast_80x24_text`. ok, unchanged.
- Gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` all pass (1421 lib tests). No `.snap.new` files. ok.
- All of the following were rendered in the scratch crate through `App::view`, with a folded prompt selected and `y` pressed. Each copy was compared with the screen rows.
  - Folded one-token URL (`https://example.com/` + `abcdefghij`×20) @ 60x20, 80x24 and 120x40: ok. The copy has no inserted space, is a prefix of the source plus `…`, and gives `Copied 1 line`. At 120x40, where the URL fits in two rows, the copy is the whole URL without `…`, as on screen.
  - Folded CJK paragraph without spaces @ 60x20, 80x24 and 120x40: ok. There is no inserted space at any size.
  - Folded quote longer than two rows (`quote3`) @ 60x20, 80x24 and 120x40: ok. The copy is one line, `│ ` appears once, and it ends in `…` where the screen cuts.
  - Folded nested quote (`│ │ `) @ 60x20, 80x24 and 120x40: ok. The copy is one line and the bars appear once.
  - Folded quote that holds a long URL, or CJK text, broken mid-word @ all three sizes: ok. One bar, and no inserted space.
  - Folded quoted list item (`│ • …`, continuation row `│   …`) @ all three sizes: ok. One line, with the bar and bullet once.
  - Folded list item whose URL wraps onto an indented row (`listurl`) @ all three sizes: ok. The copy is `• see https://…`, with the source space between `see` and the URL.
  - Folded `Fetch <long URL> please` @ all three sizes: ok. The break falls at a real space, and the copy keeps that one space.
  - Folded code block whose line is wider than the row, containing double spaces (`codewide`, `codespaces`) @ all three sizes: ok. The code indent and the source's double spaces are kept, and nothing is inserted at the break. The `ab …` ending matches the screen row.
  - Folded paragraph followed by a quote (`paraquote`) @ all three sizes: ok. The copy is two lines, `Short intro.` and `│ quoted …`, the same as the screen.
  - Folded quote of two source lines that render as one row (`twoquote`): ok. The copy is one line, as on screen.
  - Folded emoji run and tab-separated words @ all three sizes: ok. Wide characters and the tabs, which are expanded on screen, copy as drawn.

## Findings
1. [nit] `displayed_text` doc comment in `src/components/turn_block/plain.rs`. The reworded comment was not reflowed, so one line stops early: `…the whitespace the\n/// source has there); without\n/// it, summaries are cut…`. Change: reflow the paragraph. This is cosmetic only.
