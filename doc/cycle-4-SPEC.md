# Chronoscope — Cycle 4 Specification: Subagent Title Rows

## 0. How to read this document

**Scope.** Plan and build only the requirements in this document. The application already exists and is complete: do not scaffold it, and do not re-plan anything else.

**Base spec.** [product.md](product.md) describes the application as built. It is context, not work to plan. Where the two documents conflict, this one wins. Other documents linked from it are context too.

**References.** `§n` means a section of this document, and `base §n` a section of the base spec.

**Wireframes.** The wireframes in §2 are exact for the subagent title rows: their columns, gaps and widths are requirements. Everything else in them (the block, its title, the marker, the indent, the body) is drawn as it is today and must not change.

**Base rules.** These sections of the base spec bind every task and every review. Quote the ones that apply into each task file, because reviewers look for guidelines in this document:

| Section | Rule |
|---|---|
| §1.3 | Components; the transcript is built from small parts, each its own type with its own tests |
| §5.2 | Blocks, sections, indentation, fold markers and tool call rows |
| §5.4 | Subagents |
| §6.2 | Transcript search: the text shown is the text searched |
| §6.4 | Copy, save, export |
| §6.9 | Mouse |
| §8.1, §8.2 | Theme tokens and emphasis; components never hard-code colours |
| §9.2 | Scale and performance budgets |
| §9.6 | Testing |

**Task ids.** Numbering restarts at `001`, and the factory recognises a merged task by its commit subject. Do not reuse a task id from an earlier cycle (see `git log --format=%s main`).

**Order.** §2 is one change and fits one task. The last task does §4.

## 1. Background

A subagent call is folded by default, and its row looks like every other tool call's: `▶ <description> (<agent>) · <duration>`. In a long Assistant turn, the calls that started subagents, often the turn's main work, are hard to pick out among the shell, read and edit calls around them. The key that opens a subagent's transcript is named only on the last row of the unfolded body, so a folded subagent row does not say that it can be opened at all.

This cycle gives every subagent title row a horizontal rule, and moves the `a to open transcript` hint from the end of the body onto that row. What the row says, and how the section folds, selects, opens and is searched, stay as they are.

## 2. Subagent title rows

### 2.1 Which rows

The **title row** of a subagent section is:

- folded: its one row;
- unfolded: the last of the rows its description and duration are word-wrapped into (base §5.2, tool call rows).

The change applies to every section of a `ToolKind::Subagent` call, on both harnesses: folded and unfolded, selected or not, pending or finished, with or without a known agent type, in a session and in an opened subagent's transcript (a subagent's own subagents). No other tool call changes.

### 2.2 Layout

The title row holds, from left to right:

1. The description, ` (<agent>)` and ` · <duration>`, exactly as today: the same text, order, cut and styles.
2. A gap of 2 cells.
3. A rule of `─`.
4. A gap of 2 cells.
5. The hint `<key> to open transcript`, where `<key>` is the label of the `open_subagent` key (`a` by default, `Ctrl-o` or another label when remapped), as in today's body hint.

The hint is right-aligned with **one blank cell between it and the block's right border**, the same gap as between the left border and the fold marker. The text area (the block's width `W` less 8, base §5.2) leaves 3 cells of padding before the right border. A title row reaches 2 cells into that padding, so it ends at column `W − 3` (counting from 0), with column `W − 2` blank and the border at `W − 1`. Title rows are the only rows that use this padding, and only the rule and the hint go there. The description, the agent type and the duration still end within the text area. The rule fills every cell between the two gaps. Widths are display cells, not characters.

Folded, 100 columns:

```
╭─ Assistant ── claude-opus-5.5 @ high ───────────────────────────────────── 2026-10-21 08:35:18Z ─╮
│                                                                                                  │
│ ▶ Explore the UVC descriptors (Explore)  ─────────────────────────────────  a to open transcript │
│                                                                                                  │
╰──────────────────────────────────────────────────────────────────────────────────────────────────╯
```

Unfolded, 100 columns. The body's last two rows (a blank row and the hint) are gone (§2.5):

```
╭─ Assistant ── claude-opus-5.5 @ high ───────────────────────────────────── 2026-10-21 08:35:18Z ─╮
│                                                                                                  │
│ ▼ Explore the UVC descriptors (Explore)  ─────────────────────────────────  a to open transcript │
│                                                                                                  │
│   Prompt:                                                                                        │
│                                                                                                  │
│      Look through lsusb.txt and list:                                                            │
│                                                                                                  │
│      • every extension unit                                                                      │
│      • its GUID                                                                                  │
│                                                                                                  │
│   Result:                                                                                        │
│                                                                                                  │
│      Found two extension units:                                                                  │
│                                                                                                  │
│      1. exposure                                                                                 │
│      2. pan and tilt                                                                             │
│                                                                                                  │
╰──────────────────────────────────────────────────────────────────────────────────────────────────╯
```

### 2.3 Narrow rows

The rule and the hint use only the room the description leaves. They never cut, wrap or move the description, the agent type or the duration. The rule is at least 3 cells wide. Let:

- `d` be the width of item 1 of §2.2 on the title row;
- `h` be the width of the hint;
- `t` be the width of the text area (`W − 8`), within which the description is laid out as today;
- `r = t + 2` be the width of the title row, from the text column to the cell before the right border's gap (§2.2).

| Room | Title row |
|---|---|
| `d + 7 + h ≤ r` | Everything in §2.2. The rule is `r − d − 4 − h` cells, so it shrinks first, down to 3 cells |
| `d + 5 ≤ r < d + 7 + h` | The hint is left out. The gap and the rule run to the row's end, one cell before the border: the rule is `r − d − 2` cells |
| `r < d + 5` | Neither rule nor hint. The row is exactly as today, including a description cut with `…` |

A folded row whose hint is left out shows it nowhere else. An unfolded one moves it to a row of its own (§2.4).

Folded at 80 columns, with the default key. The row keeps its hint, with a rule of 5 cells:

```
╭─ Assistant ── claude-opus-5.5 @ high ───────────────── 2026-10-21 08:35:18Z ─╮
│                                                                              │
│ ▶ Explore the repository (story-writer) · 5m 2s  ─────  a to open transcript │
│                                                                              │
╰──────────────────────────────────────────────────────────────────────────────╯
```

The same row with `open_subagent` bound to `Ctrl-o`. The 25-cell hint does not fit, so only the rule is shown:

```
╭─ Assistant ── claude-opus-5.5 @ high ───────────────── 2026-10-21 08:35:18Z ─╮
│                                                                              │
│ ▶ Explore the repository (story-writer) · 5m 2s  ─────────────────────────── │
│                                                                              │
╰──────────────────────────────────────────────────────────────────────────────╯
```

At 40 columns, the row of the snapshot `folded_subagent_too_narrow_for_its_description_at_40` already has its description cut today. It has no room for a rule, and stays as it is.

### 2.4 Unfolded

The description and duration are word-wrapped as today, and §2.2 and §2.3 apply to the last of their rows. There, `d` is the width of that last row only: the end of the description, with the duration when it ends there, or the duration alone when it is on a row of its own. It is not the width of the whole description. An unfolded section always shows the hint, as it does today:

- When the hint fits on that row (the first case of §2.3), the rule and the hint go there.
- Otherwise the hint goes on a row of its own, directly below the description, and the description's rows have no rule. That row starts at the text column and is `r` cells wide, like a title row. Its rule runs from the text column to the gap before the hint, and the hint ends one cell before the right border (§2.2).
- When `r < h + 5`, that row holds only the hint, from the text column, cut with `…` when it is wider than `r`, as today's body hint is cut.

Unfolded at 80 columns, with `open_subagent` bound to `Ctrl-o`:

```
╭─ Assistant ── claude-opus-5.5 @ high ───────────────── 2026-10-21 08:35:18Z ─╮
│                                                                              │
│ ▼ Explore the repository (story-writer) · 5m 2s                              │
│   ───────────────────────────────────────────────  Ctrl-o to open transcript │
│                                                                              │
│   Prompt:                                                                    │
│                                                                              │
│      List the crates.                                                        │
│                                                                              │
│   Result:                                                                    │
│                                                                              │
│      Found three crates.                                                     │
│                                                                              │
╰──────────────────────────────────────────────────────────────────────────────╯
```

The hint's own row belongs to the description rows. It is followed by the blank row that today follows the description.

### 2.5 The body

- The body no longer ends with a blank row and the hint. An unfolded subagent shows `Prompt:` and `Result:` as today, and nothing after the result.
- The `… N more lines` row of a truncated result follows the result directly, as it does for every other tool call (today it comes after the hint). It is still the section's last row and a selection stop of its own.

### 2.6 Style and emphasis

- No background colour is painted. The `search_match` highlight and the selector's inverted band (base §8.1, §8.2) stay the only backgrounds.
- The rule is drawn in `border`, the colour of the block's own borders, whatever the selection and `dim_unselected`. It is never dimmed and never bold. A rule that followed the row's emphasis would become `muted_foreground` when unselected, which in most themes is brighter than the `border` it would have when selected.
- The hint is `muted_foreground` and takes the section's emphasis, as today's body hint does: bold when the section is selected.
- The description, the agent type and the duration keep their styles and emphasis.
- No theme token is added. A theme change recolours the rule and the hint with the rest of the transcript.

### 2.7 Text, mouse and keys

The rule and the hint are chrome, like the fold marker: they are drawn, but they are not the section's text.

- **Search** (base §6.2) finds the same matches as today. It matches the description row as drawn without the rule and the hint (`drawn_description`), so `open transcript` is not a match.
- **Highlights** are never drawn on the rule or the hint, even when the query occurs in them. This is a change: highlights are found again in each drawn row's text (`TurnBlock::highlight` in `src/components/turn_block/widget.rs`), so today a query such as `transcript` also lights up the body hint. Now that the hint is on every subagent row, folded ones included, that would mark every subagent while searching. Highlights in the description stay on the cells of the match.
- **Copy and save as displayed** (`y`, `w`) give the description rows without the rule and the hint, and a folded subagent gives its description row, as today. Because the body's hint row is gone, the text of an unfolded subagent no longer ends with a blank row and `a to open transcript`. This is the only change to text that is copied, saved or exported.
- **Full copy, full save and both exports** (`Y`, `W`, `e`, `E`) are unchanged: none of them contains the hint today.
- **Mouse** (base §6.9): the hint is not a link, and clicks are unchanged. A click on a section's first row selects it and toggles its fold, and a click on any other row selects it (`src/components/transcript/mouse.rs`), whether it lands on text, the rule or the hint. So a click on the rule or the hint of a folded row toggles the fold, and a click on the hint's own row (§2.4) only selects. Today a click in the right padding does nothing (`CONTENT_MARGIN`). On a row that draws the rule or the hint into the padding (§2.2), its two cells count as part of that row. Every other padding cell, on that row and on every other row, still does nothing. The wheel is unchanged.
- **Keys:** the `open_subagent` key works as today. Remapping it changes the hint on every title row, and can move an unfolded hint onto or off a row of its own (§2.4). The transcript already lays every section out again when the key's label changes.

### 2.8 Structure

- The title row's layout (the gaps, the rule, the hint and the cases of §2.3 and §2.4) is a small function or type of its own beside `row::folded` and `row::wrapped` in `src/components/turn_block/renderers/tool/row.rs`, with its own unit tests (base §1.3). `ToolCallSection` composes it. It is not written inline in a render function.
- `Body::hint`, `PlainTool::hint` and the `with_hint` parameter of `plain_lines` have no use left and are removed, as is the documentation that describes the hint as part of the body (for example `SubagentBody`'s).
- The hint text still comes from `SectionCtx::open_key`.
- `TurnBlock` clips every row at the text area today (`draw_row` in `src/components/turn_block/widget.rs`). It must draw title rows, and the hint's own row, up to column `W − 3`, without moving the padding or the border of any other row. Rows keep their width: a title row is `r` cells after its lead.
- Drawing stays within the existing budgets (base §9.2).

### 2.9 Acceptance

Renderer tests:

- Folded with room: the row is the description, 2 blanks, the rule, 2 blanks and the hint. Its width after the lead is `r = t + 2`. Drawn in a block, the hint's last cell is at column `W − 3`, followed by one blank cell and the border.
- At `r = d + 7 + h` the rule is 3 cells and the hint is shown. One cell narrower, the hint is left out and the rule is `r − d − 2` cells, ending at column `W − 3`. At `r = d + 5` the rule is 3 cells. At `r = d + 4` the row equals today's row.
- A description that fills the text area (`d = t`) has neither rule nor hint, and nothing of it is drawn in the padding.
- Rows other than title rows and the hint's own row leave all 3 cells of padding blank, as today.
- A description cut with `…` has neither rule nor hint, and keeps its agent type and duration as today.
- The hint follows a remapped key (`o`, `Ctrl-o`), on folded and unfolded rows.
- Unfolded: a description that wraps over two rows has the rule and the hint on the second row only, after the duration. A duration on a row of its own has them after it.
- Unfolded, with a hint that does not fit on the last row: the hint is on a row of its own below the description, ending at column `W − 3`, and no description row has a rule. With `r < h + 5`, that row is the hint alone, cut with `…` to `r` cells.
- A pending subagent and a subagent without an agent type (`▶ Do the thing`) get the rule and the hint. A Shell call and an Other call get neither.
- The unfolded body's last row is the result. With a truncated result, the `… N more lines` row directly follows it and is the section's last row.
- The rule's foreground is `border` and it is not bold when the section is selected, unselected with `dim_unselected` on, and unselected with it off. The hint is `muted_foreground`, and bold when selected. No span of a title row has a background colour.

Text, search and mouse:

- `displayed_text` of a folded subagent is its description row, with no `─` and no hint. Unfolded, it contains no `to open transcript` and ends with the result.
- `full_text` and both exports give the same text as before this change for the subagent fixtures.
- A query matching the description of a folded subagent row that shows the rule and the hint highlights exactly the matched cells. A query for `open transcript` finds no match in a subagent whose input and result do not contain it.
- A query that matches the section elsewhere and also occurs in the hint (`transcript`, with a result containing it) highlights no cell of the hint, folded or unfolded, on the title row or on the hint's own row. A query of `─` highlights no cell of the rule.
- A click on the rule and a click on the hint of a folded subagent row each select the section and unfold it. This includes the hint's last cell, in the padding. A click on column `W − 2` of that row, or on the padding of any other row, does nothing. A click on the hint's own row of an unfolded subagent selects the section and leaves it unfolded.

Snapshots:

- The turn block snapshots of the wireframes: folded and unfolded at 100 columns; folded and unfolded at 80 with the default key and with `Ctrl-o`; the folded row at 40, unchanged.
- A styled snapshot shows the rule in `border` and the hint muted, with the section selected and unselected.
- Changing the theme recolours the rule and the hint.

## 3. Acceptance for the whole cycle

Every task must pass:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The change is to rendering, so the budgets must also hold:

```
cargo test --release -- --ignored budgets --nocapture
```

Snapshot changes are limited to the subagent title rows and the body rows that §2.5 removes, and each is reviewed one by one. These snapshots are expected to change:

- `src/components/turn_block/snapshots/`: `snapshots__subagent_folded_at_100_text`, `snapshots__subagent_unfolded_at_100_text`, `durations__folded_subagent_with_a_duration_at_80`, `durations__folded_subagent_with_a_duration_at_120`, `durations__unfolded_subagent_with_a_duration_at_80` and `durations__unfolded_subagent_with_a_duration_at_120`.
- `src/app/browser_tests/snapshots/`: `subagent__parent_with_a_subagent_selected_120x40_text`, `subagent__after_backspace_the_screen_is_the_parent_as_it_was_120x40_text`, `subagent__the_subagent_120x40_text`, `subagent__the_subagent_120x40_styled`, `subagent__the_subagent_80x24_text` and `sidebar_follow__going_back_from_a_subagent_shows_the_pane_as_it_was_with_a_list_longer_than_the_pane`.

`durations__folded_subagent_too_narrow_for_its_description_at_40` must not change. Any other snapshot change that is not a subagent title row is accidental, and a failure, not something to accept.

These tests assert the hint as the body's last row, and are updated to the title row: the tests in `src/components/turn_block/renderers/tool/subagent.rs`, the hint test in `src/components/turn_block/tests.rs` (the keymap's label in the hint), the hint and truncation tests in `src/components/transcript/tests/truncation.rs`, and the hint assertions in `src/app/browser_tests/subagent.rs`.

## 4. Documentation (final task)

- Fold the changes into `doc/SPEC.md`:
  - §5.2: the subagent call row of the sections table (folded: the description row, then the rule and the hint; unfolded: the same title row, then the prompt and the final result), and a paragraph on subagent title rows after the tool call rows paragraph: the layout (with the hint one cell from the right border, in the padding the text area leaves), the cases of §2.3 and §2.4, the styles and the rule's exemption from dimming.
  - §5.4: the hint is on the title row, not in the body.
  - §6.4: the rule and the hint are not part of copied or saved text.
  - §6.9: a click on the rule or the hint is a click on its row.
  - §8.2: the rule is drawn in `border` and is never dimmed or bold.
- Add a folded subagent row with its rule and hint, and a callout naming it, to the session browser sketch in `doc/ui-sketch.md`, using the sketch's conventions (`>` and `-` for the fold markers).
- Update the README only if it describes the hint.
