VERDICT: FAIL

## Summary
The command line is implemented well. The clap struct, the pure resolver (`src/launch.rs`), direct open, `--theme`, `--config` and error reporting before the terminal is set up all have real tests, and the gate passes (fmt, clippy, all tests). One behavioural bug blocks: the filtered selector does not use the match set the resolver computed. It re-derives matches by prefix from whatever harness tab is current, so `--harness` no longer restricts the lookup once the user switches tabs.

## Acceptance criteria
- [x] `--help` lists the positional argument and `--latest`, `--harness`, `--project`, `--theme`, `--config`. `--latest` with a positional argument is rejected by clap. Implemented in `src/cli.rs`. Tests: `cli::tests::help_lists_the_positional_argument_and_every_option`, `cli::tests::latest_conflicts_with_the_positional_argument`, `tests/cli.rs::help_lists_…` and `latest_with_a_positional_argument_is_rejected_by_clap` (exit code 2).
- [x] Resolution unit tests on fixture indexes, in `src/launch/tests.rs`:
  - exact id: `an_exact_id_opens_that_session`, `an_exact_id_wins_over_longer_ids_that_start_with_it`;
  - unique prefix: `a_unique_prefix_opens_that_session`, `a_unique_prefix_is_found_in_any_harness`;
  - ambiguous prefix: `an_ambiguous_prefix_filters_the_selector_to_exactly_the_matches`, `…across_harnesses_lowest_number_first`;
  - no match: `no_match_is_an_error_that_names_the_argument`;
  - `--harness opencode` restricts: `harness_opencode_restricts_the_lookup`, `harness_restricts_an_ambiguity_to_one_harness`.

  The resolver itself is correct. See finding 1 for how the filtered selector later departs from its result.
- [x] A `.jsonl` path opens as a Claude session, including a file outside the data dir, and a missing path is an error. Implemented in `launch::resolve_target`, `ClaudeHarness::open_file` and the `Opened` map in `claude/subagent.rs`. Tests: `tests/claude_open_file.rs` (load and subagents next to a file outside the data dir), `browser_tests::launch::a_path_outside_the_data_directory_gives_a_claude_session_to_open`, `a_missing_jsonl_path_is_an_error`, `tests/cli.rs::a_missing_jsonl_path_exits_with_1_and_a_message`.
- [x] `--latest` opens the newest session of the `$PWD` project or of `--project`, and no match is an error. Implemented in `launch::latest`. Tests: `latest_*` in `src/launch/tests.rs`, the browser test `latest_opens_the_newest_session_of_the_working_directory_project`, and `tests/cli.rs::latest_without_a_project_for_the_directory_exits_with_1`.
- [x] `--project` by path (absolute, relative, compared by components) and by name or alias (case-insensitive) preselects the project. An unknown project warns and falls back. Implemented in `launch::find_projects` and `SelectorState::preselection` (the hint). Tests: `a_project_by_*`, `an_unknown_project_warns_and_falls_back`, and the browser tests `a_project_by_path_is_preselected`, `a_project_by_name_in_any_case_is_preselected`, `the_project_stays_preselected_when_the_index_worker_reports`, `an_unknown_project_warns_and_the_normal_preselection_is_used` (asserts the toast text through `launch_warnings`).
- [x] `--theme gruvbox` applies for the run without writing the config, and an unknown theme warns. Implemented in `AppInit::load_with_theme`. Tests: `startup::tests::theme_override_*`, `an_unknown_theme_override_warns_and_uses_the_default`, `browser_tests::launch::a_theme_given_for_the_run_is_applied_and_choosing_one_still_saves`.
- [x] `--config <path>` loads that file, and theme saving writes to it. Implemented with `Paths::with_config_file` in `run_app`. Tests: `startup::tests::the_config_option_loads_that_file_instead_of_the_default_one`, `a_missing_config_file_at_the_given_path_means_defaults`, `browser_tests::launch::choosing_a_theme_saves_it_to_the_config_file_given_with_config`.
- [x] An error exits with code 1 and a `chronoscope: …` message on stderr without touching the terminal. `resolve_startup` runs before `TerminalGuard::enter`, and `main.rs` prints the message and returns FAILURE. Tests in `tests/cli.rs` run the binary with a temp `HOME`/XDG dirs and assert exit code 1, the exact stderr and an empty stdout.
- [ ] The filtered selector looks and behaves as specified, and `Esc` leaves filtered mode. The startup view, Enter, arrows and Esc are correct and tested (`browser_tests::launch::the_filtered_selector_*`, `esc_leaves_the_filter_instead_of_quitting`, `after_esc_the_screen_is_the_normal_startup_screen`). After a tab switch, however, the list stops being "exactly the matching sessions" when `--harness` was given (finding 1).
- [x] Every UI state has a snapshot: `the_filtered_selector_120x40_text`, `the_filtered_selector_120x40_styled`, `after_esc_from_the_filtered_selector_120x40_text`, `a_direct_open_80x24_text` (utc timestamps, fixture data).
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass (re-run by the reviewer, with `INSTA_UPDATE=no` for the tests). The working tree is clean, with no `.snap.new` files.

## Findings
1. [must-fix] The filtered selector ignores the resolver's match set, so `--harness` stops restricting the lookup after a tab switch.
   - `start_with` (`src/app/runtime.rs`) uses `Target::Filtered { matches }` only to pick the starting tab (`matches.first()`).
   - From then on, `SelectorState::filtered_hits` (`src/app/selector.rs`) recomputes the rows from the current harness's data using only `starts_with(prefix)`.
   - Example: `chronoscope --harness opencode 85a8`, where OpenCode has several `85a8…` sessions and Claude has some too. The selector opens on OpenCode correctly. Pressing `1` then lists Claude's `85a8…` sessions as matches, although the restricted lookup excluded them.
   - This contradicts the planner's rule ("Search every registered harness, or only `--harness`") and the filtered-mode rule ("showing exactly the matching sessions").
   - The existing `a_filter_with_matches_in_another_harness_starts_on_that_harness` covers only the unrestricted case.

   Change:
   - Store the resolved match set in `SelectorState`. For example, replace `filtered: Option<String>` with a struct holding the prefix and a `HashSet<(HarnessId, SessionId)>` of `matches`, passed in through `show_filtered(prefix, matches)`.
   - Make `filtered_hits` keep only sessions of the current harness whose `(harness, id)` is in that set.
   - In `src/app/browser_tests/launch.rs`, add an update test. Resolve `Request { harness: Some(OPENCODE), target: "85a8" }` against an index where both harnesses have `85a8…` sessions, start with `start_with`, press `1`, and assert that `filtered_sessions()` is empty (or that the tab doesn't switch, if you choose that design instead).
2. [nit] In `run_app` (`src/app/runtime.rs`), `--config` is silently dropped when `Paths::detect()` returns `None` (no home directory). The user asked explicitly for that file. Change: when `cli.config` is set and no home is detected, still load from that path, or at least add a startup warning naming `--config`.
3. [nit] The help line in filtered mode still reads `ESC to Cancel`, but Esc now leaves the filter (noted in the build notes). Consider wording it so it doesn't imply closing or quitting while `filtered` is set.
4. [nit] When an override is present, `AppInit::load_with_theme` (`src/app/startup.rs`) relabels every theme warning as `--theme: …`, including any that aren't about the id. Change: relabel only the warnings that `resolve` raises for an unknown id.
5. [nit] Ambiguous matches spread over several harnesses show only the first harness's matches until the user switches tabs. Nothing on screen says other tabs have more. This is a documented design choice, but consider a hint (for example a count in the title) in a later task.
6. [nit] No test covers an id that is ambiguous within one harness while `--harness` names the other. `harness_opencode_restricts_the_lookup` covers `ses_` within OpenCode only. The test asked for in finding 1 would also close this gap at the UI level.
7. [nit] `a_project_by_name_or_alias_is_preselected_case_insensitively` (`src/launch/tests.rs`) uses plain fixture names, not a configured alias. Alias matching works only through `Project::name`. Consider adding one case with a `ClaudeHarness` built with `projects.aliases`, similar to `loose_harnesses` in `browser_tests/launch.rs`.
8. [nit] A positional argument, `--latest` or `--project` triggers a synchronous `HarnessIndex::read` through `index::collect`. The asynchronous index then reads everything again. This is acceptable because of the cache, but the startup work could be shared later.
