STATUS: DONE

## Changes
- `src/keymap/{mod,defaults,tests}.rs`, `README.md`: new browser action `details` (`Action::Details`, key `i`, group Sections, "Show section details", last in the Sections group), the action table row, a remap/help-entry test, and the README browser-table row.
- `src/components/overlay_frame.rs` (new): `OverlayFrame`, the frame and scrolling both overlays share: the rounded `ring` border with a title, the `surface` fill (only `Clear` when transparent), the ` ↑ more ` / ` ↓ more ` indicators, offset and interior, overlay-context key resolution (`FrameKey`), the wheel, `INTERIOR_ATTR` (renamed `overlay_interior`), `interior_value`, `WHEEL_ROWS`. The total row count is a parameter, because the details overlay's count depends on the width.
- `src/components/help_overlay/mod.rs`: `HelpOverlay` now holds an `OverlayFrame` instead of its own copy of that code. Public names are re-exported as before.
- `src/components/details_overlay/` (new): `DetailsOverlay` component; `content.rs` (`DetailsContent`/`DetailsGroup`/`DetailsRow`, layout, char-wrap with hanging indent, desired size, the muted `unavailable` row); `rows.rs` (`section_details`: Turn, Tool call, Prompt, Environment rows from the model; More resolves to its tool call); `snapshots.rs`.
- `src/components/turn_block/renderers/stats.rs`: `format_cost` extracted from the footer (footer output unchanged).
- `src/components/transcript/{mod,details}.rs`: the `details` key answers `Msg::OpenDetails(DetailsContent)` for the selected section (nothing with no selection).
- `src/app/`: `Id::DetailsOverlay`; `Msg::{OpenDetails, CloseDetails, DetailsScrolled}`; `details.rs` (open/close/drop/interior push, size capped by the screen width before wrapping); `LayoutInput.details` + `layout()`; `mouse.rs` (`OVERLAYS`, `takes_mouse`, `outside_overlay`); `theme_selector.rs` (`THEMED`, busy list); `browser.rs` (`drop_details`); `selector.rs` (`push_viewports`); `resume.rs` and `help.rs` (busy guards).
- `src/components/global_events.rs`: `key_clause` is also off while the details overlay is mounted (`Ctrl-c` still quits).
- Existing help tests/snapshots: row counts bumped by one for the new `details` row; five help screen snapshots accepted (they differ only by the inserted row and the shift it causes in the clamped 120x40 overlay).

## Self-check
- AC: keymap tests/action table include `details`; help lists it from the active keymap — `keymap::tests::default_keymap_matches_the_planner_table`, `the_details_key_is_remappable_and_resolves_to_the_action`, `help_overlay::content::tests::the_sections_group_lists_the_details_key_from_the_active_keymap`, `browser_tests::details::the_help_overlay_lists_the_default_details_key`.
- AC: component snapshots, 9 kinds x {80x24, 120x40} x {default, terminal}, tall content top and end — `details_overlay::snapshots::every_kind_of_section_at_both_sizes_in_both_themes` (36 named snapshots), `content_taller_than_the_screen_at_80x24_scrolled_to_the_top_and_to_the_end`, `a_long_value_wraps_to_the_value_column_at_80x24`, `an_unavailable_group_is_one_muted_row`.
- AC: `App::update` tests: opening, closing with both keys, focus returning, More row, no selection — `browser_tests::details::{the_details_key_opens_the_overlay_for_the_selected_section_and_gives_it_the_focus, esc_closes_the_overlay_and_returns_the_focus_to_the_transcript, the_details_key_closes_it_too_and_does_not_reopen_it, a_more_row_opens_the_details_of_its_tool_call, with_no_selection_the_key_does_nothing, an_open_request_while_open_or_over_another_overlay_changes_nothing, opening_another_session_drops_the_overlay}`; transcript level `transcript::tests::details::*`.
- AC: remapped `details` key opens/closes, help shows it — `browser_tests::details::{a_remapped_details_key_opens_and_closes_the_overlay, the_help_overlay_shows_the_remapped_details_key}`.
- AC: snapshot semantics — `browser_tests::details::what_it_shows_does_not_change_when_the_turn_does` (with a control that the transcript took the update).
- AC: copy/save/export — `browser_tests::details::copy_save_and_export_do_nothing_while_it_is_open`, `transcript::tests::details::the_content_is_not_part_of_copy_save_or_export`.
- AC: mouse — `browser_tests::details::{the_wheel_scrolls_the_overlay_and_a_press_outside_closes_it, nothing_under_the_overlay_gets_the_mouse, with_mouse_off_the_overlay_takes_no_mouse}`, `mouse_tests::nothing_under_an_overlay_is_a_target`, `layout` tests.
- AC: global keys `q`, `t`, `?`, `Ctrl-o` do nothing, `Ctrl-c` quits — `browser_tests::details::{the_global_keys_do_nothing_while_it_is_open, ctrl_c_quits_from_the_overlay}`.
- AC: row tests (order, group left out, missing field, no cost, Turn group presence) — `details_overlay::rows::tests::*` (`the_groups_come_in_the_order_of_the_table`, `a_missing_field_is_left_out`, `there_is_no_cost_row_without_a_cost`, `the_turn_group_is_on_every_section_of_an_assistant_turn`, `a_users_sections_have_no_turn_group`, `a_compaction_has_no_turn_group_but_its_timestamp`), `content::tests::a_group_without_rows_is_left_out`.
- AC: shared frame; help snapshots differ only by the new row — `help_overlay` now delegates to `OverlayFrame`; the help tests were not rewritten beyond the row-count constants; the five changed help screen snapshots show the added row.
- AC: README row and `tests/release_docs.rs` — `release_docs::readme_keybindings_match_the_default_keymap` passes.
- AC: budgets — `cargo test --release -- --ignored budgets --nocapture`: first content 2.4 ms, full parse 136.7 ms / 306 ms, frames worst 3.2 ms, memory 0.58x; all within budget.
- UI: every kind @ 80x24 and 120x40 in `default` and `terminal` — `every_kind_of_section_at_both_sizes_in_both_themes`; tall @ 80x24 top/end in both themes — `content_taller_than_the_screen_at_80x24_scrolled_to_the_top_and_to_the_end`; app screens — `browser_tests::details::the_details_over_the_browser_{80x24,120x40}_text`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2754 passed, 0 failed)

Notes: the fixtures of the component snapshots populate every group of the spec table from plain data (Message, Environment, OpenCode cost/provider/agent); the rows the model does not know yet are for tasks 008 and 009. Raw-field labels in those fixtures are placeholders for display only.

Deliberate choice: the overlay's wanted size is fixed when it opens (the wanted width is capped by the screen width then, and the wrapped height follows from it). A resize re-announces the interior (rows and columns) and `help_overlay_rect` caps the rect, as for the help overlay, but the wanted height is not recomputed.
