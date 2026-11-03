# Chronoscope — Cycle 2 Specification: Transcript Block Metadata

## 0. How to read this document

**Scope.** Plan and build only the requirements in this document. The application already exists and is complete: do not scaffold it, and do not re-plan anything else.

**Base spec.** [SPEC.md](spec/SPEC.md) describes the application as built. It is context, not work to plan. Where the two documents conflict, this one wins. Other documents linked from it are context too.

**Wireframe.** [cycle-2-ui-sketch.md](spec/cycle-2-ui-sketch.md) is the wireframe for the turns sidebar (§6). It defines only the sidebar and where it sits. Its transcript content is illustrative: fold markers, tool rows and everything else in the transcript follow the base spec and this document.

**Base rules.** These sections of the base spec bind every task and every review. Quote the ones that apply into each task file, because reviewers look for guidelines in this document:

| Section | Rule |
|---|---|
| §1.1 to §1.4 | Architecture, application structure, components, events, focus and keys |
| §4.3 | Overlay appearance |
| §5.2 | Blocks, sections, indentation and fold markers |
| §6.9 | Mouse |
| §7.2 | Live updates in the browser: following, and selection preserved |
| §8.1, §8.2 | Theme tokens and emphasis; components never hard-code colours |
| §9.2 | Scale and performance budgets |
| §9.5 | Robustness: unknown or changed harness data degrades, it does not fail |
| §9.6 | Testing |

**Task ids.** Numbering restarts at `001`, and the factory recognises a merged task by its commit subject. Do not reuse a task id from an earlier cycle (see `git log --format=%s main`).

**Order.** Build §2 to §6 in that order, smallest first. They are independent, except that the sidebar's durations (§6.5) show the duration fixed in §2.3. The last task does §8.

## 1. Background

Transcript blocks show part of what the harnesses record. Three of the numbers in the stats footer are wrong or misleading, tool call durations are computed but never shown, OpenCode errors are not shown at all, and most other metadata is not reachable from the UI.

This cycle fixes the footer, shows tool durations, shows abnormal endings and errors, and adds an on-demand details overlay. It also adds a turns sidebar for seeing the shape of a session and moving around it.

Observations below come from local data written by Claude Code 2.1.x and from a local OpenCode database. They explain the requirements. Field availability varies by version, so every field stays optional (§9.5).

## 2. Stats footer

The footer of an Assistant block becomes:

```
5m 2s · 12 messages · 45.2k tokens · 359.4M cached
```

Unknown parts are left out as today. Cost, when known, stays last (`· $0.42`).

### 2.1 Tokens

The footer adds only input and output tokens, which leaves out almost all input. One Claude session recorded 2.8k input, 336k output, 1.2M cache writes and 359M cache reads, so the number is effectively output alone.

Requirements:

- `TurnStats.tokens` is replaced by a `TokenUsage` with `input`, `output`, `cache_read`, `cache_write` and `reasoning`.
- Each message counts once, using its latest usage, as today.
- `tokens` in the footer = input + output + cache writes.
- `cached` in the footer = cache reads summed over the turn's messages. It is hidden when zero.
- **`output` includes reasoning, on every harness**, and `reasoning` is only an "of which" figure that is never added to a total.
  - Claude: `message.usage.output_tokens_details.thinking_tokens` is already part of `output_tokens` (it never exceeded it in 1438 messages). Do not add it.
  - OpenCode: `tokens.reasoning` is separate from `tokens.output` (one row has reasoning 683, output 120). The adapter adds it to `output`.
- Sources:
  - Claude: `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`.
  - OpenCode: `tokens.input`, `tokens.output`, `tokens.reasoning`, `tokens.cache.read`, `tokens.cache.write`.

### 2.2 Message count (Claude)

`turn_duration.messageCount` is a running total for the session, not a count for the turn. It never decreased in any of 14 sampled sessions, and a 3-second turn reported 4124.

Requirements:

- The count is the number of distinct message ids in the turn, excluding messages whose model is `<synthetic>`.
- It stays current when messages arrive after a `turn_duration` entry.
- `messageCount` is no longer read.
- The `TurnStats.message_count` documentation no longer says "as reported by the harness".
- OpenCode's count is unchanged.

### 2.3 Duration (Claude)

One Assistant turn can contain many `turn_duration` entries, and the footer keeps only the last. Deliveries such as `queued_command` attachments with a non-human origin (task notifications, peer messages) can restart `durationMs` without starting a new User turn (base spec §5.1). One such turn shows 16s but ran from 07:18:45 to 07:25:36. Summing does not work either: in another turn the entries add up to more than the elapsed time.

Requirements:

- The duration is the timestamp of the turn's last `turn_duration` entry minus the timestamp of the prompt that started the turn.
- Without a prompt (for example a turn following a compaction), use the turn's first assistant entry.
- Without a usable timestamp, use that entry's `durationMs`.
- Entries are never summed.
- This includes any time spent waiting on background agents, which is the documented wall-clock meaning of `TurnStats.duration`.
- A footer still appears only once a `turn_duration` entry exists, as today.
- OpenCode's duration is unchanged.

### 2.4 Duration format

Durations use **one format everywhere**: the session selector's rows, the stats footer, the turns sidebar (§6.5), the details overlay (§5.3), and tool calls of a minute or longer (§3). Today the selector and the footer each have their own `format_duration` and they disagree: the selector shows `12m` and the footer `12m 05s`.

| Duration | Resolution | Examples |
|---|---|---|
| Under 1 minute | 1 second | `0s`, `45s` |
| 1 minute up to, but not including, 1 hour | 1 second | `1m 0s`, `4m 3s`, `59m 59s` |
| 1 hour or longer | 1 minute | `1h 0m`, `3h 4m`, `2h 45m`, `26h 30m` |

- Numbers are **not zero-padded**.
- Values are truncated, not rounded, so `59m 59.9s` shows `59m 59s`.
- There is one shared formatter. Remove `format_duration` from both `src/components/session_list.rs` and `src/components/turn_block/renderers/stats.rs`, and put the shared one where neither component depends on the other.
- Only display changes: cached session stats store milliseconds, so the stats cache keeps working as is.

The selector's rows gain seconds under an hour, so `59m` can become `59m 59s`, up to 4 cells wider. No layout change is needed: `row_parts` already measures the stats, gives the title the remaining room, and drops the stats below `MIN_TITLE_WIDTH`. On narrow panes, titles lose those cells and stats can drop out slightly sooner.

### 2.5 Acceptance

- [ ] Format tests cover 0s, 59.9s, 60s, 5m 2s, 59m 59.9s, exactly 1h, 1h 5m and more than 100h.
- [ ] Selector and footer tests assert the new format, including a selector row whose stats no longer fit beside a short title.
- [ ] Only one duration formatter remains for these uses (tool calls under a minute keep their own, §3).
- [ ] Assembler tests cover: cache tokens on both harnesses; each message counted once when its lines repeat; Claude thinking tokens not added to output; OpenCode reasoning added to output.
- [ ] Assembler tests cover a turn with several `turn_duration` entries and a `messageCount` larger than the turn: the duration is wall-clock and the count is per turn.
- [ ] Tests cover each duration fallback and a `<synthetic>` message being left out of the count.
- [ ] `format_stats` tests cover `cached` shown and hidden, and cost remaining last.
- [ ] Footer search still works: the searched text matches the footer as displayed.

## 3. Tool durations

`ToolCall.duration` is computed but no renderer uses it.

Requirements:

- A finished tool call shows a muted ` · 1.2s` after its description, and after the agent-type suffix of a subagent call, both folded and unfolded.
- When the folded row is too narrow, the description is cut with `…` first and the duration is kept, as the agent-type suffix is today (`src/components/turn_block/renderers/tool/mod.rs`).
- Tool calls add sub-second detail under a minute, because the shared format (§2.4) has 1-second resolution and would show most calls as `0s`:

| Duration | Shown as |
|---|---|
| Under 1 second | `340ms` |
| Under 60 seconds | `1.2s` |
| Otherwise | The shared format (§2.4): `5m 2s`, `1h 5m` |

- A pending call shows no duration.
- A call without a known duration shows none.
- **Asynchronous subagents show no duration.** When `toolUseResult.isAsync` is `true`, the result only records the launch, so the assembler leaves `duration` empty.
- **Parsing `toolUseResult`:** it is usually an object but is sometimes a string, and a known field of the wrong JSON type fails the whole line (`src/harness/claude/wire.rs`). It can also be very large (stdout, whole files), so it must not be materialised as a `Value`. Read it through an untagged object-or-`IgnoredAny` head, like `wire::Content`.
- Rendered text and plain text stay consistent, and copy and export follow what is displayed. Search highlights stay aligned with the displayed text.

### 3.1 Acceptance

- [ ] Fixtures set `duration: Some(..)` (none do today), with snapshots folded and unfolded, including a subagent suffix and a row too narrow for the whole description.
- [ ] Format tests cover the boundaries at 1 second and 60 seconds.
- [ ] Assembler tests cover: `isAsync: true` gives no duration; a string `toolUseResult` still parses the line and its result.
- [ ] A search match in a tool description highlights the correct cells when a duration is shown.

## 4. Stop reasons and errors

OpenCode rows with `finish: "error"` have empty content, so the turn shows nothing about what went wrong. The local database has 12 such rows: `aborted` 4, `provider.invalid-request` 6, `provider.transport` 2.

### 4.1 Notices

Add an inline item to the model, next to `Item::ModelChange`:

```
Item::Notice { id, level: Error | Warning, text }
```

| Property | Requirement |
|---|---|
| Section | A new section kind in the turn's order; not foldable; selectable |
| Text | Word-wrapped, indented like other non-foldable lines (base spec §5.2) |
| Style | `Error` uses `error_output`; `Warning` uses the muted style |
| Search, copy, save, export | Included, like the `model → …` note |
| File name word when saved | `error` for `Error`, `notice` for `Warning` |
| Id | `notice:<message or row id>`, upserted so a row updated in place replaces its notice rather than adding another |

Only abnormal endings produce a notice. A normal ending never does.

### 4.2 When a notice is produced

| Harness | Input | Result |
|---|---|---|
| OpenCode | `finish: "error"`, `error.type` is `aborted` | `Warning`: `aborted`, followed by `: <error.message>` when there is one |
| OpenCode | `finish: "error"`, any other type | `Error`: `<error.type>: <error.message>`, using whichever parts exist |
| OpenCode | `finish` other than `stop`, `tool-calls`, `error`; absent is normal | `Warning`: `stopped: <finish>` |
| Claude | `isApiErrorMessage: true` | `Error` with the message's text, **replacing** its text block so it isn't shown twice |
| Claude | `stop_reason` other than `end_turn`, `tool_use`, `stop_sequence`, `pause_turn` | `Warning`: `stopped: <reason>` |

- Claude's `stop_reason` is null on many lines. Use the last non-null value per message id. Null alone produces no notice.
- Synthetic messages without `isApiErrorMessage`, such as `No response requested.`, stay as ordinary text.
- `max_tokens` and `refusal` occur in no local Claude transcript, so tests use constructed fixtures.

### 4.3 Acceptance

- [ ] Assembler tests cover each row of §4.2, an OpenCode row updated in place producing one notice, and an OpenCode row going from streaming to error.
- [ ] A Claude API error shows one notice and no duplicate text block.
- [ ] Snapshots cover both levels at 80 and 120 columns, including a long message that wraps.
- [ ] Tests cover selecting, copying, saving, exporting and searching a notice.

## 5. Details overlay

An on-demand overlay shows the metadata of the selected section, so blocks stay uncluttered.

### 5.1 Key and behaviour

- New browser action `details`, default key `i`, configurable under `[keys.browser]`, described as "Show section details" in the Sections help group.
- It opens for the selected section. A `… N more lines` row resolves to its parent tool call. With no selection it does nothing.
- It reuses `open_overlay` and the help overlay's frame, scrolling and overlay keys (base spec §4.3). The `details` key also closes it.
- It is a snapshot taken at opening and does not update while open.
- It is not part of copy, save or export.

### 5.2 Loading

Most of these fields vary across Claude Code versions and are rarely looked at, so they must not be parsed into the model.

- Turns store **only cheap source references**: a byte range for a Claude line, a row id for OpenCode. They never store the field values.
- A tool call has two references, for its call and its result. On Claude, permission, sandbox and `toolUseResult` data live on the result's line.
- The source is re-read when the overlay opens, following `extract_result_text` and `ByteSource` (`src/harness/claude/wire.rs`, `src/model/output.rs`).
- The harness adapter interface provides the read, so the UI stays harness-agnostic (base spec §1.1). Codex and Pi adapters can provide none.
- Reading `toolUseResult` for this view uses a typed head too (§3), and never loads the whole value.
- The §9.2 budgets must still be met.

### 5.3 Content

Rows are `label  value`, in the groups below, in this order. Groups that do not apply to the selected section are left out.

| Group | Applies to | Claude | OpenCode |
|---|---|---|---|
| Turn | Every section of an Assistant turn | Model, effort, start, duration, messages, tokens by input / output / thinking / cache read / cache write | The same, plus cost, provider (`model.providerID`) and agent |
| Message | Text, thinking, notice | `message.id`, `requestId`, `timestamp`, `requestedModel`, `stop_reason`, `service_tier`, `speed`, `inference_geo`, 5m and 1h cache writes, web search and fetch counts, `diagnostics.cache_miss_reason` | Row id, `time.created`, `time.streamed`, `time.completed`, `finish`, `rawFinish`, `error` |
| Tool call | Tool call | Start, duration, status, `permissionDecision`, `sandboxed`, `resolvedModel`, `isAsync` | Start, duration, status |
| Prompt | User prompt | `timestamp`, `permissionMode`, `promptSource` | `time.created` |
| Environment | Every section with a source | `gitBranch`, `cwd`, `version`, `entrypoint` | `snapshot.start`, `snapshot.end` |

- Timestamps use the configured format (base spec §8.6).
- Durations use the shared format (§2.4), and token counts use the existing format.
- Missing fields are left out. A group with no rows is left out.
- Claude records no cost, so none is shown. **Do not build a price table.**
- An unreadable source (file gone, row deleted, line unparseable) shows one muted `unavailable` row in the affected group. It never panics or closes the overlay.
- Compaction and system sections show what their source provides under Environment, plus their timestamp.

### 5.4 Acceptance

- [ ] Keymap tests and the action table in `src/keymap/tests.rs` include `details`. Help content lists it from the active keymap.
- [ ] Component snapshots cover one section of each kind at 80x24 and 120x40, in the `default` and `terminal` themes, including content taller than the screen.
- [ ] `App::update` tests cover opening, closing with both keys, focus returning, a `More` row resolving to its tool call, and doing nothing without a selection.
- [ ] Adapter tests cover each group on both harnesses, a line missing the fields, an unreadable source, and a `toolUseResult` string.
- [ ] A test or benchmark shows that assembling a turn does not parse the detail fields, and the budgets below hold.

## 6. Turns sidebar

A sidebar in the session browser lists the session's turns and compaction events, so the shape of a long session is visible and any block is a few keys away.

```
╭─ [1] Turns  ───────────────╮
│                            │
│  ┬   Start                 │
│  │                         │
│  ├   User                  │
│  │                         │
│  ├   Assistant      4m 3s  │
│  │                         │
│  ├   User                  │
│  │                         │
│  ├   Assistant      3h 4m  │
│  │                         │
│  ├╶╶ Compaction ╶╶╶╶╶╶╶╶╶  │
│  │                         │
│  ├   Assistant     2h 45m  │
│  │                         │
│  ┴   End                   │
│                            │
│                            │
╰────────────────────────────╯
```

### 6.1 Layout

- A new `TurnSidebar` component (base spec §1.3), one mounted instance for the browser.
- It sits to the left of the transcript, below the header. Header and help line stay full width.
- It is **30 columns wide, with a 1-column gap** before the transcript, and exactly as tall as the transcript area.
- **It stays in a fixed position:** scrolling the transcript never moves the pane. Only its own list scrolls, and only as far as needed to keep the selected row visible, as in the selector's lists.
- The areas come from the pure layout function (base spec §1.4), so drawing and mouse routing agree. The transcript's viewport and its mouse coordinates use the narrower area.
- **Below 90 columns the sidebar is hidden** and the transcript takes the full width as today, because 90 is what leaves the transcript 59 columns. While hidden, the focus keys do nothing. If it has focus when it becomes hidden, focus moves to the transcript.
- It is not drawn when `EmptySession` replaces the transcript. An open subagent shows that subagent's turns, and going back shows the parent's again.

### 6.2 Rows

There is one entry per turn with at least one non-system section, in transcript order, taken from `derive_sections` (`src/components/turn_block/section.rs`). This matches the blocks `]` / `[` can stop on, because system content never forms a block of its own, so toggling it (`s`) does not change the entries. Do not call `turn_stop` for every turn to build the list: it renders the block, which §6.6 forbids.

| Row | Content | Selectable |
|---|---|---|
| `┬ Start` | First row of the list | No: it has no block |
| `├ User` | A User turn | Yes |
| `├ Assistant` | An Assistant turn, with its duration (§6.5) | Yes |
| `├╶╶ Compaction ╶╶╶` | A Compaction turn; `╶` fills to the right margin | Yes |
| `┴ End` | Last row of the list | No: it has no block |

- Columns (from 0): `┬`, `├`, `┴` and the marker in column 3; labels in column 7, with the Compaction rule running from column 4; the right margin is two cells before the border, as in the selector.
- Between two rows of the list there is one row with `│` in column 3, so the timeline runs unbroken through a Compaction too.
- One blank row separates the list from the top border.
- Unselected rows, `┬`, `├`, `┴`, `│` and `╶` use `muted_foreground`.
- Long lists scroll by rows. `Start` and `End` scroll with the list. Connector rows cannot be selected or clicked.

### 6.3 Selection and highlighting

Selection looks and behaves like the session selector's lists (base spec §8.2, §4.1), except that its marker is `▒`. Reuse `list_pane` where it fits.

| State | Appearance |
|---|---|
| Selected, sidebar focused | `▒` in `primary` replaces the `├` in column 3, never inverted. Everything after it is one inverted band (`foreground` with reverse video, not bold) up to the right margin; the label, gaps and duration all take the band's style |
| Selected, sidebar unfocused | `▒` in `secondary_foreground`, text bold in `secondary_foreground`, not inverted |
| Border | `ring` when focused, `border` otherwise |

- The list never wraps around.
- **While the transcript has focus, the sidebar's selection follows it:** the selected row is the turn containing the transcript's selected section, and it stays visible. This keeps the sidebar showing the current position, and returning to it never jumps. Pushing that selection to the sidebar produces no message.
- When the sidebar has focus and nothing is selected, it selects the first entry on the first move. An empty list has no selection, and its keys do nothing.

### 6.4 Keys and focus

A new keymap context `[keys.turns]` holds the sidebar's keys (base spec §1.4). Its defaults match the selector's:

| Context | Action | Default keys | Effect |
|---|---|---|---|
| `browser` | `focus_turns` | `1` | Focus the sidebar |
| `browser` | `next_pane`, `prev_pane` | `tab`, `shift-tab` | Focus the sidebar |
| `turns` | `up`, `down` | `up`, `k` and `down`, `j` | Select the previous or next entry, and scroll the transcript to it |
| `turns` | `open` | `enter` | Move focus to the selected entry's block |
| `turns` | `next_pane`, `prev_pane` | `tab`, `shift-tab` | Focus the transcript, leaving it as it is |
| `turns` | `focus_transcript` | `right`, `l` | The same |

- **`1` and `Tab` in the browser must not affect the selector,** where `1` still switches harness and `Tab` still switches pane. Contexts already keep these apart.
- `1` does nothing while the sidebar has focus, and neither does `←` / `h`, matching the selector, where they don't wrap around.
- Selecting an entry **scrolls the transcript to its block**, with the same result as `]` / `[`: the block's top at the top of the viewport (or as near as the end of the transcript allows), and the transcript's selection on the block's first selectable section. Reuse that code rather than writing a second path. The usual follow rule applies (base spec §7.2): moving away from the bottom stops following.
- A turn can have an entry but nothing selectable, when its text renders to no rows (for example, only an HTML comment). Selecting that entry still scrolls to the block and leaves the transcript's selection unchanged.
- `Enter` does the same and also gives the transcript focus.
- Focusing the sidebar does not by itself scroll the transcript.
- While the sidebar has focus, other browser keys (`/`, `y`, fold keys and so on) do nothing. Global keys still work (`q`, `?`, `t`, `Ctrl-o`). Overlays return focus to the sidebar when they close (base spec §1.4).
- The help line shows hints for the sidebar's keys while it has focus, generated from the active keymap, e.g. `ENTER to go to block | TAB to switch to transcript`.
- The help overlay lists the new keys in a "Turns sidebar" group, and they can be remapped like any other.

**Mouse** (base spec §6.9): the wheel works as in the selector's lists. Clicking a selectable row focuses the sidebar and selects that row, double-clicking does what `Enter` does, and clicking the transcript gives it focus.

### 6.5 Durations

Each Assistant entry shows `TurnStats.duration` in the shared format (§2.4), right-aligned to the right margin. Turns without stats (for example, still running) show none, and it appears in place once known.

### 6.6 Performance

Sessions can have thousands of turns (base spec §9.2). Building rows must not lay out or render transcript blocks, and turn updates must not rebuild more than the rows that changed. The sidebar draws only the rows it shows, and a key press in the sidebar stays within the frame budget on the 100 MB fixture.

### 6.7 Acceptance

- [ ] Layout tests cover widths 89 and 90, the 30-column pane and 1-column gap, full-width header and help line, and mouse routing to the right component.
- [ ] Component snapshots at 120x40 and 90x24 in the `default` and `terminal` themes cover: focused and unfocused selection on each selectable kind, no selection, a list longer than the pane scrolled to each end, and the widest durations.
- [ ] Row tests cover: each kind and its order, `Start` and `End` not selectable, connector rows around a Compaction, a turn with only redacted thinking or only system content having no entry, toggling system content leaving the entries unchanged, and an Assistant turn gaining a duration.
- [ ] `App::update` tests cover: `1`, `Tab` and `Shift-Tab` focusing the sidebar; `Tab`, `→` and `l` returning without moving the transcript; `Enter` moving focus to the block; an entry whose text renders to no rows; each `j` / `k` scrolling the transcript and moving its selection; no wrap-around at either end.
- [ ] Tests cover the selection following the transcript through `j` / `k`, `]` / `[`, page keys and search; focus returning after an overlay; opening and leaving a subagent; focus moving when the sidebar becomes hidden.
- [ ] Keymap tests show `1` and `Tab` still act on the selector there, and that `[keys.turns]` can be remapped.
- [ ] Tests cover live updates: an appended turn adds a row without moving the selection, and a following transcript's selection is tracked.
- [ ] Mouse tests cover clicking and double-clicking a row, a click on a connector row, and the wheel.

## 7. Acceptance for the whole cycle

Every task must pass:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tasks that change parsing, the model or rendering must also stay within budget on:

```
cargo test --release -- --ignored budgets --nocapture
```

Snapshot changes are limited to the rows a task intends to change. A snapshot changed by accident is a failure, not something to accept. Two changes are expected to touch many snapshots, and those are still reviewed one by one: the shared duration format (§2.4) changes every snapshot showing a footer or selector-row duration of a minute or longer, and the sidebar changes every browser screen snapshot at 90 columns or wider.

## 8. Documentation (final task)

- Fold the changes into `doc/SPEC.md`:
  - §3.1, §3.2: fields now read.
  - §5.1: the model, and rule 6 of Claude turn assembly.
  - §5.2: the footer row, tool call rows and a Notice row.
  - §4.3, §6.5: the details overlay and its key.
  - §4.1, §5.2: the shared duration format.
  - §1.3, §4.2, §6.3, §6.9: the turns sidebar, its component, keys and mouse behaviour.
  - §8.5: the `[keys.turns]` context.
- Add `details`, `focus_turns`, `next_pane` and `prev_pane` to the README's browser key table, and add a `[keys.turns]` table.
- Update `docs/performance.md` if the measured figures change.
