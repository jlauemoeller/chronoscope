VERDICT: FAIL

## Summary
The subagent stack is well built. `LoadView` holds per-load state, `Id::Subagent(depth)` is used for each level, updates are routed by generation from a counter that never goes back, and the parent transcript is kept mounted and untouched. Every acceptance criterion has an effective test, including the real-Claude fixture test, and the snapshots match the task's UI. One SPEC §1.3 guideline breach blocks the pass: the header's title render helper grew past the length limit.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1556 unit tests plus all integration suites; no `.snap.new` files in the tree)

## Findings
1. [must-fix] `src/components/browser_header.rs`, `BrowserHeader::top` (lines 157-225). The function is now 68 lines; on `main` it was 31. SPEC §1.3 says: "No long render functions. A render function (`view`, or a helper it calls) that draws more than one distinct element, or grows past about 50 lines, is split into smaller components or widgets." `top` now mixes three jobs: the room/id-dropping decision, the shrink-longest-first width allocation loop, and span assembly. Change:
   - Extract the allocation into a pure free function, e.g. `fn fit_widths(widths: &[usize], floors: &[usize], room: usize) -> Vec<usize>`. Move the `while widths.iter().sum() > names_room { … }` loop into it.
   - Give that function its own unit tests: everything fits; the longest name shrinks first; nothing goes below the floors until the floors themselves don't fit; zero room.
   - Optionally move the breadcrumb span building (the `for agent in cut` loop with `AGENT_SEPARATOR`) into a second small helper.
   - `top` should end up comfortably under 50 lines. Behaviour and snapshots must not change.
2. [nit] `src/app/browser.rs`, `open_subagent`. `self.browser.load_seq = generation` is assigned only after `mount_subagent` succeeds, but `loader.load(&id, forward_to(generation, sink))` has already started a worker with that number. If the mount fails, the next `a` reuses the number, which contradicts the "never reused" design. On that failure path the new watch is also dropped with only a log line and no toast. Change: assign `self.browser.load_seq = generation;` right after computing `generation` (before `load`), and show a quiet toast (e.g. `Cannot open the subagent`) when `mount_subagent` fails.
3. [nit] `src/app/browser.rs` / `src/app/transcript_search.rs`. A transcript search's `pending` lives in the per-level `LoadView`. If its answer is still in flight when `a` opens a subagent, `transcript_matches_arrived` compares against the subagent's `pending` (None) and drops the answer. The parent's `pending` then stays `Some(..)` until its next search. This is rare and heals itself. Possible fixes: clear the top view's `search.pending` before pushing a level, or route matches to the level whose `pending` matches.
4. [nit] `src/app/browser.rs`, the `impl Deref/DerefMut for BrowserState { type Target = LoadView }`. Deref on a non-pointer type hides which level a field access such as `self.browser.loaded` or `self.browser.search` touches. This is what makes item 3 easy to miss. Consider an explicit `fn top(&self) -> &LoadView` / `fn top_mut(&mut self) -> &mut LoadView` instead, so call sites state that they mean the top level.
5. [nit] `src/components/browser_header.rs`, `top`. At 80x24 the 53-character subagent id is dropped from the right title. That follows the header's existing rule, and the task's UI section only pins the format, so it is acceptable. A tail-truncated id (e.g. `…/agent-r2`) would still tell nested levels apart at narrow widths.
6. [nit] `src/app/browser.rs`, `browser_hints`. In a subagent view the help line does not mention the back key. The task does not require it, but a `BACKSPACE to go back` hint while `depth() > 0` would make the feature discoverable.

Rulings on the deviations listed in the build notes (not findings):
- Covered transcripts keep receiving their own load's updates. This is accepted. Dropping `Appended` batches would lose turns, and the presentation-state restore asserted by the AC test still holds.
- The 80x24 id drop is covered in nit 5.
- I checked snapshot 4 (`after_backspace_…_120x40_text`) against snapshot 1 (`parent_with_a_subagent_selected_120x40_text`): the bodies are identical.
