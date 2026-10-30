STATUS: DONE

## Changes
Only `SectionView`/`row_of` in `src/components/transcript/search.rs` (plus tests) changed, as the rework note asks.
- **Strict gutter.** `GUTTER` (12 bytes of anything) is gone. A source line counts as drawn at a place only if it ends where a row ends and the part of the first row in front of it holds only `is_gutter` characters: ASCII digits, `.`, `+`, `-`, `•`, `│` and the fold markers `▶ ▼` (checked against the Read, Edit/Write and `markdown/layout.rs` renderers). Letters and other text are never accepted. `squash` also drops the `→` of Claude-numbered Read lines, which the renderer replaces by a gutter.
- **One rule on both sides.** `earlier` counts the earlier source lines by the same predicate (`Source::is_drawing_of`: gutter characters followed by this line), so the n-th drawing is the n-th source occurrence. Without truncation a line drawn fewer times than written (diff context) falls back to its last drawing. Markdown line markers (`- `, `1. `, `> `, `#`) are stripped from the source line before comparing, since the renderer draws them as `•`, `1.`, `│` or not at all. The tail fallback (`nth_ending`) is removed.
- **Anchored fallback** replaces the section top: for a line that is not found, the nearest earlier source line that is placed (looking back at most 200 lines) anchors the search; the query is looked for in the drawn rows after it (labels like `Result:` skipped, same smart-case rule via `SearchQuery`), and the j-th occurrence is taken, j = matches between the anchor line and this match (including earlier ones on the same line). `None` (section top) only if no such occurrence is drawn. When the output is truncated and the line is not found, the caller still expands and retries.
- **Once per section:** the source text is materialised, split and squashed once per jump (`Source`), and shared by the up to three `SectionView`s of a jump (also addresses the review nits on re-reading lazy output and re-splitting).

## Self-check
- Review (approver, code 1, feature 1): `a_line_that_ends_an_earlier_one_beyond_the_limit_is_expanded_and_shown`, `..._within_the_limit_is_shown`, `a_longer_earlier_line_with_a_prefix_beyond_the_limit_is_expanded_and_shown`, `..._within_the_limit_is_shown` (80x17; position `2/2`, expansion, hit row on screen and highlighted).
- Review (UI 1): `a_hit_before_a_code_span_is_scrolled_into_view`, `a_hit_before_bold_text_...`, `a_hit_before_a_link_...`, `a_hit_before_a_code_span_further_on_the_line_...`, `a_hit_in_a_multi_line_string_of_a_generic_tool_input_is_scrolled_into_view`, `two_hits_with_code_spans_each_land_on_their_own_row` (guards the j-th count).
- Other renderers still land on their own row: `a_hit_in_numbered_read_output_lands_on_its_own_row`, `a_hit_on_a_plus_line_of_an_edit_diff_lands_on_its_own_row`, `a_hit_on_a_markdown_bullet_lands_on_its_own_row`, `a_hit_on_a_quote_line_lands_on_its_own_row`, `a_hit_on_a_numbered_list_line_lands_on_its_own_row`.
- The listed round-2 tests and the five UI snapshots plus the styled companion are unchanged and green.
- Dropped as instructed: content-only/wrapped highlighting, layout provenance.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (1322 lib tests, 0 failed; integration tests pass)
