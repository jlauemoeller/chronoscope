# Turn blocks and section renderers

## Goal
Render one conversation turn as a rounded block of sections (§5.2). This task delivers:
- **`TurnBlock` widget.** A rounded border, with role, model and effort on the left of the title and the timestamp on the right. It draws the `▒` selection marker over the left border for the rows of the selected section.
- **Section model.** It derives sections from a turn, with their kinds and default fold states.
- **Section renderers.** One per kind: user prompt, assistant text, thinking, tool call (folded line plus a generic unfolded body), model-change note, turn stats footer, compaction, and system/injected.
- **Layout rules.** Padding, blank-line separation and the common text column.
- **Emphasis rules.** Selected versus unselected sections, including `dim_unselected`.

The `Transcript` component in task 018 stacks these blocks into a scrolling, virtualised view. Task 019 adds the per-tool bodies.

## Spec excerpts
From SPEC.md §5.2:

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

From SPEC.md §5.1:

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

5. The model shown in the Assistant title is that of the turn's first message. If the model changes within a turn, the new model is shown as a muted inline note.

From SPEC.md §5.5:

- Assistant text, user prompts and thinking are rendered as markdown (`pulldown-cmark`): headings, emphasis, lists, inline code, block quotes, tables, and fenced code blocks with syntax highlighting (`syntect`, mapped to theme colours).
- Prose is word-wrapped. Commands and tool output are char-wrapped (as in the sketch) so paths and long tokens are never hidden.
- Wrapping is recomputed when the terminal is resized.

From SPEC.md §5.6:

- System/injected content is shown by default (muted, folded). A toggle key hides or shows all of it; the default is configurable (`show_system = true`).
- It is rendered as **System sections inside the adjacent turn**: in the next turn when it precedes a prompt, in the current turn otherwise. Consecutive injected entries collapse into one section with a count (`3 injected items`). They never become turn blocks of their own, so `[` / `]` only stop at User, Assistant and Compaction blocks.

From SPEC.md §4.2:

- Transcript: one rounded block per turn (§5.2), scrolling vertically, with the selected section marked by `▒` over the block's left border.

From ui-sketch.md (session browser; see the planner notes for where SPEC.md overrides it):

```text
                                                                          Session Browser                                    ┃
                                                                                                                             ▼
                                                                         ╭─ Project BRIO ───────────────────────────────────────────────────── 85a86657─11a7─4318─a21a─0f4476babd5c ─╮  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  Session ID
                                                                         │                                                                                                           │
              Working directory of session  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶  │ /home/jacob/Development/projects/BRIO                                                                     │
                                                                         │                                                                                                           │
                                                                         ╰───────────────────────────────────────────────────────────────────────────────────────────────────────────╯
                                 User turn  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶  ╭─ User ───────────────────────────────────────────────────────────────────────── 2026─10─21 08:35:15.800Z ─╮  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  Timestamp for entry
                                                                         │                                                                                                           │
                                                                         │ - I have a Logitech BRIO web camera attached to this machine via USB. It works fine - both                │
                                                                         │   camera and microphone - but I don't have a way to access settings that would have been                  │
                                                                         │   available on say a mac or windows, since there are no proprietary drivers: we just get                  │  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  User input is always one block. Can be folded
                                                                         │   the vanilla USB profiles. Can you help me investigate what additional profiles the                      │                               (default) in which case only the first two lines
                                                                         │   camera has and how to access them? The ideal solution would be to be able to write an                   │                               are displayed with an ellipsis at the end
                                                                         │   app that allowed me to change the settings, if possible. However, before we get to that                 │
                                                                         │                                                                                                           │
                                                                         ╰───────────────────────────────────────────────────────────────────────────────────────────────────────────╯
         Agent turn shows model and effort  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶  ╭─ Assistant ── claude─opus─5.5 @ high  ───────────────────────────────────────── 2026─10─21 08:35:18.271Z ─╮
                                                                         │                                                                                                           │
                                                                         │ I'll start by looking at what the camera exposes over USB and what Linux's UVC driver makes of it.        │  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  Thinking. Not foldable
                                                                         │                                                                                                           │
   Command section. Foldable (default). If  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶  │ > Find the BRIO on USB and check available tools                                                          │
   folded, only the command description is                               │                                                                                                           │
                 shown as in this example.                               │ - Dump full USB descriptors for the BRIO                                                                  │  ◀━━━━━━━━━━━━━━━━━━━━━━━━━━  Example of unfolded command section. Description
                                                                         │                                                                                                           │                               is followed by a Command section showing the
                                                                         │   Command:                                                                                                │                               entire command line invoked, and a Result section
                                                                         │                                                                                                           │                               showing the entire command output.
Currently selected section is highlighted  ━━━━━━━━━━━━━━━━━━━━━━━━━━▶   ┃      sudo -n true 2>&1 && echo SUDO_OK; lsusb -v -d 046d:085e >                                           │
                                                                         ┃      /tmp/claude-1000/-home-jacob-Development-projects-brio/85a86657-11a7-4318-a21a-0f4476babd5c/scratc   │
                                                                         ┃      hpad/lsusb.txt 2>&1; wc -l                                                                           │
                                                                         ┃      /tmp/claude-1000/-home-jacob-Development-projects-brio/85a86657-11a7-4318-a21a-0f4476babd5c/scratc   │
                                                                         ┃      hpad/lsusb.txt; grep -E                                                                              │
                                                                         ┃      'bInterfaceClass|bInterfaceSubClass|bInterfaceNumber|iInterface|bcdUVC|bcdDevice|iProduct|iSerial|   │
                                                                         ┃      guidExtensionCode|bUnitID|bTerminalID|wTerminalType|bNumControls|bmControls|bControlSize|Unit        │
                                                                         ┃      Descriptor|VideoControl Interface Descriptor|bDescriptorSubtype|Couldn'                              │
                                                                         ┃      /tmp/claude-1000/-home-jacob-Development-projects-brio/85a86657-11a7-4318-a21a-0f4476babd5c/scratc   │
                                                                         ┃      hpad/lsusb.txt | head -200                                                                           │
                                                                         │                                                                                                           │
                                                                         │   Result:                                                                                                 │
                                                                         │                                                                                                           │
                                                                         │      sudo: a password is required                                                                         │
                                                                         │      1799                                                                                                 │
                                                                         │      /tmp/claude-1000/-home-jacob-Development-projects-brio/85a86657-11a7-4318-a21a-0f4476babd5c/scratc   │
                                                                         │      hpad/lsusb.txt                                                                                       │
                                                                         │      Couldn't open device, some information will be missing                                               │
                                                                         │        bcdDevice            0.17                                                                          │
                                                                         │        iProduct                2 Logitech BRIO                                                            │
                                                                         │        iSerial                 3 8DF1E759                                                                 │
                                                                         │            bInterfaceNumber        0                                                                      │
                                                                         │            bInterfaceClass        14 Video                                                                │
                                                                         │            bInterfaceSubClass      1 Video Control                                                        │
                                                                         │            iInterface              0                                                                      │
                                                                         │            VideoControl Interface Descriptor:                                                             │
                                                                         │                                                                                                           │
                                                                         ╰───────────────────────────────────────────────────────────────────────────────────────────────────────────╯
```

From SPEC.md §8.2:

Selection is shown with **text brightness and markers**. The one background band is the session selector's focused selection:

- Unselected list rows and transcript sections: `muted_foreground`.
- Selected transcript section and theme-selector row: `foreground`, bold, with a marker (`>` in the theme list, `▒` over the left border in the transcript) in `primary`.
- Session selector (Projects and Sessions lists):
  - Selected row in the **focused** list: the `>` marker in `primary`, never inverted. Everything after the marker is one **inverted** band (`foreground` with the reverse-video modifier, not bold), running up to the row's right margin (two cells before the pane border). Inside the band, the timestamp, live dot, title, gaps and stats all take the band's style.
  - Selected row in the **unfocused** list (e.g. Projects while Sessions has focus): text **bold** in `secondary_foreground`, not inverted; the `>` marker in the same `secondary_foreground` (not bold), so the context stays visible.
- Focused pane border: `ring`; unfocused borders: `border`.
- Selected harness tab: `primary`, underlined; other tabs: `muted_foreground`.
- In the transcript, dimming of unselected sections is controlled by `dim_unselected` (default `true`). When `false`, all sections use their normal token colours and only the marker + bold indicate selection.

From SPEC.md §8.1:

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

From SPEC.md §1.3:

**Every custom UI element is its own component.** The code is organised around components, not around long render functions or a central key handler.

- A component implements tui-realm's `Component` trait (`view`, `query`/`attr`, `state`, `perform(Cmd) -> CmdResult`), plus `AppComponent` (`on(&Event) -> Option<Msg>`) if it handles events. An element that appears on several screens (e.g. the help line) is one component type, mounted with different props.
- A component draws only itself. Its `view` draws its own border and content, and leaves sub-elements to child types: a child component it owns, or a small `Widget`/`StatefulWidget` type.
- **No long render functions.** A render function (`view`, or a helper it calls) that draws more than one distinct element, or grows past about 50 lines, is split into smaller components or widgets. Rendering has no side effects.
- Components are `pub` types that can be built from plain data (props and the theme) and rendered into a `TestBackend` frame on their own, without an `Application` or a terminal (§9.6).
- All colours and modifiers come from theme tokens (§8.1), passed in as a prop. Components never hard-code colours.
- `tui-realm-stdlib` is not used. The look in §4 and §8 is specific enough that wrapping stdlib components would mean overriding their rendering. Prompt text editing uses `tui-input` inside the `Prompt` component.

Components (each is its own type and module):

| Component | Where | Draws / handles | Takes focus |
|---|---|---|---|
| `SelectorFrame` | Selector | Overlay border, sizing (§4.1) and background; nothing inside | No |
| `HarnessTabs` | Selector | Harness tabs; click switches tab (§6.6) | No |
| `ProjectList` | Selector | Projects pane, selection rules of §4.1 | Yes |
| `SessionList` | Selector | Sessions pane, including search results with the project prefix and the `●` marker | Yes |
| `WorkingDir` | Selector | Working Directory box | No |
| `BrowserHeader` | Browser | Project and session titles, cwd, `● live`, breadcrumb, parse-warning note | No |
| `Transcript` | Browser | The turn blocks: selection, folding, navigation, search highlights (§5, §6.1–§6.3) | Yes |
| `HelpLine` | Both | Key hints and status items (`3/17`, `↓ N new`, `Loading… 34%`) | No |
| `Prompt` | Both | Search and path prompts and `y/n` confirmations, drawn in place of the help line while active | Yes (input mode, §6) |
| `Toast` | Both | Bottom-right toast; expires on `Tick` | No |
| `HelpOverlay` | Overlay | §4.3 | Yes |
| `ThemeSelector` | Overlay | §8.4 | Yes |
| `GlobalEvents` | Both | Nothing drawn; turns global keys and worker events into `Msg`s (§1.4) | No |
| `EmptyState` | Both | "Terminal too small", "No sessions for this harness", "Session file is empty" | No |

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (§9.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (§5.2), one body renderer per `ToolKind` (§5.3), the markdown renderer (§5.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (§5.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

From SPEC.md §8.6:

- Displayed in local time using `timestamp_format`. With `utc = true`, UTC is shown with a `Z` suffix.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **SPEC.md overrides the sketch.**
  - Fold markers are `▶` (folded) and `▼` (unfolded), not `>` and `-`.
  - The selection marker is `▒` in `selection_marker`, not `┃`. It covers every row of the selected section: for an unfolded tool call, from its `▼ description` row through the last body row. It does not cover only the command lines, as the sketch shows.
  - Assistant text starts at the common text column (column 4), not column 2.
  - The sketch's annotation "Thinking. Not foldable" points at assistant text. Assistant text is not foldable; readable thinking is foldable.
  - Timestamps follow §8.6 (local time with `timestamp_format`, `Z` only with `utc = true`), not the sketch's `08:35:15.800Z`. The `─` inside the sketch's dates and model names is a drawing artefact; render real `-`.
- **Block geometry** (block width `W`, columns from the block's left edge):
  - **Border.** Rounded border in `border`.
  - **Top border.** `╭─ <left title> ─…─ <right title> ─╮`.
    - The left title starts at column 2 with one space of padding: ` User `, ` Assistant `, ` Compaction `.
    - Assistant blocks continue the title as ` Assistant ── <model> @ <effort> `, with ` @ <effort>` omitted when there is no effort.
    - The right title is ` <timestamp> ` followed by `─╮`.
    - Title colours: `User` in `user_title`, `Assistant` and the model text in `assistant_title`, `Compaction` in `system_title`, timestamp in `timestamp`, the `──` joiner in `border`.
  - **Rows.**
    - Row 1 is blank.
    - Sections follow, with one blank row between consecutive sections.
    - One blank row precedes the bottom border.
    - A turn with no visible sections (e.g. only redacted thinking) is top border + one blank row + bottom border.
  - **Columns.**
    - Section text starts at column 4. Foldable sections put `▶ `/`▼ ` in columns 2–3. Non-foldable lines (assistant text, model notes, stats footer) leave columns 2–3 blank.
    - Text wraps at column `W − 5`, i.e. a text width of `W − 8`, matching the sketch's 3-cell right padding.
  - **Body labels and content.** Labels (`Input:`, `Result:`, `Command:` …) sit at column 4 and their content at column 7 (body width `W − 11`). Each label is followed by a blank row, then its content. The description row is followed by a blank row before the first label.
- **Sections per item** (default fold in brackets):
  - **User prompt** [folded].
    - Folded: `▶ ` + the first 2 wrapped lines of the markdown-rendered prompt. When there is more, the second line ends with `…`, truncating it if needed.
    - Unfolded: `▼ ` + all lines.
    - Text in `foreground`.
  - **Assistant text** [not foldable]. Rendered markdown in `foreground`.
  - **Thinking (readable)** [folded].
    - Folded: `▶ Thinking: <first line>`, truncated to one row with `…`.
    - Unfolded: `▼ Thinking`, then the full markdown text on the following rows.
    - All in `thinking`.
  - **Thinking (redacted)**: no section.
  - **Tool call** [folded].
    - Folded: `▶ <description>` in `tool_description`.
    - Unfolded, generic body used for every kind until task 019:
      - `▼ <description>`, blank;
      - `Input:` (in `label`), blank, pretty-printed input JSON char-wrapped in `command`;
      - blank, `Result:` (in `label`), blank, result text char-wrapped in `output` (`error_output` when the status is Error), or `(pending)` in `muted_foreground` while pending.
  - **Model change** [not foldable]. `model → <model>` in `muted_foreground`.
  - **Turn stats footer** [not foldable]. `<duration> · <n> messages · <tokens> tokens` (` · $<cost>` when known; omit unknown parts) in `muted_foreground`.
    - Duration: `12s`, `5m 02s`, `1h 05m`.
    - Tokens: `950`, `45.2k`, `1.2M`.
    - Cost: two decimals.
  - **Compaction** [folded]. Its own block titled `Compaction`.
    - Folded: `▶ ── context compacted ──`.
    - Unfolded: `▼ ── context compacted ──`, then the summary as markdown.
    - All in `muted_foreground`.
  - **System / injected** [folded].
    - Folded: `▶ <kind>: <first line>` for one entry, or `▶ <n> injected items` for several.
    - Unfolded: the same header with `▼`, then for each entry its `<kind>` line followed by its full text.
    - All in `muted_foreground`.
    - Placed where the item sits in the turn (injected items of a User turn come before the prompt).
    - Omitted entirely when the `show_system` prop is false.
- **Emphasis.**
  - The selected section's text is bold, keeping its normal token colours. Its rows get `▒` in `selection_marker` at column 0, replacing `│`.
  - With `dim_unselected = true`, the text of every other section is drawn in `muted_foreground` (all token colours replaced, modifiers kept).
  - With `false`, unselected sections keep their token colours and are not bold.
  - Titles and borders are never dimmed.
- **API shape.** Section derivation and rendering are pure.
  - Use a `SectionId` (turn id + item id, or a fixed key for the prompt/footer) and a `FoldState` lookup with defaults from the table.
  - Provide one renderer type per section kind, each returning lines for a width.
  - `TurnBlock` composes them, reports its height for a width/fold state/`show_system`, and renders into a `Buffer` area. It must support skipping the first `k` rows and limiting the height (clipping), which task 018 needs for partially visible blocks.
  - Timestamps are formatted with the task-003 `TimestampFormatter`.

## Acceptance criteria
- [ ] Section derivation turns each `Item` into the section kinds above with the default folds of §5.2. Redacted thinking yields no section. Injected items yield no section when `show_system` is false (unit tests).
- [ ] The user prompt folded shows exactly 2 wrapped lines with `…` when longer, and no `…` when the prompt fits in 2 lines. Unfolded shows all lines (tests).
- [ ] Thinking, tool call, compaction and system sections render the folded and unfolded forms from the planner notes, with the listed tokens (styled tests).
- [ ] The stats footer formats `durationMs = 302646`, `messageCount = 112`, `tokens = 45213` as `5m 02s · 112 messages · 45.2k tokens` (test).
- [ ] A model change renders `model → <model>` at column 4 (test).
- [ ] Block geometry:
  - the titles match the planner-notes format, including ` Assistant ── <model> @ <effort> ` and the right-aligned ` <timestamp> ─╮`;
  - text starts at column 4 and wraps at `W − 5`;
  - tool-body content starts at column 7;
  - rows are 1 blank, sections separated by blank rows, 1 blank before the bottom.

  Tests at width 80 and 120.
- [ ] `TurnBlock::height` equals the number of rows drawn, for every snapshot state (asserted in tests).
- [ ] Clipped rendering (skip `k` rows, limit to `h` rows) draws exactly the corresponding rows of the full rendering (test).
- [ ] Selection and dimming:
  - the selected section has `▒` in `selection_marker` on exactly its rows and bold text;
  - with `dim_unselected = true` the other sections are `muted_foreground`; with `false` they keep token colours.

  Styled snapshot tests.
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
A new widget, `TurnBlock`, rendered on its own with its exact height. Blocks follow the planner-notes geometry: the rounded border, `╭─ User ─…─ 2026-10-21 08:35:15 ─╮`-style titles, text at column 4, fold markers at column 2, and body labels at 4 with content at 7.

Snapshots (component, fixture turns, `utc = true`):
1. User turn, prompt folded (2 lines + `…`), at width 80 and 120 (text).
2. The same User turn unfolded at width 80 (text), and selected (styled: `▒` on the prompt rows, bold).
3. User turn with a preceding injected item (folded `▶ file: …`) and the prompt, at width 80 (text).
4. Assistant turn with:
   - text with a list and inline code;
   - folded thinking;
   - a folded tool call;
   - an unfolded tool call (generic `Input:` / `Result:` body);
   - a model-change note;
   - the stats footer.

   At width 120 (text + styled), with the unfolded tool call selected and `dim_unselected = true`.
5. State 4 with `dim_unselected = false` (styled).
6. Assistant turn with an unfolded thinking section, and with an unfolded 3-entry system section (`▼ 3 injected items` + entries), at width 80 (text).
7. A tool call with status Error, unfolded (styled: result in `error_output`), and a pending tool call unfolded (`(pending)`), at width 80.
8. A Compaction block folded and unfolded at width 80 (text).
9. An Assistant turn containing only redacted thinking (empty 3-row block) at width 80 (text).
10. Assistant title without effort (`╭─ Assistant ── claude-sonnet-5.5 ─…`) at width 80 (text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 007-claude-injected-content
- 016-markdown-renderer
