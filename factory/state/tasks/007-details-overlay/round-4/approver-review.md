## Rework note

This note replaces the changes suggested in the round-3 reviews. It measures a value's graphemes the way the buffer draws them, so that the end of a value is never lost, and it adds a test for the overlay's size at opening. Nothing else changes.

**1. Measure by grapheme, in `src/components/details_overlay/content.rs`.**

Replace the imports at the top with these. `UnicodeWidthChar` goes away, and `UnicodeWidthStr` stays for the labels:

```rust
use ratatui::buffer::CellWidth;
use ratatui::layout::Size;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
```

Use `cell_width`, not `UnicodeWidthStr::width`, for the graphemes. It is what the paragraph and the buffer draw with, and the two differ for a halfwidth katakana sound mark.

Replace `columns` and `char_wrap`, with their doc comments, with:

```rust
/// The graphemes of `text` that are drawn, with the columns each takes, as the
/// buffer does it: it drops those with a control character and draws the rest
/// whole.
fn drawn_graphemes(text: &str) -> impl Iterator<Item = (&str, usize)> {
    text.graphemes(true)
        .filter(|g| !g.contains(char::is_control))
        .map(|g| (g, usize::from(g.cell_width())))
}

/// The columns `text` takes, counted as [`char_wrap`] counts them.
fn columns(text: &str) -> usize {
    drawn_graphemes(text).map(|(_, cols)| cols).sum()
}

/// `text` cut into pieces at most `width` columns wide, at any grapheme
/// (paths and long tokens are never hidden, SPEC §5.5). Always at least one
/// piece. A grapheme wider than `width` gets a piece of its own. A space
/// the cut falls on is dropped, so a piece never starts with it and the
/// text stays under the value column.
fn char_wrap(text: &str, width: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut used = 0;
    for (grapheme, cols) in drawn_graphemes(text) {
        if used + cols > width && !current.is_empty() {
            pieces.push(std::mem::take(&mut current));
            used = 0;
            if grapheme == " " {
                continue;
            }
        }
        current.push_str(grapheme);
        used += cols;
    }
    // Only spaces after the last cut are not a row.
    if pieces.is_empty() || !current.trim().is_empty() {
        pieces.push(current);
    }
    pieces
}
```

Add this test to the `tests` module of the same file, after `wide_characters_wrap_by_columns`:

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

**2. Test what is drawn, in `src/app/browser_tests/details.rs`.**

Split the helper in the resizing section, so that `long_model_turn` is:

```rust
/// An Assistant turn whose model name is 110 columns wide, so its details
/// are wider than a small screen and wrap there.
fn long_model_turn() -> Arc<Turn> {
    turn_with_model(format!("claude-{}", "m".repeat(103)))
}

/// An Assistant turn whose model name is `model`.
fn turn_with_model(model: String) -> Arc<Turn> {
```

The rest of the old body becomes the body of `turn_with_model`, with its assignment changed to `assistant.model = model;`.

Add this test before `a_wider_screen_gives_the_overlay_the_width_its_rows_want`:

```rust
#[test]
fn a_value_is_drawn_to_its_end_however_its_graphemes_are_measured() {
    // A sign with a variation selector is wider than its characters, and the
    // buffer gives a halfwidth sound mark cells of its own.
    let values = [
        format!("{}END", "\u{26a0}\u{fe0f}".repeat(40)),
        format!("{}END", "\u{ff76}\u{ff9e}".repeat(7)),
    ];
    for value in values {
        for (width, height) in [(80, 24), (120, 40), (200, 30)] {
            let mut rig = rig_with(theme("default"), width, height);
            rig.open_with(vec![turn_with_model(value.clone())]);
            rig.open_details();
            let rows = overlay_rows(&mut rig, width, height).join("\n");
            assert!(rows.contains("END"), "{width}x{height}\n{rows}");
        }
    }
}
```

**3. Test the size at opening, in the same file.** In `a_wider_screen_gives_the_overlay_the_width_its_rows_want`, replace the comment and the `assert_eq!(rect_of(&rig).width, 76);` line under `open_long_model_at(80, 24)` with the following. The rest of the test stays:

```rust
// Capped by the screen: 76 columns, the model wrapped. Read before anything
// draws, because `overlay_rows` sends `Msg::Resized`, which asks the overlay
// for its size again.
let at_open = rect_of(&rig);
assert_eq!(at_open.width, 76);
assert_eq!(at_open.height, 14, "as high as the wrapped rows");
```

**Leave as is.**
- The behaviour of a value of only spaces (ui-review finding 2). It is dropped on purpose: nothing is hidden, and the suggested rule would also remove spaces between two parts of a value, which breaks the rule that a wrapped value loses only the spaces at the cuts.
- `src/markdown/wrap.rs` and every other wrapping code outside the overlay.
- `open_details`, the chord handling, all snapshots and every other existing test.

**Done when.**
- The 2 new tests pass, and every other existing test passes unchanged, including `what_is_not_drawn_does_not_widen_the_overlay`.
- Each of these temporary edits fails the named tests. Make each edit, check it, restore the code, and say so in the build notes:
  - `columns` and `char_wrap` measuring per `char` with `ch.width().unwrap_or(0)`, as they do now: `a_grapheme_is_measured_as_it_is_drawn` and `a_value_is_drawn_to_its_end_however_its_graphemes_are_measured`.
  - `g.width()` in place of `usize::from(g.cell_width())` in `drawn_graphemes`: `a_value_is_drawn_to_its_end_however_its_graphemes_are_measured`.
  - `overlay.desired_size(u16::MAX)` in place of `overlay.desired_size(max_width)` in `open_details` in `src/app/details.rs`: `a_wider_screen_gives_the_overlay_the_width_its_rows_want`.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass, with 2776 tests passing (2774 plus the 2 new ones).
- `INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture` stays within budget.
- No `.snap` file changes in this round, and no `.snap.new` file is left behind. A changed snapshot is a failure, so do not accept one.
