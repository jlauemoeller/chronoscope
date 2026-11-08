# Chronoscope — Architecture

This file says how Chronoscope is built: the data flow from the harness adapters to the screen, the application structure, the components and how events reach them, and the tech stack. It is a living document: a cycle's docs task updates it when the cycle changes the architecture. Most of it describes what exists today: the diagram, the component table, the routing and the crate list are context, and adding to them is not a breach. Its rules are the statements of what must or must not be done, such as the **Rules** under [Application structure](#application-structure). Rules are binding, and a rule that starts with `Prefer:` is a preference. `product §n` is a section of [product.md](product.md). How to write the code is in [coding.md](coding.md), and how to test it in [testing.md](testing.md).

## Data flow

```
            ┌───────────── adapters (one per harness) ─────────────┐
            │ claude: JSONL files     opencode: SQLite (read-only) │
            └──────────────┬──────────────────────┬────────────────┘
                 discover projects/sessions   load + watch session
                           ▼                      ▼
                    Session index         Normalised model (product §4.1)
                           └──── worker threads ──┘
                                      │ channels, polled by Ports → Event::User
                                      ▼
  crossterm keys/mouse ──► tui-realm Application ──► focused / subscribed components (Components)
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
    fn section_details(&self, source: &SectionSource) -> Vec<SourceGroup>;
    // re-reads a section's metadata from its source for the details overlay (product §3.3, §4.1);
    // the default provides nothing, which the Codex and Pi adapters would inherit
}
```

- The UI is a tui-realm `Application` ([Tech stack](#tech-stack)) driven by a single-threaded loop: the loop calls `Application::tick`, passes every returned `Msg` to `App::update`, and redraws only when something changed (at most every 50 ms, product §6.1).
- Parsing, watching, stats computation and search indexing run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used. (Reading the details of one section for the overlay is an exception, like materialising a large output when it is unfolded: it re-reads one or two lines or rows on the UI thread, product §8.2.)

## Application structure

The UI follows tui-realm's model: components turn events into messages, and one update function acts on them.

- **`App`** owns the `Application` and the domain state: config, theme registry and active theme, keymap, session index and stats, the open session's summary, watch handle and subagent stack, worker handles and the clipboard.
- **`Id`**: one enum variant per mountable component ([Components](#components)).
- **`Msg`**: what happened, in application terms (`OpenSession(id)`, `CopyText(text)`, `PreviewTheme(id)`, `Quit`, …), never raw keys. Split it into nested enums per screen or concern once it grows.
- **`UserEvent`**: what the Ports deliver: index updates, stats results, session `ModelUpdate`s, search results. tui-realm requires `UserEvent: Eq + Clone`, so large payloads are `Arc`-wrapped and compared by a sequence number.
- **`App::update(Msg)`** is the only place that changes domain state, mounts and unmounts components, moves focus and pushes new data into components. It is a dispatcher: each `Msg` arm calls a small handler, grouped in modules by concern (selector, browser, copy/save/export, resume, theme, live updates). It never interprets keys.
- **`App::view`** only computes the screen layout (product §3) and calls `Application::view(&Id, frame, area)` for each visible component, overlays last. It draws nothing itself.

Rules:
- Data flows **down** into components through `attr` (typed data via `AttrValue::Payload(PropPayload::Any(..))`, `Arc`-wrapped when large). Intent flows **up** as `Msg`. Components never see the `App`, and the `App` never reaches into a component's internals.
- No shared mutable state between components, or between a component and the `App`: no `Rc<RefCell<…>>`, `Arc<Mutex<…>>` or globals for UI state.
- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (product §4.1), so a 100 MB session is held once (product §8.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

## Components

Every custom UI element is its own component. The rules for writing one are in [coding.md](coding.md#components).

Components (each is its own type and module):

| Component | Where | Draws / handles | Takes focus |
|---|---|---|---|
| `SelectorFrame` | Selector | Overlay border, sizing (product §3.1) and background; nothing inside | No |
| `HarnessTabs` | Selector | Harness tabs; click switches tab (product §5.6) | No |
| `ProjectList` | Selector | Projects pane, selection rules of product §3.1 | Yes |
| `SessionList` | Selector | Sessions pane, including search results with the project prefix and the `●` marker | Yes |
| `SessionTime` | Selector | The `Session Time` panel below the Sessions pane: the selected session's total and agent time (product §3.1) | No |
| `SessionTokens` | Selector | The `Session Tokens` panel below `Session Time`: the selected session's token totals (product §3.1) | No |
| `WorkingDir` | Selector | Working Directory box | No |
| `BrowserHeader` | Browser | Project and session titles, cwd, `● live`, breadcrumb, parse-warning note | No |
| `TurnSidebar` | Browser | The turns and compaction events of the open transcript, with selection and mouse (product §3.2, §5.3, §5.9) | Yes |
| `SidebarGap` | Browser | Paints the column between the sidebar and the transcript; nothing else | No |
| `Transcript` | Browser | The turn blocks: selection, folding, navigation, search highlights (product §4, §5.1–§5.3) | Yes |
| `HelpLine` | Both | Key hints and status items (`3/17`, `↓ N new`, `Loading… 34%`) | No |
| `Prompt` | Both | Search and path prompts and `y/n` confirmations, drawn in place of the help line while active | Yes (input mode, product §5) |
| `Toast` | Both | Bottom-right toast; expires on `Tick` | No |
| `HelpOverlay` | Overlay | product §3.3 | Yes |
| `DetailsOverlay` | Overlay | The metadata of the selected section (product §3.3) | Yes |
| `ThemeSelector` | Overlay | product §7.4 | Yes |
| `GlobalEvents` | Both | Nothing drawn; turns global keys and worker events into `Msg`s ([Events, focus and keys](#events-focus-and-keys)) | No |
| `EmptyState` | Both | "Terminal too small", "No sessions for this harness", "No matching sessions", "Session file is empty", and for a session of local commands only the multi-line "No turns. Local commands only:" and the commands (product §3.2) | No |

`SessionTime` and `SessionTokens` are two components, not one, and each owns only what it displays. They share a small helper that draws one label and value row (`PanelRow`, and `PanelRows` for all the rows of a panel), and `format_tokens`, the one formatter of token counts (product §4.2), lives in `src/tokens.rs` beside the duration formatter, so that neither the panels nor the renderers depend on another component's internals.

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (product §8.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (product §4.2), one body renderer per `ToolKind` (product §4.3), the markdown renderer (product §4.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (product §4.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

The `TurnSidebar` is likewise one mounted instance. It keeps no turns, only the rows built from them (product §3.2): the `App` forwards it the same `ModelUpdate`s as the transcript, and when a subagent is opened or closed it swaps the rows for those of the transcript on top.

## Events, focus and keys

- **Focus:** exactly one component has focus at a time, set by `App::update` through `Application::active`. In the selector, the pane keys of product §5.6 move focus between `ProjectList` and `SessionList`. In the browser, the keys of product §5.3 move it between `Transcript` and `TurnSidebar`. A focused component shows it through `Attribute::Focus` (e.g. the `ring` border, product §7.2).
- **Overlays and prompts:** opening one mounts its component and gives it focus. Closing it unmounts it and returns focus to the component that had it before.
- **Keys go through the keymap.** A component's `on()` resolves the key with the shared keymap (product §7.5), for its own context, to an action. The action becomes a `Cmd` (handled internally with `perform`) or a `Msg`. Components never match literal `KeyEvent`s, except that `Prompt` takes printable characters as text. This keeps remapping and the generated help overlay (product §3.3) correct.
- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay`, `DetailsOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, product §5; `Esc` / `?` closes help, `Esc` / `i` closes details). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (product §5.6).
- **Mouse:** wheel and clicks act on the element under the pointer, whether or not it has focus (product §5.9). The event goes to the topmost component drawn there, for example through mouse subscriptions whose row/column ranges are that component's area. That component turns the event into a `Cmd` or `Msg`.
  - While an overlay is open (selector, help, details, theme selector), the components under it get no mouse events, and a click outside the overlay closes it.
  - Component areas come from a pure layout function that `App::view` also uses. Subscriptions are refreshed from it when the screen or terminal size changes, never from inside `view`.
- **Ticks:** `Event::Tick` drives toast expiry, the `● live` marker and the redraw limit (product §6.1).
- **Worker results** take one path: Port → `Event::User` → `GlobalEvents` (subscribed to all user events) turns it into a `Msg` → `App::update` applies it to domain state and pushes what changed to the components that display it (rows and stats to `ProjectList` / `SessionList`, the selected session's stats to `SessionTime` and `SessionTokens`, `ModelUpdate`s to `Transcript` and `TurnSidebar`, and the local commands of a session without turns to the `EmptyState` of the browser; `Transcript` and `TurnSidebar` ignore the `LocalCommands` update). The panels are pushed only when their values change, and also when the selection changes by any route, so they never show the previous session's values under a new selection. Other components don't subscribe to worker events.

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

Candidate crates: `tuirealm` (with the `crossterm` feature), `ratatui`, `crossterm`, `serde`, `serde_json`, `toml`, `rusqlite` (bundled), `notify`, `textwrap`, `unicode-width`, `tui-input`, `arboard` (+ OSC52 fallback), `nucleo`, `clap`, `etcetera`.
