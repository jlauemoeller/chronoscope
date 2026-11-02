VERDICT: PASS

## Summary
The `HelpOverlay` is implemented as its own component. Its entries are generated from the active keymap through `Keymap::labels`, grouped per context as the planner notes require, centred over the current screen with a `ring` border on `surface`, and scrollable with clamping and `↑ more` / `↓ more` indicators. `GlobalEvents` opens it with `Msg::OpenHelp(context)` and stays silent while it is mounted, except for `Ctrl-c`. Every acceptance criterion has a test that would fail if the behaviour regressed. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (run with `INSTA_UPDATE=no`) all pass.

## Acceptance criteria
- [x] From the browser, the overlay lists every browser action and the relevant global actions in the planner-notes groups and order, with keys from `Keymap::labels`.
  - Implemented in `src/components/help_overlay/content.rs`, `HelpContent::new` / `row_of`. The groups come from `BROWSER_GROUPS`; the context's own rows come first in table order, followed by `open_selector`, `theme`, `help` and `quit`.
  - Tests: `content::tests::the_browser_lists_its_groups_in_order`, `the_browser_lists_every_browser_and_relevant_global_action_with_its_keys` (compares against `Keymap::labels` per group, checks total = browser bindings + 4, and checks the global tail order), `the_rows_follow_the_planner_notes`.
- [x] Remapping `[keys.browser] copy = "c"` shows `c` on the copy row.
  - Tests: `content::tests::a_remapped_copy_key_shows_on_the_copy_row`, plus the screen test `help::a_remapped_copy_key_shows_in_the_drawn_overlay_120x40_text`.
- [x] From the selector, it lists the Session selector group (only registered harness rows) and Application.
  - Implemented by `SELECTOR_GROUPS` and `is_listed`. `open_selector` is excluded in the selector context.
  - Tests: `the_selector_lists_the_session_selector_group_and_application`, `the_selector_lists_only_the_registered_harness_rows` (Claude and Codex, which are non-contiguous, and the empty set), `the_selector_does_not_list_the_browser_only_rows`.
- [x] The quit row shows `q / Ctrl-q / Ctrl-w / Ctrl-c`.
  - Implemented in `row_of`.
  - Test: `the_quit_row_ends_with_ctrl_c`, which also covers a remapped quit and an unbound quit.
- [x] The Shift note appears only when `mouse = true`.
  - Tests: `content::tests::the_shift_note_is_a_last_row_only_with_the_mouse_on` and `help::the_shift_note_is_drawn_only_when_the_mouse_is_on`, which goes through `config.mouse` in the app.
- [x] `?` opens the overlay and focus moves to it; `Esc` and `?` close it and restore the previous focus; `q` and `t` do nothing while it is open; `Ctrl-c` quits.
  - Implemented in `src/app/help.rs` (`open_overlay` / `close_overlay` with the focus stack) and `src/components/global_events.rs`. `key_clause` excludes the case where `HelpOverlay` is mounted; `Ctrl-c` keeps the prompt-only clause.
  - I checked that the only assignment to `self.screen` is now inside `set_screen`, and that the default `Screen::Selector` matches the `GlobalEvents` default context. So the help context cannot go stale.
  - Tests in `src/app/browser_tests/help.rs` drive keys through the ports and subscriptions: `the_help_key_in_the_browser_opens_the_overlay_and_gives_it_the_focus`, `esc_closes_the_overlay_and_returns_the_focus_to_the_transcript`, `the_help_key_closes_the_overlay_too`, `quit_and_theme_keys_do_nothing_while_it_is_open` (with a control check that `q` quits after the overlay closes), `ctrl_c_quits_from_the_overlay`, `the_focus_returns_to_the_pane_that_had_it`, `the_selector_over_the_browser_opens_the_selectors_help_and_gives_back_both_focuses`.
- [x] Scrolling with `j`/`k`/`↓`/`↑`/`PgDn`/`PgUp` works and clamps, and the `↑ more` / `↓ more` indicators appear accordingly.
  - `PgUp`/`PgDn` are routed through the new `overlay.page_up` / `overlay.page_down` keymap actions. This follows the binding planner note against literal keys.
  - Component tests: `the_down_keys_scroll_one_row_and_clamp_at_the_end`, `the_up_keys_scroll_one_row_and_clamp_at_the_top`, `the_page_keys_scroll_by_the_rows_of_the_interior_and_clamp`, `the_top_shows_a_down_indicator_only`, `the_middle_shows_both_indicators`, `the_end_shows_an_up_indicator_and_the_shift_note`, `a_tall_enough_overlay_shows_no_indicator`, `the_indicators_are_muted_on_the_ring_border`.
  - App-level test: `help::the_scroll_keys_move_the_overlay_through_the_app`.
- [x] Every UI state below has a snapshot. All are taken through `App::view`, and I checked their contents.
  - 120x40 top: text and styled. The styled one shows the ring/surface colours, bold headings, `help_key` / `help_text`, and a muted `↓ more`.
  - 120x40 end: `↑ more` and the Shift note.
  - 80x24.
  - 200x60 with no indicators.
  - Selector at 120x40, with the selector visible around the overlay.
  - The `copy = "c"` state.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. Verified locally; the tree stayed clean.

## Findings
1. [nit] In `src/components/help_overlay/mod.rs`, test `the_top_shows_a_down_indicator_only`, the assertion `rows[1].ends_with("─╮") || rows[1].contains("╮")` is weak: the second alternative makes the first pointless. Change: assert `rows[1].trim_end().ends_with("─╮")` directly, alongside the existing `!rows[1].contains("↑ more")`.
2. [nit] The `t` part of `quit_and_theme_keys_do_nothing_while_it_is_open` (`src/app/browser_tests/help.rs`) cannot fail yet, because nothing subscribes to the theme key so far. The AC is met. Change: none now. When the theme-selector task adds the `t` subscription in `GlobalEvents::subscriptions`, it must use `key_clause()` so that this test becomes meaningful.
3. [nit] `HelpContent::desired_size` (`src/components/help_overlay/content.rs`) sizes the width from the entries only. It does not count the Shift note at column 3 (44 columns needed) or the group headings. No realistic keymap reaches that case, because the widest entry already gives about 51 or more columns. Change, optional: include `HEADING_COLUMN - 1 + SHIFT_NOTE.width() + 2` (when `mouse_note` is set) in the width maximum.
