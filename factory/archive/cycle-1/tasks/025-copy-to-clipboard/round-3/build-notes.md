STATUS: DONE

## Changes
- `soft_joined` (`src/components/turn_block/plain.rs`) joins a wrapped row with the whitespace the source has at that spot (`locate` returns the skipped gap): nothing where the screen broke a word (URL, path, code line, CJK), the source's own whitespace elsewhere. A joined line is exactly the matching part of the logical line plus a trailing `…` where the fold cut it.
- Rows of a quote are joined under one bar: if the logical line starts with `│ ` bars (`quote_prefix`) and the continuation row has the same prefix, it is stripped before matching and appending. Rows that match nothing stay lines of their own.
- Dropped the redundant `.filter(|_| !out.is_empty())`; updated the doc comments of `soft_joined` and `displayed_text` ("whitespace the source has there"). Reworded the `verbatim_lines` doc comment (a `\r\n` ending becomes a plain one), per the round-2 verdict.

## Self-check
- Approver 1 (source whitespace) — `plain::tests::soft_joined_joins_with_the_sources_own_whitespace` (`abc`/`def` -> `abcdef`; `ab`/`cd` against `ab cd` -> `ab cd`), `a_folded_single_token_prompt_is_copied_without_inserted_spaces`, `a_folded_cjk_prompt_is_copied_without_inserted_spaces`, `a_folded_code_block_line_wider_than_the_row_has_no_inserted_space`.
- Approver 2 (quotes) — `plain::tests::soft_joined_joins_the_rows_of_a_quote_under_one_bar` (incl. nested), `a_folded_quoted_prompt_is_one_line_with_one_bar`; the unmatched case `["│ alpha", "│ beta"]` against logical lines without `│` is kept in `soft_joined_matches_rows_to_the_unwrapped_lines`.
- Approver 3 (tidy-ups) — done as above.
- Round-2 reviews: code 1 / feature 1 / ui 1a (inserted space) — fixed; code 3 / ui 1b (quote rows) — fixed; code 4 (redundant filter) — fixed; feature 4 (CRLF doc) — doc reworded. Ignored by the verdict: Read gutter under `Y`, whole description of a folded tool call, the 80-cell rule, the synchronous lazy read, scratchpad collision.
- Existing folded-prompt tests, including `browser_tests::copy::y_copies_the_selected_section_as_displayed`, pass unchanged; the four toast snapshots pass with `INSTA_UPDATE=no`, no `.snap.new` files.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1421 lib tests, 0 failed; other targets pass)
