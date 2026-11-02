STATUS: DONE

## Changes (round 2 on top of round 1; round-1 notes are in `round-1/build-notes.md`)
- Round 2: see the Review section below.
- `src/app/resume.rs` (new): `ResumePlan { cwd, program, args }` (built from the harness's `Command`; never a shell), the `App` side (`request_resume`, `resume_answered`, `resume_declined`, `take_resume_plan`, `resume_prompt_open`, `set_dir_exists` seam for tests), and the hand-off: the `Steps` trait (`restore_terminal`, `change_dir`, `run`), the pure `hand_off(plan, &mut dyn Steps) -> i32` (restore, then chdir, then run; a failed step prints to stderr and gives 1) and `SystemSteps` (Unix: `CommandExt::exec`, only returns on error, behind `#[cfg(unix)]`; elsewhere: `status()` and its code).
- `src/app/mod.rs`: `Msg::RequestResume(Option<(HarnessId, SessionId)>)` (`None` = the open session, also from a subagent view, which resumes the top-level session); guarded `PromptSubmit` / `PromptCancel` / `PromptChanged` / `PromptNav` arms, placed above the other prompt arms, route the `y/n` answer to the resume state (so `y` in the selector does not reach the search); `ResumeState` field.
- `src/app/runtime.rs`: after the loop, `shutdown()` (index cache saved), `drop(app)`, then if a plan was confirmed the terminal guard is dropped as the first step of `hand_off` and the process exits with its code.
- `src/components/transcript/mod.rs`: `browser.resume` answers `Msg::RequestResume(None)`.
- `src/components/{list_pane,session_list,project_list}.rs`: `selector.resume` -> `PaneKey::Resume`; `SessionList` answers `RequestResume(Some((harness, selected)))` (nothing with no selection); `ProjectList` ignores it.
- `src/components/prompt.rs`: `Prompt::confirm_two` (question with two `foreground` subjects, the rest in `help_text`) for `Resume <id> in <Harness>? (y/n)`; `confirm` is unchanged.
- Tests: `src/app/browser_tests/resume.rs` (new, with FakeHarness gaining a resume template), component tests in transcript/session_list/prompt, and 4 snapshots.

Decisions not in the task:
- The request is ignored while a prompt, the help overlay or the theme selector is open.
- Checks in the planner's order: no resume command -> toast `No resume command configured for <Harness>`; then the cwd (the session's, else its project's; none known -> toast `Cannot resume: the working directory is unknown`); then `is_dir()` -> toast `Cannot resume: <cwd> no longer exists` (a path too long for the toast is cut at its start with `…`).
- The prompt takes the browser's help line styling (Background fill, inset 1) in the browser and the selector's (default fill, no inset) when the selector is shown, like the existing prompts.
- `App::set_dir_exists` is a test seam (default `Path::is_dir`), so the snapshots can use the fixed fixture path `/home/user/projects/brio`.
- `Ctrl-c` in the question quits without a plan.
- The unix `exec` path and `SystemSteps::run` are not run for a real program in tests (only a nonexistent program, whose `exec` error is returned).

## OpenCode verification (SPEC §11)
`opencode` is installed (`opencode v2.2.2`). `opencode --help` lists `--session, -s string    Session ID to continue, or to create if it does not exist`. So the default `opencode --session {id}` is valid on this version.

## Review (round 1 findings)
- Feature must-fix 1 (`R` in Projects with a session selected did nothing) — `ProjectList` now answers the new `SelectorMsg::Resume`; the `App` resolves it from the selector's current harness and selected session and does nothing when no session is selected. Tests: `r_with_the_projects_pane_focused_resumes_the_session_that_is_still_selected` (prompt text and plan, `y`), `r_with_the_projects_pane_focused_and_a_new_project_selected_does_nothing`, `r_with_no_session_selected_does_nothing`, component test `r_in_the_projects_list_leaves_the_selected_session_to_the_app`.
- Feature nit 2 (magic 56) — the test derives it from `MAX_TOAST_WIDTH - TOAST_CHROME`.
- UI nit 1 (toast cut at 60-63 columns) — `missing_dir_toast` now sizes the path for the real box, `min(MAX_TOAST_WIDTH, width - 4)`; test `the_missing_cwd_toast_fits_the_box_of_a_narrow_terminal` (60x24 reads `Cannot resume: …/user/projects/brio no longer exists`, all on screen).
- Code nit 1 (stale pending question) — `unmount_selector` clears the pending question; test `a_question_whose_prompt_is_unmounted_by_opening_a_session_is_forgotten`.
- Code nit 4 (stdout flush before exec) — done.
- Not done (nits): code nits 2 and 3 (enum for the target, option for the second subject), feature nit 3 (run_app wiring is a thin untested function, accepted by the planner), UI nit 2 (OpenCode short id keeps the `ses_` prefix; the planner binds the first 8 characters).
- The snapshots are unchanged.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2152 passed, 0 failed)
