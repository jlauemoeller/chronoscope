STATUS: DONE

## Changes
- `src/components/prompt.rs` (new): the `Prompt` component (`PromptMode::{Text, YesNo}`), `tui-input` editing, `Ctrl-c` quits, horizontal scroll, cursor cell with bg `cursor`, painted with the `surface` fill (skipped for transparent themes). Messages: `PromptChanged`, `PromptSubmit`, `PromptCancel`, `PromptNav(Direction)`, plus `PromptMoved` (cursor-only move; asks for a redraw only, so `PromptChanged` always means "text changed" and re-runs the search). It also takes `Home`/`End`/`Delete` and pasted text (editing keys; no printable key is intercepted).
- `src/app/mod.rs`: `Id::Prompt`, the new `Msg`s, `Msg::SearchResults`, `UserEvent::Search`, the App-level **focus stack** (`open_overlay` pushes the focused id and activates the overlay; `close_overlay` unmounts and pops/restores; later overlays reuse them), a message queue (`emit` / `take_queued`; `tick` hands queued messages out after the components'), `set_search_worker`, `shutdown` also stops the search worker, `production_listener(index, search)`.
- `src/components/global_events.rs`: the key subscriptions now use `key_clause()` = `Not(IsMounted(Prompt))`; resize, tick, index and search events keep `Always` (previously one shared clause; gating that would also have stopped resizes/ticks during a search). `Ctrl-c` is handled by the Prompt itself. Maps `UserEvent::Search` to `Msg::SearchResults`.
- `src/app/layout.rs`: `LayoutInput::prompt`; an active prompt replaces `HelpLine` in the same rect (toast stays last).
- `src/app/search.rs` (new): `search_sessions(query, projects, sessions) -> Vec<Hit>` (pure; `nucleo` `Pattern` with `CaseMatching::Smart`, fuzzy atoms; best of title / first prompt / project name, several words may match across the fields; sorted by score, ties newest `started_at` first, then id) and the App's search handlers (start, change, nav, submit, cancel, listing results).
- `src/app/search_worker.rs` (new): `SearchWorker` thread (skips requests overtaken by a newer one), `SearchPort` (sync `Poll` -> `UserEvent::Search`), `SearchUpdate` (Arc payload, equal by seq). The App ignores answers whose seq is not the pending one. Without a worker (tests) the App searches in place through the same code path.
- `src/app/runtime.rs`: starts the search worker and its port in the real app.
- `src/app/selector.rs`: `SelectorMsg::Search`, `search` state, results replace the project's sessions in `push_sessions` while a query is non-empty (so live snapshots re-run the search instead of clobbering results), stats reach result rows from any project, `active_ids` covers the whole harness while results are listed (so a tick does not wipe the dots of other projects' rows).
- `src/components/session_list.rs`, `list_pane.rs`, `project_list.rs`, `empty_state.rs`, `mod.rs`: results mode (`SessionRows::search`, `SessionRow::project`), title ` Sessions matching "<q>" `, rows `<ts>  <dot><project> › <title>`, `No matching sessions`, `SEARCH_NAV_ATTR`, `PaneKey::Search` (resolved from `Action::Search` in the keymap) in both lists.
- `src/app/fixtures.rs`: `search_snapshot` (new multi-project fixture; existing fixtures untouched) and `big_data` (5 000 sessions).

## Measurement (5 000 sessions, release build, 24-core machine)
Synthetic harness: 50 projects, 5 000 sessions, titles ~55 chars, first prompts ~430 chars. Test: `cargo test --release --lib -- --ignored --nocapture five_thousand` (`five_thousand_sessions_one_keystroke_timing`). Whole path of one keystroke (`PromptChanged`: match, sort, build rows, hand them to the list), on the UI thread, mean / worst of 20:

| query | results | mean | worst |
|---|---|---|---|
| `i` | 5000 | 2.2 ms | 3.8 ms |
| `im` | 5000 | 9.8 ms | 10.0 ms |
| `imp` | 5000 | 10.1 ms | 10.3 ms |
| `impo` | 5000 | 13.0 ms | 13.7 ms |
| `brio` | 5000 | 9.5 ms | 9.6 ms |
| `flaky timeout` | 5000 | 23.6 ms | 25.9 ms |
| `zzz` | 0 | 0.2 ms | 0.5 ms |

Matching alone (`search_sessions`) measured 12.4 ms (`im`), 17.5 ms (`impo`) and 33 ms (`flaky timeout`). Worst case > 16 ms, so by the planner rule the search **runs on a worker** (`SearchWorker`, answers delivered as `UserEvent::Search`). With the worker the UI thread pays about 2 ms mean / 2.9 ms worst per keystroke (sending the request and listing 5 000 answer rows), 0.05 ms for a query with no results.

## Decisions (not in the spec)
- `/` from Projects follows §4.1: Sessions gets the focus and, with no session selected, the newest session is selected (that is why `Esc` must "return" to Projects / no session). Snapshot 1 therefore shows the Sessions ring and the newest session with the normal content, Projects unfocused-selected.
- `Esc` restores the exact state the search started from (pane, selected session). The one case where the planner text and the focus-stack AC differ is "search started from Projects with a session selected" (reachable via `Tab` back to Projects): it returns to Projects with that session, as the focus stack says; if the session has vanished meanwhile, Sessions gets the newest one / Projects gets none.
- `Enter` in the prompt queues `Msg::OpenSession(harness, id)` (`update` returns nothing; `take_queued()` observes it in tests, `tick()` hands it out so the browser, task 021, handles it). The search stays open after `Enter` (the browser closes the selector). With no result, or an empty query, nothing happens. `Enter` pressed while the worker is still searching opens the first result as soon as the answer arrives.
- While a worker request is pending the pane keeps showing the previous rows (about one frame).
- With an empty (or blank) query the Sessions pane shows the normal content; `↑`/`↓` do nothing then.

## Self-check
- AC: Prompt component tests (abc / Backspace / Left give text, cursor and PromptChanged / PromptMoved) — `components::prompt::tests::typing_inserts_characters_and_reports_the_text`, `backspace_deletes_the_character_before_the_cursor`, `left_moves_the_cursor_and_typing_inserts_there`, `backspace_after_left_deletes_before_the_cursor`
- AC: Enter/Esc/Up/Ctrl-c — `enter_submits_the_text`, `esc_cancels`, `up_and_down_navigate`, `ctrl_c_quits_in_both_modes`
- AC: `q` and `t` are text — `q_t_digits_and_h_are_text`
- AC: YesNo — `y_submits_and_n_cancels`, `every_other_key_cancels_a_yes_no_prompt`
- AC: global keys don't fire while the prompt has focus — `app::search_tests::q_typed_in_the_prompt_does_not_quit_but_is_typed` (drives the real event path through a test Port and `tick()`, with the control `without_the_prompt_the_same_key_quits`), `the_other_default_quit_keys_are_typed_or_ignored_in_the_prompt_too`, `after_the_prompt_closes_the_global_keys_work_again`, `ctrl_c_quits_from_the_prompt`, `resizes_and_index_events_still_arrive_while_the_prompt_has_focus`
- AC: focus stack — `cancelling_from_projects_returns_the_focus_to_projects`, `cancelling_from_sessions_returns_the_focus_to_sessions`, `the_focus_stack_is_empty_again_after_every_search`
- AC: `brio` matching across projects, not other harnesses, sorted by score then newest — `app::search::tests::the_title_the_first_prompt_and_the_project_name_are_searched`, `a_session_matches_through_the_first_prompt_alone`, `a_session_matches_through_its_project_name_alone`, `only_the_projects_given_are_searched`, `results_are_sorted_by_score_then_newest_first`, `equal_scores_are_ordered_newest_first_then_by_id`; App level: `typing_brio_lists_the_matches_of_every_project`, `only_sessions_of_the_current_harness_are_listed`
- AC: Tab/←/→ don't change the pane, ↑/↓ move the selection, Enter opens — `tab_arrows_h_and_l_do_not_change_the_pane_during_a_search`, `up_and_down_move_the_selection_of_the_results`, `enter_opens_the_selected_result`, `enter_without_a_result_does_nothing`
- AC: Esc restores — `esc_from_projects_without_a_session_returns_to_exactly_that`, `esc_from_sessions_returns_to_sessions_with_the_previous_selection`, `after_esc_the_screen_is_the_startup_screen_120x40_text` (also asserts equality with the startup render), `after_esc_the_styles_are_the_startup_styles_too`
- AC: measurement recorded and approach matches the rule — see "Measurement"; worker tests: `with_a_worker_the_results_arrive_as_an_event`, `answers_to_older_queries_are_ignored`, `a_stale_answer_after_a_newer_one_is_ignored`, `app::search_worker::tests::*`
- UI: prompt open, empty query @ 120x40 (text + styled) — `prompt_open_with_an_empty_query_120x40_text`, `prompt_open_with_an_empty_query_120x40_styled`
- UI: query `brio` @ 120x40 (text + styled), 80x24 (text) — `query_brio_120x40_text`, `query_brio_120x40_styled`, `query_brio_80x24_text`
- UI: no results @ 80x24 (text) — `query_with_no_results_80x24_text` (+ `query_with_no_results_message_is_centred_in_muted_foreground`)
- UI: after Esc @ 120x40 (text) — `after_esc_the_screen_is_the_startup_screen_120x40_text`
- UI: Prompt component with text longer than width 30 — `components::prompt::tests::scrolled_text_snapshot_width_30`, `scrolled_text_with_the_cursor_inside_snapshot_width_30`
- Also: `SessionList` results mode — `components::session_list::tests::results_are_titled_with_the_query_and_prefixed_with_their_project`, `the_selected_result_is_the_focused_band` (styled), `no_results_show_the_message_centred_in_muted_foreground`, `the_search_nav_attribute_moves_the_selection_without_wrapping`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (647 passed in the lib, 0 failed; 1 ignored = the release-only timing measurement)
