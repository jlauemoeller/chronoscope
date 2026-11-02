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
