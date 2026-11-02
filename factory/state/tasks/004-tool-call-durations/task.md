# Tool call durations

## Goal
Show how long each finished tool call took, folded and unfolded. `ToolCall.duration` is computed but no renderer uses it. The assembler also learns to read `toolUseResult` cheaply, so asynchronous subagents, whose result only records the launch, show no duration.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §3:

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

From SPEC.md §2.4 (the shared format, built by task 001 as `crate::duration::format_duration`):

| Duration | Resolution | Examples |
|---|---|---|
| Under 1 minute | 1 second | `0s`, `45s` |
| 1 minute up to, but not including, 1 hour | 1 second | `1m 0s`, `4m 3s`, `59m 59s` |
| 1 hour or longer | 1 minute | `1h 0m`, `3h 4m`, `2h 45m`, `26h 30m` |

- Numbers are **not zero-padded**.
- Values are truncated, not rounded, so `59m 59.9s` shows `59m 59s`.
- There is one shared formatter. Remove `format_duration` from both `src/components/session_list.rs` and `src/components/turn_block/renderers/stats.rs`, and put the shared one where neither component depends on the other.
- Only display changes: cached session stats store milliseconds, so the stats cache keeps working as is.

From SPEC.md §7:

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

From spec/SPEC.md §5.2:

Each turn is rendered as a rounded block. Its title shows the role on the left and the timestamp on the right. Assistant blocks also show `model @ effort` after the role. A **section** is the unit of selection, folding, copying and saving.

| Section | Default fold | Folded shows | Unfolded shows |
|---|---|---|---|
| User prompt | Folded | First 2 lines + `…` | Full prompt |
| Assistant text | Not foldable | n/a | Rendered markdown |
| Thinking (readable) | Folded | `Thinking` + first line, muted | Full text, muted |
| Thinking (redacted) | Not shown | n/a | n/a |
| Tool call | Folded | `> <description>` | Description, then per-tool body (§5.3) |
| Subagent call | Folded | `> <description>` + agent name | Prompt + final result + "open transcript" hint (§5.4) |
| Turn stats footer | Not foldable | One muted line: duration · messages · tokens/cost (if available) | n/a |
| Compaction marker | Folded | Divider `── context compacted ──` | Summary text |
| System / injected | Folded | Muted one-liner: kind + first line | Full content, muted |

Redacted thinking (no readable text) is kept in the model (§5.1) but is not a section. It is not drawn, selected, searched, copied or exported, because it has nothing to show. A turn whose only content is redacted thinking is drawn as an empty block.

The folded/unfolded marker prefix is `▶` when folded and `▼` when unfolded (the sketch uses `>` and `-`).

Every section's text starts at the same column, two cells in from the block's padding:
- Foldable sections put their marker (`▶`/`▼` and a space) in those two cells.
- All other non-foldable lines (assistant text, `model → …` notes, the turn stats footer) are indented by two spaces.

From spec/SPEC.md §6.2:

- `/` opens a query in the help line. Matching is smart-case substring (case-insensitive unless the query contains an uppercase letter).
- Searches **all** text, including folded sections, tool input/output and system content.
- `Enter` confirms and jumps to the first match; `n` / `N` go to the next / previous match. All matches are highlighted with `search_match`, and the help line shows `3/17`.
- Jumping to a match inside a folded section unfolds it temporarily. It refolds when the selection moves to a different section, unless the user unfolded it explicitly in the meantime.
- `ESC` clears the search and highlights.

From spec/SPEC.md §6.4 (copy, save, export):

| Key | Action |
|---|---|
| `y` | Copy the selected section **as displayed** (folded: summary line; unfolded: everything shown) as plain text |
| `Y` | Copy the selected section's **full content** (description + full input/command + full result), regardless of fold state or truncation |
| `w` / `Ctrl-s` | Save the selected section as displayed to a file |
| `W` | Save the selected section's full content to a file |
| `e` / `Ctrl-e` | Export the session to markdown **as currently folded** (folded sections become their summary lines) |
| `E` | Export the **full** session to markdown (tool calls as `<details>` blocks with fenced input/output) |

- Save and export prompt for a path in the help line. Defaults:
  - export: `./<project>-<yyyymmdd-hhmm>-<short-id>.md`
  - save: `./<project>-<short-id>-<section-kind>.txt`
- Saving or exporting asks for confirmation before overwriting an existing file.
- Clipboard: `arboard`, falling back to OSC52 when there's no display server or over SSH/tmux. A toast confirms the copy (`Copied 42 lines`).

From spec/SPEC.md §8.1 (theme tokens):

Two layers, modelled on ratcn's shadcn-inspired `Theme`:

1. **Base roles** (required in a theme file): `name`, `foreground`, `muted_foreground`, `background`, `surface`, `field`, `primary`, `primary_foreground`, `secondary`, `secondary_foreground`, `accent`, `destructive`, `destructive_foreground`, `warning`, `border`, `ring`, `cursor`.
2. **Component tokens** (optional overrides; each derived from a base role by default): see the table below.

| Token | Default | Used for |
|---|---|---|
| `user_title` | `primary` | `User` block title |
| `assistant_title` | `accent` | `Assistant` block title, model @ effort |
| `system_title` | `muted_foreground` | System / compaction block titles |
| `timestamp` | `muted_foreground` | Right-aligned block titles |
| `tool_description` | `foreground` | `> description` lines |
| `label` | `muted_foreground` | `Command:` / `Result:` labels |
| `command` | `foreground` | Command / tool input text |
| `output` | `muted_foreground` | Tool results |
| `error_output` | `destructive` | Failed tool results |
| `diff_added` / `diff_removed` | `primary` / `destructive` | Edit diffs |
| `thinking` | `muted_foreground` | Thinking text |
| `selection_marker` | `primary` | `▒` and `>` markers |
| `search_match` | fg `primary_foreground` on `primary` | Search hits (the only explicit background colour; the selector's focused selection uses reverse video, §8.2) |
| `help_key` / `help_text` | `primary` / `muted_foreground` | Help line and help overlay |
| `toast` | fg `foreground` on `surface` | Toast notifications |
| `syntax.*` | derived from `accent`, `primary`, `secondary`, `warning` | Code-block syntax highlighting |

Colours accept `#rrggbb`, ANSI names (`red`, `bright_blue`, …), ANSI indices (`0`–`255`) and `reset` (terminal default).

Per-theme option `transparent_background` (bool): when true, `background` is not painted and the terminal's own background shows through. Users can override it per theme in config.

From spec/SPEC.md §8.2 (emphasis):

Selection is shown with **text brightness and markers**. The one background band is the session selector's focused selection:

- Unselected list rows and transcript sections: `muted_foreground`.
- Selected transcript section and theme-selector row: `foreground`, bold, with a marker (`>` in the theme list, `▒` over the left border in the transcript) in `primary`.
- Session selector (Projects and Sessions lists):
  - Selected row in the **focused** list: the `>` marker in `primary`, never inverted. Everything after the marker is one **inverted** band (`foreground` with the reverse-video modifier, not bold), running up to the row's right margin (two cells before the pane border). Inside the band, the timestamp, live dot, title, gaps and stats all take the band's style.
  - Selected row in the **unfocused** list (e.g. Projects while Sessions has focus): text **bold** in `secondary_foreground`, not inverted; the `>` marker in the same `secondary_foreground` (not bold), so the context stays visible.
- Focused pane border: `ring`; unfocused borders: `border`.
- Selected harness tab: `primary`, underlined; other tabs: `muted_foreground`.
- In the transcript, dimming of unselected sections is controlled by `dim_unselected` (default `true`). When `false`, all sections use their normal token colours and only the marker + bold indicate selection.

From spec/SPEC.md §1.3 (components):

**Every custom UI element is its own component.** The code is organised around components, not around long render functions or a central key handler.

- A component implements tui-realm's `Component` trait (`view`, `query`/`attr`, `state`, `perform(Cmd) -> CmdResult`), plus `AppComponent` (`on(&Event) -> Option<Msg>`) if it handles events. An element that appears on several screens (e.g. the help line) is one component type, mounted with different props.
- A component draws only itself. Its `view` draws its own border and content, and leaves sub-elements to child types: a child component it owns, or a small `Widget`/`StatefulWidget` type.
- **No long render functions.** A render function (`view`, or a helper it calls) that draws more than one distinct element, or grows past about 50 lines, is split into smaller components or widgets. Rendering has no side effects.
- Components are `pub` types that can be built from plain data (props and the theme) and rendered into a `TestBackend` frame on their own, without an `Application` or a terminal (§9.6).
- All colours and modifiers come from theme tokens (§8.1), passed in as a prop. Components never hard-code colours.
- `tui-realm-stdlib` is not used. The look in §4 and §8 is specific enough that wrapping stdlib components would mean overriding their rendering. Prompt text editing uses `tui-input` inside the `Prompt` component.

From spec/SPEC.md §9.2:

Target: sessions up to **~100 MB** stay smooth.
- Parsing is streaming (line by line for JSONL, paged queries for SQLite) and runs off the UI thread. The UI shows a loading state and becomes interactive as soon as the first screen of content is parsed.
- Large tool outputs: the model stores line counts and byte ranges up front. The full text is materialised only when the section is unfolded, searched, copied or exported.
- Layout is computed lazily per block for the current width. Only blocks overlapping the viewport are rendered.
- Session list stats are computed in the background and cached in `$XDG_CACHE_HOME/chronoscope/`, keyed by (path, size, mtime) or by (db, session id, `time_updated`).
- Session discovery reads each Claude transcript at most once per index build. Each file's scan result (cwd, start time, first prompt, last `ai-title`) is kept in memory, keyed by (path, size, mtime). The initial build and live rebuilds rescan only new or changed files, scanning several files in parallel. Results for files that are gone are dropped.
- Budgets (to be measured with benchmarks on a synthetic 100 MB fixture):
  - first content within 300 ms;
  - full parse under 2 s;
  - each frame rendered in under 16 ms;
  - memory use within roughly 3× the file size.

From spec/SPEC.md §9.5:

- Never writes to harness data. The SQLite database is opened read-only.
- Unknown entry types, unknown content blocks and malformed lines are skipped and counted, not fatal.
  - A muted `N entries could not be parsed` note appears in the browser header.
  - Session metadata entries (known ones such as `agent-name`, `queue-operation`, `continued-in`, `bridge-session`, `tools-used`, and unknown types without a `message`) carry no conversation content: they are skipped and logged, but not counted in the note.
  - Details are written to a log file at `$XDG_STATE_HOME/chronoscope/chronoscope.log`.
- The schema version is checked on open (OpenCode `migration` table / Claude `version` field). An unknown newer version logs a warning, and Chronoscope still tries best-effort parsing.
- Terminal state is always restored on exit, panic or signal (panic hook + raw-mode guard).

From spec/SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):

- Under a minute, tool durations are truncated like the shared format: 999.9 ms shows `999ms`, 59.99 s shows `59.9s`. The boundaries are `999ms`, `1.0s`, `59.9s` and `1m 0s`. The tool formatter delegates to `crate::duration::format_duration` from 60 seconds up, so it holds no second copy of that format.
- A call with status `Error` that has a duration is finished, and shows it.
- The folded row extends the logic in `ToolCallSection::description` (`src/components/turn_block/renderers/tool/mod.rs`): the kept suffix is the agent type (if any) plus the duration. When even the suffix leaves no room for the description, the existing fallback applies and the whole row is cut with `…`.
- The unfolded description is word-wrapped with the duration as its muted last words. Do not let the wrapped row lose the muted style or the description's style.
- `ToolCallSection::plain_parts` builds plain text from the same rows, so the as-displayed copy, save and export follow without their own logic. `details()` in `src/components/turn_block/export.rs` uses `description_text()`, which has no duration: the summary of a full export gains the same ` · 1.2s`, so full and as-displayed exports agree. `full_text` (`Y`, `W`) goes through the rendered rows and shows it too.
- **Search trap to verify.** `SectionView::place` in `src/components/transcript/search.rs` finds a source line through a drawn row that *ends where the line ends*, and `tool_text` in `src/transcript_search.rs` gives the bare description. A ` · 1.2s` suffix breaks that match on the description row. Make the §3.1 search test exercise this path, and fix it at the source: `tool_text` includes the duration as drawn, so the text shown is also the text searched (§6.2 searches all text). Check that a query for `1.2s` finds and highlights the duration.
- **`toolUseResult`.** Add it to `wire::Entry` as an untagged object-or-`IgnoredAny` head, as the spec says, reading only `isAsync`. A non-boolean `isAsync` falls to the `IgnoredAny` arm or counts as absent, and must not fail the line. `attach_result` leaves `duration` empty for `isAsync: true`. `upsert` already carries `duration` across a repeated `tool_use`.
- **Budgets must cover the new path.** Serde's `untagged` buffers the value while it tries the variants, and the synthetic generator (`src/bench_support/generate.rs`) writes no `toolUseResult` at all, so today's budget run would not measure it. Add `toolUseResult` to the generated tool-result lines the way Claude Code writes it, as an object repeating the output (and sometimes a string), and bump `GENERATOR_VERSION` so that cached fixtures are regenerated. Report the budget figures in the build notes. `docs/performance.md` is updated by the last task of this cycle.
- Existing fixtures keep `duration: None` where a test is not about durations, so unrelated snapshots do not move: add new fixtures with `duration: Some(..)` for this task.

## Acceptance criteria
- [ ] Fixtures set `duration: Some(..)` (none do today), with snapshots folded and unfolded, including a subagent suffix and a row too narrow for the whole description.
- [ ] Format tests cover the boundaries at 1 second and 60 seconds.
- [ ] Assembler tests cover: `isAsync: true` gives no duration; a string `toolUseResult` still parses the line and its result.
- [ ] A search match in a tool description highlights the correct cells when a duration is shown.
- [ ] Render tests show that a pending call and a call without a known duration show no duration and no separator.
- [ ] A test shows a finished call with status `Error` shows its duration.
- [ ] An assembler test shows that a `toolUseResult` object with `isAsync` of a wrong JSON type still parses the line and its result, and that the call gets a duration.
- [ ] `toolUseResult` is not read as a `serde_json::Value` by the assembler's `Entry`.
- [ ] Tests show the duration in: the as-displayed copy of a folded and an unfolded call, the full copy, the saved file, the as-folded export and the full export's summary.
- [ ] A test shows a search for the duration's text (for example `1.2s`) finds it and highlights the duration's cells.
- [ ] The narrow-row snapshot shows the description cut with `…` and the duration intact.
- [ ] Generated fixtures contain `toolUseResult`, `GENERATOR_VERSION` is bumped, and `cargo test --release -- --ignored budgets --nocapture` stays within budget on them.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] Snapshot changes are limited to tool rows of calls that have a duration. Existing snapshots of calls built with `duration: None` do not change.

## UI
A finished tool call's description row gains a muted duration (`muted_foreground`). The description keeps `tool_description`, and the fold marker and indentation are unchanged.

```
▶ Dump full USB descriptors · 1.2s
▶ Explore the repository (story-writer) · 5m 2s
▶ A description much too long for this narrow r… · 340ms
▼ Dump full USB descriptors · 1.2s
```

- Folded: one row. The description is cut with `…` first; the agent-type suffix and the duration are kept.
- Unfolded: the description row or rows are word-wrapped, with the duration after the description and after the agent type. The body below is unchanged.
- Pending calls, calls without a known duration and asynchronous subagents show the row as it is today.
- Selection and dimming follow the existing rules: the selected section is emphasised with the `▒` marker, and the duration stays muted.

Snapshots: folded and unfolded calls at block widths 80 and 120, a subagent call at both, and one folded row at a width narrow enough to cut the description (for example 40).

## Depends on
- 001-shared-duration-format
