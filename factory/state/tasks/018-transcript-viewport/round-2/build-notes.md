STATUS: DONE

## Changes
Round 2 addresses the round-1 reviews (feature must-fix, UI must-fix, nits).
- `nav.rs` (`page_through`): a section taller than the viewport is paged through only while it overlaps the viewport. A selection that has scrolled out of view is left alone by the paging, so `Move` selects the neighbour and reveals it.
- `updates.rs` / `nav.rs` (`reveal_selection`): when the selection moves by itself (the selected section disappeared through an `Updated` turn or `show_system = false`), it is scrolled into view with the same minimal reveal `select` uses, before the window is rebuilt.
- `updates.rs`: an `Updated` turn above the top visible block that changes height moves the offset by the difference, so the screen does not jump; `restore_selection` is skipped for a batch that did not touch the selected turn (no layout of the selected block per `Appended` batch); types imported at the top.
- `mod.rs`, `content.rs`, `window.rs`: `Cmd::Scroll` rebuilds the window with the previous window's layouts reused (`Window::into_layouts`, `Measurer::reusing`), so only newly exposed blocks are laid out.
- `mod.rs`: the fallback in `view` for an unannounced area logs at debug level; test-only instrumentation (`cache()`, `layouts_run()`, `LayoutCache::computed/is_empty`) is `cfg(test)`/`pub(crate)`, the cache re-export is `pub(crate)`; `set_truncate_output_lines` only stores the value (no renderer reads it yet; comment says to invalidate when one does).
- `nav.rs`: `scroll_to` returns `Changed` directly; `move_selection` returns `Visual` when it had to create the selection.
- `src/model/mod.rs`: docs: `Appended` with a known id is an update in the transcript; `Reset` wording is the intended contract.
- Tests: cleaned `cache_lookup` test (dead variable removed, renamed).

Not done (nits, deliberate): reusing the window's section rows instead of laying out the selected block in `cursor()` (feature nit 4; navigation only runs per key press); an empty 3-row shell for a user block with no visible sections (comes from `TurnBlock`, task 017).

## Self-check
- Review (feature 1): tall off-screen selection paged blindly — `a_tall_selection_scrolled_out_of_view_is_revealed_by_move_not_paged` (both directions)
- Review (UI 1): selection off-screen after show_system / Updated — `a_selection_removed_by_hiding_system_sections_is_scrolled_into_view`, `a_selection_removed_by_an_update_is_scrolled_into_view` (both fail without the reveal; checked by mutation)
- Review (code 1): scroll reuses layouts — `scrolling_lays_out_only_the_newly_exposed_blocks`
- Review (code 8): update above viewport — `an_update_above_the_viewport_does_not_move_what_is_on_screen`
- Review (feature 5): clipped styles — `a_clipped_block_keeps_its_styles`
- Review (feature 7): in-place position asserted in `updated_replaces_a_turn_in_place_keeping_selection_and_folds`
- Review (UI 3): added snapshot `before_a_resize_at_120x24_text`
- Review (code 2,3,4,5,6,7,9,10; feature 2,3,6,8): see Changes
- All round-1 ACs and the 7 UI snapshots remain covered by the tests listed in round-1 notes.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1093 passed, 0 failed)
