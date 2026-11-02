VERDICT: FAIL

## Summary
Transcript search is implemented cleanly across a pure matcher/search module (`src/transcript_search.rs`), the transcript's search state with temporary unfold/expand (`src/components/transcript/search.rs`), draw-time highlighting in `TurnBlock`, and the App flow with a worker thread, Port, `UserEvent::TranscriptSearch` -> `GlobalEvents` -> `Msg`. Ownership rules of SPEC §1.2 are respected (data down via attrs, intent up via `Msg`, no shared mutable state), generations are unique over the run, and the tests are thorough and would fail on wrong behaviour; all five UI states have accepted snapshots. One must-fix remains: a temporary output expansion made by the search leaks into the fold state when the user folds the section, and that path is untested.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (lib 1296 passed, all integration tests pass; no `.snap.new` files, working tree clean)

## Findings
1. [must-fix] `src/components/transcript/search.rs`, `fold_command_on_temp`, and `src/components/transcript/folding.rs`, `set_all_folded` (via `forget_temp_unfold`). When the temporary state has `expanded == true` and the user's command folds the section (`fold`/`h`, `Enter` on a section the search only expanded, i.e. `temp.unfolded == false`, or fold-all `-`), the `TempUnfold` record is dropped but the search's `FoldState` expansion stays. The next time the user unfolds the section, the output is shown untruncated although they never expanded it. This breaks the planner rule that the expansion beyond the truncation limit is temporary. No test covers this path: `leaving_the_section_undoes_the_expansion_as_well_as_the_unfolding` only moves the selection away. Change:
   - (a) In `fold_command_on_temp`, when the taken temp has `expanded` and the command will fold the section (op `Fold`, or `Toggle` with `!temp.unfolded`), call `self.content.folds.clear_expanded(section)` and invalidate the section's block.
   - (b) In `set_all_folded(true)`, do the same for the current temp before forgetting it. `set_all_folded(false)` may keep it, since unfolding makes the state explicit.
   - (c) Add a component test in `src/components/transcript/tests/search.rs`: `long_log(500, 350)`, search `needle` (expanded), press `h`, then unfold again with `l`. Assert that `!t.content.folds.is_expanded(&tool())` and that the rendered rows show the `… N more lines` row, not `the needle line`. Add the same check for fold-all.
2. [nit] `src/components/transcript/search.rs`, `fold_command_on_temp` — the first `Enter` on a temporarily unfolded section only converts it to an explicit unfold and changes nothing on screen (a second `Enter` folds). This follows the AC literally and is documented in the build notes, but it is worth a line in the module doc / help text so it does not look like a dropped key.
3. [nit] `src/app/mod.rs`, `Msg::PromptChanged(text) if self.browser.search.prompt_open => { let _ = text; … }` — bind with `Msg::PromptChanged(_)` instead of `let _ = text;`.
4. [nit] `src/transcript_search.rs`, `SearchQuery::is_match` and `SearchQuery::is_case_sensitive` are `pub` but used only by tests. Remove them (test via `find_all`) or mark them `#[cfg(test)]` to avoid dead public API.
5. [nit] `src/components/transcript/search.rs`, `MAX_SKIPPED = 1000` in `goto_match` — if more than 1000 consecutive matches lie in sections that are no longer listed (e.g. a search with system content shown, then `s` hides it), `n`/`N` give up even though later matches exist. Consider skipping by section (jump past all matches of an unlisted section using `first_of`) rather than capping the number of tries.
6. [nit] `src/components/transcript/search.rs`, `reveal_match` — `self.refresh()` is called both before and after `scroll_to_match`; if the first is only needed to make `cursor()` valid, a short comment would help, otherwise drop one.
