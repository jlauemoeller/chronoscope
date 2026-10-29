# Per-tool body renderers, subagent sections and output truncation

## Goal
Replace the generic unfolded tool-call body from task 017 with one body renderer per `ToolKind` (§5.3):
- Shell: command and result, char-wrapped;
- Read: path, line range and numbered content;
- Write: syntax-highlighted content;
- Edit: a coloured unified diff built with `similar`;
- Grep/Glob, WebFetch/WebSearch, and Other (pretty JSON).

Also render Subagent sections: the folded line names the agent, and the unfolded body shows the prompt, the final result and an "open transcript" hint.

Long results are truncated at `truncate_output_lines` with a `… 1 234 more lines` row. That row is a selection stop, and `Enter` on it expands the rest. Lazy outputs (task 006) are materialised only when a body is rendered unfolded.

## Spec excerpts
From SPEC.md §5.3:

| ToolKind | Unfolded body |
|---|---|
| Shell | `Command:` (char-wrapped) + `Result:` (char-wrapped, error output in `error_output`) |
| Read | Path + line range, then file content with line numbers |
| Write | Path, then content (syntax-highlighted by extension) |
| Edit | Path, then a unified diff of old → new (`similar`), with added/removed lines coloured |
| Grep / Glob | Pattern (+ path / filters), then matches |
| WebFetch / WebSearch | URL or query, then result |
| Other (incl. MCP) | Pretty-printed input JSON + result text |

Results longer than N lines (default 200, configurable) are truncated, with a `… 1 234 more lines` row; pressing `Enter` on that row expands the rest.

From SPEC.md §5.2:

| Section | Default fold | Folded shows | Unfolded shows |
|---|---|---|---|
| Tool call | Folded | `> <description>` | Description, then per-tool body (§5.3) |
| Subagent call | Folded | `> <description>` + agent name | Prompt + final result + "open transcript" hint (§5.4) |

Every section's text starts at the same column, two cells in from the block's padding:
- Foldable sections put their marker (`▶`/`▼` and a space) in those two cells.
- All other non-foldable lines (assistant text, `model → …` notes, the turn stats footer) are indented by two spaces.

From SPEC.md §5.4:

- Subagent sections show the prompt and the final result inline.
- A key (§6) opens the subagent's own transcript as a full session view: Claude `subagents/agent-<id>.jsonl`, OpenCode child `session_v2` via `parent_id`.

From SPEC.md §5.5:

- Prose is word-wrapped. Commands and tool output are char-wrapped (as in the sketch) so paths and long tokens are never hidden.

From SPEC.md §8.1:

| Token | Default | Used for |
|---|---|---|
| `tool_description` | `foreground` | `> description` lines |
| `label` | `muted_foreground` | `Command:` / `Result:` labels |
| `command` | `foreground` | Command / tool input text |
| `output` | `muted_foreground` | Tool results |
| `error_output` | `destructive` | Failed tool results |
| `diff_added` / `diff_removed` | `primary` / `destructive` | Edit diffs |

From SPEC.md §8.5:

```toml
truncate_output_lines = 200
```

From SPEC.md §9.2:

- Large tool outputs: the model stores line counts and byte ranges up front. The full text is materialised only when the section is unfolded, searched, copied or exported.

From SPEC.md §1.3:

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (§9.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (§5.2), one body renderer per `ToolKind` (§5.3), the markdown renderer (§5.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (§5.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

From ui-sketch.md (unfolded Shell tool call):

```text
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
```

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Layout.** Use the task-017 geometry. The `▼ <description>` row comes first, then a blank row, then label/content pairs. Each label (in `label`) sits at column 4 and is followed by a blank row. Content (column 7) follows, and a blank row separates pairs. Input/command text uses `command`. Results use `output`, or `error_output` when the call failed. Every body is char-wrapped (`char_wrap` from task 016) unless stated otherwise.
- **Per kind.**

  | Kind | Body |
  |---|---|
  | Shell | `Command:` with the full command. `Result:` with the output. |
  | Read | `Path:` with `<path>` plus ` (lines a–b)` when the input has an offset/limit. `Content:` with numbered lines in a right-aligned gutter: the number in `muted_foreground`, then two spaces, then the text in `output`. If the result already carries line-number prefixes (Claude's `     1→text` or `1\ttext`), parse them into the gutter instead of numbering twice. |
  | Write | `Path:` with the path. `Content:` with the file content highlighted by the path's extension (`highlight_code` from task 016), char-wrapped. |
  | Edit | `Path:` with the path. `Diff:` with a unified diff (`similar`, 3 context lines) of `old_string` → `new_string` (for MultiEdit, each edit in turn): `@@` hunk headers in `muted_foreground`, `+` lines in `diff_added`, `-` lines in `diff_removed`, context lines in `output`. |
  | Grep | `Pattern:` with the pattern, plus ` in <path>` and any glob/type filters. `Matches:` with the result text. |
  | Glob | `Pattern:` with the pattern, plus ` in <path>`. `Matches:` with the result text. |
  | WebFetch | `URL:` with the URL (and the prompt, if any, on the next line). `Result:` with the result text. |
  | WebSearch | `Query:` with the query. `Result:` with the result text. |
  | Other (incl. MCP) | `Input:` with pretty-printed input JSON (2-space indent). `Result:` with the result text. |

  A pending call shows `(pending)` in `muted_foreground` as its result.
- **Subagent.**
  - Folded: `▶ <description> (<agent type>)`. The agent type comes from the input's `subagent_type` (Claude) or `agent` (OpenCode), and is omitted when unknown.
  - Unfolded:
    - `Prompt:` with the prompt rendered as markdown;
    - `Result:` with the final result as markdown, or `(pending)`;
    - a final row `<key> to open transcript` in `muted_foreground`, where `<key>` is `Keymap::label(browser, open_subagent)` (default `a`).
- **Truncation.**
  - When a body's result has more than `truncate_output_lines` lines, only the first N lines are shown, followed by a row `… <remaining> more lines` in `muted_foreground`. Count logical lines before wrapping, and group thousands with a space (`… 1 234 more lines`).
  - The truncation row is its own selection stop, directly after its tool section. `Cmd::Move` can land on it, and it then gets the `▒` marker on that row only.
  - `Cmd::Submit` on it expands the full output, removes the row, and keeps the selection on the tool section. Expansion state is stored per `SectionId`.
  - Folding and unfolding the tool call keeps its expansion state.
  - Copying or saving while the row is selected acts on the parent tool section (later tasks).
- **Lazy materialisation.** Bodies call `ToolOutput::text()` only when rendered unfolded. Folded sections never materialise. A materialisation error renders `(output unavailable: <error>)` in `error_output` and is logged.
- The renderer is chosen by `ToolKind`, one type per kind (e.g. a `ToolBodyRenderer` trait or enum dispatch), each with its own unit tests.

## Acceptance criteria
- [ ] Each `ToolKind` renders the body from the planner-notes table. There is one test per kind asserting its labels, columns (labels at 4, content at 7) and tokens.
- [ ] Shell command and result are char-wrapped: a 200-character path breaks at the body width, as in the sketch (test).
- [ ] Read with Claude-style numbered output shows each number once in the gutter. Plain output is numbered from the input offset (tests).
- [ ] Write content is highlighted by extension (`.rs` gives `syntax.keyword` on `fn`) (styled test).
- [ ] An Edit diff shows `+` lines in `diff_added` and `-` lines in `diff_removed`, with `@@` headers (styled test).
- [ ] A subagent section folds to `▶ <description> (<agent type>)` and unfolds to prompt, result and the `a to open transcript` hint. The hint follows a remapped `open_subagent` key (tests).
- [ ] A result of 1 434 lines with `truncate_output_lines = 200` shows 200 lines plus `… 1 234 more lines`. Selecting the row (`Cmd::Move`) and submitting (`Cmd::Submit`) shows all lines, and the state survives fold/unfold (component tests).
- [ ] A folded tool call never calls `ToolOutput::text()`. Unfolding calls it (test with an instrumented or lazy fixture output).
- [ ] Layout-cache heights still match rendering for all new bodies (the task-018 proptest covers the new kinds).
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
Changes to unfolded tool-call sections inside `TurnBlock`/`Transcript`, laid out as in the sketch excerpt: `▼ description`, blank, a label at column 4, blank, content at column 7, blank, the next label… The `▒` marker uses SPEC.md (§5.2) rather than the sketch's `┃`, and covers the whole section.

Snapshots (component, `TurnBlock` at width 100 unless noted):
1. Shell unfolded with a long char-wrapped command and multi-line result, matching the sketch structure (text + styled).
2. Read unfolded with numbered content (text).
3. Write unfolded with `.rs` content (styled).
4. Edit unfolded with a diff (text + styled).
5. Grep, Glob, WebFetch and WebSearch unfolded (text, one snapshot each).
6. Other (an `mcp__` tool) unfolded with pretty JSON (text).
7. Subagent folded and unfolded (text).
8. A truncated result showing `… 1 234 more lines` with that row selected (`▒` on that row only) at 80x24 in a `Transcript` (text + styled), and after expansion (text).
9. A failed Shell call (result in `error_output`) (styled).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 018-transcript-viewport
