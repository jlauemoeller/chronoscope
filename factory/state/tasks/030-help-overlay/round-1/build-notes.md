STATUS: DONE

## Changes
- `src/components/help_overlay/content.rs` (new): `HelpContext` (Browser / Selector), `HelpContent` (the entries model, generated from the `Keymap` via `Keymap::labels`; harness rows filtered by registered harnesses; quit row ends in ` / Ctrl-c`; Shift note only with `mouse = true`), the row layout (headings at column 3 in bold `foreground`, keys at column 5 in `help_key` padded to widest keys + 6, descriptions in `help_text`) and `desired_size` (widest entry + 8, at least 40; content + 2).
  - Order: groups as in the planner notes. Within a group the context's own rows come first in keymap table order, then the global ones: `open_selector` (Sessions), then `theme`, `help`, `quit` (Application). Decision: the notes list theme, help, quit in that order.
  - The Shift note is drawn at column 3 (the heading column) in `help_text`, right after the last group's blank row.
- `src/components/help_overlay/mod.rs` (new): the `HelpOverlay` component. Rounded `ring` border with the `Keybindings` title (`titled_block`), `Clear` plus `surface` fill (only `Clear` when transparent), scrolling (`overlay.up`/`down`, page keys, clamped), ` ↑ more ` / ` ↓ more ` on the borders in `muted_foreground` with one `─` before the corner, `Esc` (`overlay.close`) or the global help key closes (`Msg::CloseHelp`). `view` has no side effects (scroll is clamped on the fly from the area). The interior size is pushed by the app through `INTERIOR_ATTR`.
- `src/keymap/defaults.rs`, `src/keymap/tests.rs`: new `overlay.page_up` (`pgup`) and `overlay.page_down` (`pgdn`) actions, so `PgUp`/`PgDn` also go through the keymap (the binding planner note forbids literal keys). Deliberate decision; the expected-table test is updated.
- `src/components/global_events.rs`: subscribes to the global help key and gives `Msg::OpenHelp(context)`; the context comes from `HELP_CONTEXT_ATTR`, which the app sets on every screen change. The key clause is now "not Prompt and not HelpOverlay". `Ctrl-c` keeps the prompt-only clause, so it quits from the overlay.
- `src/app/mod.rs`, `src/app/help.rs` (new), `src/app/layout.rs`, `src/app/selector.rs`, `src/app/browser.rs`: `Id::HelpOverlay`, `Msg::OpenHelp`/`CloseHelp`/`HelpScrolled`; opening goes through `open_overlay` (mounts, pushes the focus; a second open request is ignored) and closing through `close_overlay` (restores the focus). `LayoutInput::help` + `help_overlay_rect` place the overlay (centred, `min(w-4, wanted)` x `min(h-2, wanted)`), after the screen and before the toast; it is hidden on the too-small screen. Every `self.screen = ..` goes through `set_screen`, which tells `GlobalEvents` which help to open. `open_session` drops an open overlay.
- `src/app/browser_tests.rs`: `build_rig_from` so tests can build the rig from a custom `AppInit`.

## Self-check
- AC: browser lists every browser and relevant global action, groups and order, keys from `Keymap::labels` — `content::tests::the_browser_lists_its_groups_in_order`, `the_browser_lists_every_browser_and_relevant_global_action_with_its_keys`, `the_rows_follow_the_planner_notes`
- AC: remapping `[keys.browser] copy = "c"` — `content::tests::a_remapped_copy_key_shows_on_the_copy_row`, `help::a_remapped_copy_key_shows_in_the_drawn_overlay_120x40_text`
- AC: selector lists Session selector (registered harness rows only) and Application — `content::tests::the_selector_lists_the_session_selector_group_and_application`, `the_selector_lists_only_the_registered_harness_rows` (Claude + Codex, non-contiguous)
- AC: quit row `q / Ctrl-q / Ctrl-w / Ctrl-c` — `content::tests::the_quit_row_ends_with_ctrl_c`
- AC: Shift note only with `mouse = true` — `content::tests::the_shift_note_is_a_last_row_only_with_the_mouse_on`, `help::the_shift_note_is_drawn_only_when_the_mouse_is_on`
- AC: `?` opens and takes focus; `Esc`/`?` close and restore focus; `q`/`t` do nothing; `Ctrl-c` quits — `help::the_help_key_in_the_browser_opens_the_overlay_and_gives_it_the_focus`, `esc_closes_the_overlay_and_returns_the_focus_to_the_transcript`, `the_help_key_closes_the_overlay_too`, `quit_and_theme_keys_do_nothing_while_it_is_open`, `ctrl_c_quits_from_the_overlay`, `the_help_key_in_the_startup_selector_opens_the_selectors_help`, `the_selector_over_the_browser_opens_the_selectors_help_and_gives_back_both_focuses`, `the_focus_returns_to_the_pane_that_had_it` (all through `App::tick` and the key port, so the subscription clauses are exercised)
- AC: scrolling with `j`/`k`/`↓`/`↑`/`PgDn`/`PgUp`, clamping, indicators — `help_overlay::tests::the_down_keys_scroll_one_row_and_clamp_at_the_end`, `the_up_keys_scroll_one_row_and_clamp_at_the_top`, `the_page_keys_scroll_by_the_rows_of_the_interior_and_clamp`, `the_top_shows_a_down_indicator_only`, `the_middle_shows_both_indicators`, `the_end_shows_an_up_indicator_and_the_shift_note`, `a_tall_enough_overlay_shows_no_indicator`, `help::the_scroll_keys_move_the_overlay_through_the_app`
- UI: browser 120x40 top with `↓ more` (text + styled) — `help::the_browser_help_120x40_top_text`, `the_browser_help_120x40_top_styled`
- UI: browser 120x40 scrolled to the end, `↑ more` + Shift note — `help::the_browser_help_120x40_end_text`
- UI: browser 80x24 — `help::the_browser_help_80x24_text`
- UI: browser 200x60, everything fits — `help::the_browser_help_200x60_text`
- UI: selector 120x40 with the selector visible around it — `help::the_selector_help_120x40_text`
- UI: `copy = "c"` at 120x40 — `help::a_remapped_copy_key_shows_in_the_drawn_overlay_120x40_text`
- Extra: component-level styled snapshot `help_overlay::tests::the_overlay_snapshot_at_a_scrolled_position_is_styled`; colour/modifier cell checks in `the_colours_come_from_the_theme`, `the_indicators_are_muted_on_the_ring_border`, `a_transparent_theme_clears_but_does_not_fill`; layout tests in `app::layout::tests`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1991 passed, 0 failed)
