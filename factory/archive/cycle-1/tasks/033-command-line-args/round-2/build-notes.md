STATUS: DONE

## Changes
- `app/runtime.rs`: the pre-terminal part of `run_app` is now `prepare(cli, paths, env, working_dir) -> Prepared { init, harnesses, startup }`, which maps `--config` to `Paths::with_config_file`, `--theme` to `AppInit::load_with_theme`, builds the harnesses and resolves the command line. `run_app` calls it. `--config` without a home directory now adds a startup warning (`--config: ignored, the home directory is unknown`). With `--harness`, only that harness is read synchronously. A `.jsonl` path with Claude enabled but no home directory gets its own error (no longer "disabled in the config"). Command-line warnings use `hints::COMMAND_LINE_KEY`.
- `app/selector.rs`: the filter now stores the resolver's match set (`Filter { prefix, matches }`, `show_filtered(prefix, &matches)`); `filtered_hits` lists only sessions of the current harness that are in it, so `--harness` keeps restricting the lookup after a tab switch. `show_filtered` no longer moves the Projects selection / Working Directory to the first match's project: it uses the startup preselection (`startup_project`, `preselection_in`) of the shown harness. In filtered mode the help line reads `ESC to show all sessions` (`filtered_selector_hints`); `Esc` restores the normal hints.
- `app/hints.rs`: `COMMAND_LINE_KEY`, `head` naming in `startup_toast_text`, `filtered_selector_hints`.
- Snapshots `the_filtered_selector_120x40_{text,styled}` re-accepted (only the hint text changed).

## Self-check
- Review (code 1, must-fix): `--config`/`--theme` wiring untested — `browser_tests::launch::the_config_option_makes_the_app_load_and_save_that_file`, `the_theme_option_makes_the_app_show_that_theme_and_write_nothing` (both go through `prepare`; they fail if the `with_config_file` mapping or the theme argument is removed); plus `the_config_file_decides_which_harnesses_a_path_may_use` and e2e `tests/cli.rs::a_jsonl_path_with_claude_disabled_in_the_given_config_exits_with_1`.
- Review (feature 1, must-fix): `--harness` after a tab switch — `harness_keeps_restricting_the_filter_after_a_tab_switch`, `without_harness_both_tabs_list_their_matches`.
- Review (UI 1, must-fix): Projects pane changes with first match — `the_projects_pane_keeps_the_startup_selection_whatever_project_the_match_is_in` (first match in BRIO, startup project Agentmon; compares Projects selection, cwd box and the left part of the screen with the normal startup, before and after Down).
- Review (code 3, nit): component test `session_list::tests::esc_leaves_the_filter_only_while_the_rows_are_filtered`.
- Review (code 4, nit): test renamed `a_theme_given_for_the_run_is_applied_without_writing` and extended to choose a theme and see it saved.
- Review (code 7, nit): `COMMAND_LINE_KEY`, `head`.
- Review (code 8, nit): separate no-home error. (code 9, nit): read only the `--harness` harness.
- Review (feature 2, nit): `--config` without a home warns — `the_config_option_without_a_home_directory_warns`.
- Review (code 11 / UI 3 / feature 3, nits): filtered help line — `the_filtered_selector_hints_say_esc_shows_all_sessions`.
- Not done (nits): code 2 beyond the above (tab switch ends no filter), 5 (`is_path` against `working_dir`), 6 (relabelling in `load_with_theme`: all warnings of `theme::resolve` are about the id), 10 (Windows CI), feature 5/7/8, UI 2.
- UI snapshots (unchanged names): filtered 120x40 text/styled, after Esc 120x40 text, direct open 80x24 text.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2249 passed, 0 failed)
