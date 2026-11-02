STATUS: DONE

## Changes
- `src/model/mod.rs`: added `ModelUpdate::Reset` (clears all turns; `apply` handles it) with a test.
- `Cargo.toml`: `proptest` as a dev-dependency.
- `src/components/transcript/` (new, `pub mod transcript`, re-exported from `components`):
  - `mod.rs`: the `Transcript` component (`Component`: `view`, `query`, `attr`, `state`, `perform`; `AppComponent::on` is a stub returning `None` until task 020 binds keys). Attrs: `THEME_ATTR`, `DIM_UNSELECTED_ATTR`, `SHOW_SYSTEM_ATTR`, `TRUNCATE_OUTPUT_LINES_ATTR`, `TIMESTAMP_FORMATTER_ATTR`, `MODEL_UPDATE_ATTR` (an `Arc<ModelUpdate>` payload), `VIEWPORT_ATTR` (the pre-render step). `state()` is the scroll offset.
  - `cache.rs`: `LayoutCache`: per-block heights for the current width plus prefix sums. Heights are measured lazily (the prefix grows one block at a time, only as far as a lookup, a `covers`, or a `top` needs). Row to block lookup is `partition_point` (binary search). `invalidate(i)` forgets one block, `set_width` forgets all. A `computed` counter counts measured heights.
  - `content.rs`: turns, `TurnId` index, fold overrides (`FoldState`, keyed by `SectionId`), selection, display options; `Measurer` (measures a block with `TurnBlock::layout` and keeps the last layout so drawing a just-measured block does not lay it out twice).
  - `window.rs`: `Window`: the blocks overlapping `[offset, offset+height)`, laid out, drawn with `TurnBlock::skip_rows` + `render_layout` (clipped first and last block). `placements()` is the pure geometry.
  - `updates.rs`: `Appended` / `Updated` (in place, by `TurnId`) / `Reset`; selection initialised to the first selectable section (from `BlockLayout::section_rows`), kept across updates, and, when its section disappears, moved to the nearest following section, else the first section of a later block, else the last section. `show_system` change invalidates only blocks that hold injected items.
  - `nav.rs`: `Cmd::Scroll` (one row, clamped so the last row cannot go above the viewport bottom), `Cmd::Move` (across blocks, scrolls minimally; includes the block's title row / bottom border when that fits; SPEC 6.3: a selected section taller than the viewport is paged through a page at a time before the selection moves), `Cmd::Submit` (toggle fold, invalidates only that block). Public helpers `scroll_to`, `select`, `set_folded`, `is_folded` for the App/task 020.
- `src/components/turn_block/mod.rs`: test fixtures made `pub(crate)` (cfg(test)) so the transcript tests reuse them.

Design notes:
- `view` is pure. The pre-render step is `VIEWPORT_ATTR` / `Transcript::set_viewport` (the App sends it when the area changes). It re-wraps for a new width (the top visible block stays at the top), fills the cache for the visible blocks, builds the window and clamps the offset. Every later change (attr, Cmd, update) refreshes the window. If `view` gets an area that no viewport was announced for, it builds the window on a clone of the cache and drops it (documented in the module docs); no state is mutated during `view`.
- Laziness: blocks below the viewport are never measured (see `scrolling_far_down_lays_out_only_what_it_passes_and_shows`). Selection initialisation and navigation lay out blocks outside the counted `layouts_run`, which only counts layouts run to measure or draw blocks.
- `truncate_output_lines` is accepted, stored and queryable, and changing it invalidates all heights, but `TurnBlock` (task 017) does not consume it yet; it is not wired into 017's renderers here.
- The App does not mount the Transcript yet (no Browser screen in this task); `AppComponent::on` is a stub.

## Self-check
- AC: Implements `Component`, built from props alone and rendered in a `TestBackend` — `builds_from_props_alone_and_renders_without_a_viewport`, `the_unprepared_render_equals_the_prepared_one`, `view_changes_nothing_in_the_transcript`, `props_round_trip_through_attr_and_query`, `the_formatter_and_viewport_attrs_apply`
- AC: Appended adds blocks; Updated in place keeps selection and folds; Reset clears — `appended_updates_add_blocks`, `updated_replaces_a_turn_in_place_keeping_selection_and_folds`, `an_update_invalidates_only_its_block`, `reset_clears_turns_folds_selection_and_offset`, vanished-selection tests (`a_vanished_selection_*`), `model::tests::reset_clears_the_turns`
- AC: Width change recomputes and matches rendering — `a_width_change_recomputes_heights_and_they_match_the_rendering`, `a_resize_keeps_the_top_block_at_the_top`, proptest `a_resize_leaves_heights_matching_the_new_width`
- AC: Fold toggle invalidates only that block (recompute counter) — `a_fold_toggle_invalidates_only_that_block`, `selection_moves_do_not_recompute_heights`, `show_system_invalidates_only_the_blocks_with_injected_items`
- AC: Row to block lookup not a linear scan — `cache_lookup::locate_finds_the_block_containing_a_row`, `locate_is_a_binary_search_over_known_prefix_sums`, `locate_measures_only_the_blocks_it_has_to` (implementation: `LayoutCache::locate` uses `partition_point`)
- AC: Only visible blocks rendered (2000 turns, counter) — `only_the_blocks_overlapping_the_viewport_are_laid_out`, `drawing_a_frame_runs_no_layout_when_prepared`, `scrolling_far_down_lays_out_only_what_it_passes_and_shows`, `only_overlapping_blocks_are_placed`
- AC: Clipping equals the slice of the full rendering — `clipped_blocks_equal_the_slice_of_the_full_rendering`, `a_block_clipped_at_both_edges_shows_the_right_rows`, `nothing_is_drawn_outside_the_area`
- AC: Scroll clamps; Move crosses blocks and scrolls into view; Submit toggles — `scroll_clamps_at_the_top`, `scroll_clamps_at_the_bottom`, `scroll_to_clamps_to_the_content`, `move_crosses_block_boundaries`, `move_up_walks_back_through_every_section`, `move_scrolls_the_new_selection_into_view`, `move_scrolls_minimally`, `a_section_taller_than_the_viewport_is_paged_through_before_moving`, `moving_up_through_a_tall_section_pages_back_up`, `submit_toggles_the_fold_of_the_selected_section`, `submit_on_a_section_that_cannot_fold_does_nothing`, `submit_keeps_the_unfolded_section_visible`
- AC: Proptest, cached heights equal rendered heights for random widths (20-200) and fold states — `property::cached_heights_equal_rendered_heights` (rendered height read off a taller buffer: bottom border row, blank below; also total = sum, and the frame equals the stacked-block slice), `property::a_resize_leaves_heights_matching_the_new_width`
- UI: Top, first section selected @ 80x24 text — `snapshots::top_of_the_transcript_at_80x24_text`
- UI: Top @ 120x40 text — `snapshots::top_of_the_transcript_at_120x40_text`
- UI: Top @ 120x40 styled (`▒`, dimming) — `snapshots::top_of_the_transcript_at_120x40_styled`
- UI: Scrolled to the middle, clipped at top and bottom @ 80x24 text — `snapshots::scrolled_to_the_middle_with_both_edges_clipped_at_80x24_text`
- UI: Selection in the 5th block after Move steps @ 80x24 text — `snapshots::selection_in_the_fifth_block_after_moves_at_80x24_text`
- UI: After a resize from 120 to 80 columns, re-wrapped, @ 80x24 text — `snapshots::after_a_resize_from_120_to_80_columns_text_is_wrapped_again_at_80x24_text`
- UI: `dim_unselected = false` @ 120x40 styled — `snapshots::dim_unselected_false_at_120x40_styled`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1087 passed, 0 failed)
