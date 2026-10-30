VERDICT: FAIL

## Summary
The Prompt component, the focus stack, the `nucleo` search with its worker, and the results mode of `SessionList` are well built and well tested, and my release re-run confirms the worker choice. Two defects remain. A live snapshot that arrives while the worker is answering a keystroke makes the new results keep the old selection instead of selecting the first result, and the prompt drops AltGr characters on Windows.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

All test binaries pass, with one test ignored: the release-only timing test. No `.snap.new` files exist, and the working tree is clean after the run.

I re-ran the timing test with `cargo test --release --lib -- --ignored --nocapture five_thousand`. On the UI thread, the worst per-keystroke times were 20.5 ms (`impo`), 17.8 ms (`brio`) and 37.4 ms (`flaky timeout`). With the worker, the UI thread never took more than 3.8 ms. The worker approach matches the planner rule.

## Findings
1. [must-fix] `src/app/search.rs`, `App::push_search_results`: a snapshot that arrives between a keystroke and its answer replaces the pending request's `keep_selection: false` with `true`.
   - How it happens: the keystroke sets `pending = {seq N, keep_selection: false}`. An index snapshot arrives before the answer (for example, a live session appended). `push_sessions` calls `push_search_results(true)`, which overwrites it with `{seq N+1, keep_selection: true}`. The answer to N is dropped as stale. When the answer to N+1 is listed, the row that was selected under the *previous* query stays selected (if it is still among the hits) instead of the first result.
   - Why it matters: the planner notes and the UI section require "The first result is selected" whenever the query changes. `typing_again_selects_the_first_result_again` only covers the synchronous path. Live sessions append often, so this race is reachable in normal use.
   - Change: when building the new `PendingSearch`, combine the flag with the one already pending, so that a query change is never downgraded to "keep the selection":
     ```rust
     let keep = keep_selection && search.pending.is_none_or(|p| p.keep_selection);
     ```
     Compute it before `search.pending` is overwritten. The synchronous path needs no change.
   - Add a worker test in `src/app/search_tests.rs` (`app_with_worker`):
     1. Type a query and drain the answers.
     2. Send `PromptNav(Down)`, so the selection is not row 0.
     3. Send another `PromptChanged` whose results still contain the selected session.
     4. Before draining, apply a *modified* snapshot, for example with one more session appended, as in `a_new_snapshot_refreshes_the_results_and_keeps_the_selected_one`. That makes sure it goes through `push_sessions`.
     5. Drain the answers and apply them.
     6. Assert `session_rows(&app).selected == Some(0)`.

2. [must-fix] `src/components/prompt.rs`, `is_typed` (used by `on_text_key`): the prompt only accepts characters with no modifiers or with Shift alone.
   - Why it fails on Windows: crossterm 0.29 (`event/sys/windows/parse.rs`, `From<&ControlKeyState> for KeyModifiers`) reports AltGr as `CONTROL | ALT`, because Windows synthesises AltGr as LeftCtrl + RightAlt. On non-US layouts (Danish, German, …), characters such as `@ { } [ ] \ | ~ $` are typed with AltGr, so the prompt silently drops them.
   - Why it matters: this breaks SPEC §6 ("every printable key goes into the prompt") on a CI platform (§9.1). It will break the file-path prompt of later tasks badly, because `\` is an AltGr key on those layouts. Windows is tier 3, but the §9.1 degradation clause covers only features that need *substantial* platform-specific work. This fix is a one-line predicate.
   - Change: treat a `Key::Char` as typed when its modifiers are a subset of `SHIFT`, or when they contain both `CONTROL` and `ALT` (AltGr):
     ```rust
     fn is_typed(m: KeyModifiers) -> bool {
         (m - KeyModifiers::SHIFT).is_empty() || m.contains(KeyModifiers::CONTROL | KeyModifiers::ALT)
     }
     ```
     `Ctrl-c` is checked before this in `on`, so it still quits.
   - Add tests to `prompt::tests`:
     - `Char('@')` with `CONTROL | ALT` inserts `@`.
     - `Char('{')` with `SHIFT | CONTROL | ALT` inserts `{`.
     - `Ctrl-q` and `Alt-q` alone are still ignored. The existing test `shifted_characters_are_text_and_ctrl_or_alt_ones_are_not` already covers this.

3. [nit] `src/components/prompt.rs`, `Prompt::visible_text`: horizontal scrolling misplaces the cursor around wide characters.
   - Problem: the skip loop drops whole characters while `skipped < scroll`. When the scroll boundary falls inside a double-width character, `skipped` ends one column past `scroll`. The cursor column, `visual_cursor() - skipped`, is then one cell too far left, and one more column of text is shown than intended.
   - Change: compute the cursor column from the columns of the characters actually shown. Alternatively, skip a straddling wide character and pad with one blank cell.
   - Add a test with CJK text longer than the width, asserting that exactly one cursor cell is drawn and that it sits under the expected character.

4. [nit] `src/components/prompt.rs`, `Prompt::paste` and the `Event::Paste` arm: `src/terminal.rs` never enables bracketed paste, so `Event::Paste` cannot arrive and this code is unreachable today. Change: either enable `EnableBracketedPaste` at terminal setup and `DisableBracketedPaste` on restore, or remove the paste handling until a task needs it.

5. [nit] `src/app/search_worker.rs`, `search_event_kind`: it builds the subscription discriminant through `SearchResult::of(..)`, which runs `search_sessions`. Change: construct `SearchResult { seq: 0, query: String::new(), projects: Arc::default(), sessions: Arc::default(), hits: Vec::new() }` directly.

6. [nit] `src/app/search.rs`, `App::selected_row`: this is the first place in non-test code where the `App` reads a component's state back (`application.query(&Id::SessionList, SELECTED_ATTR)`), both on `Enter` and for `keep_selection`. It goes through the public attribute API, so it is acceptable. However, SPEC §1.2 prefers "a `Msg` carries everything the `App` needs". Change: add a short comment saying why it is needed (in search mode the list's cursor moves by attribute and reports no `Msg`). Alternatively, have `SessionList` report the moved selection.

7. [nit] `src/components/prompt.rs`, `Prompt::with_fill`: the method is public but only tests use it. Change: keep it if a later task needs a non-`surface` fill (say so in its doc comment); otherwise remove it.

8. [nit] `factory/state/tasks/015-selector-search/round-1/build-notes.md`, "Measurement": the notes say matching alone takes 12.4 / 17.5 / 33 ms. That is more than the whole-path figures in the table (9.8 / 13.0 / 23.6 ms), even though the whole path includes the matching. Change: re-measure both in the same run, or say that they come from different runs. The conclusion (worker) is unaffected.
