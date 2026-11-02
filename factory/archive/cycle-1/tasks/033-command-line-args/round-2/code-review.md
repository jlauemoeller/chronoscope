VERDICT: PASS

## Summary
Round 2 fixes all three round-1 must-fixes. The `--config`/`--theme` wiring now lives in a testable `runtime::prepare`, which `run_app` calls, and tests go through it. The filtered selector keeps the resolver's match set, so `--harness` keeps restricting it after a tab switch, and the Projects pane keeps the startup preselection. Several round-1 nits are also addressed: the component test for `SESSION_FILTERED_ATTR`, the renamed theme test, `COMMAND_LINE_KEY`/`head`, the separate no-home error, reading only the `--harness` harness, and the filtered help line. The test gate passes, the working tree is clean and no `.snap.new` files exist. Only minor nits remain.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Round-1 findings re-checked
- Code 1 (must-fix, option wiring untested): fixed.
  - `prepare(cli, paths, env, working_dir)` maps `--config` to `Paths::with_config_file` and passes `cli.theme` to `AppInit::load_with_theme`.
  - `the_config_option_makes_the_app_load_and_save_that_file` and `the_theme_option_makes_the_app_show_that_theme_and_write_nothing` parse a real `Cli` and assert `theme_id`/`config_path`. They would fail if either mapping were dropped.
  - The e2e test `a_jsonl_path_with_claude_disabled_in_the_given_config_exits_with_1` shows the binary reads the given file.
- Feature 1 (must-fix, `--harness` lost after a tab switch): fixed. `Filter { prefix, matches }` holds the resolver's set, and `filtered_hits` lists only `(current harness, id)` pairs in it. `harness_keeps_restricting_the_filter_after_a_tab_switch` covers this and would fail under the old prefix-only filter.
- UI 1 (must-fix, Projects pane follows the first match): fixed. `show_filtered` uses `startup_project(harness)` (`preselection_in(Some(harness))`). `the_projects_pane_keeps_the_startup_selection_whatever_project_the_match_is_in` compares the Projects selection, the cwd and the left 38 columns with a normal startup, before and after `Down`.
- Code nits 3, 4, 7, 8, 9 and 11: addressed as the build notes describe. Nits 5, 6 and 10 are left open with reasons; they stay nits.

## Findings
1. [nit] `src/app/selector.rs` `filtered_hits`: `filter.matches.contains(&(harness, s.id.clone()))` allocates a `SessionId` clone for every session of the current harness on each push. This is off the hot path, but you can avoid it by storing `matches` as a `HashMap<HarnessId, HashSet<SessionId>>` (then `get(&harness).is_some_and(|ids| ids.contains(&s.id))`), or by borrowing through a `(HarnessId, &str)`-keyed lookup.
2. [nit] `src/app/selector.rs` `switch_harness` → `enter_harness` while filtered: switching tabs in filtered mode selects the new harness's *first* project, while `show_filtered` now uses `startup_project(harness)`. So the Projects pane after a tab switch can differ from the one shown at the start of the filter. This matches the normal tab-switch behaviour and the spec doesn't pin it. For consistency with the "Projects pane as at startup" rule, consider `enter_harness_at(harness, self.selector.startup_project(harness))` when `filtered.is_some()`.
3. [nit] `src/app/runtime.rs` `prepare`: `paths.clone()` appears in both match arms. `paths.cloned().map(|paths| match &cli.config { Some(c) => paths.with_config_file(c.clone()), None => paths })` reads slightly cleaner. This is cosmetic.
4. [nit] `src/app/browser_tests/launch.rs` `harness_keeps_restricting_the_filter_after_a_tab_switch`: the test passes both harnesses' index to `start_with`. In production, `resolve_startup` now reads only the `--harness` harness synchronously, and the other tab fills in from the async worker. The test still checks the right thing, because the filter must reject Claude's `85a8…` sessions whenever they arrive. A short comment saying so would make the intent clearer.
