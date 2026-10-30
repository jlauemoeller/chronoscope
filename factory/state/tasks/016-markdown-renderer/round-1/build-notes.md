STATUS: DONE

## Changes
- `Cargo.toml` / `Cargo.lock`: added `unicode-segmentation = "1"` as a direct dependency (it was already in the lock file) so wrapping and truncation never split a grapheme.
- `src/lib.rs`: `pub mod markdown;`.
- `src/markdown/mod.rs`: public API `render_markdown(src, width, base, theme)`, re-exports of `highlight_code`, `char_wrap` and `Markdown`; the `Styles` bundle (base, `command`, muted, heading styles) derived from the base style and theme tokens.
- `src/markdown/blocks.rs`: pulldown-cmark events (tables and strikethrough enabled) to a small block tree (paragraph/heading, code, rule, quote, list, table) with styled inline text; headings, emphasis, strong, strike, links, inline code get their styles here.
- `src/markdown/layout.rs`: block tree to lines. Quote bars, list markers and hanging indents use one prefix mechanism. Code is indented 2, highlighted, char-wrapped. One blank line between blocks, none trailing.
- `src/markdown/prose.rs`: styled inline text and word wrapping with `textwrap` (`Fragment` + `wrap_first_fit`); words wider than the line are broken between graphemes.
- `src/markdown/wrap.rs`: `char_wrap` (and the styled-span variant the code blocks use).
- `src/markdown/highlight.rs`: `highlight_code`; syntect `SyntaxSet` loaded once in a `OnceLock`; scopes mapped to `syntax.*` tokens (innermost scope with a rule wins) with no syntect colour theme.
- `src/markdown/table.rs`: column sizing, widest-first narrowing, `…` truncation, alignment, `─┼─` header rule.
- `src/markdown/widget.rs`: the `Markdown` widget (re-layouts at the area width on every render; `.base(style)`).
- `src/markdown/tests.rs` and `src/markdown/snapshots/*.snap`: unit and snapshot tests.

Notes / decisions:
- In the default theme `syntax.string` and `syntax.number` (both `warning`) and `syntax.keyword` and `syntax.type` (both `accent`) have the same colour, so the code-only snapshot shows them with equal colours. Unit tests use a theme with distinct syntax colours (derived from roles) to prove each mapping.
- `char_wrap`: empty text gives no lines; a trailing newline does not add a line; tabs become four spaces.
- Ordered list markers are padded to the widest marker of the list (`9.  a` / `10. b`); nested content hangs under the item text.
- Below a width of about 3 (prefix width + 1) quote/list/code prefixes can overflow; tables need at least 3 cells per column plus separators. Tests cover widths 3..=40 for prose/code/rule and 9..=100 for the full sample.

## Self-check
- AC: each element per planner notes — `headings_of_levels_one_to_three`, `emphasis_strong_and_strike`, `inline_code_uses_the_command_token_and_shows_no_backticks`, `link_text_is_underlined_and_the_url_is_not_shown`, `nested_bullet_lists_wrap_with_a_hanging_indent`, `ordered_lists_wrap_under_the_item_text`, `block_quote_prefixes_every_line_with_a_muted_bar`, `a_table_has_aligned_columns_and_a_header_rule`, `a_wide_table_truncates_its_widest_cells_with_an_ellipsis`, `horizontal_rule_spans_the_width_in_the_muted_colour`, `fenced_code_is_indented_highlighted_and_not_word_wrapped`, `blocks_are_separated_by_one_blank_line_and_nothing_trails`.
- AC: prose wraps at word boundaries; long word broken; wide chars — `prose_wraps_at_word_boundaries_within_width`, `a_word_longer_than_the_width_is_broken`, `wide_characters_count_two_cells`, `graphemes_are_never_split`, `no_line_is_wider_than_the_width`, `text_and_code_fit_down_to_the_narrowest_width`.
- AC: `char_wrap` exact width, indentation, blank lines — `char_wrap_breaks_at_exactly_the_width`, `char_wrap_ignores_spaces_when_breaking`, `char_wrap_keeps_indentation_and_blank_lines`, `char_wrap_measures_wide_characters_as_two_cells`, `char_wrap_lines_never_exceed_the_width`, `long_code_lines_are_char_wrapped_to_the_remaining_width`.
- AC: highlighting (keyword/string/comment/number, unknown language plain) — `rust_tokens_use_the_syntax_tokens`, `scopes_map_to_tokens_by_the_innermost_match`, `a_comment_keeps_its_punctuation_and_a_string_its_quotes`, `an_unknown_language_is_plain`, `an_unknown_code_language_uses_the_plain_token`, `language_names_and_extensions_both_select_a_syntax`, `multi_line_constructs_keep_their_state_across_lines`.
- AC: theme changes colours, none hard-coded — `changing_the_theme_changes_the_colours`, `every_colour_comes_from_the_theme` (every colour in the output is a role/token colour of the theme, over four built-in themes), `the_base_style_is_the_starting_point_of_every_element`; no `Color::`/`Rgb` literal in non-test module code (grepped).
- AC: deterministic, `SyntaxSet` loaded once — `rendering_is_deterministic`, `the_syntax_set_is_loaded_once` (pointer equality and a load counter == 1).
- AC: widget — `the_widget_draws_what_render_markdown_returns`, `the_widget_cuts_off_lines_beyond_its_height`, `the_widget_draws_inside_its_area_and_wraps_to_its_width`, `the_widget_rewraps_when_the_area_is_resized`, `the_widget_base_style_sets_the_text_style`.
- UI: sample @ 80 (text) — `sample_document_at_80_columns_text`
- UI: sample @ 80 (styled) — `sample_document_at_80_columns_styled`
- UI: sample @ 40 (text; prose wraps, code char-wraps, table truncates with `…`) — `sample_document_at_40_columns_wraps_and_truncates`
- UI: code block only @ 60 (styled) — `code_block_only_at_60_columns_styled`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites; 715 lib tests passed, 0 failed)
