STATUS: DONE

## Changes
- `src/harness/opencode/notice_tests.rs`: new test for the index rebuild in `remove` (see must-fix).
- `src/harness/opencode/assemble.rs`: `RowError` derives only `Deserialize`; `ending_notice` no longer clones `kind` and `part` takes `Option<&str>`; the `replace_row` doc states both the late-gained notice and the removal case.
- `src/components/turn_block/export.rs`: notices are emphasised per line with `emphasized_lines`: indent outside the `_…_`, `_` and `*` escaped. Model note and stats footer keep `_text_`.
- `src/components/turn_block/renderers/{mod,text}.rs`: `NoticeSection` wraps with `word_wrap_unhyphenated` (only notices; the model note and tool rows are unchanged) and trims trailing whitespace. Shared `without_controls` helper.
- `src/transcript_search.rs`: a notice is searched as drawn (trailing whitespace trimmed, control characters other than `\n` as spaces).
- `src/components/transcript/tests/notice.rs`: comment reworded; new tests for export escaping/indent, hyphen non-split with search highlight, control characters in search, trailing whitespace.
- Re-accepted the three snapshots whose wrap changed: `notice_snapshots__error_long_{selected,dimmed,undimmed}_at_80` (break moved from `non-`/`empty` to `must be`/`non-empty`). No other snapshot changed.

## Self-check
- Review (code-review 1): unused `Default` — removed.
- Review (code-review 2): clone in `ending_notice` — replaced by a `match`; `part` takes `Option<&str>`.
- Review (code-review 3): `replace_row` doc — updated.
- Review (code-review 4 / feature 2 / ui 3): export emphasis — `emphasized_lines`; `an_exported_notice_keeps_its_indent_outside_the_emphasis_and_escapes_markers`, `a_notice_of_several_paragraphs_is_emphasised_per_line`.
- Review (code-review 5 / feature 3): search test comment — reworded.
- Review (feature 1, must-fix): `notice_tests::a_row_pushed_after_a_removed_notice_replaces_the_item_it_names_not_a_shifted_one`; verified it fails with the rebuild in `remove` deleted and passes with it.
- Review (ui 1): hyphen wrap — `word_wrap_unhyphenated` for notices only; `a_hyphenated_word_is_not_split_across_rows_so_a_search_highlights_it`.
- Review (ui 2): control characters — `section_text`; `a_control_character_in_a_notice_is_searched_as_the_space_it_is_drawn_as`.
- Review (ui 4 / feature 2): trailing newline — trimmed in `NoticeSection` and `section_text`; `trailing_whitespace_of_a_notice_draws_no_extra_marked_row` (also checks copy has no trailing newline).
- All round-1 ACs and UI states still covered as in `round-1/build-notes.md`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (0 failed)
- cargo test --release -- --ignored budgets --nocapture: within budget (full parse worst ~284 ms of 2000, frames worst 3.2 ms of 16)
