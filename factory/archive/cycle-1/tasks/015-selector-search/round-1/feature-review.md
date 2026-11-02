VERDICT: FAIL

## Summary
The `Prompt` component, the focus stack, gating the global keys while the prompt is mounted, the `nucleo` search across the harness's projects, the results mode of `SessionList` and the worker path are all in place. Each acceptance criterion has a test that would catch a regression. The gate passes: fmt, clippy and `INSTA_UPDATE=no cargo test` are all green, and I re-ran the release timing myself (worst `flaky timeout` about 23 ms, so the worker is required). One bug remains on the worker path, which is the production path: a live snapshot that arrives while a keystroke's search is still pending turns "select the first result" into "keep the old selection".

## Acceptance criteria
- [x] `Prompt` component tests (abc / Backspace / ← text, cursor and `PromptChanged`; Enter / Esc / ↑ / Ctrl-c; `q`, `t` as text; YesNo `y` / `n`). Implemented in `src/components/prompt.rs`. Tests: `typing_inserts_characters_and_reports_the_text`, `backspace_*`, `left_moves_the_cursor_and_typing_inserts_there`, `enter_submits_the_text`, `esc_cancels`, `up_and_down_navigate`, `ctrl_c_quits_in_both_modes`, `q_t_digits_and_h_are_text`, `y_submits_and_n_cancels`, `every_other_key_cancels_a_yes_no_prompt`.
- [x] Global keys don't fire while the prompt has focus. Implemented by `key_clause()` = `Not(IsMounted(Prompt))` in `src/components/global_events.rs`. `q_typed_in_the_prompt_does_not_quit_but_is_typed` drives the real Port → `tick()` path and has a control test, `without_the_prompt_the_same_key_quits`. Resize, tick and worker events keep `Always`.
- [x] Focus stack: Projects → Projects, Sessions → Sessions. Implemented by `open_overlay` / `close_overlay` in `src/app/mod.rs`. Tests: `cancelling_from_projects_returns_the_focus_to_projects`, `cancelling_from_sessions_returns_the_focus_to_sessions`, `the_focus_stack_is_empty_again_after_every_search`.
- [x] `brio` matches by title, first prompt or project name, across all projects of the current harness only, sorted by score with ties newest first. Implemented by `search_sessions` in `src/app/search.rs`. Unit tests: `the_title_the_first_prompt_and_the_project_name_are_searched`, `a_session_matches_through_*_alone`, `only_the_projects_given_are_searched`, `results_are_sorted_by_score_then_newest_first`, `equal_scores_are_ordered_newest_first_then_by_id`. App-level tests: `typing_brio_lists_the_matches_of_every_project`, `only_sessions_of_the_current_harness_are_listed`.
- [x] During a search, Tab / ← / → don't change the pane, ↑ / ↓ move the result selection (`SEARCH_NAV_ATTR`), and Enter queues `Msg::OpenSession`. Tests: `tab_arrows_h_and_l_do_not_change_the_pane_during_a_search`, `up_and_down_move_the_selection_of_the_results`, `enter_opens_the_selected_result`, `enter_without_a_result_does_nothing`.
- [x] Esc from "Projects focused, no session selected" restores exactly that state; Esc from Sessions restores Sessions with the previous selection. Tests: `esc_from_projects_without_a_session_returns_to_exactly_that` (compares pane, session and both lists' rows), `esc_from_sessions_returns_to_sessions_with_the_previous_selection`, and the after-Esc screen and style equality tests.
- [x] The 5 000-session measurement is in the build notes, and the worker choice follows the rule (worst case > 16 ms). My release run on this machine: `flaky timeout` 23.0 / 23.2 ms, `impo` worst 19.8 ms, about 1.6 ms per keystroke with the worker.
- [x] Every UI state has a snapshot:
  - prompt open with an empty query, 120x40, text and styled. "Panes unchanged" here means normal content, not results. SPEC §4.1 makes `/` focus Sessions and select the newest session, which is what the snapshot shows.
  - `brio`, 120x40 text and styled, and 80x24 text. Results come from Agentmon, BRIO and Calira; the ring is on Sessions; Projects is drawn bold in `secondary_foreground` (250,250,250 in `default`) with `>`.
  - no results, 80x24 text.
  - after Esc, 120x40 text.
  - the `Prompt` component at width 30, scrolled.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran all three.

## Findings
1. [must-fix] With the worker, a live update can overwrite the pending keystroke request's `keep_selection: false`. Steps:
   - The user types a key. `search_changed` → `push_search_results(false)` sets `pending = {seq N, keep_selection: false}`.
   - An index snapshot arrives before answer N. `push_sessions` (`src/app/selector.rs:619`) calls `push_search_results(true)`, and `src/app/search.rs:299-302` replaces the pending entry with `{seq N+1, keep_selection: true}`.
   - Answer N is dropped as stale. `list_search_results` for N+1 then takes `previous` from the list, which still shows the old query's rows, and keeps that row selected if it is among the new hits.

   The planner note says the first result is selected when the query changes; with this race it isn't. Snapshots arrive often while any session is active, so this can happen during normal typing, and no test covers it.

   Change: in `push_search_results`, combine the flags instead of overwriting them, i.e. `keep_selection: keep_selection && search.pending.map_or(true, |p| p.keep_selection)`. Then add a worker test to `src/app/search_tests.rs`:
   - Type a query and drain its answer.
   - Move the selection down.
   - Send `PromptChanged(<new query>)` and, before draining answers, `send` a snapshot.
   - Drain the answers and assert that `session_rows(&app).selected == Some(0)`.
2. [nit] Deviation from the planner text, documented in the build notes. A search started from Projects *with* a session selected (reached via `Tab` back to Projects) returns to Projects on Esc. The planner note says "Otherwise it returns to Sessions focused". This follows SPEC §1.4 ("returns focus to the component that had it before") and the focus-stack AC, which both outrank the planner note, and `esc_from_projects_with_a_session_selected_returns_to_that_state` pins it. I'm flagging it so the Approver sees the choice; no change needed unless the planner disagrees.
3. [nit] `submit_when_ready` (`src/app/search.rs:248`) survives later keystrokes. Enter pressed while a search is pending, followed by more typing before the answer arrives, opens the first result of the *newer* query. Change: in `search_changed`, reset `submit_when_ready = false` when the query text actually changes, not only when it becomes empty.
4. [nit] The timing numbers in the build notes vary by machine. On this machine `im` / `imp` averaged about 15 ms and `impo` reached a 19.8 ms worst case, against the build notes' 9.8 / 10.1 / 13.7 ms. The conclusion (worker) stands. Consider noting in the build notes that the figures are machine-dependent, and that queries other than `flaky timeout` also exceed 16 ms on some machines.
5. [nit] `PromptSubmit`, `PromptCancel`, `PromptNav` and `PromptChanged` are routed unconditionally to the search handlers in `App::update` (`src/app/mod.rs`). That is correct today because search is the only prompt. When the path and y/n prompts arrive, the routing will need to know which prompt is open, for example an owner or kind kept next to the focus-stack entry.
