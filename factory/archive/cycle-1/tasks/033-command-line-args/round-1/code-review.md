VERDICT: FAIL

## Summary
A careful, well-structured implementation. The clap derive is clean. `launch::resolve` is a genuinely pure resolver with thorough fixture tests. Errors are reported before `TerminalGuard::enter`. Filtered mode, the direct-open path (including `Harness::open_file` with the `Opened` map) and the snapshots match the task's UI section. The test gate passes. One must-fix remains: nothing tests the wiring from the `--config` / `--theme` command-line options to their effect. The ACs for those options are backed only by tests that build `Paths` / `AppInit` by hand, so dropping the wiring in `run_app` would not fail any test.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [must-fix] `src/app/runtime.rs` `run_app` (the `Paths::detect().map(|paths| match &cli.config { ... with_config_file ... })` block and the `AppInit::load_with_theme(..., cli.theme.as_deref())` call). No test covers the step from the parsed `--config` / `--theme` options to `Paths::with_config_file` / `load_with_theme`.
   - The existing tests miss it:
     - `startup::tests::*` and `browser_tests::launch::choosing_a_theme_saves_it_to_the_config_file_given_with_config` / `a_theme_given_for_the_run_is_applied_...` construct `Paths`/`AppInit` directly.
     - `cli::tests` only checks parsing.
     - `tests/cli.rs::an_error_is_reported_with_an_alternative_config_file_too` passes an empty config file and would pass whether or not the file is read.
   - So deleting the `with_config_file` mapping, or passing `None` for the theme, would leave the suite green. The ACs "`--config <path>` loads that file ... (test)" and "`--theme gruvbox` applies Gruvbox for the run ... (tests)" are not effectively tested at the command-line level.
   - Change (required): extract the pre-terminal part of `run_app` into a testable function, e.g. `fn prepare(cli: &Cli, paths: Option<Paths>, env: &TerminalEnv, working_dir: &Path) -> anyhow::Result<(AppInit, Startup)>`, and have `run_app` call it. Add unit tests for both options:
     - a `Cli` parsed from `["chronoscope", "--config", <tmp file with theme = "nord">]` yields `init.theme_id == "nord"` and `init.config_path == Some(<tmp file>)`;
     - `--theme gruvbox` yields `theme_id == "gruvbox"` and leaves the file unchanged.
     The suite must fail if the `with_config_file` mapping is removed, and also if `None` is passed for the theme.
   - Optional extra end-to-end check in `tests/cli.rs`: write a config file with `[harness.claude]\nenabled = false`, pass `--config <it>` and an *existing* `.jsonl` path, and assert exit code 1 with `chronoscope: the Claude harness is disabled in the config; cannot open a .jsonl file`.
2. [nit] `src/app/selector.rs` `filtered_hits` / `switch_harness`: in filtered mode the rows are recomputed from the *current tab's* data by prefix. The resolved `Target::Filtered { matches }` is used only to choose the first tab. With `--harness claude 85a8`, pressing `2` lists OpenCode sessions starting with `85a8`, which the `--harness`-restricted lookup excluded. The initial screen is correct, and tab switching in filtered mode isn't specified, so this is not a must-fix. Consider either:
   - ending the filter on a tab switch to a harness outside `--harness`; or
   - keeping the resolved match set (or the `--harness` scope) on `SelectorState` and filtering `filtered_hits` by it.
   Add an update test for whichever you choose.
3. [nit] `src/components/session_list.rs`: there is no component-level test for the new `SESSION_FILTERED_ATTR` flag. SPEC §9.6 asks for component `on()` → `Msg` checks. Add one in the module's tests: with the flag set, `Esc` returns `Msg::Selector(SelectorMsg::LeaveFilter)`; with it cleared, the usual cancel message. The behaviour is already covered at App level (`esc_leaves_the_filter_instead_of_quitting`).
4. [nit] `src/app/browser_tests/launch.rs` `a_theme_given_for_the_run_is_applied_and_choosing_one_still_saves`: the name promises that choosing a theme still saves, but the test never opens the theme selector or chooses a theme. Either rename it (e.g. `a_theme_given_for_the_run_is_applied_without_writing`), or add `rig.open_themes(); rig.app.update(Msg::SelectTheme("nord".into()))` and assert the file then contains `theme = "nord"`.
5. [nit] `src/launch.rs` `is_path` / `resolve_target`: `Path::new(target).is_file()` is resolved against the process cwd rather than the `working_dir` parameter, so the "pure over `working_dir`" contract doesn't fully hold. They are the same in production. Consider `working_dir.join(target)` for the existence checks, and keep the original string for messages.
6. [nit] `src/app/startup.rs` `load_with_theme`: when an override is present, *every* warning from `theme::resolve` is relabelled `--theme: ...`. Today `resolve` only emits the unknown-id warning, so this is correct, but it is fragile if more warning kinds are added. Consider relabelling only when the warning concerns the id.
7. [nit] `src/app/hints.rs` `startup_toast_text`: the `Config: ` prefix is chosen from `warnings[0]` only. A config warning followed by a command-line warning reads `Config: ... (+1 more, see log)`, which is acceptable. You already bind `first`, so consider naming the first warning once (`let (head, rest) = ...`) instead of indexing `warnings[0]` again. Also, the synthetic key `"--args"` in `runtime::launch_warnings` is opaque. A named constant (e.g. `COMMAND_LINE_KEY = "--command-line"`) would document the convention the toast relies on.
8. [nit] `src/app/runtime.rs` `resolve_startup`: when Claude is enabled but no harnesses were built (no home directory), a `.jsonl` path reports `the Claude harness is disabled in the config`, which is misleading in that case. Consider a separate error, e.g. "the Claude data directory is unknown".
9. [nit] `src/app/runtime.rs` `resolve_startup`: with `--harness` set, the synchronous read still reads every harness (`HarnessIndex::read` over all `harnesses`), although `resolve` only looks at the restricted one. Filtering by `request.harness` before reading would save a full OpenCode/Claude scan at startup.
10. [nit] Windows CI (§9.1/§9.3), mostly a pre-existing pattern:
    - `tests/cli.rs::latest_without_a_project_for_the_directory_exits_with_1` compares against `cwd.path().canonicalize()`, which on Windows yields a `\\?\C:\...` verbatim path that will not match `current_dir()`.
    - The `launch::tests` fixtures use `/work/...` cwds, which are not `is_absolute()` on Windows, so the `latest_*` / deepest-ancestor tests would not match there.
    - The existing `selection_tests` on `main` use the same Unix-path pattern, so this is not new. When Windows CI is addressed, compare by the `current_dir()` the child reports (or strip the verbatim prefix), and build fixture cwds from a platform root.
11. [nit] The selector help line still reads `ESC to Cancel` in filtered mode, where `Esc` leaves the filter (noted in the build notes). This is acceptable since the spec doesn't pin it; a `ESC to clear filter` hint would be clearer.
