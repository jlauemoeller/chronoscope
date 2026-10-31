STATUS: DONE

## Changes
- `src/app/resume.rs` (new): `ResumePlan { cwd, program, args }` (built from the harness's `Command`; never a shell), the `App` side (`request_resume`, `resume_answered`, `resume_declined`, `take_resume_plan`, `resume_prompt_open`, `set_dir_exists` seam for tests), and the hand-off: the `Steps` trait (`restore_terminal`, `change_dir`, `run`), the pure `hand_off(plan, &mut dyn Steps) -> i32` (restore, then chdir, then run; a failed step prints to stderr and gives 1) and `SystemSteps` (Unix: `CommandExt::exec`, only returns on error, behind `#[cfg(unix)]`; elsewhere: `status()` and its code).
- `src/app/mod.rs`: `Msg::RequestResume(Option<(HarnessId, SessionId)>)` (`None` = the open session, also from a subagent view, which resumes the top-level session); guarded `PromptSubmit` / `PromptCancel` / `PromptChanged` / `PromptNav` arms, placed above the other prompt arms, route the `y/n` answer to the resume state (so `y` in the selector does not reach the search); `ResumeState` field.
- `src/app/runtime.rs`: after the loop, `shutdown()` (index cache saved), `drop(app)`, then if a plan was confirmed the terminal guard is dropped as the first step of `hand_off` and the process exits with its code.
- `src/components/transcript/mod.rs`: `browser.resume` answers `Msg::RequestResume(None)`.
- `src/components/{list_pane,session_list,project_list}.rs`: `selector.resume` -> `PaneKey::Resume`; `SessionList` answers `RequestResume(Some((harness, selected)))` (nothing with no selection); `ProjectList` ignores it.
- `src/components/prompt.rs`: `Prompt::confirm_two` (question with two `foreground` subjects, the rest in `help_text`) for `Resume <id> in <Harness>? (y/n)`; `confirm` is unchanged.
- Tests: `src/app/browser_tests/resume.rs` (new, with FakeHarness gaining a resume template), component tests in transcript/session_list/prompt, and 4 snapshots.

Decisions not in the task:
- `R` is handled by the Sessions pane only. With the Projects pane focused it does nothing, even when a session is still selected from an earlier visit to Sessions: the spec's "selected row" is a session row, and the Projects pane has the focus on a project row.
- The request is ignored while a prompt, the help overlay or the theme selector is open.
- Checks in the planner's order: no resume command -> toast `No resume command configured for <Harness>`; then the cwd (the session's, else its project's; none known -> toast `Cannot resume: the working directory is unknown`); then `is_dir()` -> toast `Cannot resume: <cwd> no longer exists` (a path too long for the toast is cut at its start with `…`).
- The prompt takes the browser's help line styling (Background fill, inset 1) in the browser and the selector's (default fill, no inset) when the selector is shown, like the existing prompts.
- `App::set_dir_exists` is a test seam (default `Path::is_dir`), so the snapshots can use the fixed fixture path `/home/user/projects/brio`.
- `Ctrl-c` in the question quits without a plan.
- The unix `exec` path and `SystemSteps::run` are not run for a real program in tests (only a nonexistent program, whose `exec` error is returned).

## OpenCode verification (SPEC §11)
`opencode` is installed (`opencode v2.2.2`). `opencode --help` lists `--session, -s string    Session ID to continue, or to create if it does not exist`. So the default `opencode --session {id}` is valid on this version.

## Self-check
- AC: `R` in the browser shows `Resume <short id> in Claude? (y/n)` — `r_in_the_browser_asks_to_resume_the_open_session`, `resume_confirmation_in_the_browser_80x24_text`/`_styled`, `the_confirmation_names_and_the_harness_are_in_foreground_and_the_rest_in_help_text`, transcript `r_asks_to_resume_the_open_session_whatever_is_selected`.
- AC: `R` on a selected session in the selector / nothing selected does nothing — `r_on_a_selected_session_in_the_selector_asks_about_that_session`, `r_with_no_session_selected_does_nothing`, `r_in_the_selector_over_the_browser_resumes_the_selected_session_not_the_open_one`, `r_in_a_subagent_transcript_resumes_the_top_level_session`, `r_on_an_opencode_session_asks_about_opencode_and_plans_its_session_flag`, session_list `r_asks_to_resume_the_selected_session` / `r_with_no_session_selected_does_nothing` / `r_in_the_projects_list_does_nothing`.
- AC: `y` gives the plan (cwd, `claude`, `["--resume", id]`) and quits; `n`/`Esc` give none and restore focus — `y_confirms_the_plan_and_quits`, `capital_y_confirms_too`, `y_in_the_selector_confirms_instead_of_submitting_a_search`, `n_esc_and_other_keys_cancel_and_return_the_focus_in_the_browser`, `n_and_esc_cancel_and_return_the_focus_in_the_selector`, `ctrl_c_in_the_question_quits_without_a_plan`, `a_question_cancelled_can_be_asked_again`.
- AC: a custom template is honoured — `a_custom_template_is_honoured_in_the_plan`, `the_template_from_the_config_is_what_the_plan_is_made_of` (real `ClaudeHarness::new` / `OpenCodeHarness::new` with `Config`).
- AC: a missing cwd gives the toast and no prompt — `a_cwd_that_no_longer_exists_shows_a_toast_and_no_prompt` (temp dir removed, real check), `a_cwd_that_is_a_file_is_not_a_directory_to_resume_in`, `a_long_missing_cwd_is_cut_at_its_start_so_the_toast_fits`.
- AC: a harness without a command gives the toast — `a_harness_without_a_resume_command_shows_a_toast_and_no_prompt`, `an_unusable_template_counts_as_no_command`, `a_selected_session_of_a_harness_that_is_not_loaded_has_no_command`, `an_opencode_without_a_command_says_so_by_its_name`.
- AC: terminal restored before chdir and exec, verified through a seam; Unix `exec` behind `#[cfg(unix)]` — `the_terminal_is_restored_before_the_directory_is_changed_and_the_command_runs`, `the_exit_code_of_the_command_is_the_exit_code_of_the_process`, `a_directory_that_cannot_be_entered_ends_with_code_1_and_nothing_runs`, `a_command_that_cannot_be_started_ends_with_code_1`, `the_real_steps_replace_the_process_only_on_unix_and_report_a_missing_program`.
- AC: opencode verification recorded — see above.
- UI: Browser with the confirmation @ 80x24 (text + styled) — `resume_confirmation_in_the_browser_80x24_text`, `resume_confirmation_in_the_browser_80x24_styled`
- UI: Selector with the confirmation, a session selected @ 120x40 (text) — `resume_confirmation_in_the_selector_120x40_text`
- UI: Missing-cwd error toast over the browser @ 80x24 (text) — `the_missing_cwd_toast_over_the_browser_80x24_text`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2148 passed, 0 failed)
