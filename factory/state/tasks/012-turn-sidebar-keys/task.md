# Turns sidebar keys and focus

## Goal
Make any block a few keys away: the sidebar takes focus, its keys select an entry and scroll the transcript to its block, and `Enter` moves focus there. This adds the `[keys.turns]` keymap context, the focus rules, the help line hints and the "Turns sidebar" help group.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §6.4 (the mouse paragraph at its end is the next task's):

A new keymap context `[keys.turns]` holds the sidebar's keys (base spec §1.4). Its defaults match the selector's:

| Context | Action | Default keys | Effect |
|---|---|---|---|
| `browser` | `focus_turns` | `1` | Focus the sidebar |
| `browser` | `next_pane`, `prev_pane` | `tab`, `shift-tab` | Focus the sidebar |
| `turns` | `up`, `down` | `up`, `k` and `down`, `j` | Select the previous or next entry, and scroll the transcript to it |
| `turns` | `open` | `enter` | Move focus to the selected entry's block |
| `turns` | `next_pane`, `prev_pane` | `tab`, `shift-tab` | Focus the transcript, leaving it as it is |
| `turns` | `focus_transcript` | `right`, `l` | The same |

- **`1` and `Tab` in the browser must not affect the selector,** where `1` still switches harness and `Tab` still switches pane. Contexts already keep these apart.
- `1` does nothing while the sidebar has focus, and neither does `←` / `h`, matching the selector, where they don't wrap around.
- Selecting an entry **scrolls the transcript to its block**, with the same result as `]` / `[`: the block's top at the top of the viewport (or as near as the end of the transcript allows), and the transcript's selection on the block's first selectable section. Reuse that code rather than writing a second path. The usual follow rule applies (base spec §7.2): moving away from the bottom stops following.
- A turn can have an entry but nothing selectable, when its text renders to no rows (for example, only an HTML comment). Selecting that entry still scrolls to the block and leaves the transcript's selection unchanged.
- `Enter` does the same and also gives the transcript focus.
- Focusing the sidebar does not by itself scroll the transcript.
- While the sidebar has focus, other browser keys (`/`, `y`, fold keys and so on) do nothing. Global keys still work (`q`, `?`, `t`, `Ctrl-o`). Overlays return focus to the sidebar when they close (base spec §1.4).
- The help line shows hints for the sidebar's keys while it has focus, generated from the active keymap, e.g. `ENTER to go to block | TAB to switch to transcript`.
- The help overlay lists the new keys in a "Turns sidebar" group, and they can be remapped like any other.

From SPEC.md §6.1 (focus while hidden):

- **Below 90 columns the sidebar is hidden** and the transcript takes the full width as today, because 90 is what leaves the transcript 59 columns. While hidden, the focus keys do nothing. If it has focus when it becomes hidden, focus moves to the transcript.

From SPEC.md §6.3 (selection):

- The list never wraps around.
- **While the transcript has focus, the sidebar's selection follows it:** the selected row is the turn containing the transcript's selected section, and it stays visible. This keeps the sidebar showing the current position, and returning to it never jumps. Pushing that selection to the sidebar produces no message.
- When the sidebar has focus and nothing is selected, it selects the first entry on the first move. An empty list has no selection, and its keys do nothing.

From SPEC.md §6.2 (which rows can be selected):

| Row | Content | Selectable |
|---|---|---|
| `● Start` | First row of the list | No: it has no block |
| `● User` | A User turn | Yes |
| `● Assistant` | An Assistant turn, with its duration (§6.5) | Yes |
| `═══ Compaction ═══` | A Compaction turn; `═` fills to the right margin | Yes |
| `● End` | Last row of the list | No: it has no block |

From SPEC.md §6.6:

Sessions can have thousands of turns (base spec §9.2). Building rows must not lay out or render transcript blocks, and turn updates must not rebuild more than the rows that changed. The sidebar draws only the rows it shows, and a key press in the sidebar stays within the frame budget on the 100 MB fixture.

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

From spec/SPEC.md §1.2:

The UI follows tui-realm's model: components turn events into messages, and one update function acts on them.

- **`App`** owns the `Application` and the domain state: config, theme registry and active theme, keymap, session index and stats, the open session's summary, watch handle and subagent stack, worker handles and the clipboard.
- **`Id`**: one enum variant per mountable component (§1.3).
- **`Msg`**: what happened, in application terms (`OpenSession(id)`, `CopyText(text)`, `PreviewTheme(id)`, `Quit`, …), never raw keys. Split it into nested enums per screen or concern once it grows.
- **`UserEvent`**: what the Ports deliver: index updates, stats results, session `ModelUpdate`s, search results. tui-realm requires `UserEvent: Eq + Clone`, so large payloads are `Arc`-wrapped and compared by a sequence number.
- **`App::update(Msg)`** is the only place that changes domain state, mounts and unmounts components, moves focus and pushes new data into components. It is a dispatcher: each `Msg` arm calls a small handler, grouped in modules by concern (selector, browser, copy/save/export, resume, theme, live updates). It never interprets keys.
- **`App::view`** only computes the screen layout (§4) and calls `Application::view(&Id, frame, area)` for each visible component, overlays last. It draws nothing itself.

Rules:
- Data flows **down** into components through `attr` (typed data via `AttrValue::Payload(PropPayload::Any(..))`, `Arc`-wrapped when large). Intent flows **up** as `Msg`. Components never see the `App`, and the `App` never reaches into a component's internals.
- No shared mutable state between components, or between a component and the `App`: no `Rc<RefCell<…>>`, `Arc<Mutex<…>>` or globals for UI state.
- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (§5.1), so a 100 MB session is held once (§9.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

From spec/SPEC.md §1.3:

**Every custom UI element is its own component.** The code is organised around components, not around long render functions or a central key handler.

- A component implements tui-realm's `Component` trait (`view`, `query`/`attr`, `state`, `perform(Cmd) -> CmdResult`), plus `AppComponent` (`on(&Event) -> Option<Msg>`) if it handles events. An element that appears on several screens (e.g. the help line) is one component type, mounted with different props.
- A component draws only itself. Its `view` draws its own border and content, and leaves sub-elements to child types: a child component it owns, or a small `Widget`/`StatefulWidget` type.
- **No long render functions.** A render function (`view`, or a helper it calls) that draws more than one distinct element, or grows past about 50 lines, is split into smaller components or widgets. Rendering has no side effects.
- Components are `pub` types that can be built from plain data (props and the theme) and rendered into a `TestBackend` frame on their own, without an `Application` or a terminal (§9.6).
- All colours and modifiers come from theme tokens (§8.1), passed in as a prop. Components never hard-code colours.
- `tui-realm-stdlib` is not used. The look in §4 and §8 is specific enough that wrapping stdlib components would mean overriding their rendering. Prompt text editing uses `tui-input` inside the `Prompt` component.

From spec/SPEC.md §1.4:

- **Focus:** exactly one component has focus at a time, set by `App::update` through `Application::active`. In the selector, the pane keys of §6.6 move focus between `ProjectList` and `SessionList`. A focused component shows it through `Attribute::Focus` (e.g. the `ring` border, §8.2).
- **Overlays and prompts:** opening one mounts its component and gives it focus. Closing it unmounts it and returns focus to the component that had it before.
- **Keys go through the keymap.** A component's `on()` resolves the key with the shared keymap (§8.5), for its own context, to an action. The action becomes a `Cmd` (handled internally with `perform`) or a `Msg`. Components never match literal `KeyEvent`s, except that `Prompt` takes printable characters as text. This keeps remapping and the generated help overlay (§4.3) correct.
- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, §6; `Esc` / `?` closes help). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (§6.6).
- **Mouse:** wheel and clicks act on the element under the pointer, whether or not it has focus (§6.9). The event goes to the topmost component drawn there, for example through mouse subscriptions whose row/column ranges are that component's area. That component turns the event into a `Cmd` or `Msg`.
  - While an overlay is open (selector, help, theme selector), the components under it get no mouse events, and a click outside the overlay closes it.
  - Component areas come from a pure layout function that `App::view` also uses. Subscriptions are refreshed from it when the screen or terminal size changes, never from inside `view`.
- **Ticks:** `Event::Tick` drives toast expiry, the `● live` marker and the redraw limit (§7.1).
- **Worker results** take one path: Port → `Event::User` → `GlobalEvents` (subscribed to all user events) turns it into a `Msg` → `App::update` applies it to domain state and pushes what changed to the components that display it (rows and stats to `ProjectList` / `SessionList`, `ModelUpdate`s to `Transcript`). Other components don't subscribe to worker events.

From spec/SPEC.md §4.3:

- **Help overlay** (`?`): grouped keybindings as in ui-sketch.md, generated from the active keymap. `Esc` / `?` closes it.
- **Theme selector** (`t`): see §8.4.
- **Confirmation prompts** (overwrite file, resume in harness) appear in the help line as `y/n` questions, not as dialogs.

Overlays are centred, use rounded borders with `ring` colour, and paint `surface` behind them (unless the background is transparent, in which case `Clear` is still used so underlying text doesn't bleed through).

From spec/SPEC.md §6.3:

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

From spec/SPEC.md §6.6 (the selector's keys, which must not change):

| Key | Action |
|---|---|
| `1`–`4` | Switch harness tab (the number shown next to its name) |
| `Tab` / `Shift-Tab` | Move focus between Projects and Sessions |
| `←` / `h`, `→` / `l` | Focus Projects / Sessions (no wrap-around: `←` in Projects and `→` in Sessions do nothing) |
| `↑↓` / `j` `k` | Move in the focused list |
| `/` | Fuzzy search sessions (§4.1) |
| `Enter` | Open the selected session; in the Projects pane, move focus to Sessions |
| `Esc` | Clear search; otherwise close the selector (if a session is already open) or quit (at startup) |
| `R` | Resume the selected session in its harness (§6.7) |
| `t`, `?`, `q` / `Ctrl-q` / `Ctrl-c` | Theme selector, help overlay, quit |

- Harness numbers are stable: `1` Claude, `2` OpenCode, `3` Codex, `4` Pi.
- Harness tabs are switched only by number or by clicking a tab (§6.9). `←` / `→` move between the panes, never between tabs; there is no previous/next-harness action.
- Codex and Pi tabs are hidden until their adapters exist; their numbers stay reserved.
- Tabs for harnesses with no data on this machine are shown muted and can't be selected.

From spec/SPEC.md §7.2:

- **Follow when at bottom:** if the viewport is at the end of the transcript, new content scrolls into view automatically. If the user has scrolled up, the position is kept and a `↓ N new` indicator appears in the help line; `G` / `End` jumps to the end and resumes following.
- Updates to content already displayed (e.g. a tool result arriving for a pending tool call, or a streamed message growing) update the section in place. The current selection and fold states are preserved.
- A `● live` marker is shown in the browser header while the session is active: file/rows modified within the last 60 s, or OpenCode `time_idle` is null.

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

From spec/SPEC.md §8.5 (key syntax):

Key syntax: `a`, `A`, `ctrl-d`, `shift-tab`, `enter`, `esc`, `pgdn`, `f5`, and two-key chords as space-separated keys (`"g g"`; only in user config, since no default binding is a chord). The help overlay is generated from the active keymap, so it always reflects the user's remappings.

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

From spec/SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):

- **Keymap.** `Context::Turns`, name `turns`. `Context::ALL` and `Keymap.contexts` are fixed-size arrays of 4 and grow to 5, and the unknown-context warning in `Keymap::from_config` lists the known contexts. `supports_chords` is as for the selector. New actions: `Action::FocusTurns`, `Action::FocusTranscript`. Existing `Up`, `Down`, `Open`, `NextPane` and `PrevPane` are reused, since an action is identified by its context and itself. Add `NextPane` and `PrevPane` to the browser context. Group `Turns sidebar` for all of them.
- **Descriptions.** Browser: `focus_turns` "Focus turns sidebar", `next_pane` "Next pane", `prev_pane` "Previous pane". Turns: `up` "Previous entry", `down` "Next entry", `open` "Go to block", `next_pane` "Next pane", `prev_pane` "Previous pane", `focus_transcript` "Focus transcript".
- **README.** `tests/release_docs.rs::readme_keybindings_match_the_default_keymap` fails `cargo test` when a default binding is missing from the README, and every task must pass `cargo test`. So this task adds `focus_turns`, `next_pane` and `prev_pane` to the README's browser table, and a new table under a heading containing `` `[keys.turns]` `` (the test's parser finds the context in the heading), even though §8 lists these under the final documentation task.
- **Help overlay.** `BROWSER_GROUPS` in `src/components/help_overlay/content.rs` gains `Turns sidebar`, before `Application`. `HelpContent::new` handles only one `own` context, so it also has to list the `Turns` context's bindings under that group for the browser. A row with the same keys and description as one already in the group is listed once, so `TAB` is not listed twice.
- **Scrolling to a block.** Factor the scroll in `jump_turn` (`src/components/transcript/paging.rs`) into one entry point that takes a block index, and have both `]` / `[` and the sidebar call it. No second path. When `turn_stop` returns nothing (the text renders to no rows), it scrolls to the block and leaves the selection unchanged. The `App` reaches it through an attribute carrying the turn's id. It returns what the keys return, so the follow rule and `↓ N new` behave as for `]` / `[`: call `sync_new_sections` afterwards.
- **`Msg`s.** The sidebar reports a selected entry and `Enter` with the turn's id, and asks for the transcript's focus. The transcript reports the focus keys. The `App` decides: while the sidebar is hidden (below 90 columns, or `EmptySession`), the focus keys do nothing.
- **Hiding.** A helper runs on `Msg::Resized` and whenever `session_empty` changes. If the sidebar has focus when it becomes hidden, focus moves to the transcript on top. Trap: focus can also be *remembered* in `focus_stack` while an overlay or the selector is open, and `close_overlay` and `close_selector` would then return it to a hidden pane (`close_selector` only checks that the component is mounted). Rewrite that entry when hiding.
- **While the sidebar has focus,** the selection does not follow the transcript. Extend task 011's check to cover the focus being on the sidebar or on an overlay opened from it, using `focus_stack` to tell who opened the overlay. When focus returns to the transcript, the sync runs again and the row becomes the transcript's selected turn.
- **First move.** With no selection, the first move in either direction selects the first entry. `ListCursor::step_up` does not do this, so handle it here. It never wraps.
- **Title.** The pane's title now uses the active label of `focus_turns`: `[1] Turns` by default, and just `Turns` when the action has no key.
- **Hints.** A `turns_hints` function next to `browser_hints_at` in `src/app/hints.rs`, reading `ENTER to go to block | TAB to switch to transcript` with the default keys. `HINTS_ATTR` is pushed when the sidebar gets the focus, and `browser_hints_at(keymap, depth)` when it loses it.
- **Budget.** Add to `tests/budgets.rs` a sidebar key press: move the selection, scroll the transcript to the block, and draw both, on both 100 MB fixtures. It is checked against the 16 ms frame budget.

## Acceptance criteria
- [ ] `App::update` tests cover: `1`, `Tab` and `Shift-Tab` focusing the sidebar; `Tab`, `→` and `l` returning without moving the transcript; `Enter` moving focus to the block; an entry whose text renders to no rows; each `j` / `k` scrolling the transcript and moving its selection; no wrap-around at either end.
- [ ] Keymap tests show `1` and `Tab` still act on the selector there, and that `[keys.turns]` can be remapped.
- [ ] Tests cover focus returning after an overlay (help, theme selector, session selector) and focus moving when the sidebar becomes hidden, including while an overlay opened from the sidebar is open.
- [ ] A test shows the block's top border at the top of the viewport after a selection, or as near as the end of the transcript allows, the same result as `]` / `[`, for each entry of a fixture.
- [ ] A test shows moving away from the bottom stops following, and `↓ N new` appears when a turn then arrives.
- [ ] A test shows focusing the sidebar does not by itself scroll the transcript.
- [ ] Tests show that while the sidebar has focus, `/`, `y`, the fold keys, `1`, `←` and `h` do nothing, and `q`, `?`, `t` and `Ctrl-o` still work.
- [ ] Tests show that below 90 columns, and with `EmptySession`, `1`, `Tab` and `Shift-Tab` do nothing.
- [ ] Tests show that with no selection the first move in either direction selects the first entry, and that on an empty list the keys do nothing.
- [ ] A test shows that with the sidebar focused, an appended turn adds a row without moving the selection or the transcript.
- [ ] A test shows the help line's hints while the sidebar has focus are generated from the active keymap, including a remapped key, and that they change back when it loses focus.
- [ ] The help overlay lists the new keys in a "Turns sidebar" group, from the active keymap, with each row once.
- [ ] The action table in `src/keymap/tests.rs` includes every new binding, and an unknown context's warning lists `turns`.
- [ ] The README has the new rows and table, and `tests/release_docs.rs` passes.
- [ ] `cargo test --release -- --ignored budgets --nocapture` stays within budget, and a key press in the sidebar is measured under 16 ms on the 100 MB fixtures and printed.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] Snapshot changes are limited to the new sidebar focus snapshots and the help overlay snapshots that gain the group.

## UI
- **Focused sidebar.** The border is `ring`. The selected row is drawn as in §6.3: `>` in `primary` in column 3, and one inverted band from column 4 to column 26.
- **Help line while it has focus.** Hints from the active keymap, for example `ENTER to go to block | TAB to switch to transcript`, using `help_key` and `help_text`, at the bottom of the browser, full width.
- **Help overlay.** A new `Turns sidebar` group before `Application`, in the browser's help.
- **Title.** `[<key>] Turns` using the active key.

Screen snapshots at 120x40 and 90x24 in the `default` and `terminal` themes: the sidebar focused with the selection on User, Assistant and Compaction, with the transcript scrolled to that block. Help overlay snapshots at their existing sizes.

## Depends on
- 007-details-overlay
- 011-sidebar-follows-transcript
