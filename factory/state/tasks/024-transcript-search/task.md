# Transcript search

## Goal
Implement transcript search in the browser (§6.2).
- **Prompt.** `/` opens the `Prompt` (task 015) in the help line.
- **Matching.** Smart-case substring matching runs over all text: folded sections, tool input and output (including lazily stored output), and system content. It runs on a worker thread, and results come back as a `UserEvent`.
- **Navigation.** `Enter` jumps to the first match. `n`/`N` go to the next/previous match, and the help line shows the position (`3/17`). Jumping into a folded section unfolds it temporarily; it refolds when the selection moves elsewhere, unless the user unfolded it explicitly in the meantime.
- **Highlighting.** All visible matches are highlighted with `search_match`.
- **Clearing.** `Esc` clears the search and the highlights.

## Spec excerpts
From SPEC.md §6.2:

- `/` opens a query in the help line. Matching is smart-case substring (case-insensitive unless the query contains an uppercase letter).
- Searches **all** text, including folded sections, tool input/output and system content.
- `Enter` confirms and jumps to the first match; `n` / `N` go to the next / previous match. All matches are highlighted with `search_match`, and the help line shows `3/17`.
- Jumping to a match inside a folded section unfolds it temporarily. It refolds when the selection moves to a different section, unless the user unfolded it explicitly in the meantime.
- `ESC` clears the search and highlights.

From SPEC.md §6:

**Input mode:** while a help-line prompt is active (search, file path, y/n confirmation), the `Prompt` component has focus (§1.4) and every printable key goes into the prompt. Only `Enter`, `Esc`, `Backspace`, the arrow keys and `Ctrl-c` keep their meaning there. This prevents conflicts such as `j` / `t` / `1`–`4` while typing a search.

From SPEC.md §4.2:

- Help line under the transcript: context-sensitive key hints. It turns into the search/path prompt when active and shows search position (`3/17`), the `↓ N new` indicator and parse warnings.

From SPEC.md §1.1:

- Parsing, watching, stats computation and search indexing run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used.

From SPEC.md §1.2:

- **`UserEvent`**: what the Ports deliver: index updates, stats results, session `ModelUpdate`s, search results. tui-realm requires `UserEvent: Eq + Clone`, so large payloads are `Arc`-wrapped and compared by a sequence number.

Rules:
- Data flows **down** into components through `attr` (typed data via `AttrValue::Payload(PropPayload::Any(..))`, `Arc`-wrapped when large). Intent flows **up** as `Msg`. Components never see the `App`, and the `App` never reaches into a component's internals.
- No shared mutable state between components, or between a component and the `App`: no `Rc<RefCell<…>>`, `Arc<Mutex<…>>` or globals for UI state.
- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (§5.1), so a 100 MB session is held once (§9.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

From SPEC.md §1.3:

| Component | Where | Draws / handles | Takes focus |
|---|---|---|---|
| `Transcript` | Browser | The turn blocks: selection, folding, navigation, search highlights (§5, §6.1–§6.3) | Yes |
| `Prompt` | Both | Search and path prompts and `y/n` confirmations, drawn in place of the help line while active | Yes (input mode, §6) |

From SPEC.md §8.1:

| Token | Default | Used for |
|---|---|---|
| `search_match` | fg `primary_foreground` on `primary` | Search hits (the only explicit background colour; the selector's focused selection uses reverse video, §8.2) |

From SPEC.md §9.2:

- Large tool outputs: the model stores line counts and byte ranges up front. The full text is materialised only when the section is unfolded, searched, copied or exported.

From SPEC.md §5.6:

- System/injected content is shown by default (muted, folded). A toggle key hides or shows all of it; the default is configurable (`show_system = true`).

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Flow** (respecting the ownership rules of §1.2):
  1. `browser.search` in the `Transcript` returns a `Msg` that makes the `App` mount the `Prompt` with label `/` in place of the help line, pushing focus.
  2. `PromptSubmit(query)` with a non-empty query makes the `App` ask the `Transcript` to start a search by setting an attr with the query.
  3. The `Transcript` replies with a `Msg::StartTranscriptSearch { query, corpus, generation }`. The corpus is an `Arc` snapshot of its turns (cheap `Arc<Turn>` clones), plus the section order and the current `show_system` state.
  4. The `App` runs the search on a worker thread. A Port delivers `UserEvent::SearchResults { generation, matches }`.
  5. `GlobalEvents` turns that into a `Msg`, and the `App` pushes the matches to the `Transcript`, which jumps to the first match at or after the current selection. If there is none, it wraps to the first match.
  6. `PromptCancel`, or an empty query, unmounts the prompt without searching.
- **Matching.**
  - Case-insensitive unless the query contains an uppercase character.
  - Plain substring, not regex.
  - The searched text of each section is its full unfolded plain text: prompt, assistant text, readable thinking, tool description plus full input plus full result (materialised via `ToolOutput::text()`), system entries, compaction summary, stats footer.
  - Hidden system content (`s` toggled off) is not searched.
  - A match is `(SectionId, line index, byte range)` within that plain text. Several matches in one section are separate matches.
- **Navigation.**
  - `search_next`/`search_prev` move to the next/previous match, wrapping around. They scroll the match's line into view and select its section.
  - The help line status item is `<i>/<n>` (e.g. `3/17`). With no matches it is `0/0`, and a toast says `No matches for "<query>"`.
- **Temporary unfold.** When the jump target is folded, unfold it and mark it as temporarily unfolded. A match beyond the truncation limit also expands the output temporarily. When the selection moves to a different section, the temporary unfold is undone, unless the user toggled the fold of that section in the meantime, which makes it explicit.
- **Highlighting.** While a search is active, every occurrence of the query in rendered rows of unfolded content is drawn with the `search_match` style. Apply it per rendered row by matching the query in the row text with the same case rule. A match split across a wrap boundary may be highlighted as two parts. The current match is the one in the selected section.
- **Clearing.** `browser.clear_search` (`Esc`) clears matches, highlights, the position item and any temporary unfolds. A new `/` search replaces the previous one.
- **Stale results.** Results carry the generation. Results for an older generation are ignored, and so are results that arrive after a `Reset` of the transcript.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] The smart-case rule holds: `foo` matches `Foo` and `FOO`, and `Foo` matches only `Foo` (unit tests on the matcher).
- [ ] Search covers folded sections, tool input, full tool output (including a lazy output over 4 KiB and output beyond the truncation limit), system content and compaction summaries. Hidden system content is excluded (tests).
- [ ] The search runs off the UI thread and results arrive via Port → `UserEvent` → `GlobalEvents` → `Msg`. Results with an old generation are ignored (update tests).
- [ ] `Enter` jumps to the first match, `n`/`N` cycle with wrap-around, and the help line shows `i/n`. No matches shows `0/0` plus a toast (update and component tests).
- [ ] Jumping into a folded section unfolds it, and moving the selection to another section refolds it. If the user pressed `Enter` (fold toggle) on it in between, it stays unfolded (component tests).
- [ ] `Esc` removes highlights, the position item and temporary unfolds (component test).
- [ ] Typing `n`, `q` or `t` in the search prompt inserts the characters and triggers no action (update test).
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
Changes to the browser screen:
- **Prompt.** In place of the help line: `/` in `help_key`, then the query and a cursor cell (bg `cursor`), as in task 015.
- **Matches.** Every visible occurrence is drawn with `search_match` (fg `primary_foreground` on bg `primary`). The selected section shows `▒` as usual.
- **Position.** The help line status item `3/17` is right-aligned in `help_text`. With `↓ N new` also present, the order is `↓ N new  3/17`.

Snapshots (screen through `App::view`, fixture session, `utc = true`):
1. Prompt open with the query `usb` typed, at 80x24 (text + styled for the cursor cell).
2. After `Enter`: the first match selected, with highlights in an unfolded assistant text section and `1/5` in the help line, at 120x40 (text + styled).
3. After `n` into a folded tool call: the section temporarily unfolded, the match highlighted in the result, and `2/5`, at 120x40 (text).
4. After moving with `j` off that section: refolded, and `2/5` still shown, at 120x40 (text).
5. After `Esc`: no highlights and no position item, at 80x24 (styled).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 023-browser-live-updates
