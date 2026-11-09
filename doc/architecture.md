# Chronoscope — Architecture

This file says how Chronoscope is built: the data flow from the harness adapters to the screen, the application structure, the components and how events reach them, and the tech stack. It is a living document: a cycle's docs task updates it when the cycle changes the architecture. Most of it describes what exists today: the diagram, the component table, the routing and the crate list are context, and adding to them is not a breach. Its rules are the statements of what must or must not be done, such as the **Rules** under [Application structure](#application-structure). Rules are binding, and a rule that starts with `Prefer:` is a preference. `product §n` is a section of [product.md](product.md). How to write the code is in [coding.md](coding.md), and how to test it in [testing.md](testing.md).

## Data flow

```
            ┌───────────── adapters (one per harness) ─────────────┐
            │ claude: JSONL files     opencode: SQLite (read-only) │
            └──────────────┬──────────────────────┬────────────────┘
                 discover projects/sessions   look up subagent, load + watch session
                           ▼                      ▼
                    Session index         Normalised model (product §4.1)
                           └──── worker threads ──┘
                                      │ channels, polled by Ports → Event::User
                                      ▼
  crossterm keys ────────► tui-realm Application ──► focused / subscribed components (Components)
  resize, ticks, mouse ──►   (routes Event)          GlobalEvents: global keys, resize, tick,
                                                     worker events; mouse → Msg::Pointer
                                                                 │ Msg
                                                                 ▼
          ┌─────────────► App::update ◄──────────────────────────┘
          │                 │  changes domain state, mounts/unmounts components,
          │                 │  moves focus, pushes data into components (attr)
          │                 │
          │ queued Msg      │  Msg::Pointer → router: hit_test over the pure layout
          │ (emit)          │  → component under the pointer: on(UserEvent::Pointer),
          └─────────────────┤    in its own coordinates → its Msg
                            ▼
                          App::view: screen layout only; Application::view(&Id, frame, area)
```

```rust
trait Harness: Send + Sync {
    fn id(&self) -> HarnessId;
    fn available(&self) -> bool;                       // data store found
    fn refresh(&self) -> Result<()>;
    // re-reads the store so that projects and sessions reflect what is on disk now (worker)
    fn watch_paths(&self) -> Vec<PathBuf>;             // what the index worker watches, recursively
    fn watch_files(&self) -> Vec<PathBuf>;             // those of them that are single files;
                                                       // default: none
    fn projects(&self) -> Result<Vec<Project>>;
    fn sessions(&self, project: &ProjectId) -> Result<Vec<SessionSummary>>;
    fn stats(&self, session: &SessionSummary) -> Result<SessionStats>;  // full parse (worker)
    fn stats_key(&self, session: &SessionSummary) -> String;
    // keys the stats cache: changes whenever the data the stats depend on does; never reads files
    fn load(&self, session: &SessionId, tx: Sender<ModelUpdate>) -> Result<WatchHandle>;
    // streams the initial parse, then incremental updates, as ModelUpdate events
    fn subagent(&self, parent: &SessionId, call: &ToolCallId) -> Option<SessionId>;
    // may list directories, read files or scan the database: called on a worker
    fn resume_command(&self, session: &SessionSummary) -> Option<Command>;
    fn section_details(&self, source: &SectionSource) -> Vec<SourceGroup>;
    // re-reads a section's metadata from its source for the details overlay (product §3.3, §4.1);
    // default: nothing, which the Codex and Pi adapters would inherit
    fn open_file(&self, path: &Path) -> Result<SessionSummary>;
    // reads a transcript file given on the command line (product §5.8);
    // default: an error, the harness's sessions are not files
}
```

- The UI is a tui-realm `Application` ([Tech stack](#tech-stack)) driven by a single-threaded loop (`run_loop` in `src/app/runtime.rs`). Each iteration calls `Application::tick` and hands every returned `Msg` to `App::handle`, which passes it to `App::update` and then handles the messages that `update` queued (`emit`), in order, before the next polled message. The loop redraws only when something changed, at most every 50 ms, except a redraw that follows a resize, which is drawn at once so the screen is never shown at a stale size (`RedrawLimiter` and `RedrawRequest::Resized` in `src/app/redraw.rs`, product §6.1).
- Parsing, watching, stats computation, search indexing and the subagent lookup run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used. (Reading the details of one section for the overlay is an exception, like materialising a large output when it is unfolded: it re-reads one or two lines or rows on the UI thread, product §8.2.)
- The subagent lookup (`Harness::subagent`) may list directories, read `*.meta.json` files, stream the parent transcript or scan the parent's rows in the database, so `App::update` never calls it. Opening a subagent starts a worker that looks it up (`look_up_subagent` in `src/app/session_port.rs`). Its answer comes back over the session port as `UserEvent::Subagent { generation, found }`, and an answer to a lookup that is no longer current is dropped by the same generation check as load results.

## Application structure

The UI follows tui-realm's model: components turn events into messages, and one update function acts on them.

- **`App`** owns the `Application` and the domain state: config, theme registry and active theme, keymap, session index and stats, the open session's summary, watch handle and subagent stack, worker handles and the clipboard.
- **`Id`**: one enum variant per mountable component ([Components](#components)). One component type may be mounted under several `Id`s: `BackgroundFill` as `SidebarGap` and `ScreenBackground`, `HelpLine` as `HelpLine` and `BrowserHelpLine`, `EmptyState` as `EmptyState` and `EmptySession`, and `Transcript` as `Transcript` and `Subagent(depth)`.
- **`Msg`**: what happened, in application terms (`OpenSession(harness, id)`, `CopyText(text)`, `Preview(id)`, `Quit`, …), never raw keys. It is split into nested enums, one per concern. The top level holds only what is global (`Start`, `Quit`, `Resized`, `Tick`, `ToastExpired`) and `Pointer`. Everything else is in the nested enum of its concern, which that concern's handler module defines: `Selector(SelectorMsg)`, `Browser(BrowserMsg)`, `Transcript(TranscriptMsg)`, `Overlay(OverlayMsg)`, `Theme(ThemeMsg)`, `Prompt(PromptMsg)`, `Output(OutputMsg)` and `Worker(WorkerMsg)`. A new message goes into the enum of its concern, not the top level. `Msg::Pointer`, the raw pointer event that `GlobalEvents` hands to `update`, is the one `Msg` that is not in application terms, and it exists for the pointer routing only ([Events, focus and keys](#events-focus-and-keys)).
- **The prompt says what it is for.** The `Prompt` is mounted with a `PromptPurpose`: `SelectorSearch`, `TranscriptSearch`, `SavePath`, `Overwrite` or `Resume`. Every `PromptMsg` it sends carries that purpose (`Changed`, `Submit`, `Cancel`, `Nav` and `Confirm`), except `Moved`, which only asks for a redraw. A `y/n` prompt answers `Confirm(purpose, bool)`. The prompt's dispatcher hands each message to the owner its purpose names; it never picks the owner from the `App`'s own state, and no answer is compared with text such as `"y"`.
- **`UserEvent`**: what the Ports deliver: `Index` (session index snapshots and stats), `Search` (the selector's search results), `TranscriptSearch` (the transcript search's matches), `Session { generation, update }` (a load's `ModelUpdate`s) and `Subagent { generation, found }` (the subagent lookup's answer). The pointer router builds three more and gives each to one component, never through a Port: `Pointer(RoutedMouse)`, `PointerOutside` and `PointerElsewhere` ([Events, focus and keys](#events-focus-and-keys)). tui-realm requires `UserEvent: Eq + Clone`, so large payloads are `Arc`-wrapped and compared by a sequence number.
- **`App::new`** is construction: it builds the `Application`, mounts the components every screen needs (`GlobalEvents`, the selector's components, `EmptyState`) and sets the first focus. `run_app` then hands the app its workers, harnesses, clipboard and session channel.
- **Startup goes through `update`.** `run_app` resolves the command line before the terminal is set up, then starts the application with one message, `Msg::Start(Box<Startup>)`: the startup warnings' toast, the preselection, the lists the command line was resolved against, and the session to open or the sessions to list. The loop then sends `Msg::Resized` with the terminal size. The handlers that start it (`set_preselection`, `seed_index`, `open_summary`, `show_filtered`, `show_toast`) are not `pub`: only `update` reaches them. The `pub` setters (`set_harnesses`, `set_session_sink`, `set_index`, the workers and the clipboard) are wiring that `run_app` calls around startup, and `set_clock` and `set_click_clock` are the tests' hooks for a fixed clock.
- **`App::update(Msg)`** is the only place that changes domain state, mounts and unmounts components, moves focus and pushes new data into components. It is a dispatcher, with no guards on its arms: each global message calls a small handler, and each nested enum goes to the dispatcher of its concern (`selector_msg`, `browser_msg`, …), which calls a small handler. It never interprets keys and never calls itself: a message it makes for itself (a session the selector opens) and a component's answer to a pointer event are queued with `emit`, and `App::handle` handles them.
- **Handler modules** in `src/app.rs` and `src/app/`, by concern:

  | Concern | Messages | Modules |
  |---|---|---|
  | Global | `Start`, `Quit`, `Resized`, `Tick`, `ToastExpired` | `src/app.rs`, `runtime.rs` (`Start`) |
  | Selector | `SelectorMsg` | `selector.rs` (dispatcher), `search.rs`, `resume.rs` (`R` on a session) |
  | Browser | `BrowserMsg`: opening a session and the updates of its load, the selector over the browser, subagents, the turns sidebar | `browser.rs` (dispatcher), `sidebar.rs` |
  | Transcript | `TranscriptMsg`: what the transcript on top reports of its view, and its search | `transcript.rs` (dispatcher), `browser.rs`, `transcript_search.rs` |
  | Overlay | `OverlayMsg`: the help and details overlays | `overlay.rs` (dispatcher), `help.rs`, `details.rs` |
  | Theme | `ThemeMsg` | `theme_selector.rs` |
  | Prompt | `PromptMsg` | `prompt.rs` (dispatcher), and the owner each purpose names: `search.rs`, `transcript_search.rs`, `save.rs`, `resume.rs` |
  | Output | `OutputMsg`: copy, save, export, resume | `output.rs` (dispatcher), `copy.rs`, `save.rs`, `resume.rs` |
  | Worker | `WorkerMsg`: the session index and the two searches | `worker.rs` (dispatcher), `selector.rs`, `search.rs`, `transcript_search.rs` |
  | Pointer | `Msg::Pointer` | `mouse.rs` (the router), with `hit_test` in `layout.rs` |

  The other modules handle no concern's messages: `layout.rs` (the pure layout and `hit_test`), `redraw.rs` (the redraw limit), `startup.rs` (the domain state built at startup), `hints.rs` (the help line's hints and the startup toast), `session_panels.rs` (the session panels' values, pushed at the end of every `update`), and the Ports and workers (`index_port.rs`, `session_port.rs`, `search_worker.rs`). Beside their handlers, `runtime.rs` holds the run loop, and `transcript_search.rs` the transcript search's worker and port.
- **`App::view`** only computes the screen layout (product §3) and calls `Application::view(&Id, frame, area)` for each visible component, overlays last. It draws nothing itself.

Rules:
- Data flows **down** into components through `attr` (typed data via `AttrValue::Payload(PropPayload::Any(..))`, `Arc`-wrapped when large). Intent flows **up** as `Msg`. Components never see the `App`, and the `App` never reaches into a component's internals.
- **Facts a component publishes.** In tui-realm a component can send a `Msg` only from `on(&Event)`, and `attr` returns nothing, so a component cannot answer data the `App` pushes with a message. Right after pushing data with `attr`, the `App` may `query` a fact the component computed from that data, for example a count (`NEW_SECTIONS_ATTR`, `TURN_COUNT_ATTR`), rows built from the turns (`TURN_ROWS_ATTR`), a search request or position (`SEARCH_REQUEST_ATTR`, `SEARCH_POSITION_ATTR`), or the selection that the push moved (`SELECTED_SECTION_ATTR` after a live update, a search jump or a subagent swap). Each such query is made right after the push it depends on, in a small helper beside the push, never on every message. The `App` never queries presentation state that a key or a click moved, such as a cursor or a selection: that comes up as a `Msg` (for example `TranscriptMsg::SelectionChanged`). A value the `App` itself pushed down is kept by the `App`, never read back.
- No shared mutable state between components, or between a component and the `App`: no `Rc<RefCell<…>>`, `Arc<Mutex<…>>` or globals for UI state.
- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (product §4.1), so a 100 MB session is held once (product §8.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

## Components

Every custom UI element is its own component. The rules for writing one are in [coding.md](coding.md#components).

Components (each is its own type and module):

| Component | Where | Draws / handles | Takes focus |
|---|---|---|---|
| `SelectorFrame` | Selector | Overlay border, sizing (product §3.1) and background, in its own area; nothing inside. Over the browser, a press outside it closes it | No |
| `HarnessTabs` | Selector | Harness tabs; click switches tab (product §5.6) | No |
| `ProjectList` | Selector | Projects pane, selection rules of product §3.1, its own double-clicks | Yes |
| `SessionList` | Selector | Sessions pane, including search results with the project prefix and the `●` marker, its own double-clicks | Yes |
| `SessionTime` | Selector | The `Session Time` panel below the Sessions pane: the selected session's total and agent time (product §3.1) | No |
| `SessionTokens` | Selector | The `Session Tokens` panel below `Session Time`: the selected session's token totals (product §3.1) | No |
| `WorkingDir` | Selector | Working Directory box | No |
| `BrowserHeader` | Browser | Project and session titles, cwd, `● live`, breadcrumb, parse-warning note | No |
| `TurnSidebar` | Browser | The turns and compaction events of the open transcript, with selection, mouse and its own double-clicks (product §3.2, §5.3, §5.9) | Yes |
| `Transcript` | Browser | The turn blocks: selection, folding, navigation, search highlights (product §4, §5.1–§5.3) | Yes |
| `BackgroundFill` | Both | Paints its area with the background and nothing else; takes no pointer. Mounted as `SidebarGap`, the column between the sidebar and the transcript (product §3.2), and as `ScreenBackground`, the whole screen under the selector when the selector is the base screen, drawn first | No |
| `HelpLine` | Both | Key hints and status items (`3/17`, `↓ N new`, `Loading… 34%`) | No |
| `Prompt` | Both | Search and path prompts and `y/n` confirmations, drawn in place of the help line while active; mounted with its purpose | Yes (input mode, product §5) |
| `Toast` | Both | Bottom-right toast; expires on `Tick` | No |
| `HelpOverlay` | Overlay | product §3.3 | Yes |
| `DetailsOverlay` | Overlay | The metadata of the selected section (product §3.3) | Yes |
| `ThemeSelector` | Overlay | product §7.4, with its own double-clicks | Yes |
| `GlobalEvents` | Both | Nothing drawn; turns global keys, resizes, ticks, worker events and the pointer into `Msg`s ([Events, focus and keys](#events-focus-and-keys)) | No |
| `EmptyState` | Both | "Terminal too small", "No sessions for this harness", "No matching sessions", "Session file is empty", and for a session of local commands only the multi-line "No turns. Local commands only:" and the commands (product §3.2) | No |

`SessionTime` and `SessionTokens` are two components, not one, and each owns only what it displays. They share a small helper that draws one label and value row (`PanelRow`, and `PanelRows` for all the rows of a panel), and `format_tokens`, the one formatter of token counts (product §4.2), lives in `src/tokens.rs` beside the duration formatter (`src/duration.rs`) and the cost formatter (`format_cost` in `src/cost.rs`), so that neither the panels nor the renderers depend on another component's internals.

Like `PanelRow` (`src/components/panel_row.rs`), these helpers that several components share are modules of their own in `src/components/`, not parts of one component ([coding.md](coding.md#shared-code)):
- `text_fit`: fitting text into a number of display columns, cut at the end with `…` after it (`truncate_with_ellipsis`) or at the start with `…` before it, so the end of a path stays visible (`truncate_start`). The `App` uses it too.
- `titled_block`: the rounded, titled box that the panes, panels and overlays are drawn in, and the colour of its border.
- `list_cursor`: a list's selection and scroll offset (`ListCursor`) and the attributes that set them (`SELECTED_ATTR`, `VISIBLE_ROWS_ATTR`), shared by `ProjectList`, `SessionList`, `ThemeSelector` and `TurnSidebar`.
- `turn_rows`: what the turns sidebar lists (`TurnRows`, `entry_for`, `TURN_ROWS_ATTR`): the `Transcript` builds the rows from its turns and the `TurnSidebar` shows them.
- `double_click`: the last press (`LastPress`), the injectable clock (`ClickClock`) and the 400 ms limit (`DOUBLE_CLICK`) of the four components that have double-clicks.
- `list_pane` holds only what `ProjectList` and `SessionList` share, and `overlay_frame` the frame and scrolling that the help and details overlays share.

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (product §8.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (product §4.2), one body renderer per `ToolKind` (product §4.3), the markdown renderer (product §4.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (product §4.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

The `TurnSidebar` is likewise one mounted instance. It keeps no turns, only the rows built from them (product §3.2): the `App` forwards it the same `ModelUpdate`s as the transcript, and when a subagent is opened or closed it swaps the rows for those of the transcript on top, which the `App` queries from that transcript right after the swap.

## Events, focus and keys

- **Focus:** exactly one component has focus at a time, set by `App::update` through `Application::active`. In the selector, the pane keys of product §5.6 move focus between `ProjectList` and `SessionList`. In the browser, the keys of product §5.3 move it between `Transcript` and `TurnSidebar`. A focused component shows it through `Attribute::Focus` (e.g. the `ring` border, product §7.2).
- **Overlays and prompts:** opening one mounts its component and gives it focus. Closing it unmounts it and returns focus to the component that had it before.
- **Keys go through the keymap.** A component's `on()` resolves the key with the shared keymap (product §7.5), for its own context, to an action. The action becomes a `Cmd` (handled internally with `perform`) or a `Msg`. Components never match literal `KeyEvent`s, with two exceptions, both in `Prompt`: it takes printable characters as text (and `y` as the yes of a `y/n` question), and it matches the input-mode keys that product §5 fixes (`Enter`, `Esc`, `Backspace` and the arrow keys), because the keymap has no input context (product §7.5). This keeps remapping and the generated help overlay (product §3.3) correct.
- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay`, `DetailsOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, product §5; `Esc` / `?` closes help, `Esc` / `i` closes details). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (product §5.6). `GlobalEvents` also subscribes, always, to resizes (`Msg::Resized`), ticks (`Msg::Tick`), worker events and, with `mouse = true`, the mouse.
- **Mouse:** wheel and clicks act on the element under the pointer, whether or not it has focus (product §5.9). One pure function decides which component that is, and the component alone decides what the event means.
  - **The pointer arrives as a message.** `GlobalEvents` has one mouse subscription over the whole screen and turns each left press and wheel step without modifiers into `Msg::Pointer(MouseEvent)`, in screen coordinates. The raw `Event::Mouse` that tui-realm also forwards to the focused component is ignored by every component.
  - **Who is on top is pure.** `hit_test` in `src/app/layout.rs` decides where a pointer event goes, from the pure layout (`layout`, which `App::view` draws from too) and its input: to the topmost component that takes the pointer and contains the point, with that component's area; as an outside press to the topmost overlay, for a left press outside it; or nowhere. It is the one place z-order is defined. Its structural blocking depends on the screen, not on any one component:
    - Nothing under the topmost open overlay (the selector over the browser, help, details, the theme selector) is hit, and a wheel step outside that overlay reaches nothing.
    - While the prompt is open, only the overlays take the pointer, and the Sessions pane during the selector's search.
    - A component that does not take the pointer (the toast, a help line, the header, a panel, the selector's frame) blocks what is under it, so a press on the toast reaches no component.
    - The paint-only areas (`SidebarGap`, `ScreenBackground`) are never hit and hide nothing.
  - Rule: which component gets a pointer event is decided only by `hit_test` (the router only gives "the transcript" to the one on top). No other `App` code decides by component `Id` what a pointer event does; a condition of one component's own (for example, the Projects pane ignores the wheel while the selector is filtered) is a prop of that component, which it checks itself.
  - **The routing areas** are that same layout, recomputed by `refresh_routing` after every `update` and when the viewport or a toast changes, never from inside `view`.
  - **Delivery.** `App::update` (`App::pointer` in `src/app/mouse.rs`) gives the component found `Event::User(UserEvent::Pointer(RoutedMouse))`: the event with its position relative to the component's area (its top-left cell is 0, 0) and the area's size. "The transcript" in the layout is the transcript on top. The component's answer is queued with `emit`, not handled by calling `update` again, and `App::handle` handles it before any later event.
  - **The press-elsewhere notice.** The router remembers the `Id` of the component that got the last left press, and nothing else: no row, no time. When a later left press goes anywhere else (another component, outside an overlay, or no component), that component, if it is still mounted, first gets `UserEvent::PointerElsewhere`. It forgets its last press and sends no message. The wheel sends no notice.
  - **Overlays close themselves.** The overlay on top gets a left press outside it as `UserEvent::PointerOutside` and sends what its `Esc` sends: `HelpOverlay` and `DetailsOverlay` close, `ThemeSelector` cancels, and `SelectorFrame` over the browser closes the selector. The selector as the base screen, and during the resume confirmation, ignores it.
  - **Double-clicks and click intent are the components'.** `ProjectList`, `SessionList`, `ThemeSelector` and `TurnSidebar` each keep their own last press, measured with the clock the `App` hands them when it mounts them (`ClickClock`, which tests replace through `App::set_click_clock`). A double-click is two left presses on the same row at most 400 ms apart. A component turns a click into one message of intent, which says whether the click completed a double-click: `SelectorMsg::ChooseProject { id, open }` and `ChooseSession { id, open }`, `ThemeMsg::Choose { id, keep }`, `BrowserMsg::ChooseTurn { id, enter }`. The handlers of these intents do no hit testing. The `App` interprets no click and keeps no click state, apart from which component got the last left press.
  - **Why not mouse subscriptions.** tui-realm 4.1 cannot route the mouse by position: it does not know where a component was drawn (`Application::view` keeps no area); a mouse clause matches on position alone, ignoring its kind and modifiers (`MouseEventClause::is_in_range`); overlapping subscriptions all get the event, with no z-order and no way to consume it; and `unsubscribe` removes more than the one subscription it names. So no component subscribes to mouse events by area, and routing is the pure `hit_test` above.
- **Ticks:** `Event::Tick` arrives every 250 ms. The `Toast` subscribes to it and expires itself (`Msg::ToastExpired`). `GlobalEvents` turns it into `Msg::Tick`, on which the `App` re-evaluates the `● live` marker of the open session and the active sessions of the selector. The redraw limit is not driven by ticks: it belongs to the run loop's `RedrawLimiter` ([Data flow](#data-flow)).
- **Worker results** take one path: Port → `Event::User` → `GlobalEvents` (subscribed to all user events) turns it into a `Msg` (`Msg::Worker`, or `Msg::Browser` for a session's load and a subagent lookup) → `App::update` applies it to domain state and pushes what changed to the components that display it (rows and stats to `ProjectList` / `SessionList`, the selected session's stats to `SessionTime` and `SessionTokens`, `ModelUpdate`s to `Transcript` and `TurnSidebar`, and the local commands of a session without turns to the `EmptyState` of the browser; `Transcript` and `TurnSidebar` ignore the `LocalCommands` update). The panels are pushed only when their values change, and also when the selection changes by any route, so they never show the previous session's values under a new selection. Other components don't subscribe to worker events.

## Tech stack

**Decision:** Rust + [tui-realm](https://github.com/veeso/tui-realm) 4.x (`tuirealm`, built on Ratatui 0.30 + crossterm) as the application framework, with custom components for every UI element ([Components](#components)). ratcn is not a runtime dependency; its theme role model is adopted (see product §7).

Rationale:
- tui-realm imposes a structure: components with props and state, events turned into messages, a single update function, and focus and subscriptions managed by the `Application`. That keeps the UI split into small, separately testable parts instead of a central event handler and long render functions.
- It doesn't get in the way of the custom transcript. A component's `view` receives the Ratatui `Frame` and its area, so rendering stays immediate-mode and only the visible slice is drawn.
- The transcript view (variable-height, foldable, virtualized, live-updating sections) must be custom in any stack. Immediate-mode rendering of only the visible slice suits it.
- Selection marker (`▒` replacing part of a block's left border) is a trivial post-render buffer write in the transcript's `view`.
- Rounded borders (`BorderType::Rounded`) and left/right-aligned block titles are native to Ratatui.
- Single static binary; fast parsing of multi-MB JSONL; `rusqlite` for OpenCode.
- ratcn (0.0.x) was rejected as a dependency: unstable API, no text input, and its event/focus runtime duplicates what tui-realm provides.
- OpenTUI was rejected: retained component tree is a poor fit for the virtualized transcript, Bun + Zig toolchain, heavier distribution. (Its main draw — Hunk's diff view — can be approximated with `similar`.)

Runtime crates (`[dependencies]` in `Cargo.toml`):
- UI: `tuirealm` (with the `crossterm` feature), `ratatui`, `crossterm`, `tui-input`, `textwrap`, `unicode-width`, `unicode-segmentation`.
- Data: `serde`, `serde_json`, `toml`, `toml_edit` (writes the chosen theme back to the config file, keeping its comments), `rusqlite` (bundled), `notify`, `nucleo` (fuzzy search), `chrono` (timestamps).
- Rendering: `pulldown-cmark` (markdown), `syntect` (code highlighting), `similar` (diffs).
- Platform: `arboard` (+ OSC52 fallback, with `base64`), `etcetera` (directories), `ctrlc` (termination signals), `shell-words` (resume command templates), `clap` (command line).
- Errors and logging: `anyhow`, `log`.
