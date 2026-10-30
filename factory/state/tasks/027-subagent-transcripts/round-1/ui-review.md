VERDICT: FAIL

## Summary
All four states in the `UI` section have snapshots. They match the description: the breadcrumb segments, roles and bold project name in the styled snapshot, the subagent id on the right at 120x40, the parent's cwd row, the subagent prompt as the first User block, and a byte-identical screen after `Backspace`. The gate passes with `INSTA_UPDATE=no`. One real rendering bug is outside the snapshots. When the breadcrumb is deeper than the width allows, `BrowserHeader` shrinks agent names to nothing and keeps their ` › Agent: ` separators. This blanks the name of the agent on screen, and at depth 5 and 60 columns it clips the `─╮` corner. Also, at 80x24 the subagent view drops its session id entirely, although the task requires the right title to show it and a cut id would fit.

## States checked
- Test gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `INSTA_UPDATE=no cargo test` (1556 unit tests plus integration suites). All pass. No snapshot was changed.
- State 1, parent with the subagent section unfolded and selected @ 120x40 — snapshot `parent_with_a_subagent_selected_120x40_text` — ok. It shows `▼ Ask story-writer (story-writer)`, `Prompt:`, `Result:` and `a to open transcript` with the selection marker.
- State 2, after `a`, text @ 120x40 — snapshot `the_subagent_120x40_text` — ok. The header reads `╭─ Project BRIO › Agent: story-writer ─…─ <id>/subagents/agent-r2 ─╮` and the cwd row is the parent's. The User prompt block comes first.
- State 2, styled @ 120x40 — snapshot `the_subagent_120x40_styled` (and component `header_with_a_breadcrumb_styled_snapshot`) — ok:
  - `Project` and ` › Agent: ` use `muted_foreground` (161,161,161);
  - `BRIO` is `foreground` and BOLD;
  - `story-writer` is `foreground` and not bold;
  - the id uses the `timestamp` role, as in the root header.
- State 2, text @ 80x24 — snapshot `the_subagent_80x24_text` — **issue**. The breadcrumb is whole, but the right title (subagent id) is missing (finding 2).
- State 3, nested depth 2 @ 120x40 — snapshot `nested_subagents_120x40_text` — ok. It shows `Project BRIO › Agent: story-writer › Agent: Explore` and the depth-2 id.
- State 4, after `Backspace` @ 120x40 — snapshot `after_backspace_the_screen_is_the_parent_as_it_was_120x40_text` — ok. The test asserts the screen equals the pre-`a` capture, and the bodies of the state 1 and state 4 `.snap` files are byte-identical (`diff`).
- `BrowserHeader`, depths 1–5 @ 60/80/100/120 columns, with long (64-character description) and CJK labels — rendered in the scratch crate:
  - ok for depths 1–2 at every width, and for long and CJK labels (cut with `…`, row exactly `width`, corner kept);
  - **issue** for depth ≥ 3 @ 60 and depth 5 @ 80 (finding 1).
- Real binary on the Claude subagent fixture, under a throwaway HOME in tmux, at 80x24 and 60x20:
  - `a` on `Write the story (story-writer)` shows `Project loading › Agent: story-writer ─… pppp0001/subagents/agent-r2` and the prompt as the first User block — ok;
  - `Backspace` returns to the identical parent screen — ok;
  - `Backspace` at depth 0 does nothing — ok;
  - `a` on `No linkage` shows the toast `No transcript for this subagent` — ok;
  - a zero-byte `agent-m1.jsonl` opens with the breadcrumb and `Session file is empty` centred, and `Backspace` restores the parent — ok.

## Findings
1. [must-fix] `BrowserHeader` top border, subagent view with a breadcrumb of depth ≥ 3 at 60 columns (the app's `MIN_WIDTH`) and depth 5 at 80 columns. See `ui/header-deep-breadcrumb-60-80.txt`.
   - **Problem.** `top()` shrinks names first to `MIN_NAME_WIDTH` and then from the end down to 0, but always emits every ` › Agent: ` separator.
     - Depth 3 @ 60 renders `╭─ Project BRIO › Agent: story… › Agent: Expl… › Agent:  ──╮`. The agent being viewed has no name.
     - Depth 4 @ 60 renders three empty `Agent:` segments.
     - Depth 5 @ 60 renders `╭─ Project  › Agent:  › Agent:  › Agent:  › Agent:  › Agent:`. The project name is gone, the separators alone overflow the row, and the `─╮` corner is clipped.
     - Depth 5 @ 80 has three empty segments.
   - **Change.** Keep row 0 exactly `width` cells ending in `─╮`. Never emit an `Agent:` segment with an empty label, and always name the current (innermost) agent.
     - **Within the per-level rule (the normal case).** Keep one ` › Agent: <label>` segment per level, as the planner note requires. Shrink the names evenly, giving width to the innermost label first, and never shrink a name below a 1-character `…`.
       - Depth 3 @ 60 has 15 cells for 4 names, so every segment keeps a label. For example: `╭─ Project BRIO › Agent: s… › Agent: E… › Agent: general… ─╮`.
       - Depth 4 @ 60 and depth 5 @ 80 fit the same way.
     - **Last resort (the one deliberate exception to the planner note).** Use this only when the separators alone exceed the room, for example depth 5 @ 60, where the separators take 60 cells against 55. Collapse the outermost agent segments into one muted ` › …` segment, as few as needed. Keep the project name (at least `…`) and the innermost agent's segment.
   - **Test.** Add a `browser_header` test for depths 3–5 at 60 and 80 columns. Assert that row 0 is `width` cells, ends with `─╮`, contains the last label (or its `…` prefix), and contains no `Agent:` followed by a blank or `─`.
2. [must-fix] Browser screen, state 2 @ 80x24 — the right title (subagent session id) is dropped entirely. See `ui/subagent-80x24-id-dropped.txt`.
   - **Problem.** The binding planner note says "The right title shows the subagent's session id", and 80x24 is a listed size for state 2, but the snapshot has no id. The header applies its old rule (drop the id when the names would not fit). Task 021's `UI` section does not pin that rule, and dropping the id is not forced: the flexible part of the `─…─ <id> ─╮` template is the fill, and a cut id fits beside the whole breadcrumb.
   - With realistic Claude ids (`<uuid>/subagents/agent-<hex>`), the subagent view almost never shows its id at 80 columns, while the root view at the same width keeps its 36-character id (second render in the file).
   - **Change.** When the full id does not fit, cut it from the start and keep its distinguishing tail. Use `…/subagents/agent-r2`, or `…agent-r2` when even that is too wide. Drop the id only when not even `…` plus 6 characters fit after the names reach their floors. Add a `browser_header` test at 80 columns for one breadcrumb level with the 53-character id. Re-take `the_subagent_80x24_text`; its row 0 should end in `… …agent-r2 ─╮` or similar.
3. [nit] Browser help line in a subagent view, all sizes — the hints are the same as the root view's (`/ to search | ENTER to fold or unfold sections | y to copy current | ? for keybindings`). Nothing tells the user how to get back. §4.2 calls the help line context-sensitive. Change: while the subagent depth is > 0, add a `BACKSPACE to go back` hint, resolved through the keymap's `browser.back` binding and placed before `? for keybindings`.
4. [nit] Screen snapshots for states 1–4 use synthetic turns (`blocks::` fixtures). The scrubbed Claude subagent fixture is only exercised by the assertion-based `with_the_claude_harness_…` tests. Change (optional): add one text snapshot of state 2 taken from `claude_rig` at 120x40, so the real header id format (`pppp0001/subagents/agent-r2`) and the real transcript rendering are pinned.
5. [nit] No test checks a covered parent that receives `Appended` batches while it is scrolled to the bottom. Follow mode may legitimately move its offset; its selection and folds should not change. Change: in `a_covered_session_keeps_receiving_its_turns_and_the_header_stays_the_subagents`, assert that the parent's `presentation().selected` and `.folds` are unchanged after the append and the `back_to(0)`.
