STATUS: DONE

## Changes
- `src/components/transcript/keys.rs` (new): the `Cmd::Custom` names (`NEXT_TURN`, `HALF_PAGE_DOWN`, `FOLD_ALL`, `TOGGLE_SYSTEM`, ...) and the map from browser `Action` to `Cmd`. `Transcript::custom` dispatches them.
- `src/components/transcript/mod.rs`: `Transcript` owns a `Keymap` (builder `.keymap(..)`, default keymap otherwise) and a `KeyResolver` for `Context::Browser`. `AppComponent::on` feeds the key to the resolver and `perform`s the `Cmd` of the resulting action; it returns no `Msg` (nothing needs the App yet; actions of other tasks do nothing). `perform` also handles `GoTo(Begin/End)`, `Submit` (toggle), and `Custom`.
- `src/components/transcript/paging.rs` (new): `]`/`[` (`jump_turn`), `Ctrl-d/u` and `PgDn/PgUp` (`scroll_pages`, clamped, selection moves to the topmost fully visible section, or the one at the top row if none is fully visible; only when the offset changed), `g`/`G` (`jump_to_start`/`jump_to_end`).
- `src/components/transcript/folding.rs` (new): `fold_command` (toggle/fold/unfold; replaces the old `toggle_fold` in `nav.rs`), `set_all_folded` (fold all / unfold all, hidden system sections included), `toggle_system`, and the screen-row anchor (`anchor_selected` / `restore_anchor`) that keeps the selected section's first row at the same screen row after a fold change, fold all, or `s`. If that row was off screen before, the section is only revealed in place.
- `src/components/transcript/nav.rs`: `j`/`k` logic from task 018 is unchanged (it already pages through tall sections); some items made `pub(super)`; `toggle_fold` removed.
- Tests: `src/components/transcript/tests/keys.rs` (behaviour, remap, chord, fold/screen-row, system toggle); `src/components/transcript/key_snapshots.rs` (+ 9 accepted `.snap` files).

## Decisions worth a look
- `]`/`[` select the first section of the block that is not System content, and skip blocks that hold only system content. For a User turn that starts with an injected section, this selects the prompt (the block's top border is still the top row). Reading: AC "never stop on system content".
- AC1 wording "three presses on a 3-page section scroll by pages, then the fourth selects the next": task 018's paging gives ceil(N/h)-1 scrolls for an N-row section. Both readings are tested: an exactly 3-page section (30 rows, 10-row viewport) where press 1 lands on it, presses 2 and 3 scroll a whole page each, press 4 selects the next (`three_presses_scroll_by_whole_pages_through_a_three_page_section`), and a 3.5-page section where three presses scroll (the last by the remaining 5 rows) and the fourth moves on (`three_presses_page_through_a_tall_section_and_the_fourth_selects_the_next_one`).
- Page keys do not move the selection when the offset does not change (already at the end / transcript fits).
- `Cmd::Scroll(Up/Down)` (one row, task 018) does not touch the selection; the mouse-wheel task can use `scroll_pages`-like logic.
- `s` also keeps the selected section's screen row (not required by the spec).

## Self-check
- AC: j/k/arrows across sections and blocks, paging through a tall section — `j_and_down_select_the_next_section_k_and_up_the_previous`, `moving_crosses_blocks_and_stops_at_the_first_and_last_section`, `three_presses_page_through_a_tall_section_and_the_fourth_selects_the_next_one`, `three_presses_scroll_by_whole_pages_through_a_three_page_section`, `paging_up_mirrors_paging_down_from_the_section_top`, `a_selection_move_scrolls_the_section_into_view`
- AC: `]`/`[` between blocks, never on system content, top border at the viewport top — `right_bracket_and_left_bracket_jump_between_blocks_with_the_top_border_on_top`, `a_jump_selects_the_first_section_of_the_block`, `a_jump_near_the_end_scrolls_as_far_as_the_transcript_allows`, `turn_jumps_skip_blocks_that_hold_only_system_content`, `a_jump_into_a_block_that_starts_with_system_content_selects_its_prompt`
- AC: Ctrl-d/u half page, PgDn/PgUp page, clamped, selection to the topmost fully visible section — `ctrl_d_and_ctrl_u_scroll_half_a_page`, `page_down_and_page_up_scroll_a_full_page`, `an_odd_height_rounds_a_half_page_down`, `page_scrolling_stops_when_the_last_row_reaches_the_bottom`, `scrolling_moves_a_selection_that_left_the_viewport_to_the_topmost_fully_visible_section`, `a_selection_that_stays_fully_visible_is_kept_while_scrolling`, `every_page_key_moves_the_selection_to_the_topmost_fully_visible_section` (PgUp, Ctrl-u, PgDn, Ctrl-d), `when_no_section_is_fully_visible_the_selection_moves_to_the_one_at_the_top_row`, `page_keys_do_nothing_when_the_transcript_fits_the_viewport`, `page_keys_without_a_viewport_do_nothing`
- AC: g/Home, G/End — `g_and_home_select_the_first_section_and_scroll_to_the_start`, `shift_g_and_end_select_the_last_section_and_scroll_to_the_end`, `start_and_end_report_no_change_when_already_there`
- AC: fold keys, non-foldable ignored, screen row kept — `enter_and_space_toggle_the_selected_section`, `h_and_left_fold_l_and_right_unfold`, `fold_keys_ignore_sections_that_cannot_fold`, `minus_folds_everything_plus_and_equals_unfold_everything`, `fold_all_and_unfold_all_include_hidden_system_sections`, `a_fold_change_keeps_the_selected_sections_screen_row`, `unfolding_near_the_bottom_does_not_scroll_the_view`, `fold_all_keeps_the_selected_sections_screen_row`, `fold_all_on_a_selected_truncation_row_moves_to_its_tool_call`, `enter_on_a_truncation_row_expands_and_fold_keys_ignore_it`
- AC: `s` toggles system sections, `show_system = false` starts hidden, selection moves to a visible neighbour — `s_hides_and_shows_every_system_section`, `with_show_system_false_system_sections_start_hidden_and_s_shows_them`, `hiding_system_content_moves_a_selected_system_section_to_the_next_visible_one`, `hiding_system_content_selects_the_previous_section_when_none_follows`, `hiding_system_content_never_leaves_the_selection_on_a_hidden_section`
- AC: remapping — `a_remapped_key_works_and_the_old_one_does_nothing`, `shifted_letters_arrive_with_the_shift_modifier`, `a_remap_applies_to_every_action_it_names`, `a_chord_binding_waits_for_its_second_key`
- UI: after `j` x3 @ 80x24 — `after_j_three_times_the_selection_is_in_the_second_block_at_80x24_text`
- UI: paging 1st/2nd `j` @ 80x24 — `paging_through_a_tall_section_after_the_first_j_at_80x24_text`, `..._after_the_second_j_at_80x24_text`
- UI: `]` x2 @ 80x24 — `after_the_right_bracket_twice_the_third_block_is_at_the_top_at_80x24_text`
- UI: Ctrl-d @ 80x24 — `after_ctrl_d_the_selection_is_the_topmost_fully_visible_section_at_80x24_text`
- UI: `+` and `-` @ 120x40 — `after_plus_everything_is_unfolded_at_120x40_text`, `after_minus_everything_is_folded_at_120x40_text`
- UI: `s` hidden vs default @ 80x24 — `with_system_sections_shown_by_default_at_80x24_text`, `after_s_the_system_sections_are_hidden_at_80x24_text`
- No styled snapshots: the task specifies no colours or modifiers.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites, 0 failed; lib 1035 passed)
