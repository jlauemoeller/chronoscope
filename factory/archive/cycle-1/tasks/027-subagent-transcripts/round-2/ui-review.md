VERDICT: PASS

## Summary
Both round-1 must-fixes are resolved. A deep breadcrumb never blanks a name or loses the `─╮` corner, and outer levels collapse into a muted ` › …` only when needed. The 80x24 subagent header now keeps a start-cut id (`…a21a-0f4476babd5c/subagents/agent-r2`). The new back hint, the real-fixture snapshot and the covered-parent presentation test cover the round-1 nits. Every state in the `UI` section still has a matching snapshot, and the gate passes with `INSTA_UPDATE=no`. One cosmetic nit remains, about how width is split between the session id and the names.

## States checked
- Test gate. `cargo fmt --check` passes, and `cargo clippy --all-targets -- -D warnings` passes with no warnings. `INSTA_UPDATE=no cargo test` passes all 22 result lines with 0 failed (1578 unit tests). The working tree stayed clean, and there are no `.snap.new` files.
- State 1, parent with the subagent section unfolded and selected @ 120x40. Snapshot `parent_with_a_subagent_selected_120x40_text` — ok, unchanged from round 1.
- State 2, after `a`, text @ 120x40. Snapshot `the_subagent_120x40_text` — ok.
  - The header reads `╭─ Project BRIO › Agent: story-writer ─…─ 85a86657-…/subagents/agent-r2 ─╮`, and the cwd row is the parent's.
  - The User prompt block comes first.
  - The only change is the help line, which now reads `/ to search | Backspace to go back | …`.
- State 2, styled @ 120x40. Snapshot `the_subagent_120x40_styled` — ok.
  - Row 0 styles are unchanged from round 1: `Project` and ` › Agent: ` are `muted_foreground`, `BRIO` is `foreground` and bold, `story-writer` is `foreground`, and the id uses the timestamp role.
  - Only row 39 (the help line) changed.
- State 2, text @ 80x24. Snapshot `the_subagent_80x24_text` — ok (round-1 finding 2 fixed). Row 0 is `╭─ Project BRIO › Agent: story-writer ─ …a21a-0f4476babd5c/subagents/agent-r2 ─╮`. The test `the_subagent_id_is_cut_from_the_start_at_80_columns` pins it.
- State 2 with the real Claude fixture @ 120x40. Snapshot `with_the_claude_harness_the_subagent_120x40_text` (new) — ok. Row 0 is `Project loading › Agent: story-writer … pppp0001/subagents/agent-r2`, and `Write a short story` is the first User block.
- State 3, nested depth 2 @ 120x40. Snapshot `nested_subagents_120x40_text` — ok. It shows both segments and the `agent-n3` id, and only the help line changed.
- State 4, after `Backspace` @ 120x40. Snapshot `after_backspace_the_screen_is_the_parent_as_it_was_120x40_text` — ok. Its body is byte-identical to state 1's (`diff`), and the test asserts screen equality. The help line returns to the depth-0 hints.
- `BrowserHeader` row 0, rendered in the scratch crate at widths 40/50/60/70/80/100/120 and depths 0–6, with these inputs: a realistic Claude id, a 94-character description label, a long project name, CJK names, and a 2-character id. Results:
  - Every row is exactly `width` cells and ends in `─╮`. No agent segment is blank, the project keeps at least `B…`, and the innermost agent is always named. Round-1 finding 1 is fixed.
  - Depth 5 @ 60 renders `╭─ Project BRIO › … › Agent: Plan › Agent: code-reviewer ──╮`. In the styled render the collapsed ` › …` is muted (161,161,161).
  - The width split between the id and the names is uneven (finding 1).
- An empty agent label would render `Agent:  ─`, but it cannot happen: `label_of` falls back to `subagent`.
- The help line in a subagent view @ 120x40 and 80x24 — ok.
  - The label `Backspace` comes from the keymap, in the documented label format.
  - The hint sits second rather than before `?`, so at 80 columns `y to copy current` falls off instead of the back hint. The doc comment gives this reason, and I accept the placement.

## Findings
1. [nit] `BrowserHeader` top border, subagent view, whenever the whole names do not fit beside a 7-column id: at 60–80 columns with a long label or a realistic id, and at depth ≥ 4 at most widths. See `ui/header-id-vs-names.txt`.
   - **Problem.** `top` then sizes the id against the names' 6-column floors (`fit::names_floor`). The names drop to their floors and the id takes all the remaining room, a cliff rather than a balance. Examples:
     - A description label @ 80 renders `Agent: Inves… ─ …-4318-a21a-0f4476babd5c/subagents/agent-r2`.
     - The realistic id @ 60 renders `Project chron… › Agent: gener… ─ …ent-a1b2c3d4e5f60718`.
     - Depth 5 @ 120 cuts the agent on screen to `code-…` while the id keeps 21 cells.
   - Two related quirks:
     - The innermost agent gets no priority: at depth 4 @ 70, `Plan` is shown as `Pl…`.
     - `USEFUL_NAME_WIDTH = 3` makes depth 3 @ 60 less legible than depth 4 @ 60.
   - **Change (optional).** In the branch where the whole names do not fit, budget the id at its minimum. That is `fit_id(id, MIN_ID_WIDTH + 2)`, or a short tail such as `…/agent-r2` / `…agent-<hex>` when that fits, or what is left after the whole names if that is larger. Give every remaining column to `cut_names`. Optionally, have `fit_widths` take from outer names before the innermost one. The `UI` section pins only the template, not truncation priority, so this is cosmetic.
