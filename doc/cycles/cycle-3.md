# Chronoscope — Cycle 3 Specification: Session Totals

## 0. How to read this document

**Scope.** Plan and build only the requirements in this document. The application already exists and is complete: do not scaffold it, and do not re-plan anything else.

**Base spec.** [product.md](../product.md) describes the application as built. It is context, not work to plan. Where the two documents conflict, this one wins. Other documents linked from it are context too.

**References.** `§n` means a section of this document, and `base §n` a section of the base spec.

**Base rules.** These sections of the base spec bind every task and every review. Quote the ones that apply into each task file, because reviewers look for guidelines in this document:

| Section | Rule |
|---|---|
| §1.1 to §1.4 | Architecture, application structure, components, events, focus and keys |
| §4.1 | Session selector: layout, session selection and the shared duration format |
| §5.1, §5.2 | The model, turn assembly and injected items |
| §6.9 | Mouse |
| §8.1, §8.2 | Theme tokens and emphasis; components never hard-code colours |
| §9.2 | Scale and performance budgets; stats are computed off the UI thread and cached |
| §9.5 | Robustness: unknown or changed harness data degrades, it does not fail |
| §9.6 | Testing |

**Task ids.** Numbering restarts at `001`, and the factory recognises a merged task by its commit subject. Do not reuse a task id from an earlier cycle (see `git log --format=%s main`).

**Order.** Build §2, then §3, then §4. §2 changes what stats compute: the turn count, and agent time, because a local command no longer splits an Assistant turn. The cache version in §3 must therefore land with or after §2, or stale values would survive under the new version. The panels (§4) show the stats of §3. The last task does §6.

## 1. Background

The session selector showed 81h 33m and 8 turns for a session whose sidebar durations add up to 17h 56m. Both numbers were calculated as designed, but they do not measure what they appear to:

- **Duration.** The selector's row shows the last entry's timestamp minus the first's, idle time included. One 3-second prompt sent 53 hours after the previous activity added those 53 hours. The sidebar shows only the Assistant turns.
- **Turns.** The count includes local commands. A cancelled `/effort` counted as two turns: its command echo and its output. Across 61 local sessions, 45 local-command echoes and 11 local-command outputs were counted this way, 34 of the echoes being `/clear`.

This cycle makes the selector's row match the browser, stops counting local commands as turns, and adds two panels with the session's totals: its time and its token use.

Observations below come from local data written by Claude Code 2.1.x and from a local OpenCode database. They explain the requirements. Field availability varies by version, so every field stays optional (base §9.5).

**Worked example** (session `8535a1b4-7905-4aa7-8a19-94beab3aa4ca`, for orientation only: tests use fixtures):

| Figure | Before | After |
|---|---|---|
| Selector row | `8 turns · 81h 33m` | `6 turns · 17h 56m` |
| Sidebar User rows | 8 | 6 |
| Total time | not shown | `81h 33m` |
| Agent time | not shown | `17h 56m` |
| Input tokens | not shown | `15.8k` |
| Output tokens | not shown | `6.3M` |
| Thinking tokens | not shown | `2.2M` |
| Cache read tokens | not shown | `862.3M` |
| Cache write tokens | not shown | `30.7M` |

## 2. Turn count: local commands

A local command is handled by the harness and is not a prompt to the model. It no longer starts a turn, on any screen, and it stays visible in the transcript.

### 2.1 Detecting a local command (Claude)

A `user` entry is local-command content when all of these hold:

- It has no `origin` field.
- It is not `isMeta`, not a compact summary and holds no `tool_result` (these are already handled before this rule).
- Its text, with leading whitespace trimmed, starts with `<command-name>` (the echo), `<local-command-stdout>` or `<local-command-stderr>` (the output).

The rule needs only the entry itself, so the transcript scan can apply it line by line. It was checked against 61 local sessions: all 45 local-command echoes have no `origin` and start with `<command-name>`. All 30 echoes of commands that do reach the model (`/factory`, `/statusline`, `/doctor`) have `origin.kind` `human` and start with `<command-message>`, so they stay prompts.

Not local commands, and unchanged:

- A prompt typed with a leading space, such as ` /clear`. It is sent to the model, is recorded with `origin.kind` `human` and stays a turn.
- `!` shell commands (`<bash-input>`, `<bash-stdout>`, `<bash-stderr>`). Their output reaches the model. They are out of scope and keep the rows they have today.
- OpenCode, which records no local commands.

### 2.2 Where local commands go

- Each echo and each output becomes an injected entry of kind `local_command` through the existing held-entries path (base §5.1). This is the kind `system` `local_command` entries already use, and those are unchanged. It therefore joins the neighbouring block the way other injected entries do: into the next User turn when one follows, else into the current turn at the end of the input.
- Its text drops the markup. An echo becomes the command and its arguments (`/effort`, `/model opus`), and an output becomes the text inside the tags (`Cancelled`). A body that does not match these forms is kept as it is.
- The echo and its output are separate entries and are not merged.
- A live batch can end between a local command and the next prompt. As for every other injected entry, `flush_held` then places the entry in the current turn, and it stays there. Reloading the session places it in the next turn. Only the block holding the entry differs: the turns themselves do not.
- A local command does not end the current Assistant turn. Assistant entries after it continue that turn, as they do when no local command came between them.
- The `<local-command-caveat>` meta entry is unchanged.

### 2.3 Sessions with only local commands

3 of the 61 local sessions contain nothing else. These have no turns to attach to, and must not claim to be empty:

- The selector's row shows `0 turns · 0s`.
- The browser's empty state replaces `Session file is empty` with `No turns. Local commands only:` followed by the commands, one per line, in the order they ran. A file with truly nothing in it still shows `Session file is empty`.
- When a turn arrives in a live session, the transcript replaces this state with the normal view, and the commands are part of the first User turn.

Held entries after a trailing compaction keep today's behaviour: they appear once a later turn arrives.

### 2.4 Title and first prompt

The transcript scan uses the same prompt rule as the assembler, so local commands stop being a session's first prompt. A session without an `ai-title` that began with `/effort` is titled from its next real prompt, and one with only local commands is `(untitled)`. Search over the first prompt follows.

### 2.5 Acceptance

- The fixture sequence caveat, `/effort` echo, `Cancelled` output, `/factory` prompt, Assistant reply gives one User turn and one Assistant turn. The User turn's injected item holds the `local_command` entries.
- A local command between two prompts adds no sidebar row and no turn.
- For every fixture, the selector's turn count equals the number of User rows in the sidebar.
- Loading a fixture at once and streaming it in batches split at every line give the same User and Assistant turns and the same `local_command` entries. The block holding an entry may differ, as §2.2 describes.
- An entry with `origin.kind` `human` starting with `<command-message>` stays a prompt, and an entry with `<command-name>` text and `origin.kind` `human` also stays a prompt.
- A local-commands-only fixture gives the empty state of §2.3 and 0 turns.
- A snapshot covers the injected `local_command` entries, folded and unfolded.

## 3. Session stats

`SessionStats` replaces `duration` with two durations and gains the tokens:

```
SessionStats = { turns, agent_time, run_time, tokens: TokenUsage }
```

| Field | Claude | OpenCode |
|---|---|---|
| `turns` | The number of User turns (§2) | The number of `user` rows, as today |
| `agent_time` | The sum of the Assistant turns' `TurnStats.duration` | The same sum, from the OpenCode assembler's turns |
| `run_time` | The last entry's timestamp minus the first's, as `duration` is today. Local-command entries count | `time_updated` minus `time_created`, as `duration` is today |
| `tokens` | §3.2 | §3.2 |

The row format stays `<n> turns · <duration>`, and `<duration>` is now `agent_time`.

### 3.1 Agent time

- It is the main session's time, summed exactly from the durations the sidebar shows (base §5.1, rule 6). Subagent time is not added, so that it matches the sidebar.
- A turn without a footer (a turn still running or one interrupted before its first `turn_duration`) shows no duration in the sidebar and adds nothing.
- Sum first, then format. The shared formatter truncates, so adding the sidebar's displayed values by hand can differ by up to a minute per turn from the total.

### 3.2 Tokens

`tokens` is the session's whole use: the main session and all its subagents, in the same `TokenUsage` shape as the footer (base §5.1). `output` includes `reasoning`, and `reasoning` is never added to a total.

This total deliberately differs from the sum of the session's turn footers, which cover only the main session. In the worked example, the main session has 336.0k output tokens and its subagents 5.95M.

Claude:

- Read the main transcript and every `agent-*.jsonl` found recursively below `<session file without extension>/subagents/`. Workflow runs nest theirs under `subagents/workflows/<run>/`. Other files there, such as `journal.jsonl`, are not transcripts and are not read.
- Each message counts once per file, using its latest usage, keyed by message id. A message appeared in only one file in all 20,973 messages checked, so no deduplication across files is required.
- **Do not sum `TurnStats.tokens`.** It exists only once a turn has a footer, so turns without a footer would be lost: 12 of 250 Assistant turns locally, about 2% of output tokens.
- A missing `subagents` directory means no subagents. A subagent file that cannot be read is logged and skipped, and the rest still count. Malformed lines are skipped as they are elsewhere.
- This may use a pass lighter than the `Assembler` over subagent files, as long as the result is the same.

OpenCode:

- Sum the `tokens` of the `assistant` rows of the session and of every descendant session, following `session_v2.parent_id` at any depth, with `reasoning` added to `output` as the assembler does. Only one level exists locally, but the rule is not limited to it.
- Do not read the `session_v2.tokens_*` columns: they differ from the row sums in 4 of 160 local sessions, and the rows are what the footers use.

Total time and turn count stay those of the main session on both harnesses.

### 3.3 Cache

- The cache file becomes `stats-v2.json`, with `TokenUsage` now serialised. An old file is removed, best effort, when the cache loads. It is never read, so every session's stats are computed once again, in the background.
- Without this, sessions that have not changed would keep showing their old row, because their keys still match.

The keys must also see subagents:

- Claude: add the number of subagent transcripts, their total size and their newest mtime to the key. `stats_key` runs on the coordinator thread, so it may stat files but must not read them.
- OpenCode: use the newest `time_updated` of the session and its descendants in the key.

### 3.4 Refreshing when only a subagent changed

A write to a subagent file changes no main session summary, so `worker::build` sees an unchanged snapshot and returns before `schedule_stats` (`src/index/worker.rs`). The stats would then stay stale while subagents run. Stats must be rescheduled whenever a rebuild runs, even when the snapshot is unchanged and is not sent to the UI. The same applies to an OpenCode child row. The filesystem watcher already reports these changes, because it watches the data directory recursively.

### 3.5 Performance

The largest local session has 320 subagent files of 207 MB in total. The `Assembler` parsed all of them in about 0.3 s with the files in the OS cache, so the first computation stays background work on the existing stats pool.

A running factory session is different: it would parse all those files again on every coalesced rebuild, for hours. So each subagent file's usage is kept in memory, keyed by (path, size, mtime), as discovery does for its scans (base §9.2). A recomputation reads only the files that changed, and entries for files that are gone are dropped. `doc/performance.md` records the figures for such a session (§6).

### 3.6 Acceptance

- A fixture with two Assistant turns of known durations and idle time between them has `agent_time` equal to their sum and `run_time` equal to the span.
- The row's duration equals the sum of the fixture's sidebar durations, on both harnesses.
- A turn without `turn_duration` adds no `agent_time`, and its tokens are counted.
- A local-commands-only fixture has the row `0 turns · 0s`.
- A Claude fixture with a subagent, a nested workflow subagent and a `journal.jsonl` counts both subagents and ignores the journal.
- A repeated message id in one file counts once, using the latest usage.
- Recomputing after one subagent file changed reads only that file and the main transcript.
- An OpenCode fixture with a child and a grandchild session counts all three.
- An unreadable subagent file leaves the other tokens counted.
- Appending to only a subagent file, or changing only a child row, changes the key and updates the stats event, with an unchanged snapshot (test at the worker level).
- `stats-v1.json` present at start is removed and not read. A round trip through `stats-v2.json` preserves every field.
- `selector_index` stays within its existing budget.

## 4. Session panels

Two panels below the Sessions pane show the selected session's totals: `Session Time` and `Session Tokens`. The session rows in this wireframe are illustrative: titles follow base §4.1.

```
╭─ Projects ─────────╮ ╭─ Sessions in chronoscope2 ───────────────────────────╮
│                    │ │                                                      │
│ > chronoscope2     │ │ > 2026-10-29 23:02  Spec build 6 turns · 17h 56m     │
│   jacob            │ │   2026-10-29 22:48  Factory.md  1 turn · 10m 50s     │
│                    │ │                                                      │
│                    │ ╰──────────────────────────────────────────────────────╯
│                    │ ╭─ Session Time ───────────────────────────────────────╮
│                    │ │ Total                                        81h 33m │
│                    │ │ Agent                                        17h 56m │
│                    │ ╰──────────────────────────────────────────────────────╯
│                    │ ╭─ Session Tokens ─────────────────────────────────────╮
│                    │ │ Input                                          15.8k │
│                    │ │ Output                                          6.3M │
│                    │ │ Thinking                                        2.2M │
│                    │ │ Cache read                                    862.3M │
│                    │ │ Cache write                                    30.7M │
╰────────────────────╯ ╰──────────────────────────────────────────────────────╯
╭─ Working Directory ─────────────────────────────────────────────────────────╮
```

### 4.1 Layout

- The panels are stacked directly below the Sessions pane, `Session Time` first, with no gaps. Each is as wide as the Sessions pane. The bottom border of `Session Tokens` is on the same row as the Projects pane's.
- `Session Time` is 4 rows high (two borders and two text rows) and `Session Tokens` is 7 rows high (two borders and five text rows), 11 rows in total, with no blank rows.
- The Sessions pane loses those rows. Projects, the Working Directory box and the help line do not move.
- **Short terminals.** The panels are shown only when the Sessions pane keeps at least 3 list rows (a pane height of 6). That needs panes of 17 rows, so a terminal of at least 33 rows. Below that, neither panel is drawn and the Sessions pane has its full height. One panel is never drawn without the other.
- The list's visible rows, paging, scrolling and mouse hit area all follow its new height. The selected row stays visible when the terminal is resized and the panels appear or disappear.

### 4.2 Content

Each panel is a rounded block in `border`, titled as below, like the Working Directory box. Rows appear in this order, with the label left-aligned and the value right-aligned, one column inside each border.

`Session Time`:

| Label | Value | Format |
|---|---|---|
| `Total` | `run_time` | The shared duration format (base §4.1) |
| `Agent` | `agent_time` | The shared duration format; the same text as the row |

`Session Tokens`:

| Label | Value | Format |
|---|---|---|
| `Input` | `tokens.input` | `format_tokens`, as in the details overlay |
| `Output` | `tokens.output` | `format_tokens` |
| `Thinking` | `tokens.reasoning` | `format_tokens` |
| `Cache read` | `tokens.cache_read` | `format_tokens` |
| `Cache write` | `tokens.cache_write` | `format_tokens` |

- Every label starts with a capital letter. The panel titles say what the values measure, so the labels do not repeat "time" or "tokens". Labels are muted and values are `foreground`.
- All rows of a panel always show together. A count no message reported is `0`.
- A row that cannot hold both keeps the value and cuts the label. Every label and value fits at the narrowest supported width (a 60-column terminal).
- `format_tokens` moves to where the panel and the renderers can both use it without one component depending on another's internals.

### 4.3 Behaviour

Both panels show the selected session's stats, and both are an empty block (border and title, no rows) when:

- no session is selected (base §4.1), for example at startup, after moving to another project, or when a search has no results;
- the selected session's stats are not ready yet, or could not be computed.

Rules:

- Selecting another session by any route (keys, mouse, wheel, search, `Ctrl-o`, switching harness) updates both panels in the same frame. They never show the previous session's values under a new selection.
- When stats arrive for the selected session, the panels update without any key press. A running session's values follow its stats. Until a newer computation finishes, the last known values stay, as they do on the row.
- Stats for another session do not change the panels.
- The panels never take focus and are not in the `Tab` order. Clicks and wheel events over them do nothing, and do not reach the session list.
- Each panel is a component of its own (`SessionTime` with `Id::SessionTime`, `SessionTokens` with `Id::SessionTokens`), mounted and unmounted with the selector, and owning only what it displays. They may share a small helper that draws a label and value row. The `App` pushes their values, as it does for the Working Directory box. Both join the `THEMED` list, so they follow a theme change.
- Drawing does no I/O or parsing: the panels show stats the worker already delivered.

### 4.4 Acceptance

- `selector_areas` tests: `Session Tokens`' bottom equals the Projects pane's bottom, `Session Time` is directly above it, their width and x equal the Sessions pane's, and the Sessions pane is 11 rows shorter. At 33 terminal rows both panels are shown with 3 list rows; at 32 rows both are absent and the Sessions pane has its full height. No areas overlap at any tested size.
- Snapshots at 160×50 and at 60×33: with stats, with no session selected and with stats pending.
- Selector tests: selecting a session shows its values in both panels; moving to another project clears both; a search with no result clears both; selecting a session with no stats yet shows no values from the one before; stats arriving for the selected session show at once; stats for another session change nothing.
- A click and a wheel event over either panel change neither the selection nor the scroll, and rows just above them remain clickable.
- The `Agent` value's text equals the selected row's duration text.
- Changing the theme recolours both panels.

## 5. Acceptance for the whole cycle

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

Snapshot changes are limited to the rows a task intends to change. A snapshot changed by accident is a failure, not something to accept. The panels (§4) change every selector snapshot at a height of 33 rows or more, and those are still reviewed one by one.

## 6. Documentation (final task)

- Fold the changes into `doc/SPEC.md`:
  - §1.3: the `SessionTime` and `SessionTokens` components.
  - §3.1: local-command detection, and the subagent files read for tokens.
  - §3.2: the descendant rows read for tokens.
  - §4.1: the layout, the panels, the row's duration (agent time, no longer defined only in code) and the turn count.
  - §5.1: the `local_command` injected entries and `SessionStats`.
  - §9.2: `stats-v2.json`, the new keys and the refresh on an unchanged snapshot.
- Add the panels to `doc/ui-sketch.md`.
- Update `doc/performance.md` with the stats figures for a session with many subagents.
- Update the README if it describes what the selector's row shows.
