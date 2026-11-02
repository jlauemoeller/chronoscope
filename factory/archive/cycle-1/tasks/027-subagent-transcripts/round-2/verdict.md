VERDICT: REWORK

## Assessment
**Code review: FAIL.**
- Finding 1 (must-fix, the root header regressed): **to be fixed.**
  - I confirmed it against `main`'s and HEAD's `BrowserHeader::top`. At depth 0, HEAD now keeps the project name whole and cuts the 36-character UUID from its start whenever the whole name and the whole id do not fit together. `main` did the opposite: it cut the name down to 6 columns and kept the whole id.
  - For `chronoscope2` at 60 columns (the app's `MIN_WIDTH`), HEAD renders `╭─ Project chronoscope2 ─ …57-11a7-4318-a21a-0f4476babd5c ─╮`. `main` renders `╭─ Project chrono… ─ 85a86657-11a7-4318-a21a-0f4476babd5c ─╮`. The affected widths run from 60 up to about 53 plus the name's width.
  - This is not a cosmetic truncation preference. The start of the UUID is what a user types to find or resume a session: SPEC §6.8 matches session ids by prefix. Task 027 had no reason to change task 021's root header, and the build notes wrongly say it is unchanged.
  - The existing test `a_long_project_name_is_cut_with_an_ellipsis_and_keeps_the_session_id` passes only because its 100-character name falls through to the old path.
  - The fix is small and has a single right answer.
- Nit 2 (hard-coded `TITLE_CHROME` / `SEPARATOR_WIDTH`): optional in the rework. It is cheap and guards against drift.
- Nit 3 (no `forget_pending` test): optional. The fix itself is simple and correct.
- Nit 4 (three wordings for the mount-failure toast): optional in the rework.
- Nit 5 (mixed key-label casing): ignored. It comes from task 005's keymap labels, not this task.
- Round-1 Deref nit: already accepted with a reason; ignored.

**Feature review: PASS.** Every acceptance criterion is checked and covered by tests and snapshots.
- Nit 1 (the root header's fitting changed, "harmless"): overruled by code finding 1; see Conflicts.
- Nit 2 (no `forget_pending` test): optional, the same as code nit 3.
- Nit 3 (casing): ignored, the same as code nit 5.

**UI review: PASS.** Both round-1 must-fixes are resolved, and every UI state has a snapshot.
- Nit 1 (in a subagent view the id is sized against the names' 6-column floors, so the names fall off a cliff): ignored this round. The task's `UI` section pins the breadcrumb template, not the truncation priority. Every row stays `width` cells, names no agent blank, and keeps the corner. Applied naively, the change would also shrink the root id, the opposite of code finding 1. It is dropped from the rework note.

## Conflicts
- **Code finding 1 (must-fix) against feature nit 1 ("consistent and harmless; record it in the hand-off").** The Code Reviewer outranks the Feature Reviewer (priority 2 over 3), so the root header is restored.
  - The conflict is partly factual. The feature reviewer looked only at widths where `main` used to drop the id, and there cutting instead of dropping is an improvement, which is kept.
  - The code reviewer showed ordinary widths from 60 columns up, where `main` kept the whole id and cut the name. My reading of both versions of `top` confirms this wider scope.
- **UI nit 1 against code finding 1.** UI nit 1 asks to budget the id at its minimum whenever the whole names do not fit. At depth 0 that would cut the root UUID further, against code finding 1. Code correctness wins (priority 2 over 4), and the UI nit is dropped as cosmetic.

## Rework budget
Earlier REWORK verdicts: 0 of 2. There is no `verdict.md` in round-1. This is the first REWORK.

## Rework note
Restore the root (depth 0) session header's title fitting to what it was on `main`. Leave the subagent breadcrumb fitting exactly as it is now.

**Problem.**
- In `src/components/browser_header.rs`, `BrowserHeader::top` first tries `fit::fit_id(&self.session_id, room - chrome - whole)`, which keeps every name whole and cuts the id from its start. Only when that leaves no id does it fall back to the floors path.
- This now also runs when no subagent is open, and it changes task 021's root header:
  - With `chronoscope2` at 60 columns it renders `╭─ Project chronoscope2 ─ …57-11a7-4318-a21a-0f4476babd5c ─╮`. Before this task it rendered `╭─ Project chrono… ─ 85a86657-11a7-4318-a21a-0f4476babd5c ─╮`.
  - With a roughly 33-character project name at 80 columns, the name stays whole and the UUID loses its start.
- The start of the UUID is what users type to find or resume a session, so at depth 0 the project name must give way first (down to 6 columns) and the whole id must stay.

**Change.**
- In `BrowserHeader::top`, run the whole-names attempt (`fit_id` against `room - chrome - whole`) only when `!self.agents.is_empty()`.
- At depth 0, go straight to the floors path: `fit::fit_id(&self.session_id, room.saturating_sub(chrome + fit::names_floor(&names)))`, then `cut_names` as now.
- From 60 columns up this reproduces `main`, because the whole 36-character id always fits beside a 6-column name. Below that, cutting the id from the start instead of dropping it is fine to keep.
- Fix the build notes' claim that the root header is unchanged.

**Tests.** Add header tests in `browser_header.rs` for depth 0 (no breadcrumb):
- `chronoscope2` at 60 columns;
- a roughly 30-character project name at 80 columns.

For each, assert that:
- row 0 contains the whole `SESSION` id;
- the project name is cut with `…`;
- the row is exactly `width` cells and ends in `─╮`.

**Optional, if cheap.** Nothing else is required.
- Derive `TITLE_CHROME` and `SEPARATOR_WIDTH` in `fit.rs` from the strings they stand for. You can move `AGENT_SEPARATOR` into `fit.rs` and use `.width()`, or add a unit test asserting `AGENT_SEPARATOR.width() == SEPARATOR_WIDTH` and `" Project ".width() + 1 == TITLE_CHROME`.
- Use one wording for the subagent mount-failure toast in `src/app/browser.rs`. Today there are two: `Cannot open the subagent` and `Could not open the subagent: {err}`. For example, use `Could not open the subagent`, with the error appended where there is one.

**Do not change.**
- The subagent view's id-versus-names balance. The UI reviewer's suggestion to budget the id at its minimum is dropped: it is cosmetic, and applied to the root it would undo this fix.
- The `Backspace` / `ENTER` label casing, which comes from task 005's keymap.
- The `BrowserState` `Deref`.

**Done when:**
- the new depth-0 tests pass;
- `a_long_project_name_is_cut_with_an_ellipsis_and_keeps_the_session_id` and all the existing breadcrumb and `fit.rs` tests still pass;
- no existing snapshot changes at any depth. A changed subagent snapshot means the fix leaked past depth 0;
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass.
