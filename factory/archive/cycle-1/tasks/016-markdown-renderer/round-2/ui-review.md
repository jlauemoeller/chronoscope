VERDICT: PASS

## Summary
Round 2 fixes every round-1 UI and code-review point that affects rendering:
- Quote bars, table chrome, rules, code spans, and the indent and padding spans now all derive from `base`.
- Tabs stay visible as spaces.
- Empty list items keep their marker, and short table rows have no dangling separator.
- The widget clips to the buffer instead of panicking.

The four snapshots the `UI` section requires still match it, and a new distinct-colour code snapshot was added. I checked them, then rendered the widget again in the scratch crate at many widths and with edge-case documents. What is left is nits only.

Test gate, run here with a separate `CARGO_TARGET_DIR`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` all pass (lib: 725 passed, 1 ignored; every integration suite passes). No `.snap.new` files were created, and the working tree is clean.

## States checked
- Sample document @ 80 (text) — snapshot `sample_document_at_80_columns_text` — ok. No `#` or backticks, `• ` bullets with a 2-cell nest indent, `│ ` quote bar, ` │ ` / `─┼─` table, code indented 2 cells, full-width rule, no trailing blank line.
- Sample document @ 80 (styled) — snapshot `sample_document_at_80_columns_styled` — ok. H1 is BOLD|UNDERLINED and H2 BOLD. Em is ITALIC, strong BOLD, and link text UNDERLINED. Chrome is `muted_foreground`, and code tokens use `syntax.*`. The nest indents, table padding and code indent now carry the base foreground instead of `Reset`, which is the intended round-2 change.
- Sample document @ 40 (text): prose wraps, code char-wraps, table truncates with `…` — snapshot `sample_document_at_40_columns_wraps_and_truncates` — ok (unchanged since round 1).
- Code block only, `rust` @ 60 (styled) — snapshot `code_block_only_at_60_columns_styled` — ok. `fn`/`let`/`if`/`return`/`as` use `syntax.keyword`, `"the answer"` uses `syntax.string`, the `//` comment uses `syntax.comment` and `42` uses `syntax.number`.
- Code block with distinct syntax colours @ 60 (styled) — snapshot `code_block_with_distinct_syntax_colours_at_60_columns_styled` — ok, with nit 3.
- Sample @ 80, 40, 24, 16, 10 — rendered in scratch crate — ok. Word wrap, hanging indents, widest-first truncation and code char-wrap all work. Nit 4 applies at 24 and 16.
- Sample @ 6, 5, 3, 1 — rendered in scratch crate — no panic. The round-1 narrow-width clipping is still there (nit 5).
- Sample @ 40x10 and 80x24 (cut off by height) — rendered in scratch crate — ok.
- Base with a modifier (`thinking` + DIM) @ 12 (styled): quote, table, code, rule — rendered in scratch crate — ok. Every non-blank span keeps DIM, and the header cells are BOLD|DIM.
- Base with a background @ 20 (styled) — rendered in scratch crate — gaps inside lines are fixed. Nit 2 remains (blank lines).
- Tabs in prose, inline code and table cells @ 30 — rendered in scratch crate — ok (`Text with tab    here`, `c    d`).
- Empty bullet and ordered items @ 20 — rendered in scratch crate — ok (`•` and `1.` are kept).
- Short and partly empty table rows @ 20 — rendered in scratch crate — ok. A row ends at its last non-empty cell, and inner empty cells keep their separators.
- CRLF source @ 20 — rendered in scratch crate — ok.
- ANSI/BEL control characters in a fenced block @ 20 — rendered in scratch crate — ok as documented (ESC and BEL are dropped, `[31m` is shown as text).
- `char_wrap` at width 0 and with control characters — scratch crate — ok (no lines; control characters are dropped).
- Widget area past the buffer: 10x5 on 10x2, and Rect(4,1,20,5) on 10x3 — rendered in scratch crate — no panic. Nit 1 applies.
- Code + inline code with each of the 7 built-in themes @ 40 (styled) — rendered in scratch crate — ok (colours follow the theme; `terminal` uses ANSI colours).
- Sample with `thinking` base @ 60 (styled) — rendered in scratch crate — ok.

## Findings
1. [nit] `Markdown` widget, area partly outside the buffer, e.g. Rect(4,1,20,5) on a 10x3 buffer. `render` now calls `area.intersection(buf.area)` *before* `render_markdown`, so a horizontal clip changes the layout: the text re-wraps at the clipped width of 6 (`one` / `two`) instead of being laid out at 20 and clipped (`one tw` / `five s`). A layout cache that measured heights at the block's real width (§9.2) can then disagree with what is drawn for a block that is partly off-screen. See `ui/widget-offset-area-rewraps-10x3.txt`. Change: call `render_markdown` with the original `area.width`, then draw only the rows and columns that fall inside `area.intersection(buf.area)`. For example, skip rows with `y >= buf.area.bottom()`, and pass the clipped width to `set_line`.
2. [nit] `Markdown` widget, a base with a background, @ 20 (styled). The blank line inside a fenced code block, and the blank separator lines between blocks, are `Line::default()`, so they render `Reset/Reset`. This leaves a hole in the base background in the middle of a code block. It cannot happen today, because text tokens have no `bg`; this is the remainder of round-1 nit 2. See row 15 (and rows 1, 4, 6, 8, 13, 18) in `ui/bg-base-code-blank-line-20.txt`. Change: in `layout::code_lines`, emit `Line::default().style(base)` (or a `Span::styled(indent, base)` line) for an empty code line. Optionally give the block separators `base` as well.
3. [nit] `code_block_with_distinct_syntax_colours_at_60_columns_styled`. Compared with the default-theme code snapshot, only the `42` run changes colour. `CODE_ONLY` has no span that maps to `syntax.type`: `u32` and `str` are `storage.type`, which the planner maps to keyword. So keyword vs type is still not visible in any snapshot, and neither is `command` vs prose, since there is no inline code. This only half covers round-1 nit 3. Change (optional): give the distinct-theme snapshot a fixture with a line that yields `entity.name.type`/`support.type` (e.g. `struct Answer;` or `impl Answer {}`), and add a sample-document styled snapshot under `distinct_theme()` so inline code shows the `command` colour.
4. [nit] `Markdown` widget, sample table @ 24 and @ 16. A truncated cell keeps a trailing space before the ellipsis (`Lays …` @ 24, `in …` @ 16), so it reads like a dropped word rather than a cut. See `ui/sample-24-16-table-ellipsis-space.txt`. Change: in `table::truncate`, trim trailing whitespace from the kept text before appending `…`, giving `Lays…` and `in…`.
5. [nit] `Markdown` widget, sample document @ 6 and narrower (carried over from round-1 nit 1, now documented in the `render_markdown` doc comment). At 6 the table's third column is clipped off entirely, and at 4 the nested-list text is drawn past the area. See `../round-1/ui/sample-4-nested-list-hidden.txt` and `../round-1/ui/sample-8-table-last-column-clipped.txt`. These widths are unrealistic inside a transcript block. Change (optional): shrink or drop prefixes and trailing table columns so that at least one cell of content stays visible.
