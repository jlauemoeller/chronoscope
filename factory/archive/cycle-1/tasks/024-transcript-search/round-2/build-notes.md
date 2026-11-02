STATUS: DONE

## Changes
- `src/components/transcript/search.rs`: the row of a match is now found from the match's own source line, not from a count of the query's hits in the rendered rows (`SectionView`). The line of the section's plain text (`section_text`) is looked for among the drawn rows ignoring white space: it must end where a row ends and start at a row start or after a short gutter (numbers, diff marks); wrapped lines span rows; a line occurring twice is found as the n-th occurrence. Labels (`Result:`, `Command:`) are therefore never taken for a hit, a line that is not drawn (markdown URL, cut off by truncation) does not shift the matches that are, and a wrapped hit is placed. For a line markdown draws differently (bullets) the line is looked for by its end from the match on, unless the output is truncated. A match whose line is not drawn while the output is truncated expands the output temporarily; otherwise the section is revealed. One layout per jump when nothing changes (down from up to three).
- Folding takes the search's expansion with it: `fold_command_on_temp` / `forget_temp_unfold(folding)` clear the expansion for `fold`, `Enter` on a section the search only expanded, and fold-all; `unfold` on such a section keeps the state temporary; unfold-all and `Enter`/`unfold` on a temporarily unfolded section make it explicit.
- `goto_match` skips all matches of an unlisted section (no 1000-try cap); `first_of` and `MAX_SKIPPED` removed.
- Nits: module docs say the first `Enter` on a search-unfolded section looks inert; `Msg::PromptChanged(_)`; removed `SearchQuery::is_match`/`is_case_sensitive`; comment on the double `refresh()` in `reveal_match`.
- Not done (nits): labels are still highlighted when the query hits them (per-row highlighting, as the planner note says), and a hit wrapped over two rows is not highlighted.

## Self-check
- Review (code 1, feature 6): expansion left behind after folding — `folding_the_section_drops_the_expansion_the_search_made`, `fold_all_drops_the_expansion_the_search_made`, `enter_on_a_section_the_search_only_expanded_folds_it_and_the_expansion_goes`, `unfold_all_keeps_what_the_search_opened`.
- Review (feature 1, ui 1): label hits, beyond-limit hits, hidden/wrapped hits at 80x17 — `a_query_that_a_label_also_contains_finds_the_real_match_beyond_the_limit` (`result`, `command`), `a_label_is_not_taken_for_a_match_within_the_limit_either`, `a_hit_that_is_not_drawn_does_not_shift_the_ones_after_it`, `a_hit_that_wraps_across_rows_does_not_shift_the_ones_after_it`, `the_same_line_twice_finds_each_occurrence`.
- Review (code 5): `a_thousand_matches_in_a_hidden_section_do_not_stop_the_stepping`.
- Review (feature 2): `printable_keys_go_into_the_prompt_and_trigger_no_action` now runs a search first and asserts the position stays `1/5`, and that `t` opened nothing.
- Review (ui 4): styled companion `after_n_into_the_folded_tool_call_it_is_unfolded_and_highlighted_120x40_styled` (accepted snapshot).
- All earlier ACs and the five UI snapshots are unchanged and still pass.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1307 lib tests passed, 0 failed; integration tests pass)
