# Prompt component and fuzzy session search

## Goal
Add the `Prompt` component, the help-line prompt used for searches, file paths and `y/n` confirmations. It edits text with `tui-input`, takes focus in input mode, and is drawn in place of the help line. This task uses it for the selector's fuzzy session search (`/`):
- `nucleo` matches over title, first prompt and project name, across all projects of the current harness;
- the Sessions pane shows the matches with their project name prefixed, and keeps the focus;
- `Esc` clears the query and restores the state the search started from.

Opening and closing the prompt follows the §1.4 rule: mounting gives it focus, and unmounting returns focus to the component that had it before.

## Spec excerpts
From SPEC.md §4.1:

- When the Sessions pane gets the focus (`Tab`, `Enter` on a project, `/`, clicking a row) and no session is selected, the newest session is selected.

- While nothing is selected, live index updates keep the cursor on the newest session. `Esc` on a search that started from this state returns to it: Projects focused, no session selected.

Search (`/` in the selector):
- Fuzzy (`nucleo`) over title, first prompt and project name across **all projects of the current harness**.
- While a query is active, the Sessions pane shows matching sessions with their project name prefixed, and keeps the focus (`Tab`, `←`/`→`, `h`/`l` do nothing). `ESC` clears the query.

From SPEC.md §6:

**Input mode:** while a help-line prompt is active (search, file path, y/n confirmation), the `Prompt` component has focus (§1.4) and every printable key goes into the prompt. Only `Enter`, `Esc`, `Backspace`, the arrow keys and `Ctrl-c` keep their meaning there. This prevents conflicts such as `j` / `t` / `1`–`4` while typing a search.

From SPEC.md §6.6:

| `/` | Fuzzy search sessions (§4.1) |
| `Enter` | Open the selected session; in the Projects pane, move focus to Sessions |
| `Esc` | Clear search; otherwise close the selector (if a session is already open) or quit (at startup) |

From SPEC.md §1.3:

- `tui-realm-stdlib` is not used. The look in §4 and §8 is specific enough that wrapping stdlib components would mean overriding their rendering. Prompt text editing uses `tui-input` inside the `Prompt` component.

| Component | Where | Draws / handles | Takes focus |
|---|---|---|---|
| `SessionList` | Selector | Sessions pane, including search results with the project prefix and the `●` marker | Yes |
| `HelpLine` | Both | Key hints and status items (`3/17`, `↓ N new`, `Loading… 34%`) | No |
| `Prompt` | Both | Search and path prompts and `y/n` confirmations, drawn in place of the help line while active | Yes (input mode, §6) |


From SPEC.md §1.4:

- **Overlays and prompts:** opening one mounts its component and gives it focus. Closing it unmounts it and returns focus to the component that had it before.

- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, §6; `Esc` / `?` closes help). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (§6.6).

From SPEC.md §4.2:

- Help line under the transcript: context-sensitive key hints. It turns into the search/path prompt when active and shows search position (`3/17`), the `↓ N new` indicator and parse warnings.

From SPEC.md §4.3:

- **Confirmation prompts** (overwrite file, resume in harness) appear in the help line as `y/n` questions, not as dialogs.

From SPEC.md §1.1:

- Parsing, watching, stats computation and search indexing run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used.

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

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Prompt component.**
  - Props: a label (e.g. `/`, or `Save to: ` in later tasks), initial text, mode (`Text` or `YesNo`), and the theme.
  - In `Text` mode, printable characters are inserted. `Backspace` and `←`/`→` edit and move through `tui-input`.
  - `↑`/`↓` return a `Msg` (e.g. `PromptNav(Up/Down)`) that the owner of the prompt can use.
  - `Enter` returns `PromptSubmit(text)` and `Esc` returns `PromptCancel`.
  - Every text change returns `PromptChanged(text)`.
  - `Ctrl-c` returns `Msg::Quit`.
  - `YesNo` mode (used by later tasks) maps `y`/`Y` to `PromptSubmit("y")` and every other key except `Ctrl-c` to `PromptCancel`.
  - Rendering:
    - the label is in `help_key` and the text in `foreground`;
    - the cell under the cursor is painted with bg `cursor`;
    - text longer than the area scrolls horizontally to keep the cursor visible.
  - The prompt occupies the help line's area.
  - While it has focus, the `GlobalEvents` subscription clauses must not fire for printable keys: typing `q` or `t` in the prompt types the letter. This uses the clause hook from task 011.
- **Focus return.** The `App` keeps a focus stack: mounting an overlay or prompt pushes the current focus, and unmounting it pops and restores. Later overlays use the same mechanism.
- **Search behaviour.**
  - `/` from either pane mounts the Prompt with label `/`.
  - While the query is empty, the Sessions pane shows its normal content. Once the query is non-empty, it shows results:
    - the `App` matches `title`, `first_prompt` and the project display name of every session of the current harness;
    - matching uses `nucleo`'s matcher (`nucleo-matcher` from the same project is acceptable) with smart case;
    - results are sorted by score, ties newest first;
    - the results are pushed to `SessionList` as a results list.
  - Each result row is `<timestamp>  <dot slot><project display name> › <title>`, plus stats as in task 013. The pane title is ` Sessions matching "<query>" `.
  - No matches: the interior shows `No matching sessions`, centred, in `muted_foreground`.
  - The first result is selected. `PromptNav(Up/Down)` moves the selection, and the `App` forwards it to `SessionList` (e.g. through a custom attribute; never by reaching into it).
  - Visual focus (the `ring` border) stays on Sessions while the prompt has input focus. `Tab`, `←`/`→` and `h`/`l` don't move pane focus: in the prompt, `←`/`→` move the text cursor and `h`/`l` are text.
  - `Enter` (`PromptSubmit`) with a selected result returns/handles `Msg::OpenSession` for it. With no result it does nothing.
  - `Esc` (`PromptCancel`) clears the query, unmounts the prompt, and restores the pre-search state. If the search started with Projects focused and no session selected, it returns to exactly that. Otherwise it returns to Sessions focused with the previously selected session.
  - Search runs synchronously on the UI thread if one keystroke over 5 000 sessions stays under 16 ms in a release build. Otherwise run the matcher on a worker and deliver results as a `UserEvent`. Record the measurement in the build notes.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] `Prompt` component tests:
  - typing `abc`, `Backspace` and `←` produces the expected text/cursor and `PromptChanged` messages;
  - `Enter` → `PromptSubmit`, `Esc` → `PromptCancel`, `↑` → `PromptNav(Up)`, `Ctrl-c` → `Msg::Quit`;
  - `q` and `t` are inserted as text;
  - `YesNo` mode maps `y` to submit and `n` to cancel.
- [ ] While the prompt has focus, global keys don't fire: an update/integration test types `q` into the prompt and the app does not quit.
- [ ] Focus stack: opening the prompt from Projects and cancelling returns focus to Projects. Opening it from Sessions and cancelling returns to Sessions (update tests).
- [ ] The query `brio` matches sessions whose title, first prompt or project name fuzzy-matches, across all projects of the current harness and not other harnesses. Results are sorted by score, ties newest first (unit test on the search function).
- [ ] During a search, `Tab`/`←`/`→` do not change pane focus. `↑`/`↓` move the result selection. `Enter` opens the selected result (`Msg::OpenSession`) (update tests).
- [ ] `Esc` on a search started from "Projects focused, no session selected" returns to exactly that state. A search started from Sessions returns to Sessions with the previous selection (update tests).
- [ ] The search-time measurement for 5 000 sessions is recorded in the build notes, and the chosen synchronous/worker approach matches the planner-notes rule.
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
Changes to the selector screen:
- **Prompt.** In place of the help line (same row and columns as task 012): `/` in `help_key`, then the query in `foreground`. The cell after the last character (the cursor) has bg `cursor`.
- **Search results.**
  - The Sessions pane title becomes ` Sessions matching "<query>" `.
  - Rows are `<timestamp>  <dot slot><project> › <title>` with right-aligned stats, using the §8.2 styles.
  - The first result is selected with the focused band, and the `ring` border is on Sessions.
  - The Projects pane keeps its previous selection, drawn as unfocused-selected (bold `secondary_foreground` with `>`).
- **No results.** `No matching sessions` centred in the Sessions pane interior, in `muted_foreground`.

Snapshots (screen, fixture data spread across several projects):
1. Prompt open with an empty query (`/` + cursor), with the panes unchanged, at 120x40 (text + styled to show the cursor cell).
2. Query `brio` with results from three projects at 120x40 (text + styled) and 80x24 (text).
3. Query with no results at 80x24 (text).
4. After `Esc`: back to the startup state, at 120x40 (text).

Component snapshots: `Prompt` with label `/` and text longer than its width (width 30), showing the horizontal scroll that keeps the cursor visible.

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 014-selector-selection-rules
