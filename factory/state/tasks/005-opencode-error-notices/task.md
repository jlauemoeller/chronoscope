# Notices and OpenCode errors

## Goal
Show abnormal endings in the transcript. This task adds the `Item::Notice` inline item with its own section kind, including rendering, selection, search, copy, save and export, and produces notices from OpenCode rows. OpenCode rows with `finish: "error"` have empty content today, so the turn shows nothing about what went wrong. Claude's notices follow in the next task.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §4:

OpenCode rows with `finish: "error"` have empty content, so the turn shows nothing about what went wrong. The local database has 12 such rows: `aborted` 4, `provider.invalid-request` 6, `provider.transport` 2.

From SPEC.md §4.1:

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

From SPEC.md §4.2 (this task builds the OpenCode rows; the Claude rows are the next task's):

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

From spec/SPEC.md §3.2:

- Location: `~/.local/share/opencode/opencode.db` (SQLite, WAL mode).
- Tables: `project` (`worktree`, `name`), `session_v2` (`project_id`, `parent_id`, `directory`, `title`, `model`, tokens/cost, timestamps), `session_message` (`session_id`, `type`, `seq`, `data` JSON).
- Message types: `user`, `assistant`, `synthetic`, `system`, `idle`. Assistant `data` has `agent`, `model {id, providerID, variant}`, and a `content` array of `text` / `tool` parts.
- Subagent sessions are linked via `session_v2.parent_id`.

From spec/SPEC.md §5.1:

Harness adapters translate each native format into one shared model:

```
Session { id, harness, project, cwd, title, started_at, turns: [Turn], children: [SessionRef] }
Turn    = User      { timestamp, text, items: [Item] }     // items: injected content preceding/attached to the prompt
        | Assistant { timestamp, model, effort?, items: [Item], stats? }
        | Compaction{ timestamp, summary }
Item    = Text(markdown)
        | Injected { entries: [{ kind, timestamp, text }] }   // §5.6; consecutive entries collapsed
        | Thinking(text | Redacted)
        | ToolCall { id, tool: ToolKind, description, input, result?: ToolResult, status, duration? }
ToolKind = Shell | Read | Write | Edit | Grep | Glob | WebFetch | WebSearch | Subagent | Other(name)
```

Turn assembly (Claude):
1. A **User turn** starts at a `user` entry that is a human prompt: `origin.kind == "human"` (or, for older versions without `origin`, string/text content with no `tool_result` and no `isMeta`). A message typed while the agent is working is stored as an `attachment` of kind `queued_command` with `origin.kind == "human"`; it also starts a User turn, and the content after it forms a new Assistant turn.
2. Everything up to the next human prompt forms **one Assistant turn**: `assistant` lines, `tool_result`-only `user` lines, and allowlisted injected content (§5.6).
3. `assistant` lines with the same `message.id` are merged into one message, keeping their content blocks in order. Repeated blocks (same `tool_use.id`, or same index for text/thinking) replace the earlier version instead of being duplicated.
4. `tool_result` blocks are attached to the `ToolCall` with the matching `tool_use_id`. A tool call without a result yet has status `pending`.
5. The model shown in the Assistant title is that of the turn's first message. If the model changes within a turn, the new model is shown as a muted inline note.
6. `system` `turn_duration` entries become the stats footer of the preceding Assistant turn.

Turn assembly (OpenCode): each `user` row (with `synthetic` rows treated as injected content) starts a turn. `assistant` rows up to the next `user` row form the Assistant turn. Tool calls and their results are already combined in each `tool` part (`state.input`, `state.content`, `state.status`).

- Tool names are mapped to `ToolKind` per harness (e.g. Claude `Bash` and OpenCode `shell` → `Shell`; Claude `Agent`/`Task` and OpenCode `subagent` → `Subagent`).
- Tool description: the tool's own `description` input if present (Claude Bash). Otherwise a derived label (`Read src/main.rs`, `Grep "pattern" in path`, `WebFetch example.com/…`, `<tool name>` for unknown tools).
- `effort` is shown only when the harness records it (Claude `effort`; OpenCode `model.variant` when it isn't `default`).

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

From spec/SPEC.md §6.4:

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

From spec/SPEC.md §7.2 (browser behaviour for live updates):

- **Follow when at bottom:** if the viewport is at the end of the transcript, new content scrolls into view automatically. If the user has scrolled up, the position is kept and a `↓ N new` indicator appears in the help line; `G` / `End` jumps to the end and resumes following.
- Updates to content already displayed (e.g. a tool result arriving for a pending tool call, or a streamed message growing) update the section in place. The current selection and fold states are preserved.
- A `● live` marker is shown in the browser header while the session is active: file/rows modified within the last 60 s, or OpenCode `time_idle` is null.

From spec/SPEC.md §8.1:

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

From spec/SPEC.md §8.2:

Selection is shown with **text brightness and markers**. The one background band is the session selector's focused selection:

- Unselected list rows and transcript sections: `muted_foreground`.
- Selected transcript section and theme-selector row: `foreground`, bold, with a marker (`>` in the theme list, `▒` over the left border in the transcript) in `primary`.
- Session selector (Projects and Sessions lists):
  - Selected row in the **focused** list: the `>` marker in `primary`, never inverted. Everything after the marker is one **inverted** band (`foreground` with the reverse-video modifier, not bold), running up to the row's right margin (two cells before the pane border). Inside the band, the timestamp, live dot, title, gaps and stats all take the band's style.
  - Selected row in the **unfocused** list (e.g. Projects while Sessions has focus): text **bold** in `secondary_foreground`, not inverted; the `>` marker in the same `secondary_foreground` (not bold), so the context stays visible.
- Focused pane border: `ring`; unfocused borders: `border`.
- Selected harness tab: `primary`, underlined; other tabs: `muted_foreground`.
- In the transcript, dimming of unselected sections is controlled by `dim_unselected` (default `true`). When `false`, all sections use their normal token colours and only the marker + bold indicate selection.

From spec/SPEC.md §1.3:

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

- Model: `Item::Notice { id: ItemId, level: NoticeLevel, text: String }` with `NoticeLevel::{Error, Warning}`, next to `Item::ModelChange` in `src/model/mod.rs`.
- Sections: add `SectionContent::Notice` and `SectionKind::Notice`, not foldable. Every exhaustive match needs a branch: `src/components/turn_block/section.rs` (`derive_sections`, `foldable`, `kind`, `file_kind`), `renderers/mod.rs`, `plain.rs`, `export.rs`, `src/transcript_search.rs` (`section_text`) and `src/components/transcript/search.rs` (`section_source`, not markdown).
- Rendering: a `NoticeSection` renderer next to `ModelChangeSection`, word-wrapped with `with_lead` and two spaces. The text is not markdown.
- Export treats a notice like the `model → …` note (`_text_`).
- An `Error` notice with no parts: when `finish` is `"error"` and neither `error.type` nor `error.message` exists, the text is `error`. The spec's "whichever parts exist" leaves nothing to show otherwise.
- `error` and `finish` must not fail the row. `AssistantData` reads them tolerantly: a non-object `error` with a string value counts as its message, anything else counts as missing, and a non-string or empty `finish` counts as absent (normal).
- The notice goes through the assembler's `upsert`, positioned after the row's own parts, with id `notice:<row id>`. A row updated in place keeps the notice's position and replaces its text.
- A row updated from an abnormal ending to a normal one loses its notice ("A normal ending never does"). Removing an item shifts positions, so rebuild `Builder.index` afterwards.
- The row still counts in the turn's stats exactly as before.
- Add notices to the layout-cache property tests (`src/components/transcript/tests/property.rs`).

## Acceptance criteria
- [ ] Assembler tests cover each OpenCode row of §4.2, an OpenCode row updated in place producing one notice, and an OpenCode row going from streaming to error.
- [ ] Assembler tests pin the text: `aborted`; `aborted: <message>`; `<type>: <message>`; type only; message only; `stopped: <finish>`.
- [ ] Assembler tests show that `finish` of `stop` or `tool-calls`, and an absent `finish`, produce no notice.
- [ ] An assembler test shows a row going from an abnormal ending to a normal one loses its notice, and that later in-place updates to the turn's other items still land on the right items.
- [ ] An assembler test shows an `error` of an unexpected JSON type leaves the row parsed and not counted in `unparsed_count`.
- [ ] Snapshots cover both levels at 80 and 120 columns, including a long message that wraps.
- [ ] Tests cover selecting, copying, saving, exporting and searching a notice.
- [ ] A saved notice's default file name uses the word `error` for `Error` and `notice` for `Warning`.
- [ ] A notice is not foldable: the fold keys do nothing on it, and it has no fold marker.
- [ ] The layout-cache property tests include notices.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] `cargo test --release -- --ignored budgets --nocapture` stays within budget.
- [ ] Snapshot changes are limited to new notice snapshots. No existing snapshot changes unless its OpenCode fixture has an abnormal ending.

## UI
A notice is one section of the turn's block, in the turn's order. It has no marker and is indented by two spaces like the `model → …` note, with its text word-wrapped to the block's text width.

Illustration (not from the spec):

```
│                                                                   │
│   provider.invalid-request: messages: text content blocks must be │
│   non-empty                                                       │
│                                                                   │
│   aborted                                                         │
│                                                                   │
│   stopped: length                                                 │
```

- `Error`: `error_output` (default `destructive`).
- `Warning`: `muted_foreground`.
- Emphasis is the existing rule (`emphasize`, spec/SPEC.md §8.2), with no exception for notices: selected, the level's colour in bold with the `▒` marker over the block's left border; unselected with `dim_unselected` on (the default), `muted_foreground` like every other section; unselected with it off, the level's colour.
- Search highlights use `search_match`.

Snapshots: both levels, short and wrapping, selected, unselected with `dim_unselected` on, and unselected with it off, at block widths 80 and 120.

## Depends on
none
