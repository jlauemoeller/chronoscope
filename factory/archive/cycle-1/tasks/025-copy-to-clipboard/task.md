# Copy sections to the clipboard

## Goal
Implement `y` and `Y` in the browser (§6.4).
- **`y`** copies the selected section as displayed: the folded summary when folded, everything shown when unfolded.
- **`Y`** copies the full content: description, full input or command, and full result, regardless of fold state or truncation.

The `Transcript` computes the text and sends it in `Msg::CopyText`. The `App` owns the clipboard: it tries `arboard` first, and also emits OSC52 when no display server is reachable or `$TMUX` is set (wrapped in tmux's DCS passthrough under tmux). A toast confirms the copy (`Copied 42 lines`).

## Spec excerpts
From SPEC.md §6.4:

| Key | Action |
|---|---|
| `y` | Copy the selected section **as displayed** (folded: summary line; unfolded: everything shown) as plain text |
| `Y` | Copy the selected section's **full content** (description + full input/command + full result), regardless of fold state or truncation |

- Clipboard: `arboard`, falling back to OSC52 when there's no display server or over SSH/tmux. A toast confirms the copy (`Copied 42 lines`).

From SPEC.md §6.5:

`Ctrl-c` always quits; it is never used for copy.

From SPEC.md §9.4:

- Clipboard: try `arboard` first. When no display server is reachable or `$TMUX` is set, also emit OSC52.
  - Under tmux, wrap OSC52 in the DCS passthrough sequence.
  - Document `set -g set-clipboard on` and `set -g allow-passthrough on`.
  - herdr's OSC52 behaviour must be verified.

From SPEC.md §1.2:

- **`App`** owns the `Application` and the domain state: config, theme registry and active theme, keymap, session index and stats, the open session's summary, watch handle and subagent stack, worker handles and the clipboard.

- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (§5.1), so a 100 MB session is held once (§9.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

From SPEC.md §4.2:

- Toasts (copy/save/export confirmations, config warnings) appear in the bottom-right corner for ~2 s.

From SPEC.md §5.2:

Each turn is rendered as a rounded block. Its title shows the role on the left and the timestamp on the right. Assistant blocks also show `model @ effort` after the role. A **section** is the unit of selection, folding, copying and saving.

Redacted thinking (no readable text) is kept in the model (§5.1) but is not a section. It is not drawn, selected, searched, copied or exported, because it has nothing to show. A turn whose only content is redacted thinking is drawn as an empty block.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Text "as displayed".**
  - The section's visible text as plain text: logical lines (no soft-wrap breaks), no borders, fold or selection markers, or column indentation.
  - Keep the content's own indentation: code-block indent, list bullets, the `│ ` quote prefix.
  - Labels (`Command:`, `Result:` …) are included as their own lines, and the blank lines between parts are kept.
  - A folded section copies exactly its summary line(s) without the `▶ ` marker. For a folded user prompt that is the first two lines plus `…`.
  - A truncated output includes the `… N more lines` row as shown.
  - Search highlights are irrelevant.
- **Full content.** The complete section independent of fold and truncation:
  - for a tool call: description, a blank line, the full input/command, a blank line, the full result (via `ToolOutput::text()`), with the same labels as the unfolded body;
  - for other sections: the full unfolded text.
- If the truncation row is selected, both actions act on its parent tool section.
- Both are pure functions on the `Transcript` side, each with its own unit tests (they are reused by save and export in task 026).
- **Clipboard** (owned by the `App`, behind a small trait so tests can use a fake):
  - Keep one `arboard::Clipboard` alive for the app's lifetime (on X11 the content vanishes when it is dropped). Create it lazily, and remember if creation failed.
  - Use arboard when it works. Also emit OSC52 when arboard failed, when `$TMUX` is set, or when `$DISPLAY` and `$WAYLAND_DISPLAY` are both unset on Linux. Read the env through an injected struct.
  - OSC52 is `ESC ] 52 ; c ; <base64> BEL` (add the `base64` crate). Under tmux, wrap it in the DCS passthrough form `ESC P tmux; <sequence with every ESC doubled> ESC \`.
  - Write the sequence to the terminal output through the terminal adapter, or stdout, after the frame. Tests capture it with an injected writer.
- **Toast.** On success the toast reads `Copied <n> lines` (`Copied 1 line` for one). If arboard failed and no OSC52 was emitted, it reads `Copy failed: <reason>`.
- `Ctrl-c` is never a copy key, and stays quit.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] Copy-as-displayed (unit tests on the `Transcript` text functions):
  - a folded tool call gives exactly its description line;
  - an unfolded Shell call gives description, `Command:`, the command, `Result:` and the result lines, without markers or borders;
  - a folded user prompt gives its two lines plus `…`;
  - a truncated output includes the `… N more lines` line.
- [ ] Full copy of a folded, truncated tool call gives the full input and the full untruncated result, including a lazy output over 4 KiB (unit test).
- [ ] `y`/`Y` return `Msg::CopyText(text)` from `Transcript::on`. With the truncation row selected, they copy the parent tool section (component tests).
- [ ] The `App` with a fake clipboard puts the text on it and shows the `Copied <n> lines` toast. The singular `Copied 1 line` is tested (update tests).
- [ ] OSC52 encoding is byte-exact. The tmux-wrapped form doubles `ESC` and uses `ESC P tmux; … ESC \` (unit tests).
- [ ] The OSC52 decision follows the planner-notes env rules: emitted with `TMUX` set, emitted with no display variables on Linux, and not emitted with `DISPLAY` set and no `TMUX` while arboard works (unit tests with an injected env).
- [ ] A clipboard failure with no OSC52 shows `Copy failed: …` (update test).
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
The only visible change is the toast (task 012 style: a 3-row rounded box in `toast` style, bottom-right, bottom border on row `h − 2`):
- `Copied 42 lines` after a successful copy.
- `Copy failed: <reason>` after a failure.

Snapshots (screen through `App::view` in the browser, fake clipboard):
1. Toast `Copied 42 lines` over the browser at 80x24 (text) and 120x40 (styled).
2. Toast `Copy failed: no clipboard available` at 80x24 (text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 021-browser-screen
