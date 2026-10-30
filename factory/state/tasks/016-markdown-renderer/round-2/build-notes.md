STATUS: DONE

## Changes
Round 2 addresses every finding of the round-1 reviews (src/markdown/ only; the round-1 notes still describe the module).
- `mod.rs`: `Styles::muted` is now `base.fg(muted_foreground)`, so the quote bar, table separators, header rule and horizontal rule keep the base modifiers. `render_markdown` doc states the minimum widths below which lines can exceed `width`.
- `layout.rs`: code spans are `base.patch(syntax token style)` (the standalone `highlight_code` stays base-free); code indent and list hang/nest indents are styled with `base`; the dead `tight` parameter of `layout` is removed; an empty list item keeps its marker.
- `table.rs`: padding uses `base`; a row ends at its last non-empty cell, so a short row has no dangling separator.
- `prose.rs`: `Inline::push` turns a tab into four spaces (prose, headings, inline code, table cells), so tabs no longer vanish on screen.
- `widget.rs`: `render` clips to `buf.area` (no panic when the area reaches past the buffer).
- `wrap.rs`: `char_wrap` with width 0 gives no lines (same as `render_markdown`), and control characters are dropped (documented: ANSI sequences are not interpreted, strip them first).
- `highlight.rs`: the load counter is `#[cfg(test)]`; the `comment` rule is a normal `RULES` entry.
- Snapshots: the two styled snapshots were regenerated (indent spaces now carry the base foreground instead of `Reset`); added `code_block_with_distinct_syntax_colours_at_60_columns_styled` (a theme whose syntax tokens all differ).

## Self-check
- Review: code-review 1 (base not applied to bars/rules/code) — `a_base_modifier_reaches_every_span_of_every_element` (quote, table, rule, nested list, rust block, heading all keep DIM); `the_base_style_is_the_starting_point_of_every_element`.
- Review: code-review 2 (tabs) — `tabs_in_prose_become_spaces_so_words_stay_apart` (text, widget buffer, table cell, inline code).
- Review: code-review 3 (dead `tight`) — removed.
- Review: code-review 4 (empty item) — `an_empty_list_item_keeps_its_marker`.
- Review: code-review 5 (widget outside buffer) — `the_widget_clips_to_the_buffer_instead_of_panicking`.
- Review: code-review 6 (width 0) — `char_wrap_with_zero_width_gives_no_lines_like_render_markdown`.
- Review: code-review 7 (LOADS, comment rule) — done; `the_syntax_set_is_loaded_once`, `scopes_map_to_tokens_by_the_innermost_match` still pass.
- Review: code-review 8 (nested list under ordered marker starts inside the marker column) — kept per the planner note; flag for the transcript task in case aligning under the item text reads better.
- Review: feature-review 1 — same as code-review 1. feature-review 2 — doc comment. feature-review 3 — `a_short_table_row_has_no_dangling_separator`. feature-review 4 — `char_wrap_drops_control_characters` and doc.
- Review: ui-review 2 (base background gaps) — indent/padding/bars/code now derive from `base`. ui-review 3 — new distinct-colour code snapshot. ui-review 1 (widths below the prefix width clip content) — not changed; documented in the `render_markdown` doc comment (optional nit).
- UI snapshots (unchanged set, regenerated where styles shifted): `sample_document_at_80_columns_text`, `sample_document_at_80_columns_styled`, `sample_document_at_40_columns_wraps_and_truncates`, `code_block_only_at_60_columns_styled`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass
