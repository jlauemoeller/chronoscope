# Transcript navigation, folding keys and system toggle

## Goal
Make the `Transcript` keyboard-driven through the keymap's `browser` context:
- section moves (`j`/`k`, arrows), with tall sections paged through before moving on;
- turn-block jumps (`]`/`[`);
- half-page and full-page scrolling, which moves the selection to the topmost fully visible section when the selected one leaves the viewport;
- start/end jumps;
- the fold keys: toggle, fold, unfold, fold all, unfold all;
- `s`, which toggles visibility of system/injected content (initial value from `show_system`).

## Spec excerpts
From SPEC.md §6.1:

| Key | Action |
|---|---|
| `Enter` / `Space` | Toggle fold of current section |
| `←` / `h` | Fold current section |
| `→` / `l` | Unfold current section |
| `-` | Fold all |
| `+` / `=` | Unfold all |

Fold levels (`1`/`2`/`3`) and Vim-style `z` aliases (`za`, `zc`, `zo`, `zM`, `zR`) were considered and rejected. No default binding is a chord.

From SPEC.md §6.3:

| Key | Action |
|---|---|
| `↓` / `j`, `↑` / `k` | Select next / previous section |
| `]` / `[` | Jump to next / previous turn block (User / Assistant / Compaction) |
| `Ctrl-d` / `Ctrl-u` | Scroll half a page down / up |
| `PgDn` / `PgUp` | Scroll a full page down / up |
| `g` / `Home`, `G` / `End` | Jump to start / end (`G` resumes following, §7.2) |

- `Ctrl-b` / `Ctrl-f` are deliberately not bound: `Ctrl-b` is tmux's default prefix.
- Selecting a section scrolls it into view. If the selected section is taller than the viewport, `j`/`k` first scroll through it a page at a time and only then move to the next section.
- Scrolling with page keys or the mouse wheel moves the selection to the topmost fully visible section when the current one leaves the viewport.

From SPEC.md §5.6:

- System/injected content is shown by default (muted, folded). A toggle key hides or shows all of it; the default is configurable (`show_system = true`).
- It is rendered as **System sections inside the adjacent turn**: in the next turn when it precedes a prompt, in the current turn otherwise. Consecutive injected entries collapse into one section with a count (`3 injected items`). They never become turn blocks of their own, so `[` / `]` only stop at User, Assistant and Compaction blocks.

From SPEC.md §6.5:

| Key | Action |
|---|---|
| `s` | Toggle visibility of system / injected content |

From SPEC.md §8.5:

```toml
show_system = true
```

From SPEC.md §1.4:

- **Keys go through the keymap.** A component's `on()` resolves the key with the shared keymap (§8.5), for its own context, to an action. The action becomes a `Cmd` (handled internally with `perform`) or a `Msg`. Components never match literal `KeyEvent`s, except that `Prompt` takes printable characters as text. This keeps remapping and the generated help overlay (§4.3) correct.

From SPEC.md §1.2:

- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (§5.1), so a 100 MB session is held once (§9.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Key handling.** `Transcript::on()` resolves keys with its own `KeyResolver` for the `browser` context (task 005) and turns actions into internal `Cmd`s. It returns `Msg`s only where the App must act. Actions owned by other tasks (search, copy, save, export, subagent, back, resume) return nothing yet.
- **Selection stops.** Sections in display order, including the truncation rows from task 019. Hidden system sections are not stops.
- **`next`/`prev`.**
  - Select the next/previous stop and scroll it into view minimally.
  - When the selected section is taller than the viewport, `next` first scrolls down by one viewport height (a page) at a time until the section's last row is visible, and only then moves to the next stop. `prev` mirrors this upwards from the section's top.
  - The first and last stops are the bounds: no wrap.
- **`next_turn`/`prev_turn`.** Select the first section of the next/previous User, Assistant or Compaction block. System content never forms a block. Scroll so the block's top border is at the top of the viewport when possible.
- **Scrolling.**
  - `half_page_down`/`up` scroll by ⌊viewport height / 2⌋ rows; `page_down`/`up` by the viewport height. All clamp.
  - After scrolling, if the selected section is no longer fully visible, the selection moves to the topmost fully visible section. If no section is fully visible (one section covers the viewport), it moves to the section at the top row.
- **`top`/`bottom`.** Select the first/last stop and scroll to the start/end. ("Resumes following" is task 023; here `bottom` only jumps.)
- **Folding.**
  - `fold_toggle` toggles the selected section if it is foldable. On a truncation row it expands, per task 019.
  - `fold`/`unfold` fold/unfold the selected section; no-op when not foldable or already in that state.
  - `fold_all`/`unfold_all` set every foldable section in the session.
  - After any fold change, the selected section's first row stays at the same screen row when possible, so the view doesn't jump.
- **`toggle_system`** flips the Transcript's `show_system` presentation state; the initial value comes from config via the prop. When system sections become hidden while one is selected, the selection moves to the nearest following visible section, or else the previous one.
- **Scrolling with Ctrl-d/u and PgDn/PgUp** keeps working when the transcript is shorter than the viewport (no-op).

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] `j`/`↓` and `k`/`↑` move the selection across sections and blocks. A section taller than the viewport is paged through first: three presses on a 3-page section scroll by pages, then the fourth selects the next section (component tests).
- [ ] `]`/`[` jump between User/Assistant/Compaction blocks and never stop on system content. The jumped-to block's top border is at the viewport top when possible (tests).
- [ ] `Ctrl-d`/`Ctrl-u` scroll half a page and `PgDn`/`PgUp` a full page, clamped. When the selection leaves the viewport it moves to the topmost fully visible section (tests).
- [ ] `g`/`Home` and `G`/`End` select the first/last section and scroll to the start/end (tests).
- [ ] `Enter`/`Space` toggle, `←`/`h` fold, `→`/`l` unfold, `-` folds all and `+`/`=` unfold all. Non-foldable sections ignore them. The selected section's screen row is kept after a fold change when possible (tests).
- [ ] `s` hides and shows all system/injected sections. With `show_system = false` they start hidden. A selected system section that becomes hidden moves the selection to a visible neighbour (tests).
- [ ] Remapping works: `[keys.browser] next = "J"` makes `J` move and `j` do nothing (test).
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
No new elements. The `Transcript` from tasks 018/019 in states produced by keys.

Snapshots (component `Transcript`, fixture session, 80x24 unless noted):
1. After `j` ×3 (selection in the second block, `▒` on it) (text).
2. Paging through a section taller than the viewport: after the 1st and 2nd `j`, with the same section selected and the view scrolled by a page (text).
3. After `]` ×2: the third block's top border at the top row (text).
4. After `Ctrl-d` with the selection moved to the topmost fully visible section (text).
5. After `+` (all unfolded) and after `-` (all folded) at 120x40 (text).
6. After `s` (system sections hidden), compared with the default (text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 019-tool-body-renderers
