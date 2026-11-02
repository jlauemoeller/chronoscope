STATUS: DONE

## Changes
- `src/components/browser_header.rs`, new `browser_header/fit.rs`: the top-border fitting is split out of `top` (now about 30 lines) into pure functions with their own tests: `visible_agents`, `fit_widths`, `cut_names`, `fit_id`. `title_spans` builds the left title.
  - Deep breadcrumbs: no agent segment is ever blank. Names are cut longest first (the outermost of equals), down to 6 columns, then to `…`. When the separators would leave under 3 columns per name, the outer agents collapse into one muted ` › …` segment and the innermost agent is always named. The row stays exactly `width` cells and ends in `─╮`.
  - The session id is cut from the start (`…a21a-…/subagents/agent-r2`) when the whole id does not fit beside the whole breadcrumb, and dropped only when fewer than 7 columns remain after the names are at their floors. The root header with its 36-character id is unchanged.
- `src/components/help_line.rs`, `src/app/hints.rs`, `src/app/browser.rs`: `HINTS_ATTR`; in a subagent view the help line reads `/ to search | Backspace to go back | ...` (key resolved through the keymap's `browser.back`), restored at depth 0.
- `src/app/browser.rs`: `load_seq` is taken before the worker starts, so a failed mount cannot give the number out again; a failed mount shows the toast `Cannot open the subagent`. The covered view's search `pending` is cleared before a level is pushed (`BrowserSearch::forget_pending`).
- Tests: header tests for depths 1 to 5 at 60/70/80 columns, the id cut at 80, `fit.rs` unit tests, hint tests; app tests for a scrolled-up parent whose turns change while covered (offset, selection and folds equal after `Backspace`), folds unchanged in the following-parent test, the back hint, the 80-column id. Snapshots re-taken (`the_subagent_80x24_text`, `the_subagent_120x40_text`, `the_subagent_120x40_styled`, `nested_subagents_120x40_text`: the help line and, at 80x24, the id changed) and a new real-fixture snapshot `with_the_claude_harness_the_subagent_120x40_text`.

## Self-check
- Review (code 1, must-fix): `top` over 50 lines — split into `fit.rs` functions with unit tests and `title_spans`; `top` is about 30 lines; existing header snapshots unchanged.
- Review (code 2): load number reuse / silent mount failure — `load_seq` set before `load`; quiet toast on mount failure.
- Review (code 3, feature 3): in-flight parent search lost — `forget_pending` before pushing. Not covered by a test: the answer is in flight only with a worker thread, which cannot be made deterministic.
- Review (code 4, Deref nit): not changed. `BrowserState` still derefs to the top view; the doc comment on it says so. Changing ~30 call sites would not change behaviour.
- Review (code 5, ui 2, must-fix): id dropped at 80x24 — now cut from the start and kept (`the_subagent_80x24_text`, `a_long_id_is_cut_from_the_start_and_keeps_its_end`, `the_subagent_id_is_cut_from_the_start_at_80_columns`, `fit::tests::*`).
- Review (ui 1, must-fix): deep breadcrumb — `a_deep_breadcrumb_never_blanks_a_name_and_keeps_the_corner`, `three_levels_at_60_columns_keep_a_label_each`, `five_levels_at_60_columns_collapse_the_outer_agents`, `fit::tests::*`.
- Review (code 6, ui 3): back hint — `a_subagent_view_hints_at_the_back_key_and_the_session_does_not`, `hints::tests::{a_subagent_view_adds_the_back_hint_after_search, the_back_hint_follows_a_remapped_back_key}`. The label is the keymap's `Backspace`.
- Review (ui 4): real-fixture state 2 snapshot — `with_the_claude_harness_the_subagent_120x40_text`.
- Review (feature 1, ui 5): covered-parent presentation — `a_scrolled_up_parent_keeps_its_presentation_when_its_turns_change_while_covered`, and folds asserted in `a_covered_session_keeps_receiving_its_turns_and_the_header_stays_the_subagents`. A parent that follows the end legitimately moves its selection to a new last section (SPEC 7.2), so only its folds are asserted there.
- Review (feature 6): task 031 must send theme changes to every `transcript_id(0..=depth)`; noted here for the hand-off.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (0 failed)
