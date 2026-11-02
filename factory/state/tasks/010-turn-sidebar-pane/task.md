# Turns sidebar pane

## Goal
Show the shape of a session at a glance: a fixed sidebar to the left of the transcript lists the turns and compaction events, with Assistant durations. This task delivers the component, its rows, the layout and the live row updates. The sidebar cannot take focus yet; following the transcript, keys and mouse come in the next three tasks.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §6:

A sidebar in the session browser lists the session's turns and compaction events, so the shape of a long session is visible and any block is a few keys away.

```
╭─ [1] Turns  ───────────────╮
│                            │
│                            │
│  ●   Start                 │
│  │                         │
│  ●   User                  │
│  │                         │
│  ●   Assistant      4m 3s  │
│  │                         │
│  ●   User                  │
│  │                         │
│  ●   Assistant      3h 4m  │
│                            │
│  ═══ Compaction ═════════  │
│                            │
│  ●   Assistant     2h 45m  │
│  │                         │
│  ●   End                   │
│                            │
╰────────────────────────────╯
```

From SPEC.md §6.1:

- A new `TurnSidebar` component (base spec §1.3), one mounted instance for the browser.
- It sits to the left of the transcript, below the header. Header and help line stay full width.
- It is **30 columns wide, with a 1-column gap** before the transcript, and exactly as tall as the transcript area.
- **It stays in a fixed position:** scrolling the transcript never moves the pane. Only its own list scrolls, and only as far as needed to keep the selected row visible, as in the selector's lists.
- The areas come from the pure layout function (base spec §1.4), so drawing and mouse routing agree. The transcript's viewport and its mouse coordinates use the narrower area.
- **Below 90 columns the sidebar is hidden** and the transcript takes the full width as today, because 90 is what leaves the transcript 59 columns. While hidden, the focus keys do nothing. If it has focus when it becomes hidden, focus moves to the transcript.
- It is not drawn when `EmptySession` replaces the transcript. An open subagent shows that subagent's turns, and going back shows the parent's again.

From SPEC.md §6.2:

There is one entry per turn with at least one non-system section, in transcript order, taken from `derive_sections` (`src/components/turn_block/section.rs`). This matches the blocks `]` / `[` can stop on, because system content never forms a block of its own, so toggling it (`s`) does not change the entries. Do not call `turn_stop` for every turn to build the list: it renders the block, which §6.6 forbids.

| Row | Content | Selectable |
|---|---|---|
| `● Start` | First row of the list | No: it has no block |
| `● User` | A User turn | Yes |
| `● Assistant` | An Assistant turn, with its duration (§6.5) | Yes |
| `═══ Compaction ═══` | A Compaction turn; `═` fills to the right margin | Yes |
| `● End` | Last row of the list | No: it has no block |

- Columns (from 0): `●`, `═` and the marker in column 3; labels in column 7, except that the Compaction rule runs from column 3; the right margin is two cells before the border, as in the selector.
- Between two `●` rows there is one row with `│` in column 3.
- A Compaction row has a blank row above and below it instead of `│`, so the timeline visibly breaks.
- The list starts two rows below the top border.
- Unselected rows, `●`, `│` and `═` use `muted_foreground`.
- Long lists scroll by rows. `Start` and `End` scroll with the list. Connector and blank rows cannot be selected or clicked.

From SPEC.md §6.3:

Selection looks and behaves like the session selector's lists (base spec §8.2, §4.1). Reuse `list_pane` where it fits.

| State | Appearance |
|---|---|
| Selected, sidebar focused | `>` in `primary` replaces the `●` (or the first `═`) in column 3, never inverted. Everything after it is one inverted band (`foreground` with reverse video, not bold) up to the right margin; the label, gaps and duration all take the band's style |
| Selected, sidebar unfocused | `>` in `secondary_foreground`, text bold in `secondary_foreground`, not inverted |
| Border | `ring` when focused, `border` otherwise |

- The list never wraps around.
- **While the transcript has focus, the sidebar's selection follows it:** the selected row is the turn containing the transcript's selected section, and it stays visible. This keeps the sidebar showing the current position, and returning to it never jumps. Pushing that selection to the sidebar produces no message.
- When the sidebar has focus and nothing is selected, it selects the first entry on the first move. An empty list has no selection, and its keys do nothing.

From SPEC.md §6.5:

Each Assistant entry shows `TurnStats.duration` in the shared format (§2.4), right-aligned to the right margin. Turns without stats (for example, still running) show none, and it appears in place once known.

From SPEC.md §6.6:

Sessions can have thousands of turns (base spec §9.2). Building rows must not lay out or render transcript blocks, and turn updates must not rebuild more than the rows that changed. The sidebar draws only the rows it shows, and a key press in the sidebar stays within the frame budget on the 100 MB fixture.

From SPEC.md §0 (the wireframe):

**Wireframe.** [cycle-2-ui-sketch.md](spec/cycle-2-ui-sketch.md) is the wireframe for the turns sidebar (§6). It defines only the sidebar and where it sits. Its transcript content is illustrative: fold markers, tool rows and everything else in the transcript follow the base spec and this document.

From cycle-2-ui-sketch.md (the wireframe, with its left margin removed):

```text
╭─ Project BRIO ──────────────────────────────────────────────────────────────────────────────────── 85a86657─11a7─4318─a21a─0f4476babd5c ─╮
│                                                                                                                                          │
│ /home/jacob/Development/projects/BRIO                                                                                                    │
│                                                                                                                                          │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭─ [1] Turns  ───────────────╮ ╭─ User ───────────────────────────────────────────────────────────────────────── 2026─10─21 08:35:15.800Z ─╮
│                            │ │                                                                                                           │
│                            │ │ - I have a Logitech BRIO web camera attached to this machine via USB. It works fine - both                │
│  ●   Start                 │ │   camera and microphone - but I don't have a way to access settings that would have been                  │
│  │                         │ │   available on say a mac or windows, since there are no proprietary drivers: we just get                  │
│  ●   User                  │ │   the vanilla USB profiles. Can you help me investigate what additional profiles the                      │
│  │                         │ │   camera has and how to access them? The ideal solution would be to be able to write an                   │
│  ●   Assistant      4m 3s  │ │   app that allowed me to change the settings, if possible. However, before we get to that                 │
│  │                         │ │                                                                                                           │
│  ●   User                  │ ╰───────────────────────────────────────────────────────────────────────────────────────────────────────────╯
│  │                         │ ╭─ Assistant ── claude─opus─5.5 @ high  ───────────────────────────────────────── 2026─10─21 08:35:18.271Z ─╮
│  ●   Assistant      3h 4m  │ │                                                                                                           │
│                            │ │ I'll start by looking at what the camera exposes over USB and what Linux's UVC driver makes of it.        │
│  ═══ Compaction ═════════  │ │                                                                                                           │
│                            │ │ > Find the BRIO on USB and check available tools                                                          │
│  ●   Assistant     2h 45m  │ │                                                                                                           │
│  │                         │ │ - Dump full USB descriptors for the BRIO                                                                  │
│  ●   End                   │ │                                                                                                           │
│                            │ │   Command:                                                                                                │
│                            │ │                                                                                                           │
│                            │ ┃      sudo -n true 2>&1 && echo SUDO_OK; lsusb -v -d 046d:085e >                                           │
│                            │ ┃      /tmp/claude-1000/-home-jacob-Development-projects-brio/85a86657-11a7-4318-a21a-0f4476babd5c/scratc   │
│                            │ ┃      hpad/lsusb.txt 2>&1; wc -l                                                                           │
│                            │ ┃      /tmp/claude-1000/-home-jacob-Development-projects-brio/85a86657-11a7-4318-a21a-0f4476babd5c/scratc   │
│                            │ ┃      hpad/lsusb.txt; grep -E                                                                              │
│                            │ ┃      'bInterfaceClass|bInterfaceSubClass|bInterfaceNumber|iInterface|bcdUVC|bcdDevice|iProduct|iSerial|   │
│                            │ ┃      guidExtensionCode|bUnitID|bTerminalID|wTerminalType|bNumControls|bmControls|bControlSize|Unit        │
│                            │ ┃      Descriptor|VideoControl Interface Descriptor|bDescriptorSubtype|Couldn'                              │
│                            │ ┃      /tmp/claude-1000/-home-jacob-Development-projects-brio/85a86657-11a7-4318-a21a-0f4476babd5c/scratc   │
│                            │ ┃      hpad/lsusb.txt | head -200                                                                           │
│                            │ │                                                                                                           │
│                            │ │   Result:                                                                                                 │
│                            │ │                                                                                                           │
│                            │ │      sudo: a password is required                                                                         │
│                            │ │      1799                                                                                                 │
│                            │ │      /tmp/claude-1000/-home-jacob-Development-projects-brio/85a86657-11a7-4318-a21a-0f4476babd5c/scratc   │
│                            │ │      hpad/lsusb.txt                                                                                       │
│                            │ │      Couldn't open device, some information will be missing                                               │
│                            │ │        bcdDevice            0.17                                                                          │
│                            │ │        iProduct                2 Logitech BRIO                                                            │
│                            │ │        iSerial                 3 8DF1E759                                                                 │
│                            │ │            bInterfaceNumber        0                                                                      │
│                            │ │            bInterfaceClass        14 Video                                                                │
│                            │ │            bInterfaceSubClass      1 Video Control                                                        │
│                            │ │            iInterface              0                                                                      │
│                            │ │            VideoControl Interface Descriptor:                                                             │
│                            │ │                                                                                                           │
╰────────────────────────────╯ ╰───────────────────────────────────────────────────────────────────────────────────────────────────────────╯
```

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

From spec/SPEC.md §1.3 (the transcript):

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (§9.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (§5.2), one body renderer per `ToolKind` (§5.3), the markdown renderer (§5.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (§5.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

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

From spec/SPEC.md §4.2 (session browser):

- Header block: `Project <name>` (left title), session id (right title), working directory in the body. A `● live` marker shows while the session is active (§7.2), and a breadcrumb shows when viewing a subagent (§5.4).
- Transcript: one rounded block per turn (§5.2), scrolling vertically, with the selected section marked by `▒` over the block's left border.
- Help line under the transcript: context-sensitive key hints. It turns into the search/path prompt when active and shows search position (`3/17`), the `↓ N new` indicator and parse warnings.
- Toasts (copy/save/export confirmations, config warnings) appear in the bottom-right corner for ~2 s.
- Loading state: `Loading… 34%` in the help line while the initial parse streams in.
- Empty states: "No sessions for this harness" / "Session file is empty".

From spec/SPEC.md §5.6 (system content never forms a block):

- It is rendered as **System sections inside the adjacent turn**: in the next turn when it precedes a prompt, in the current turn otherwise. Consecutive injected entries collapse into one section with a count (`3 injected items`). They never become turn blocks of their own, so `[` / `]` only stop at User, Assistant and Compaction blocks.

From spec/SPEC.md §7.2:

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

- **Component.** `TurnSidebar` in `src/components/turn_sidebar/`, a `pub` type buildable from plain data and the theme (spec/SPEC.md §1.3), with `Id::TurnSidebar`. Add it to `BROWSER_IDS` in `src/app/browser.rs` and to `THEMED` in `src/app/theme_selector.rs`.
- **Layout.** Constants for the width (30), the gap (1) and the threshold (90) in `src/app/layout.rs`. `BrowserAreas` gains the sidebar's optional area, and `browser_areas` takes what it needs to decide. `push_viewports` in `src/app/selector.rs` calls `browser_areas(area).transcript` and must get the same narrower area as drawing and mouse routing. In `layout()`, the sidebar comes before the transcript in drawing order. `EmptySession` keeps the full width.
- **Row data.** A pure row builder in its own file, tested on its own. An entry exists when `derive_sections(turn, false)` is not empty (`false` leaves system content out, so `s` cannot change the entries). It never calls `turn_stop`, which renders the block, and never materialises tool output. A notice counts as a non-system section, and so does the stats footer: a turn holding only redacted thinking but with a `turn_duration` has an entry, which is where `]` stops too. The row test for "only redacted thinking" therefore uses a turn without stats.
- **Updates.** `ModelUpdate::Appended` and `Updated` are forwarded to the sidebar along with the transcript, and it keeps no `Arc<Turn>`. Traps:
  - `Appended` can carry a turn whose id is already held, which the transcript treats as an update of that turn (see the doc comment on `ModelUpdate::Appended`). The sidebar must do the same.
  - An `Updated` turn can gain its first non-system section, gain a duration, or (rarely) lose its entry. It must keep its place in transcript order, so keep one slot per turn, including turns without an entry, and find a turn's slot through a map rather than a scan.
  - `Reset` clears the rows.
- **Subagents.** The sidebar always shows the transcript on top (`top_transcript`), and the `Transcript` owns the turns (spec/SPEC.md §1.2). Recommended: when the transcript on top changes, the `App` asks that transcript for its rows through a `query` attribute, as it does for `TURN_COUNT_ATTR`, and pushes them. That is one pass over the turns on an explicit user action, not on a turn update. Updates of a covered load do not reach the sidebar, and the rebuild on going back picks them up. No `Rc<RefCell<…>>` or `Arc<Mutex<…>>`.
- **Row geometry.** Columns are counted from the pane's left border, column 0, for the 30-column pane: `●`, `═` and the marker in column 3; labels in column 7; the right margin ends at column 26, leaving columns 27 and 28 blank before the border in column 29. List rows start on pane row 3 (border, then two blank rows), so the pane shows `height - 4` list rows. `visible_rows` in `list_pane.rs` assumes one blank row, so do not reuse it unchanged.
- **Title.** `╭─ [1] Turns  ───…╮`, with two spaces after `Turns` as in the wireframe. In this task the key is a prop defaulting to `1`; task 012 passes the active key's label.
- **Selection and focus.** The states of §6.3 are built and tested here through `Attribute::Focus` and a selection attribute. In the running app this task shows no selection and no focus. The next tasks drive them. `ListCursor` works on rows; the sidebar's selection is an entry, so map it to its row and reuse `scrolled_offset`-style reveal.
- **Existing tests.** Every browser screen snapshot at 90 columns or wider changes, intentionally. Review each one. Tests with absolute mouse coordinates in `src/app/mouse_tests.rs` and `src/app/browser_tests/` shift by 31 columns at those widths, so update them without changing what they assert.

## Acceptance criteria
- [ ] Layout tests cover widths 89 and 90, the 30-column pane and 1-column gap, full-width header and help line, and mouse routing to the right component. `hit_test` finds `Id::TurnSidebar` in the pane, nothing in the gap, and the transcript beyond it.
- [ ] Component snapshots at 120x40 and 90x24 in the `default` and `terminal` themes cover: focused and unfocused selection on each selectable kind, no selection, a list longer than the pane scrolled to each end, and the widest durations.
- [ ] Row tests cover: each kind and its order, `Start` and `End` not selectable, connector rows around a Compaction, a turn with only redacted thinking or only system content having no entry, toggling system content leaving the entries unchanged, and an Assistant turn gaining a duration.
- [ ] Row tests cover: two adjacent Compactions with one blank row between them; a Compaction right after `Start` and right before `End`; a turn with only a notice having an entry; an `Updated` turn gaining its first non-system section appearing in its transcript position; an `Appended` turn with a known id updating its row rather than adding one.
- [ ] A test shows an Assistant entry without stats shows no duration, and its duration appears in place once known, in the shared format, ending at the right margin.
- [ ] Tests show building rows neither lays out nor renders: no tool output is materialised (`text_calls`, `source_reads` unchanged) and `turn_stop` is not reached.
- [ ] A test shows a turn update rebuilds only what changed: updating one turn of a list of thousands does not touch other rows, measured by a counter or by the row storage's identity.
- [ ] A test shows the sidebar draws only the rows it shows (a list of 10 000 turns draws within the frame budget, or a counter shows only the visible rows are built).
- [ ] App tests show: below 90 columns it is not in the layout and the transcript has the full width and its viewport; at 90 the transcript's viewport is 59 columns wide; it is not drawn when `EmptySession` replaces the transcript; scrolling the transcript does not move the pane.
- [ ] App tests show an open subagent shows that subagent's turns, and going back shows the parent's, including turns the parent received while covered.
- [ ] A theme preview repaints the sidebar.
- [ ] Mouse tests show a click on the transcript at 120 columns hits the same section as before, with coordinates shifted by the sidebar.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] `cargo test --release -- --ignored budgets --nocapture` stays within budget, with rows built during the load.
- [ ] Snapshot changes are limited to the new sidebar snapshots and browser screen snapshots at 90 columns or wider. Snapshots below 90 columns do not change.

## UI
The pane, as in §6 and the wireframe, at 30 columns wide with a 1-column gap, below the header, exactly as tall as the transcript area:

```
╭─ [1] Turns  ───────────────╮
│                            │
│                            │
│  ●   Start                 │
│  │                         │
│  ●   User                  │
│  │                         │
│  ●   Assistant      4m 3s  │
│  │                         │
│  ●   User                  │
│  │                         │
│  ●   Assistant      3h 4m  │
│                            │
│  ═══ Compaction ═════════  │
│                            │
│  ●   Assistant     2h 45m  │
│  │                         │
│  ●   End                   │
│                            │
╰────────────────────────────╯
```

- Border: `ring` when focused, `border` otherwise. Title text: `foreground`.
- Unselected rows, `●`, `│` and `═`: `muted_foreground`. The Compaction row reads `═══ Compaction ═…`, with `═` from column 3 to column 5, the label at column 7 and `═` from column 18 to the right margin.
- Duration: right-aligned, ending at column 26.
- Selected, focused: `>` in `primary` replaces the `●` (or the first `═`) in column 3, never inverted. From column 4 to column 26 is one inverted band (`foreground` with reverse video, not bold), and the label, gaps and duration take the band's style.
- Selected, unfocused: `>` in `secondary_foreground`; the rest of the row's text bold in `secondary_foreground`, not inverted.
- A list longer than the pane scrolls by rows, keeping the selected row visible with the least scrolling. `Start` and `End` scroll with the list.
- Hidden below 90 columns, and when `EmptySession` replaces the transcript.
- Header and help line stay full width.

Snapshot sizes: 120x40 and 90x24, in the `default` and `terminal` themes: focused and unfocused selection on User, Assistant and Compaction; no selection; a long list at the top and at the end; the widest durations. Screen snapshots at 120x40 for the browser with the sidebar, and at 89 columns without it.

## Depends on
- 001-shared-duration-format
- 003-claude-turn-stats
- 005-opencode-error-notices
