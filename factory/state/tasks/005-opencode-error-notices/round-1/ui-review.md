VERDICT: PASS

## Summary
Notices match the task's `UI` section: position in the turn's order, two-space indent with no marker, word wrap, level colours, emphasis and `search_match` are all correct, and all 24 required snapshots exist and are new. `INSTA_UPDATE=no cargo test`, `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass, and no existing snapshot changed. There are four nits, found with synthetic text only; I did not run the budgets gate or read the real rows in the local OpenCode database.

## States checked
- Error, short, selected @ 80, 120 — snapshot `error_short_selected_at_{80,120}` — ok (`destructive` bold, `▒` in `primary`)
- Error, short, unselected, dim on @ 80, 120 — snapshot `error_short_dimmed_at_{80,120}` — ok (`muted_foreground`)
- Error, short, unselected, dim off @ 80, 120 — snapshot `error_short_undimmed_at_{80,120}` — ok (`destructive`, not bold)
- Error, wrapping, selected @ 80, 120 — snapshot `error_long_selected_at_{80,120}` — ok (`▒` on every row of the section; see finding 1 for the break position)
- Error, wrapping, unselected, dim on @ 80, 120 — snapshot `error_long_dimmed_at_{80,120}` — ok
- Error, wrapping, unselected, dim off @ 80, 120 — snapshot `error_long_undimmed_at_{80,120}` — ok
- Warning, short, selected @ 80, 120 — snapshot `warning_short_selected_at_{80,120}` — ok (`muted_foreground` bold, `▒` in `primary`)
- Warning, short, unselected, dim on @ 80, 120 — snapshot `warning_short_dimmed_at_{80,120}` — ok
- Warning, short, unselected, dim off @ 80, 120 — snapshot `warning_short_undimmed_at_{80,120}` — ok (`muted_foreground`; neighbouring text in `foreground`)
- Warning, wrapping, selected @ 80, 120 — snapshot `warning_long_selected_at_{80,120}` — ok
- Warning, wrapping, unselected, dim on @ 80, 120 — snapshot `warning_long_dimmed_at_{80,120}` — ok
- Warning, wrapping, unselected, dim off @ 80, 120 — snapshot `warning_long_undimmed_at_{80,120}` — ok
- Both levels, selected, dim off @ 60 — rendered in scratch crate — ok
- Error, long, selected @ block widths 9, 10, 12, 16, 20, 30, 40, 60, 90, 200 — rendered in scratch crate — ok (border intact, height matches rows)
- Block widths 1 to 8 — rendered in scratch crate — ok for this task (no room for text; a text-only block at width 1 shows the same height mismatch, so it is not specific to notices)
- Transcript with a selected notice, dim on and off @ 120x30, 80x24, 40x12, 20x8, 10x6 — rendered in scratch crate — ok
- Several notices in a row, as in the illustration @ 72 — rendered in scratch crate — ok (one blank row between them)
- Turn holding only a notice @ 60 — rendered in scratch crate — ok
- Blank notice text @ 30, 60, 80 — rendered in scratch crate — ok (no section)
- Long unbroken URL, CJK, emoji, combining marks @ 30, 80 — rendered in scratch crate — ok (right border intact)
- Markdown-like text @ 30, 80 — rendered in scratch crate — ok (drawn literally)
- Escape sequences and bell @ 30, 80 — rendered in scratch crate — ok (control characters become spaces, nothing reaches the terminal)
- Multi-line text, CRLF, tabs @ 30, 80 — rendered in scratch crate — issue (findings 2 and 4)
- Search highlight in selected and unselected notices @ 60 — rendered in scratch crate — ok (`search_match` fg on bg, `n` moves 1/2 to 2/2)
- Search with a match that crosses a row @ 80 — rendered in scratch crate — issue (finding 1)
- Copy and save (`y`, `Y`, `w`, `W`) on a wrapped notice @ 40x24 — rendered in scratch crate — ok (no added breaks, file word `error`; `notice` is covered by `w_saves_a_notice_under_error_or_notice`)
- Export (`e`, `E`) @ 40x24 — rendered in scratch crate — issue (finding 3 for indented lines; single-line notices ok)
- Live update: selected Warning replaced by Error in place @ 60x16 — rendered in scratch crate — ok (selection kept, colour follows the new level)
- Live update: selected notice removed @ 60x16 — rendered in scratch crate — ok (selection moves to the next section)

## Findings
1. [nit] Notice, wrapping, block width 80 and narrower — `NoticeSection` wraps with `word_wrap`, which splits at hyphens, while assistant text and thinking do not. The fixture draws `non-` / `empty` at 80, and narrower blocks draw `provider.invalid-` / `request:`. Searching `non-empty` then selects the notice and shows `1/1` but draws no `search_match`, because matches are looked for per row; text and thinking with the same string do highlight it. The `UI` section does not pin hyphen breaks and the per-row limit is documented in `widget.rs`, so this is cosmetic. Change: wrap notices without splitting at hyphens, using `textwrap::Options::new(width).word_splitter(WordSplitter::NoHyphenation)` in `NoticeSection` only, so the model note and tool rows do not move, and re-accept the wrapped notice snapshots whose breaks change. See `ui/notice-hyphen-wrap-80.txt`.
2. [nit] Notice with a tab or other control character, search, block width 80 — the renderer turns the character into a space but `section_text` returns the raw text, so the query `alpha beta` highlights the drawn row while the position says `0/0`, and `n` / `N` cannot reach it. The folded tool description already behaves the same way, so this follows an existing pattern. Change: in the `Notice` arm of `section_text`, replace control characters other than `\n` with spaces, as `word_wrap` does. See `ui/notice-search-tab-80.txt`.
3. [nit] Export of a notice with an indented line (`e`, `E`) — the line is written as `_  two_with_underscore *star* tab_`, which CommonMark does not emphasise, so the underscores show literally. Change: in the per-line emphasis in `section_markdown`, put the delimiters around the trimmed line, keeping the indent outside them. See `ui/notice-export-indented.txt`.
4. [nit] Notice whose text ends in a newline, selected, block width 80 — an extra blank row is drawn and carries the `▒` marker. OpenCode rows cannot reach this because `ending_notice` trims each part, but the next task's Claude notices take the message's text, which could end in a newline. Change: trim the end of the text in `NoticeSection`, or when the Claude notice is built. See `ui/notice-trailing-newline-80.txt`.
