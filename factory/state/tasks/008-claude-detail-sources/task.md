# Claude section details from the source

## Goal
Fill the details overlay with what Claude transcripts record. Turns keep only cheap references to their source lines, and the line is re-read when the overlay opens, so these fields, which vary across versions and are rarely looked at, are never parsed into the model. This task also adds the read to the harness adapter interface, which keeps the UI harness-agnostic.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §5:

An on-demand overlay shows the metadata of the selected section, so blocks stay uncluttered.

From SPEC.md §5.2:

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

From SPEC.md §3 (how `toolUseResult` must be read):

- **Parsing `toolUseResult`:** it is usually an object but is sometimes a string, and a known field of the wrong JSON type fails the whole line (`src/harness/claude/wire.rs`). It can also be very large (stdout, whole files), so it must not be materialised as a `Value`. Read it through an untagged object-or-`IgnoredAny` head, like `wire::Content`.

From SPEC.md §4.2 (`stop_reason` per message):

- Claude's `stop_reason` is null on many lines. Use the last non-null value per message id. Null alone produces no notice.

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

From spec/SPEC.md §1.1:

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

From spec/SPEC.md §3.1:

- Location: `~/.claude/projects/<encoded-cwd>/<session-uuid>.jsonl` (append-only JSONL).
- Directory name is a lossy encoding of the cwd (`/` and `.` → `-`); the real cwd is in each entry's `cwd` field.
- Subagent transcripts: `<session-uuid>/subagents/agent-<agentId>.jsonl` (entries have `isSidechain: true`) + `agent-<agentId>.meta.json` (`agentType`, `description`, `toolUseId`).
  - Linkage: the parent's `Agent` `tool_use.id` equals `meta.toolUseId`, and the matching `tool_result` entry's `toolUseResult.agentId` names the file.
- Relevant entry types: `user`, `assistant`, `system` (e.g. `turn_duration`), `attachment`, `ai-title`, `last-prompt`, `cost-state`.
  Noise to skip: `api-request*`, `mode`, `permission-mode`, `atis-latch`, `file-history-*`.
- Assistant entries carry `message.model`, `effort` / `perTurnEffort`, `timestamp`, `usage`.
- Content blocks: `text`, `thinking` (usually redacted — empty text + signature; ~15% of blocks observed carry readable text), `tool_use`, `server_tool_use`, `advisor_tool_result`; tool results arrive as `tool_result` blocks inside `user` entries, paired by `tool_use_id`.
- Session title available via `ai-title` (written repeatedly; use the **last** one).
- **One JSONL line per content block:** an assistant API response is split over several `assistant` lines sharing `message.id` (BRIO session: 195 lines → 73 messages, up to 16 lines per message).

From spec/SPEC.md §7.1 (reloading a transcript):

| Harness | Change detection | Incremental read |
|---|---|---|
| Claude | `notify` watch on the session `.jsonl` (and its `subagents/` dir) | Read from the last byte offset. New lines go through the same turn assembly (§5.1), so a line extending an existing `message.id` or delivering a `tool_result` updates sections in place. Keep a trailing partial line in a buffer until its newline arrives. If the file shrinks or is replaced (size < offset or inode change), reload it. |

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

From spec/SPEC.md §8.6:

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

- **Interface.** A new `Harness` method with a default that provides nothing, documented as what Codex and Pi adapters inherit. It takes the section's references and returns groups of rows; the UI knows neither harness format. Without a reference, or with an adapter providing nothing, the overlay shows the model's rows from task 007 and no `unavailable` row. It runs on the UI thread when the overlay opens, like `ToolOutput::text()`.
- **Values.** Rows carry a typed value or the `App` formats them, so that timestamps use the configured `TimestampFormatter` and counts stay plain. The UI composes the model's rows and the adapter's rows in the table's order: for Tool call that is `start` (adapter), `duration`, `status` (model), then the adapter's rest. No field appears twice: the model's Prompt `timestamp` gives way to the adapter's.
- **References.** A small model type, for example `SourceRef`, holds a byte range plus the cheap `ByteSource` the assembler already has (`ByteSource::File` is an `Arc`), so a subagent transcript's references name its own file. Turns store references and no field values. A side table on the turn keyed by section avoids touching every `Item` pattern and fixture.
- **Which line.** A tool call has two references: its `tool_use` line and its `tool_result` line. Text, thinking and notices need their message's lines: `stop_reason` and `usage` are complete only on later lines of a message, so the Message group reads the message's latest line with a non-null `stop_reason`, or else its latest line. The footer references its last `turn_duration` line, the `model → …` note its message, a compaction its boundary line, a system section its first entry's line, and a prompt its entry's line.
- **Field locations.** The spec names fields, not always their parents. `timestamp`, `requestId`, `gitBranch`, `cwd`, `version`, `entrypoint`, `permissionMode`, `promptSource` and `requestedModel` are read from the top of the line. `message.id` and `stop_reason` are under `message`. `service_tier`, `speed` and `inference_geo` are under `message.usage`. The cache writes are `message.usage.cache_creation.ephemeral_5m_input_tokens` and `ephemeral_1h_input_tokens`, and the counts are `message.usage.server_tool_use.web_search_requests` and `web_fetch_requests`. `permissionDecision`, `sandboxed`, `resolvedModel` and `isAsync` are read from the top of the result's line and then from `toolUseResult`. `diagnostics.cache_miss_reason` is read from the top of the line and then from `message`. Confirm these against a real transcript if one is available; a reader that also tolerates the other parent is fine.
- **Row labels.** The raw field names as the spec writes them, except: `5m cache writes`, `1h cache writes`, `web searches`, `web fetches`, `start` for the tool call's start.
- **Reading.** A new `src/harness/claude/details.rs` with its own typed heads, separate from `wire::Entry`, so assembly is untouched. `message.content` is not read at all, and `toolUseResult` goes through a typed head that skips everything else. A field of an unexpected JSON type counts as missing, and it never fails the line.
- **`unavailable`.** Each group whose rows depend on a source that could not be read gets one muted `unavailable` row, after any rows the model still knows. Causes: the file is gone, the range is past the end of the file, or the line does not parse as JSON. A tool call whose result line fails but whose call line reads shows `start` and the model's rows, then `unavailable`. A tool call with no result yet has no result reference, and that is not unavailable.
- **Reload.** A transcript that shrinks or is replaced is reloaded (spec/SPEC.md §7.1), and the new turns carry new references. An overlay that opens in between shows `unavailable` at worst.
- **Proof that assembly stays cheap.** A test feeds a line whose detail fields have unexpected types and sizes and shows it assembles and is not counted as unparsed. Report the budget figures in the build notes. Compare heap with the previous figures in `docs/performance.md`.
- **Budget for opening.** Print a reference line from the budget test: the time to read the details of the largest tool result (it re-reads up to a 2 MB line).

## Acceptance criteria
- [ ] Adapter tests cover each Claude group of §5.3 (Turn, Message, Tool call, Prompt, Environment), a line missing the fields, an unreadable source, and a `toolUseResult` string.
- [ ] A test or benchmark shows that assembling a turn does not parse the detail fields, and the budgets hold: `cargo test --release -- --ignored budgets --nocapture` stays within budget.
- [ ] `Harness` has the new method with a default, and a test with a stub harness shows the overlay with the model's rows only and no `unavailable` row.
- [ ] `wire::Entry` and the assembler gain no detail fields. `src/harness/claude/details.rs` reads `toolUseResult` through a typed head, and a test gives it a very large `toolUseResult` and a string one.
- [ ] A test shows a text block on the first line of a multi-line message gets the `stop_reason` recorded on a later line of that message.
- [ ] A test shows a tool call's permission, sandbox and `isAsync` rows come from its result's line and its `start` from its call's line.
- [ ] A test shows a subagent transcript's section reads its own file, not the parent's.
- [ ] Tests cover the file gone, a range past the end of the file, and an unparseable line: one muted `unavailable` row in each affected group, no panic, and the overlay stays open.
- [ ] A test shows a tool call with no result yet opens with no `unavailable` row.
- [ ] A test shows missing fields are left out and a group with no rows is left out, and Claude shows no `cost` row.
- [ ] A test through `App::update` opens the overlay on a Claude fixture and shows Message and Environment rows.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] Snapshot changes are limited to details overlay snapshots at App level, if any. No transcript snapshot changes.

## UI
The overlay's frame, layout and keys are unchanged from task 007. This task adds rows.

- Message, Tool call, Prompt and Environment rows appear in the order of §5.3, with the styles of task 007.
- `unavailable`: one row with no label column, `unavailable` in `muted_foreground`, at the row column of the affected group.

Snapshot: a Claude tool call at 80x24 and 120x40 in the `default` theme, through the App, and one section with an unreadable source.

## Depends on
- 004-tool-call-durations
- 006-claude-stop-notices
- 007-details-overlay
