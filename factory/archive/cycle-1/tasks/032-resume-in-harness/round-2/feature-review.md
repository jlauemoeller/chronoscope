VERDICT: PASS

## Summary
Round 2 fixes the round-1 must-fix. With the Projects pane focused and a session still selected, `R` now resumes that session, and with no session selected it still does nothing. Round 2 also takes up the toast-width nit, the stale-question nit and the stdout-flush nit. I traced every acceptance criterion through the code and the tests, and each one is covered by a test that would fail if the behaviour were wrong. I re-ran the gates: `cargo fmt --check` is clean, clippy with `-D warnings` is clean, and `INSTA_UPDATE=no cargo test` has 0 failures. The working tree is still clean afterwards.

## Acceptance criteria
- [x] `R` in the browser shows `Resume <short id> in Claude? (y/n)` in the help line. `browser.resume` maps to `Msg::RequestResume(None)` in `src/components/transcript/mod.rs`, and `App::request_resume` is in `src/app/resume.rs`. Covered by `r_in_the_browser_asks_to_resume_the_open_session` and the snapshots `resume_confirmation_in_the_browser_80x24_text` / `_styled`. The subagent case is covered by `r_in_a_subagent_transcript_resumes_the_top_level_session`.
- [x] `R` on a selected session in the selector shows the same prompt for that session, and with no session selected it does nothing.
  - Sessions pane: `SessionList` answers `RequestResume(Some(..))` only when a session is selected.
  - Projects pane: `ProjectList` now answers `SelectorMsg::Resume`, and `App::selector_msg` in `src/app/selector.rs` resolves it from `selector.current` and `selector.session`. With no session it does nothing, and it leaves `preselecting` alone.
  - Tests: `r_on_a_selected_session_in_the_selector_asks_about_that_session`, `r_with_the_projects_pane_focused_resumes_the_session_that_is_still_selected`, `r_with_the_projects_pane_focused_and_a_new_project_selected_does_nothing`, `r_with_no_session_selected_does_nothing`, and the component test `r_in_the_projects_list_leaves_the_selected_session_to_the_app`.
  - The Projects test goes through the real key path, so it would fail if `ProjectList` dropped the key. It also checks the prompt text and the plan after `y`.
- [x] `y` gives a `ResumePlan` with the session cwd, program `claude` and args `["--resume", id]`, and sets quit. `n`/`Esc` give no plan and return the focus. This is handled by `resume_answered` / `resume_declined` and by the guarded prompt arms at `src/app/mod.rs:637-639`. Covered by `y_confirms_the_plan_and_quits`, `y_in_the_selector_confirms_instead_of_submitting_a_search`, and the two cancel-and-focus tests.
- [x] A custom template from config is used in the plan. Covered by `a_custom_template_is_honoured_in_the_plan` and `the_template_from_the_config_is_what_the_plan_is_made_of`. The plan is built from `Command::get_program`/`get_args`, with no shell.
- [x] A missing cwd shows the `Cannot resume: … no longer exists` toast and no prompt. Covered by `a_cwd_that_no_longer_exists_shows_a_toast_and_no_prompt`, which uses a real temp dir that the test removes. The toast is now sized for the real toast box, `min(MAX_TOAST_WIDTH, width - 4)`, and `the_missing_cwd_toast_fits_the_box_of_a_narrow_terminal` checks this at 60x24.
- [x] A harness without a resume command shows the `No resume command configured` toast. Covered by `a_harness_without_a_resume_command_shows_a_toast_and_no_prompt` and `an_opencode_without_a_command_says_so_by_its_name`.
- [x] The executor restores the terminal before it changes directory and execs, the order is checked through a seam, and Unix `exec` is behind `#[cfg(unix)]`. `hand_off` and the `Steps` trait are in `src/app/resume.rs`, and the call order is recorded by `the_terminal_is_restored_before_the_directory_is_changed_and_the_command_runs`. `SystemSteps::run` has `#[cfg(unix)]` (exec) and `#[cfg(not(unix))]` (status) variants. stdout is now flushed before `exec`.
- [x] The `opencode --help` result is in the build notes: opencode v2.2.2 lists `--session, -s`.
- [x] Every UI state has a snapshot: the browser confirmation at 80x24 (text and styled), the selector confirmation at 120x40 (text), and the missing-cwd toast at 80x24 (text). The snapshots are unchanged in round 2.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran all three.

## Previous round
- Feature must-fix 1 (Projects pane): done, see AC 2.
- Feature nit 2 (magic 56): done. The test now derives the bound from `MAX_TOAST_WIDTH - TOAST_CHROME`.
- UI nit 1 (toast cut at 60-63 columns): done and tested.
- Code nit 1 (stale pending question): done. `unmount_selector` calls `clear_resume_question`, and `a_question_whose_prompt_is_unmounted_by_opening_a_session_is_forgotten` checks that a later search prompt answered with `y` does not quit.
- Code nit 4 (stdout flush): done.
- Code nits 2 and 3, feature nit 3 and UI nit 2 were left as they are, with reasons. All of them are nits.

## Findings
1. [nit] `src/app/selector.rs`, `selector_msg`: `SelectorMsg::Resume` is handled by an early `if msg == SelectorMsg::Resume { … return; }`, and the `match` below needs a dead `SelectorMsg::Resume => {}` arm. Change: drop the dead arm. Then either handle `Resume` first in the `match` (moving `self.selector.preselecting = false;` into the other arms), or use a `let SelectorMsg::Resume = msg else { … }` guard, so that only one place handles it.
2. [nit] Round-1 code nit 2 still applies: `Msg::RequestResume(Option<(HarnessId, SessionId)>)` uses `None` to mean "the open session", and the Projects path now adds a third way in through `SelectorMsg::Resume`. Change (optional): use a `ResumeTarget` enum.
