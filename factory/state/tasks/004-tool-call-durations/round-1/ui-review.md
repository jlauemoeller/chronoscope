VERDICT: PASS

## Summary
Every state and size in the `UI` section has a snapshot that matches its description, and my own renders agree at sizes down to 10 columns. The duration stays muted, the description keeps its token, and rows with no duration are byte-identical to `main`, styles included. There are two nits: where a wrapped duration can break across rows, and a doubled search count for a subagent's agent type.

## States checked
Snapshots (all under `src/components/turn_block/snapshots/`, prefix `chronoscope__components__turn_block__durations__`):
- Folded call @ 80 / 120 — snapshots `folded_call_with_a_duration_at_80`, `_at_120` — ok
- Unfolded call @ 80 / 120 — snapshots `unfolded_call_with_a_duration_at_80`, `_at_120` — ok
- Folded subagent @ 80 / 120 — snapshots `folded_subagent_with_a_duration_at_80`, `_at_120` — ok
- Unfolded subagent @ 80 / 120 — snapshots `unfolded_subagent_with_a_duration_at_80`, `_at_120` — ok
- Folded row cut @ 40, `…` then the intact duration — snapshot `folded_call_too_narrow_for_its_description_at_40` — ok
- Folded subagent cut @ 40, agent type and duration kept — snapshot `folded_subagent_too_narrow_for_its_description_at_40` — ok
- Unfolded call wrapped @ 40, description in `tool_description`, separator and duration muted — snapshot `unfolded_call_wrapped_at_40_keeps_the_duration_muted_styled` — ok
- Selected folded call @ 80, `▒` marker, muted colour on the duration — snapshot `selected_folded_call_keeps_the_duration_muted_styled` — ok
- OpenCode browser, shell tool unfolded @ 120x40 — snapshot `an_opencode_session_in_the_browser_with_a_shell_tool_unfolded_120x40_text` — ok; the only existing snapshot changed, only its four tool rows moved, and those calls have real durations

Rendered in the scratch crate (`TestBackend`, path dependency on this branch):
- Short, long, subagent, one-hour, blank, multi-line, wide-character, long-word and `Error` calls, folded and unfolded @ 120, 80, 40, 30, 26, 20, 16, 12, 10 — ok; every block ended in its bottom border
- Folded call too narrow for its suffix, for example `Long jo…` @ 16 — ok; the whole row is cut, as the planner note prescribes
- Folded subagent @ 30, `Explore the repositor…` — ok; the agent type and duration leave no room, so the planner's fallback cuts the whole row
- Pending call, pending subagent, call and subagent with no known duration, folded and unfolded, text and styles at the nine sizes — identical to a build of `main`
- Asynchronous subagent — no render of its own: `an_asynchronous_launch_has_no_duration` shows it gets no duration, and a call with no duration renders identically to `main` (above)
- Selection and dimming: dim on and off, selected and unselected, folded and unfolded @ 80, 40, 26 — ok; the duration is `muted_foreground` in every state, and bold on the selected section because selection bolds the whole section
- 4000 random unfolded wraps at widths 14–123 with five duration formats — ok; 0 text errors, every separator and duration cell muted, and no description cell muted
- Search for a duration: wrapped row, wide-character description, multi-line description, blank description, two calls with the same duration, folded neighbour with a cut row — ok; the highlighted cells are the duration's cells
- Search for description text with a duration shown: wrapped row, real-shaped input repeating the description, 300-line output, output repeating the row — ok; the highlight stays on the query's cells
- Search for a blank-description call under its tool name (`Shell`) — ok; the text shown is the text searched, and the build notes record the change
- Search for an agent type on a subagent @ 80x20 — issue, finding 2
- Duration broken across rows @ 40, 14, 10 — issue, finding 1

Test gate, run by me:
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` — pass; no snapshot files were touched
- Budgets were not rerun; the build notes report them within budget

## Findings
1. [nit] Tool call section, unfolded, wrapped description, narrow blocks (seen @ 40, 14 and 10) — the word wrap treats ` · <duration>` as ordinary words, so it can leave the separator at a row end (`… end ·` / `5m 2s`) or split the duration (`· 1h` / `5m`, `1.` / `2s`). A search for `1h 5m` on a split row reports `1/1` and highlights nothing. That follows the existing rule that a match wrapping onto the next row is not highlighted, which `main` also shows for `of the camera`; the split was rare (17 of 4000 random wraps). Change: in `ToolCallSection::description`, keep ` · <duration>` together: when it doesn't fit whole on the last wrapped row, start a new row with `· <duration>`, and only hard-break it when it is wider than the row. See `ui/unfolded-call-duration-split-40.txt`.
2. [nit] Search on a subagent call @ 80x20 — a query for the agent type now counts two hits for one drawn occurrence. `story-writer` shows `1/2` where `main` shows `1/1`, and `n` changes the position while nothing moves on screen, because `tool_text` now has the agent type in `drawn_description()` as well as in the input's `subagent_type`. The build notes anticipate more matches, and the highlight itself is correct. Change: in `tool_text`, leave out a subagent's top-level `subagent_type` or `agent` input value, since `drawn_description()` already holds it. See `ui/subagent-search-count-80.txt`.
