# Details overlay

## Goal
Add the on-demand overlay that shows the metadata of the selected section, so blocks stay uncluttered. This task delivers the `details` key, the overlay component, its wiring into the app and the rows the model already knows. The next two tasks add the fields read from the harness's source.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §5:

An on-demand overlay shows the metadata of the selected section, so blocks stay uncluttered.

From SPEC.md §5.1:

- New browser action `details`, default key `i`, configurable under `[keys.browser]`, described as "Show section details" in the Sections help group.
- It opens for the selected section. A `… N more lines` row resolves to its parent tool call. With no selection it does nothing.
- It reuses `open_overlay` and the help overlay's frame, scrolling and overlay keys (base spec §4.3). The `details` key also closes it.
- It is a snapshot taken at opening and does not update while open.
- It is not part of copy, save or export.

From SPEC.md §5.2 (for context: built by the next two tasks):

Most of these fields vary across Claude Code versions and are rarely looked at, so they must not be parsed into the model.

- Turns store **only cheap source references**: a byte range for a Claude line, a row id for OpenCode. They never store the field values.
- A tool call has two references, for its call and its result. On Claude, permission, sandbox and `toolUseResult` data live on the result's line.
- The source is re-read when the overlay opens, following `extract_result_text` and `ByteSource` (`src/harness/claude/wire.rs`, `src/model/output.rs`).
- The harness adapter interface provides the read, so the UI stays harness-agnostic (base spec §1.1). Codex and Pi adapters can provide none.
- Reading `toolUseResult` for this view uses a typed head too (§3), and never loads the whole value.
- The §9.2 budgets must still be met.

From SPEC.md §5.3:

Rows are `label  value`, in the groups below, in this order. Groups that do not apply to the selected section are left out.

| Group | Applies to | Claude | OpenCode |
|---|---|---|---|
| Turn | Every section of an Assistant turn | Model, effort, start, duration, messages, tokens by input / output / thinking / cache read / cache write | The same, plus cost, provider (`model.providerID`) and agent |
| Message | Text, thinking, notice | `message.id`, `requestId`, `timestamp`, `requestedModel`, `stop_reason`, `service_tier`, `speed`, `inference_geo`, 5m and 1h cache writes, web search and fetch counts, `diagnostics.cache_miss_reason` | Row id, `time.created`, `time.streamed`, `time.completed`, `finish`, `rawFinish`, `error` |
| Tool call | Tool call | Start, duration, status, `permissionDecision`, `sandboxed`, `resolvedModel`, `isAsync` | Start, duration, status |
| Prompt | User prompt | `timestamp`, `permissionMode`, `promptSource` | `time.created` |
| Environment | Every section with a source | `gitBranch`, `cwd`, `version`, `entrypoint` | `snapshot.start`, `snapshot.end` |

- Timestamps use the configured format (base spec §8.6).
- Durations use the shared format (§2.4), and token counts use the existing format.
- Missing fields are left out. A group with no rows is left out.
- Claude records no cost, so none is shown. **Do not build a price table.**
- An unreadable source (file gone, row deleted, line unparseable) shows one muted `unavailable` row in the affected group. It never panics or closes the overlay.
- Compaction and system sections show what their source provides under Environment, plus their timestamp.

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

From spec/SPEC.md §1.1 (architecture):

```
            ┌───────────── adapters (one per harness) ─────────────┐
            │ claude: JSONL files     opencode: SQLite (read-only) │
            └──────────────┬──────────────────────┬────────────────┘
                 discover projects/sessions   load + watch session
                           ▼                      ▼
                    Session index         Normalised model (§5.1)
                           └──── worker threads ──┘
                                      │ channels, polled by Ports → Event::User
                                      ▼
  crossterm keys/mouse ──► tui-realm Application ──► focused / subscribed components (§1.3)
          ticks ─────────►   (routes Event)                      │ Msg
                                                                 ▼
                              App::update ◄──────────────────────┘
                                │  changes domain state, mounts/unmounts components,
                                │  moves focus, pushes data into components (attr)
                                ▼
                              App::view: screen layout only; Application::view(&Id, frame, area)
```

```rust
trait Harness {
    fn id(&self) -> HarnessId;
    fn available(&self) -> bool;                       // data store found
    fn projects(&self) -> Result<Vec<Project>>;
    fn sessions(&self, project: &ProjectId) -> Result<Vec<SessionSummary>>;
    fn load(&self, session: &SessionId, tx: Sender<ModelUpdate>) -> Result<WatchHandle>;
    // streams the initial parse, then incremental updates, as ModelUpdate events
    fn subagent(&self, parent: &SessionId, call: &ToolCallId) -> Option<SessionId>;
    fn resume_command(&self, session: &SessionSummary) -> Option<Command>;
}
```

- The UI is a tui-realm `Application` (§2) driven by a single-threaded loop: the loop calls `Application::tick`, passes every returned `Msg` to `App::update`, and redraws only when something changed (at most every 50 ms, §7.1).
- Parsing, watching, stats computation and search indexing run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used.

From spec/SPEC.md §1.2 (application structure):

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

From spec/SPEC.md §1.4 (events, focus and keys):

- **Focus:** exactly one component has focus at a time, set by `App::update` through `Application::active`. In the selector, the pane keys of §6.6 move focus between `ProjectList` and `SessionList`. A focused component shows it through `Attribute::Focus` (e.g. the `ring` border, §8.2).
- **Overlays and prompts:** opening one mounts its component and gives it focus. Closing it unmounts it and returns focus to the component that had it before.
- **Keys go through the keymap.** A component's `on()` resolves the key with the shared keymap (§8.5), for its own context, to an action. The action becomes a `Cmd` (handled internally with `perform`) or a `Msg`. Components never match literal `KeyEvent`s, except that `Prompt` takes printable characters as text. This keeps remapping and the generated help overlay (§4.3) correct.
- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, §6; `Esc` / `?` closes help). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (§6.6).
- **Mouse:** wheel and clicks act on the element under the pointer, whether or not it has focus (§6.9). The event goes to the topmost component drawn there, for example through mouse subscriptions whose row/column ranges are that component's area. That component turns the event into a `Cmd` or `Msg`.
  - While an overlay is open (selector, help, theme selector), the components under it get no mouse events, and a click outside the overlay closes it.
  - Component areas come from a pure layout function that `App::view` also uses. Subscriptions are refreshed from it when the screen or terminal size changes, never from inside `view`.
- **Ticks:** `Event::Tick` drives toast expiry, the `● live` marker and the redraw limit (§7.1).
- **Worker results** take one path: Port → `Event::User` → `GlobalEvents` (subscribed to all user events) turns it into a `Msg` → `App::update` applies it to domain state and pushes what changed to the components that display it (rows and stats to `ProjectList` / `SessionList`, `ModelUpdate`s to `Transcript`). Other components don't subscribe to worker events.

From spec/SPEC.md §4.3 (overlays):

- **Help overlay** (`?`): grouped keybindings as in ui-sketch.md, generated from the active keymap. `Esc` / `?` closes it.
- **Theme selector** (`t`): see §8.4.
- **Confirmation prompts** (overwrite file, resume in harness) appear in the help line as `y/n` questions, not as dialogs.

Overlays are centred, use rounded borders with `ring` colour, and paint `surface` behind them (unless the background is transparent, in which case `Clear` is still used so underlying text doesn't bleed through).

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

From spec/SPEC.md §5.5 (text rendering):

- Prose is word-wrapped. Commands and tool output are char-wrapped (as in the sketch) so paths and long tokens are never hidden.

From spec/SPEC.md §6.9 (mouse):

- Wheel scrolls the transcript or the list under the pointer.
- Click selects a section or list row. Clicking a section's summary line (or the `▶`/`▼` marker) toggles its fold. Double-clicking a session row opens it.
- Click on a harness tab switches to it. Click outside an overlay closes it.
- Mouse events go to the component under the pointer (§1.4).
- Mouse capture can be disabled in config (`mouse = false`). Most terminals also bypass mouse capture while `Shift` is held, which allows native text selection; the help overlay mentions this.

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

From spec/SPEC.md §8.5 (key syntax):

Key syntax: `a`, `A`, `ctrl-d`, `shift-tab`, `enter`, `esc`, `pgdn`, `f5`, and two-key chords as space-separated keys (`"g g"`; only in user config, since no default binding is a chord). The help overlay is generated from the active keymap, so it always reflects the user's remappings.

From spec/SPEC.md §8.6 (timestamps):

- Displayed in local time using `timestamp_format`. With `utc = true`, UTC is shown with a `Z` suffix.
- The selector's session rows use the same format without seconds.

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

- **Component.** A `DetailsOverlay` in `src/components/details_overlay/`, with `Id::DetailsOverlay`. It is built from plain data: a `DetailsContent` of groups, each with a title and rows (`label`, `value`, whether the value is muted). So its snapshots can show every group fully populated without any harness.
- **Reuse, don't copy.** Extract the frame and scrolling of `HelpOverlay` (`src/components/help_overlay/mod.rs`: the rounded `ring` frame, the `surface` fill, the ` ↑ more ` and ` ↓ more ` indicators, `WHEEL_ROWS`, `INTERIOR_ATTR`, the overlay-context keys) into a shared type that both overlays use. The help overlay's existing snapshots must show no change other than the new `details` row.
- **Keys.** `Action::Details`, name `details`, context `Browser`, keys `["i"]`, group `Sections`, description `Show section details`. Inside the overlay, `Context::Overlay` is resolved first, as in `HelpOverlay::on`; then the active `Browser` `details` key closes it. `?` does not close it (it is not an overlay key), and the global keys stay off while it is mounted, except `Ctrl-c`.
- **README.** `tests/release_docs.rs::readme_keybindings_match_the_default_keymap` fails `cargo test` when a default binding is missing from the README, and every task must pass `cargo test`. So this task adds the row ``| `details` | `i` | Show section details |`` to the README's browser table, even though §8 lists it under the final documentation task.
- **Integration points, all of which need the new overlay:** `OVERLAYS`, `takes_mouse` and `outside_overlay` in `src/app/mouse.rs`; `key_clause` in `src/components/global_events.rs`; a field in `LayoutInput` and `layout()` using `help_overlay_rect` in `src/app/layout.rs`; `layout_input()` in `src/app/mod.rs`; `push_help_interior`'s equivalent called from `push_viewports` so the interior is announced on resize; the `THEMED` list in `src/app/theme_selector.rs`; the `drop_help` call in `src/app/browser.rs` (an open overlay is dropped without returning focus when the screen under it is replaced); the action table in `src/keymap/tests.rs`.
- **Who builds what.** The transcript resolves the key and sends a `Msg` carrying what the `App` needs, as `CopyText` does, so the `App` never reaches into the transcript. It maps a `SectionKey::More` selection with `SectionId::more_parent()`. With no selected section it answers nothing. `Msg` derives `Eq` and `TurnStats` holds an `f64`, so the message carries formatted strings (or an `Arc` compared by sequence number, spec/SPEC.md §1.2), never a bare `TurnStats`.
- **Rows in this task (from the model only).** Labels are lower case. The spec's table names the fields; where it names a raw field, the label is that name.

  | Group | Rows, in order |
  |---|---|
  | Turn | `model`, `effort`, `start`, `duration`, `messages`, `input tokens`, `output tokens`, `thinking tokens`, `cache read tokens`, `cache write tokens`, `cost` |
  | Tool call | `duration`, `status` (`pending`, `ok` or `error`) |
  | Prompt | `timestamp` |
  | Environment (compaction and system sections) | `timestamp` |

  - `thinking tokens` is `TokenUsage.reasoning`. A token row whose value is zero is left out, because the model cannot tell zero from not recorded. `cost` uses the footer's format (`$0.42`) and appears only when known, so never for Claude. **Do not build a price table.**
  - `duration` uses `crate::duration::format_duration`, including for tool calls. Token counts use `format_tokens`. `start` and `timestamp` use the transcript's `TimestampFormatter`.
  - A system section's timestamp is its first entry's.
  - The Message group, the remaining rows of Prompt and Tool call, and Environment's fields come from the harness in tasks 008 and 009.
- **Layout of the content.** As in the help overlay: a blank first row, then per group a bold `foreground` heading at overlay column 3, a blank row, its rows at column 5, a blank row. Label in the `label` token, value in `foreground`, muted rows in `muted_foreground`. Values start in one column: the widest label plus two spaces. A value wider than the interior is char-wrapped with a hanging indent at the value column, so paths and long tokens are never hidden (spec/SPEC.md §5.5). The row count follows the announced interior width.
- **Wanted size.** As wide as the widest row plus 8 columns of chrome, at least 40, and as high as the content plus the border. `help_overlay_rect` caps it to the screen.

## Acceptance criteria
- [ ] Keymap tests and the action table in `src/keymap/tests.rs` include `details`. Help content lists it from the active keymap.
- [ ] Component snapshots cover one section of each kind at 80x24 and 120x40, in the `default` and `terminal` themes, including content taller than the screen. The kinds are user prompt, assistant text, thinking, tool call, `model → …` note, stats footer, compaction, system and notice, each with the groups that apply to it, fully populated from plain data.
- [ ] `App::update` tests cover opening, closing with both keys, focus returning, a `More` row resolving to its tool call, and doing nothing without a selection.
- [ ] A remapped `details` key opens and closes the overlay, and the help overlay shows the remapped key.
- [ ] A test shows the overlay is a snapshot: a `ModelUpdate` that changes the section's turn while it is open does not change what it shows.
- [ ] A test shows copy, save and export do not include the overlay's content and their keys do nothing while it is open.
- [ ] Mouse tests show the wheel scrolls the overlay, a left press outside closes it and returns the focus, and components under it get no mouse events.
- [ ] A test shows the global keys `q`, `t`, `?` and `Ctrl-o` do nothing while it is open, and `Ctrl-c` quits.
- [ ] Row tests cover: groups in the table's order, a group with no rows left out, a missing field left out, no `cost` row without a cost, and the Turn group present for every section of an Assistant turn and absent for a User turn's sections and for a compaction.
- [ ] `HelpOverlay` and `DetailsOverlay` share one frame and scrolling implementation, and the help overlay's snapshots differ only by the new row.
- [ ] The README's browser table has the `details` row, and `tests/release_docs.rs` passes.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] `cargo test --release -- --ignored budgets --nocapture` stays within budget.
- [ ] Snapshot changes are limited to the new overlay snapshots and the help overlay snapshots that gain the `details` row.

## UI
A centred rounded overlay over the browser, titled `Details`, with a `ring` border and `surface` behind it (only `Clear` with a transparent theme), the same frame as the help overlay.

```
╭─ Details ──────────────────────────────────────────╮
│                                                    │
│  Turn                                              │
│                                                    │
│    model              claude-opus-5-5              │
│    effort             high                         │
│    start              2026-10-21 08:35:18          │
│    duration           4m 3s                        │
│    messages           12                           │
│    input tokens       2.8k                         │
│    output tokens      42.4k                        │
│    cache read tokens  359.4M                       │
│                                                    │
│  Tool call                                         │
│                                                    │
│    duration           1m 2s                        │
│    status             ok                           │
│                                                    │
╰────────────────────────────────────────────────────╯
```

- Headings: bold `foreground`. Labels: `label`. Values: `foreground`. Muted rows: `muted_foreground`.
- When the content is taller than the interior, ` ↑ more ` and ` ↓ more ` appear on the borders in `muted_foreground`, as in the help overlay.
- Keys: up and down keys scroll a row, page keys a page, `Esc` and the `details` key close. The wheel scrolls `WHEEL_ROWS` rows. A left press outside closes it.
- Focus: it takes the focus on opening and returns it on closing (spec/SPEC.md §1.4).
- A long value wraps inside the overlay, aligned to the value column.

Snapshot sizes: 80x24 and 120x40, in the `default` and `terminal` themes, one section of each kind, including content taller than the screen at 80x24 scrolled to the top and to the end.

## Depends on
- 001-shared-duration-format
- 002-turn-token-usage
- 003-claude-turn-stats
- 005-opencode-error-notices
