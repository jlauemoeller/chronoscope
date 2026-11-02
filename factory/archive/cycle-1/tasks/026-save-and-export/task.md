# Save sections and export sessions to markdown

## Goal
Implement saving and exporting from the browser (§6.4).
- **Saving.** `w`/`Ctrl-s` save the selected section as displayed, and `W` saves its full content, to a text file.
- **Exporting.** `e`/`Ctrl-e` export the whole session to markdown as currently folded (folded sections become their summary lines). `E` exports the full session, with tool calls as `<details>` blocks with fenced input and output.
- **Prompts.** Both ask for a path in the help line with a pre-filled default, and ask a `y/n` confirmation before overwriting an existing file. A toast reports success or failure.

## Spec excerpts
From SPEC.md §6.4:

| Key | Action |
|---|---|
| `w` / `Ctrl-s` | Save the selected section as displayed to a file |
| `W` | Save the selected section's full content to a file |
| `e` / `Ctrl-e` | Export the session to markdown **as currently folded** (folded sections become their summary lines) |
| `E` | Export the **full** session to markdown (tool calls as `<details>` blocks with fenced input/output) |

- Save and export prompt for a path in the help line. Defaults:
  - export: `./<project>-<yyyymmdd-hhmm>-<short-id>.md`
  - save: `./<project>-<short-id>-<section-kind>.txt`
- Saving or exporting asks for confirmation before overwriting an existing file.

From SPEC.md §4.3:

- **Confirmation prompts** (overwrite file, resume in harness) appear in the help line as `y/n` questions, not as dialogs.

From SPEC.md §6:

**Input mode:** while a help-line prompt is active (search, file path, y/n confirmation), the `Prompt` component has focus (§1.4) and every printable key goes into the prompt. Only `Enter`, `Esc`, `Backspace`, the arrow keys and `Ctrl-c` keep their meaning there. This prevents conflicts such as `j` / `t` / `1`–`4` while typing a search.

From SPEC.md §4.2:

- Help line under the transcript: context-sensitive key hints. It turns into the search/path prompt when active and shows search position (`3/17`), the `↓ N new` indicator and parse warnings.
- Toasts (copy/save/export confirmations, config warnings) appear in the bottom-right corner for ~2 s.

From SPEC.md §1.2:

- **`Msg`**: what happened, in application terms (`OpenSession(id)`, `CopyText(text)`, `PreviewTheme(id)`, `Quit`, …), never raw keys. Split it into nested enums per screen or concern once it grows.

- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (§5.1), so a 100 MB session is held once (§9.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

From SPEC.md §9.5:

- Never writes to harness data. The SQLite database is opened read-only.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Text sources.** Reuse task 025's pure functions. "As displayed" text is used for `save`, and full content for `save_full`. If the truncation row is selected, its parent section is used.
- **Flow.**
  1. The `Transcript` returns `Msg::SaveSection { text, kind }` or `Msg::Export { full: bool, document }`. The document is built by the `Transcript`, which owns turns and fold state.
  2. The `App` mounts the `Prompt` in `Text` mode with label `Save to: ` or `Export to: ` and the default path pre-filled, with the cursor at the end.
  3. On `PromptSubmit(path)`:
     - if the path exists, the `App` remounts the prompt in `YesNo` mode as `Overwrite <path>? (y/n)`;
     - `y` writes, and anything else cancels with no write;
     - if the path does not exist, it writes directly.
  4. `PromptCancel` aborts.
  5. Paths are relative to the process working directory. `~` is expanded with `PathBuf` joins.
- **Default file names.**
  - `<project>` is the project display name with every character outside `[A-Za-z0-9._-]` replaced by `-`.
  - `<short-id>` is the first 8 characters of the session id.
  - `<yyyymmdd-hhmm>` is the session start time in the display timezone.
  - `<section-kind>` is one of `user`, `assistant`, `thinking`, `shell`, `read`, `write`, `edit`, `grep`, `glob`, `webfetch`, `websearch`, `subagent`, `tool`, `system`, `compaction` or `stats`.
- **Markdown export format.**
  - Start with `# <session title>`, then a metadata list:
    - `- Project: <name>`
    - `- Session: <id>`
    - `- Working directory: <cwd>`
    - `- Started: <timestamp>`
    - `- Harness: <name>`
  - Then one `## <Role> — <timestamp>` heading per turn. For Assistant turns: `## Assistant (<model> @ <effort>) — <timestamp>`. For compaction: `## Compaction — <timestamp>`.
  - Section content:
    - **Prompts and assistant text:** their markdown verbatim.
    - **Thinking:** a block quote.
    - **Folded export (`e`):**
      - a folded section is its summary line as plain text, e.g. `▶ Find the BRIO on USB` becomes `> Find the BRIO on USB` in a paragraph;
      - unfolded sections are written as displayed, with tool bodies in fenced code blocks under their labels;
      - hidden system content is omitted.
    - **Full export (`E`):**
      - every section in full;
      - tool calls as `<details><summary><description></summary>` followed by a fenced input block (`json`, or `sh` for Shell), a fenced output block, and `</details>`;
      - system content included regardless of the `s` toggle;
      - nothing truncated.
  - Fences are long enough to contain any backtick run in the content.
- **Writing.** Write with `std::fs::write` (create or truncate) only after confirmation where needed. Create no directories, so a missing parent directory is an error.
  - Success toast: `Saved to <path>` or `Exported to <path>`.
  - Failure toast: `Save failed: <error>` or `Export failed: <error>`.
  - Never write inside the harness data directories: refuse with `Refusing to write into harness data` (§9.5).

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] `w` and `Ctrl-s` save as displayed, `W` saves full content, `e` and `Ctrl-e` export as folded, and `E` exports full. Each opens the path prompt with the correct default name (`brio-85a86657-shell.txt`, `brio-20261021-0835-85a86657.md`, with `./` prefixes per §6.4) (update tests).
- [ ] A non-existing path writes immediately. An existing path asks `Overwrite <path>? (y/n)`. `y` overwrites, and `n` or `Esc` leaves the file untouched (update tests with a temp dir).
- [ ] Folded export: folded sections appear as their summary lines, unfolded ones as displayed, and hidden system content is omitted (golden-file test with `insta`).
- [ ] Full export: every tool call is a `<details>` block with fenced input and output, outputs are untruncated (including lazy outputs), and system content is included (golden-file test).
- [ ] A fence around content containing a run of backticks is longer than that run (unit test).
- [ ] Success and failure toasts show the planner-notes texts. Writing into the Claude data dir is refused (update tests).
- [ ] The default-name sanitising turns `My Project/x` into `My-Project-x` (unit test).
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
Changes to the browser screen's help-line row:
- **Path prompt.** `Save to: ` (or `Export to: `) in `help_key`, then the editable path in `foreground` with the cursor cell (bg `cursor`) at the end.
- **Overwrite confirmation.** `Overwrite ./brio-85a86657-shell.txt? (y/n)` in `help_text`, with the path in `foreground`.
- **Toasts.** `Saved to ./brio-85a86657-shell.txt` or `Exported to ./brio-20261021-0835-85a86657.md`, in the task-012 toast style.

Snapshots (screen through `App::view`, browser fixture, `utc = true`):
1. Save path prompt with the default path at 80x24 (text + styled).
2. Export path prompt at 120x40 (text).
3. Overwrite confirmation at 80x24 (text).
4. `Exported to …` toast at 80x24 (text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 025-copy-to-clipboard
