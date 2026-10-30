STATUS: DONE

## Changes
- `src/components/transcript/subagent.rs` (new), `transcript/mod.rs`: `a` (`Action::OpenSubagent`) on a subagent tool section (or the `... N more lines` row under one) answers `Msg::OpenSubagent { call_id, label }` (label = `subagent_type`/`agent`, else the description); any other section answers nothing. `Action::Back` answers `Msg::CloseSubagent` only for a `nested(true)` transcript. New query-only `PRESENTATION_ATTR` / `Transcript::presentation()` (offset, selection, fold states) so tests can compare the full presentation state (`Component::state()` still returns only the offset).
- `src/components/browser_header.rs`: `BREADCRUMB_ATTR` and `SESSION_ID_ATTR`; the left title gets one ` › Agent: <label>` segment per level (`›`/`Agent:` muted, names foreground, project name bold); the right title is the subagent id. Truncation shrinks the longest names first and drops the id when names would not fit.
- `src/app/mod.rs`: `Id::Subagent(depth)` (depth 0 stays `Id::Transcript`), `Msg::OpenSubagent`, `Msg::CloseSubagent`; `view` draws the transcript on top in the layout's transcript slot.
- `src/app/browser.rs`: per-load state moved into `LoadView`; `BrowserState` holds the root view plus a stack of `Level { session id, label, view }` and derefs to the top view. `open_subagent` (asks `Harness::subagent`, toast `No transcript for this subagent` on `None`, starts `load` with a fresh generation, mounts and focuses `Id::Subagent(depth)`), `close_subagent` (drops the level and its `WatchHandle`, unmounts, refocuses the parent, restores header title/id/warnings/live, status, empty state). Generations come from a never-decreasing counter, so a popped load number is never reused. Updates are routed by generation: the top level as before, a covered level to its own transcript and view (header and help line untouched until it is on top again), any other generation dropped. Opening another session unmounts the stack. Viewport pushes reach every mounted transcript.
- `src/app/transcript_search.rs`, `save.rs`, `selector.rs`: use the transcript on top; saved files of a subagent are named by its own id part.
- Tests: `src/components/transcript/tests/subagent.rs`, `src/app/browser_tests/subagent.rs` (FakeHarness gained a `subagent()` map), header tests; snapshots accepted.

## Notes for reviewers
- At 80x24 the subagent's session id (53 characters) is dropped from the right title: it cannot fit beside the breadcrumb, and the header's existing rule (names first, id goes when they would not fit) applies. At 120x40 it is shown.
- Covered transcripts are not literally frozen: they keep receiving their own load's updates and viewport changes, because dropping a covered session's `Appended` batches would lose turns for good (they are sent once). Their presentation state (scroll, selection, folds) is untouched by anything that happens in the subagent; the restore test asserts it.

## Self-check
- AC: `a` on a Subagent section returns `Msg::OpenSubagent`, other sections nothing — `components::transcript::tests::subagent::{a_on_a_subagent_section_asks_to_open_its_transcript, a_on_any_other_section_returns_nothing, the_label_is_the_description_without_an_agent_type, a_on_the_more_row_of_a_subagent_opens_it_too, a_with_nothing_selected_returns_nothing}`
- AC: OpenSubagent update test (depth-1 transcript mounted and focused, load started, header and id) — `app::browser_tests::subagent::{a_on_a_subagent_section_mounts_a_transcript_at_depth_1_and_focuses_it, the_header_shows_the_breadcrumb_and_the_subagent_id}`
- AC: `None` gives the toast and mounts nothing — `a_subagent_without_a_transcript_shows_a_toast_and_mounts_nothing` (also `a_load_that_fails_...`, `a_on_any_other_section_does_nothing`)
- AC: Backspace restores the parent (offset, selection, folds, state()), drops the watch; depth 0 does nothing — `backspace_restores_the_parent_exactly_and_drops_the_watch`, `backspace_restores_the_header`, `backspace_at_depth_0_does_nothing`
- AC: nested depth 2, popped one at a time — `nested_subagents_show_two_segments_and_pop_one_at_a_time`
- AC: late updates of a popped load ignored — `late_updates_of_a_popped_subagent_are_ignored`, `a_popped_load_number_is_never_given_to_another_subagent`, `a_session_opened_later_never_reuses_the_number_of_a_subagent`, `opening_another_session_clears_the_whole_stack`
- AC: real Claude harness + fixture, prompt is the first User turn — `with_the_claude_harness_the_subagent_opens_with_its_prompt_as_the_first_user_turn` (and `..._a_call_without_a_transcript_shows_the_toast` for `toolu_gone03`)
- UI: parent with subagent unfolded and selected @ 120x40 text — `parent_with_a_subagent_selected_120x40_text`
- UI: after `a` @ 120x40 text + styled, 80x24 text — `the_subagent_120x40_text`, `the_subagent_120x40_styled`, `the_subagent_80x24_text` (header styling also `components::browser_header::tests::{the_breadcrumb_is_muted_and_the_agent_names_are_foreground, header_with_a_breadcrumb_styled_snapshot}`)
- UI: nested depth 2 breadcrumb @ 120x40 text — `nested_subagents_120x40_text`
- UI: after Backspace identical to state 1 — `after_backspace_the_screen_is_the_parent_as_it_was_120x40_text` (asserts equality with the text taken before opening, then snapshots it)

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites; 1556 unit tests passed, 0 failed)
