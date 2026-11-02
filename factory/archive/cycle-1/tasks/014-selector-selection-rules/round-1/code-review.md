VERDICT: FAIL

## Summary
The change adds startup preselection (component-wise `Path::starts_with`, deepest match wins, ties go to the lowest harness number, only available harnesses count, with a fallback to the first available harness), `Msg::OpenSession` / `Msg::CloseSelector`, a `session_open` prop that decides what `Esc` does, and an `EmptyState` child inside `SessionList` that the app switches on with an explicit `harness_empty` flag. The code mostly follows the component and keymap rules in SPEC §1.3/§1.4: keys are resolved through `pane_key`/`Action::Cancel`, no literal keys are matched, and the colours come from the theme. The tests are thorough and the gate passes. One bug fails the review: the startup preselection is lost when the shown harness's first snapshot is empty (a failed first build), so startup can land on the wrong project.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [must-fix] `src/app/selector.rs` `preselect_again` / `settle_preselection` — the startup preselection is lost when the first snapshot of the shown harness has no projects. The index worker sends exactly that when a build fails (`src/index/worker.rs` `build`: "Tell the UI there is nothing (yet): empty lists", with `available` taken from `harness.available()`, so it can be `true`). The bug, step by step (one registered harness, working directory `/w/brio/src`):
   - The empty snapshot makes `preselect_again` enter `(Claude, None)`.
   - `settle_preselection` then sets `preselecting = false`, because `all_reported()` is already true. With several harnesses, this happens as soon as the others have reported.
   - When the real snapshot arrives, `current` is set and `preselecting` is false, so `preselect_again` returns early.
   - `keep_selection` falls back to project 0, not `/w/brio`.

   The user has done nothing, and startup lands on the wrong project, which breaks the SPEC §4.1 startup rule and the planner's preselection rule. The guard also compares only the harness (`self.selector.current == Some(harness)`), not the project, so a better match inside the same harness is ignored even while `preselecting` is true. Change:
   - (a) `settle_preselection` must not end the preselection while the shown harness has no projects (`self.selector.projects().is_empty()`). Any `SelectorMsg` still ends it, as now.
   - (b) While `preselecting`, `preselect_again` returns early only when both the harness and the project equal the computed preselection. Otherwise it calls `enter_harness_at`.

   Add update tests in `src/app/selection_tests.rs`:
   - One registered harness, working directory `/w/brio/src`. Send an empty `available = true` snapshot, then a snapshot holding `/w` and `/w/brio`. Expect `/w/brio` selected, Projects focused, no session.
   - The same with two harnesses where the other one has already reported (no match there).
2. [nit] `src/app/selector.rs` `SelectorState::preselection` — `Path::starts_with` returns true for an empty prefix. A project with `cwd: Some("")` (or any relative cwd that happens to be a prefix) would match every working directory at depth 0 and beat the "first project" fallback. Change: skip projects whose cwd is not absolute (`if !cwd.is_absolute() { continue; }`).
3. [nit] `src/app/selection_tests.rs` `IndexEventParts` / `parts` — this one-off tuple struct with `unreachable!()` only rebuilds a snapshot with `available = false`. A small helper in `fixtures.rs` (for example `snapshot_of_unavailable(harness, projects)`), or a `match` into a tuple, would read more simply.
4. [nit] `src/app/runtime.rs:83` — possible follow-up, not a deviation: the planner note binds `std::env::current_dir()`, which resolves symlinks. Also trying `$PWD`, when it names the same directory, would preselect correctly in symlinked directories.
