VERDICT: PASS

## Summary
Round 3 does what the round-2 rework note asked. `soft_joined` now joins wrapped rows with the source's own whitespace through the new `locate` helper, which returns the skipped gap: nothing where the screen broke a word, and the source's exact whitespace elsewhere. Rows of a quote are joined under one `│ ` prefix via `quote_prefix`. The redundant filter is gone, and the doc comments are updated, including `verbatim_lines` (CRLF to LF). The new tests are effective. I mutated the code in a scratch copy: restoring `push(' ')` fails 4 of the new tests, and removing the quote-prefix strip fails both quote tests. Edge probes at widths 40, 60 and 80 on folded prompts (nested quotes, lists, numbered items, headings, runs of several spaces, CJK with one space, an over-wide code line, blank-line gaps) all copy exactly the matching prefix of the logical line plus `…`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (lib 1421 passed, 1 ignored; all other targets pass; working tree clean, no `.snap.new` files)

## Findings
1. [nit] `src/components/turn_block/plain.rs:36-42` (`displayed_text` doc comment): the edit leaves a ragged wrap. `source has there); without` is followed by a short `/// it, summaries are cut…` line. Change: reflow the paragraph so the lines are filled evenly.
2. [nit] `src/components/turn_block/plain.rs:121-131` (`soft_joined`): the `continued` match builds a 5-tuple `(last, line, end, gap, body)` only to unpack it right away. Change (optional, for readability): compute `(line, end, gap, body)` and handle the join inside the `(Some(..), Some(last))` arm, or use a small named struct.
3. [nit] `src/components/turn_block/plain.rs:163-170` (`quote_prefix`): a quote whose own text starts with a literal `│ ` (for example `> │ x …`) is read as a nested quote. Its continuation rows carry only one bar, so they are not stripped or joined and stay separate lines, just as the screen shows them. This is rare and harmless, because it falls back to the screen rows. Change (optional): strip the longest run of `│ ` bars on the row that is a prefix of the source's bars, rather than requiring the full source prefix.
