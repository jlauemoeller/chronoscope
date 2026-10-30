STATUS: DONE

## Changes (round 2, answering the round-1 reviews)
- `src/app/search.rs`: a refresh of the results (a live snapshot) no longer downgrades a pending request for a new query to "keep the selection": `keep_selection && pending.is_none_or(|p| p.keep_selection)`. `submit_when_ready` is reset whenever the query text changes (not only when it becomes empty). Comment on `selected_row` saying why the app reads the list's selection back.
- `src/components/prompt.rs`:
  - AltGr (`Ctrl`+`Alt`, as Windows reports it) types its character; `Ctrl` or `Alt` alone still do not.
  - `visible_text` rewritten: it scrolls by whole characters the least that shows the whole cell under the cursor, including a two-column character at the right edge (the old scroll dropped it, leaving a blank cursor cell). The cursor column is computed from the characters actually skipped.
  - The unreachable `Event::Paste` handling is removed (bracketed paste is not enabled by the terminal setup). `with_fill` documented as needed by the browser's prompt.
- `src/components/session_list.rs`: the title cuts the query inside the quotes (` Sessions matching "the quick…" `); the project name in a result row is cut to a third of the row's room (at least 8 cells) so ` › <title>` stays visible.
- `src/app/search_worker.rs`: `search_event_kind` builds an empty `SearchResult` directly instead of running a search.

## Review findings
- Code 1 / Feature 1 (keep_selection race, must-fix) — fixed as above; tests `a_snapshot_while_a_keystroke_is_pending_still_selects_the_first_result` (fails without the fix, checked), `a_snapshot_with_the_same_query_keeps_the_selection_with_a_worker`.
- Code 2 (AltGr, must-fix) — `altgr_characters_are_text` (`@` with Ctrl+Alt, `{` with Shift+Ctrl+Alt); `shifted_characters_are_text_and_ctrl_or_alt_ones_are_not` still covers Ctrl-q / Alt-q.
- UI 1 (wide character under the cursor, must-fix) — `the_wide_character_under_the_cursor_is_drawn_with_the_cursor_background` (every cursor position of a 24-character CJK text at width 30: exactly one cursor-bg cell holding the glyph), snapshot `wide_text_scrolls_snapshot_width_30` (styled; the glyph's two columns carry the cursor bg).
- Code 3 (cursor column around wide characters, nit) — same rewrite and tests.
- Code 4 (paste unreachable) — removed. Code 5 — fixed. Code 6 — comment added. Code 7 — doc comment says why `with_fill` stays.
- Code 8 / Feature 4 (timing figures) — removed the "matching alone" figures (taken in a different run) and re-measured the whole path below; the figures are machine- and run-dependent: queries other than `flaky timeout` also exceed 16 ms in some runs (`impo` 16.4 ms mean / 20.4 ms worst in this run). Conclusion unchanged: worker.
- Feature 2 — kept as documented (exact restore of the starting state; `esc_from_projects_with_a_session_selected_returns_to_that_state`).
- Feature 3 (submit_when_ready survives typing) — fixed; test `an_enter_waiting_for_an_answer_is_dropped_when_the_query_changes`.
- Feature 5 — acknowledged: `PromptChanged/Submit/Cancel/Nav` are routed to the search handlers because search is the only prompt; the later path / y/n prompts must record which prompt is open next to its focus-stack entry (documented here, not built now).
- UI 2 (title quote) — `a_long_query_is_cut_inside_the_quotes`. UI 3 (long project name) — `a_long_project_name_is_cut_and_leaves_the_title_visible`. UI 4 — snapshot `query_with_no_results_80x24_styled` added.

## Measurement (5 000 sessions, release, 24-core machine, this round's run)
Synthetic: 50 projects, 5 000 sessions, titles ~55 chars, first prompts ~430 chars; `cargo test --release --lib -- --ignored --nocapture five_thousand`. Whole path of one keystroke on the UI thread (no worker), mean / worst of 20:

| query | results | mean | worst |
|---|---|---|---|
| `i` | 5000 | 2.2 ms | 4.1 ms |
| `im` | 5000 | 11.0 ms | 15.0 ms |
| `imp` | 5000 | 15.5 ms | 15.9 ms |
| `impo` | 5000 | 16.4 ms | 20.4 ms |
| `brio` | 5000 | 9.6 ms | 9.6 ms |
| `flaky timeout` | 5000 | 23.4 ms | 24.2 ms |
| `zzz` | 0 | 0.2 ms | 0.5 ms |

Worst case over 16 ms, so the search runs on a worker (`SearchWorker`, answers delivered as `UserEvent::Search`). With the worker the UI thread pays about 2 ms mean / 3.1 ms worst per keystroke.

## Self-check
All round-1 acceptance-criteria mappings are unchanged (see the round-1 notes); new tests are listed above. No `.snap.new` files remain.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (869 passed, 0 failed across all test binaries; 1 ignored = the release-only timing measurement)
