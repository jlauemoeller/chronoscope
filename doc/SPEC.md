# Chronoscope — Specification

> See [PRD.md](PRD.md), [ui-sketch.md](ui-sketch.md) and [theme-selector.md](theme-selector.md).

## 1. Overview

Chronoscope is a keyboard-first (mouse-capable), themeable terminal UI for browsing coding-agent sessions. You pick a harness, project and session in a selector overlay, then read the conversation as a stack of rounded blocks: user prompts, assistant text, thinking, and tool calls. Detail is hidden behind folds, so you can skim the high-level flow and unfold the parts you care about. Open sessions update live while the agent is still working.

v1 harnesses: **Claude Code** and **OpenCode**. The adapter interface is designed so that **Codex** and **Pi** can be added later without UI changes.

### 1.1 Architecture

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

### 1.2 Application structure

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

### 1.3 Components

**Every custom UI element is its own component.** The code is organised around components, not around long render functions or a central key handler.

- A component implements tui-realm's `Component` trait (`view`, `query`/`attr`, `state`, `perform(Cmd) -> CmdResult`), plus `AppComponent` (`on(&Event) -> Option<Msg>`) if it handles events. An element that appears on several screens (e.g. the help line) is one component type, mounted with different props.
- A component draws only itself. Its `view` draws its own border and content, and leaves sub-elements to child types: a child component it owns, or a small `Widget`/`StatefulWidget` type.
- **No long render functions.** A render function (`view`, or a helper it calls) that draws more than one distinct element, or grows past about 50 lines, is split into smaller components or widgets. Rendering has no side effects.
- Components are `pub` types that can be built from plain data (props and the theme) and rendered into a `TestBackend` frame on their own, without an `Application` or a terminal (§9.6).
- All colours and modifiers come from theme tokens (§8.1), passed in as a prop. Components never hard-code colours.
- `tui-realm-stdlib` is not used. The look in §4 and §8 is specific enough that wrapping stdlib components would mean overriding their rendering. Prompt text editing uses `tui-input` inside the `Prompt` component.

Components (each is its own type and module):

| Component | Where | Draws / handles | Takes focus |
|---|---|---|---|
| `SelectorFrame` | Selector | Overlay border, sizing (§4.1) and background; nothing inside | No |
| `HarnessTabs` | Selector | Harness tabs; click switches tab (§6.6) | No |
| `ProjectList` | Selector | Projects pane, selection rules of §4.1 | Yes |
| `SessionList` | Selector | Sessions pane, including search results with the project prefix and the `●` marker | Yes |
| `WorkingDir` | Selector | Working Directory box | No |
| `BrowserHeader` | Browser | Project and session titles, cwd, `● live`, breadcrumb, parse-warning note | No |
| `Transcript` | Browser | The turn blocks: selection, folding, navigation, search highlights (§5, §6.1–§6.3) | Yes |
| `HelpLine` | Both | Key hints and status items (`3/17`, `↓ N new`, `Loading… 34%`) | No |
| `Prompt` | Both | Search and path prompts and `y/n` confirmations, drawn in place of the help line while active | Yes (input mode, §6) |
| `Toast` | Both | Bottom-right toast; expires on `Tick` | No |
| `HelpOverlay` | Overlay | §4.3 | Yes |
| `ThemeSelector` | Overlay | §8.4 | Yes |
| `GlobalEvents` | Both | Nothing drawn; turns global keys and worker events into `Msg`s (§1.4) | No |
| `EmptyState` | Both | "Terminal too small", "No sessions for this harness", "Session file is empty" | No |

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (§9.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (§5.2), one body renderer per `ToolKind` (§5.3), the markdown renderer (§5.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (§5.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

### 1.4 Events, focus and keys

- **Focus:** exactly one component has focus at a time, set by `App::update` through `Application::active`. In the selector, the pane keys of §6.6 move focus between `ProjectList` and `SessionList`. A focused component shows it through `Attribute::Focus` (e.g. the `ring` border, §8.2).
- **Overlays and prompts:** opening one mounts its component and gives it focus. Closing it unmounts it and returns focus to the component that had it before.
- **Keys go through the keymap.** A component's `on()` resolves the key with the shared keymap (§8.5), for its own context, to an action. The action becomes a `Cmd` (handled internally with `perform`) or a `Msg`. Components never match literal `KeyEvent`s, except that `Prompt` takes printable characters as text. This keeps remapping and the generated help overlay (§4.3) correct.
- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, §6; `Esc` / `?` closes help). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (§6.6).
- **Mouse:** wheel and clicks act on the element under the pointer, whether or not it has focus (§6.9). The event goes to the topmost component drawn there, for example through mouse subscriptions whose row/column ranges are that component's area. That component turns the event into a `Cmd` or `Msg`.
  - While an overlay is open (selector, help, theme selector), the components under it get no mouse events, and a click outside the overlay closes it.
  - Component areas come from a pure layout function that `App::view` also uses. Subscriptions are refreshed from it when the screen or terminal size changes, never from inside `view`.
- **Ticks:** `Event::Tick` drives toast expiry, the `● live` marker and the redraw limit (§7.1).
- **Worker results** take one path: Port → `Event::User` → `GlobalEvents` (subscribed to all user events) turns it into a `Msg` → `App::update` applies it to domain state and pushes what changed to the components that display it (rows and stats to `ProjectList` / `SessionList`, `ModelUpdate`s to `Transcript`). Other components don't subscribe to worker events.

## 2. Tech stack

**Decision:** Rust + [tui-realm](https://github.com/veeso/tui-realm) 4.x (`tuirealm`, built on Ratatui 0.30 + crossterm) as the application framework, with custom components for every UI element (§1.3). ratcn is not a runtime dependency; its theme role model is adopted (see §8).

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

## 3. Data sources (observed)

### 3.1 Claude Code

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
- **Most `user` entries are not human prompts:** in the BRIO session, 80 of 92 `user` entries only carry `tool_result` blocks. Human prompts have `origin.kind == "human"` and no `tool_result`. Entries with `isMeta: true` are injected, not typed.

### 3.2 OpenCode

- Location: `~/.local/share/opencode/opencode.db` (SQLite, WAL mode).
- Tables: `project` (`worktree`, `name`), `session_v2` (`project_id`, `parent_id`, `directory`, `title`, `model`, tokens/cost, timestamps), `session_message` (`session_id`, `type`, `seq`, `data` JSON).
- Message types: `user`, `assistant`, `synthetic`, `system`, `idle`. Assistant `data` has `agent`, `model {id, providerID, variant}`, and a `content` array of `text` / `tool` parts.
- Subagent sessions are linked via `session_v2.parent_id`.

## 4. Screens

### 4.1 Session selector overlay

Layout as in ui-sketch.md: harness tabs, Projects list, Sessions list, Working Directory box, and help/search line.

Sizing:
- Width: the full terminal width, up to 160 columns; on wider terminals it is 160 columns wide and centred horizontally. Height: the terminal height minus one row above and one below.
- The Projects pane takes a third of the content width, but at least 20 and at most 36 columns. The Sessions pane gets the rest, so extra width mainly goes to the session list.

Session row format: `<local start timestamp>  <title>` on the left, plus muted stats on the right (turn count · duration). The model is not shown: the browser shows it on every Assistant turn.
- Title: Claude `ai-title` → OpenCode `session_v2.title` → first user prompt (truncated) → `(untitled)`.
- Stats are computed in the background and cached (§9.2). Until they are ready, the row shows just timestamp + title.

Projects:
- Claude: one project per distinct real `cwd`, read from the session entries (not the lossy directory name).
- OpenCode: from the `project` table (`name`, else the basename of `worktree`).
- Display name: basename of the cwd, unless an alias is configured (§8.5). Projects are sorted by most recent activity.
- Sessions are sorted newest first.

Startup (no arguments):
- Opens the selector.
- Preselects the harness and project whose cwd matches `$PWD` (or the nearest ancestor), falling back to the most recently active project (the first in the list).
- Focus is on the Projects pane and **no session is selected**: the Sessions pane lists the project's sessions without a `>` marker.

Session selection:
- Opening the selector over an open session (`Ctrl-o`) selects that session, with focus on Sessions.
- When the Sessions pane gets the focus (`Tab`, `Enter` on a project, `/`, clicking a row) and no session is selected, the newest session is selected.
- Moving to another project from the Projects pane (keys, click, wheel) clears the session selection again. Switching harness tab selects the first project, and its newest session only if Sessions has focus.
- While nothing is selected, live index updates keep the cursor on the newest session. `Esc` on a search that started from this state returns to it: Projects focused, no session selected.

Search (`/` in the selector):
- Fuzzy (`nucleo`) over title, first prompt and project name across **all projects of the current harness**.
- While a query is active, the Sessions pane shows matching sessions with their project name prefixed, and keeps the focus (`Tab`, `←`/`→`, `h`/`l` do nothing). `ESC` clears the query.

### 4.2 Session browser

- Header block: `Project <name>` (left title), session id (right title), working directory in the body. A `● live` marker shows while the session is active (§7.2), and a breadcrumb shows when viewing a subagent (§5.4).
- Transcript: one rounded block per turn (§5.2), scrolling vertically, with the selected section marked by `▒` over the block's left border.
- Help line under the transcript: context-sensitive key hints. It turns into the search/path prompt when active and shows search position (`3/17`), the `↓ N new` indicator and parse warnings.
- Toasts (copy/save/export confirmations, config warnings) appear in the bottom-right corner for ~2 s.
- Loading state: `Loading… 34%` in the help line while the initial parse streams in.
- Empty states: "No sessions for this harness" / "Session file is empty".

### 4.3 Overlays

- **Help overlay** (`?`): grouped keybindings as in ui-sketch.md, generated from the active keymap. `Esc` / `?` closes it.
- **Theme selector** (`t`): see §8.4.
- **Confirmation prompts** (overwrite file, resume in harness) appear in the help line as `y/n` questions, not as dialogs.

Overlays are centred, use rounded borders with `ring` colour, and paint `surface` behind them (unless the background is transparent, in which case `Clear` is still used so underlying text doesn't bleed through).

## 5. Conversation model & rendering

### 5.1 Normalised model

Harness adapters translate each native format into one shared model:

```
Session { id, harness, project, cwd, title, started_at, turns: [Turn], children: [SessionRef] }
Turn    = User      { timestamp, text, items: [Item] }     // items: injected content preceding/attached to the prompt
        | Assistant { timestamp, model, effort?, items: [Item], stats? }
        | Compaction{ timestamp, summary }
Item    = Text(markdown)
        | Injected { entries: [{ kind, timestamp, text }] }   // §5.6; consecutive entries collapsed
        | Thinking(text | Redacted)
        | ToolCall { id, tool: ToolKind, description, input, result?: ToolResult, status, duration? }
ToolKind = Shell | Read | Write | Edit | Grep | Glob | WebFetch | WebSearch | Subagent | Other(name)
```

Turn assembly (Claude):
1. A **User turn** starts at a `user` entry that is a human prompt: `origin.kind == "human"` (or, for older versions without `origin`, string/text content with no `tool_result` and no `isMeta`). A message typed while the agent is working is stored as an `attachment` of kind `queued_command` with `origin.kind == "human"`; it also starts a User turn, and the content after it forms a new Assistant turn.
2. Everything up to the next human prompt forms **one Assistant turn**: `assistant` lines, `tool_result`-only `user` lines, and allowlisted injected content (§5.6).
3. `assistant` lines with the same `message.id` are merged into one message, keeping their content blocks in order. Repeated blocks (same `tool_use.id`, or same index for text/thinking) replace the earlier version instead of being duplicated.
4. `tool_result` blocks are attached to the `ToolCall` with the matching `tool_use_id`. A tool call without a result yet has status `pending`.
5. The model shown in the Assistant title is that of the turn's first message. If the model changes within a turn, the new model is shown as a muted inline note.
6. `system` `turn_duration` entries become the stats footer of the preceding Assistant turn.

Turn assembly (OpenCode): each `user` row (with `synthetic` rows treated as injected content) starts a turn. `assistant` rows up to the next `user` row form the Assistant turn. Tool calls and their results are already combined in each `tool` part (`state.input`, `state.content`, `state.status`).

- Tool names are mapped to `ToolKind` per harness (e.g. Claude `Bash` and OpenCode `shell` → `Shell`; Claude `Agent`/`Task` and OpenCode `subagent` → `Subagent`).
- Tool description: the tool's own `description` input if present (Claude Bash). Otherwise a derived label (`Read src/main.rs`, `Grep "pattern" in path`, `WebFetch example.com/…`, `<tool name>` for unknown tools).
- `effort` is shown only when the harness records it (Claude `effort`; OpenCode `model.variant` when it isn't `default`).

### 5.2 Blocks and sections

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

### 5.3 Per-tool renderers

| ToolKind | Unfolded body |
|---|---|
| Shell | `Command:` (char-wrapped) + `Result:` (char-wrapped, error output in `error_output`) |
| Read | Path + line range, then file content with line numbers |
| Write | Path, then content (syntax-highlighted by extension) |
| Edit | Path, then a unified diff of old → new (`similar`), with added/removed lines coloured |
| Grep / Glob | Pattern (+ path / filters), then matches |
| WebFetch / WebSearch | URL or query, then result |
| Other (incl. MCP) | Pretty-printed input JSON + result text |

Results longer than N lines (default 200, configurable) are truncated, with a `… 1 234 more lines` row; pressing `Enter` on that row expands the rest.

### 5.4 Subagents

- Subagent sections show the prompt and the final result inline.
- A key (§6) opens the subagent's own transcript as a full session view: Claude `subagents/agent-<id>.jsonl`, OpenCode child `session_v2` via `parent_id`.
- The browser header shows a breadcrumb (`Project BRIO › Agent: story-writer`). A back key returns to the parent view at the same scroll position.

### 5.5 Text rendering

- Assistant text, user prompts and thinking are rendered as markdown (`pulldown-cmark`): headings, emphasis, lists, inline code, block quotes, tables, and fenced code blocks with syntax highlighting (`syntect`, mapped to theme colours).
- Prose is word-wrapped. Commands and tool output are char-wrapped (as in the sketch) so paths and long tokens are never hidden.
- Wrapping is recomputed when the terminal is resized.

### 5.6 Visibility toggles

- System/injected content is shown by default (muted, folded). A toggle key hides or shows all of it; the default is configurable (`show_system = true`).
- It is rendered as **System sections inside the adjacent turn**: in the next turn when it precedes a prompt, in the current turn otherwise. Consecutive injected entries collapse into one section with a count (`3 injected items`). They never become turn blocks of their own, so `[` / `]` only stop at User, Assistant and Compaction blocks.
- **Allowlist** of Claude content that counts as system/injected:
  - attachments `hook_system_message`, `file`, `directory`, `opened_file_in_ide`, `selected_lines_in_ide`, `queued_command` (except human messages, see §5.1), `instructions`, `plan_mode`, `plan_mode_exit`, `compact_file_reference`;
  - `system` entries with subtype `local_command`, `informational` or `away_summary`;
  - `user` entries with `isMeta: true`.
- OpenCode: `synthetic` and `system` rows.
- **Everything else is noise and never shown:**
  - API request records, mode/permission changes, file-history snapshots, title / last-prompt / cost records;
  - all other attachment kinds (`total_tokens_reminder`, `deferred_tools_*`, `skill_listing`, `credential_org`, `batching_reminder_sent`, `environment`, `date`, `model`, `auto_mode`, …).
  - Unknown attachment kinds are treated as noise, but are counted in the log so the allowlist can be extended.

## 6. Interaction & keybindings

All keybindings are remappable in config (§8.5). The defaults avoid modifier+Enter combinations, which most terminals cannot distinguish from Enter.

**Input mode:** while a help-line prompt is active (search, file path, y/n confirmation), the `Prompt` component has focus (§1.4) and every printable key goes into the prompt. Only `Enter`, `Esc`, `Backspace`, the arrow keys and `Ctrl-c` keep their meaning there. This prevents conflicts such as `j` / `t` / `1`–`4` while typing a search.

### 6.1 Folding (session browser)

| Key | Action |
|---|---|
| `Enter` / `Space` | Toggle fold of current section |
| `←` / `h` | Fold current section |
| `→` / `l` | Unfold current section |
| `-` | Fold all |
| `+` / `=` | Unfold all |

Fold levels (`1`/`2`/`3`) and Vim-style `z` aliases (`za`, `zc`, `zo`, `zM`, `zR`) were considered and rejected. No default binding is a chord.

### 6.2 Transcript search

- `/` opens a query in the help line. Matching is smart-case substring (case-insensitive unless the query contains an uppercase letter).
- Searches **all** text, including folded sections, tool input/output and system content.
- `Enter` confirms and jumps to the first match; `n` / `N` go to the next / previous match. All matches are highlighted with `search_match`, and the help line shows `3/17`.
- Jumping to a match inside a folded section unfolds it temporarily. It refolds when the selection moves to a different section, unless the user unfolded it explicitly in the meantime.
- `ESC` clears the search and highlights.

### 6.3 Navigation (session browser)

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

### 6.4 Copy, save, export

| Key | Action |
|---|---|
| `y` | Copy the selected section **as displayed** (folded: summary line; unfolded: everything shown) as plain text |
| `Y` | Copy the selected section's **full content** (description + full input/command + full result), regardless of fold state or truncation |
| `w` / `Ctrl-s` | Save the selected section as displayed to a file |
| `W` | Save the selected section's full content to a file |
| `e` / `Ctrl-e` | Export the session to markdown **as currently folded** (folded sections become their summary lines) |
| `E` | Export the **full** session to markdown (tool calls as `<details>` blocks with fenced input/output) |

- Save and export prompt for a path in the help line. Defaults:
  - export: `./<project>-<yyyymmdd-hhmm>-<short-id>.md`
  - save: `./<project>-<short-id>-<section-kind>.txt`
- Saving or exporting asks for confirmation before overwriting an existing file.
- Clipboard: `arboard`, falling back to OSC52 when there's no display server or over SSH/tmux. A toast confirms the copy (`Copied 42 lines`).

### 6.5 Sessions, subagents, overlays, application

| Key | Action |
|---|---|
| `Ctrl-o` | Open the session selector (plain `o` is not bound) |
| `a` | Open the selected subagent's transcript (§5.4) |
| `Backspace` | Back to the parent session (after `a`) |
| `R` | Resume the session in its harness (§6.7) |
| `s` | Toggle visibility of system / injected content |
| `t` | Open the theme selector (§8.4) |
| `?` | Help overlay |
| `q`, `Ctrl-q`, `Ctrl-w`, `Ctrl-c` | Quit |

`Ctrl-c` always quits; it is never used for copy.

### 6.6 Session selector

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

### 6.7 Resume in harness

`R` (in the browser, or on a selected row in the selector) asks for confirmation, then:
1. restores the terminal;
2. changes to the session's cwd;
3. replaces the Chronoscope process with the harness's resume command (`exec` on Unix; spawn-and-wait on Windows).

Commands are configurable templates (§8.5). Defaults:
- Claude: `claude --resume {id}`
- OpenCode: `opencode --session {id}`. **Verify this flag** before release.

If the cwd no longer exists, show an error and don't resume.

### 6.8 Command line

```
chronoscope                          # selector, preselects $PWD project (§4.1)
chronoscope <session-id | path>      # open a session directly (id looked up across harnesses; path = .jsonl file)
chronoscope --latest                 # open the newest session for $PWD (or --project)
  --harness <claude|opencode>        # restrict lookup / preselect tab
  --project <path|name>              # preselect project
  --theme <name>                     # override theme for this run
  --config <path>                    # alternative config file
```

If a session id is ambiguous (prefix matches several sessions), the selector opens filtered to the matches.

### 6.9 Mouse

- Wheel scrolls the transcript or the list under the pointer.
- Click selects a section or list row. Clicking a section's summary line (or the `▶`/`▼` marker) toggles its fold. Double-clicking a session row opens it.
- Click on a harness tab switches to it. Click outside an overlay closes it.
- Mouse events go to the component under the pointer (§1.4).
- Mouse capture can be disabled in config (`mouse = false`). Most terminals also bypass mouse capture while `Shift` is held, which allows native text selection; the help overlay mentions this.

## 7. Live updates

### 7.1 Mechanisms

| Harness | Change detection | Incremental read |
|---|---|---|
| Claude | `notify` watch on the session `.jsonl` (and its `subagents/` dir) | Read from the last byte offset. New lines go through the same turn assembly (§5.1), so a line extending an existing `message.id` or delivering a `tool_result` updates sections in place. Keep a trailing partial line in a buffer until its newline arrives. If the file shrinks or is replaced (size < offset or inode change), reload it. |
| OpenCode | `notify` watch on `opencode.db` and `opencode.db-wal`, debounced 200 ms; fallback poll every 1 s if watching fails | `SELECT … FROM session_message WHERE session_id = ? AND seq > ?`, plus re-reading rows whose `time_updated` changed (streaming messages are updated in place) |

- The database is opened read-only (`SQLITE_OPEN_READ_ONLY`). Chronoscope never writes to any harness store.
- Watch events are coalesced. Workers parse new data off the UI thread and send `ModelUpdate`s; the UI thread applies them (§1.4) and redraws at most every 50 ms.

### 7.2 Browser behaviour

- **Follow when at bottom:** if the viewport is at the end of the transcript, new content scrolls into view automatically. If the user has scrolled up, the position is kept and a `↓ N new` indicator appears in the help line; `G` / `End` jumps to the end and resumes following.
- Updates to content already displayed (e.g. a tool result arriving for a pending tool call, or a streamed message growing) update the section in place. The current selection and fold states are preserved.
- A `● live` marker is shown in the browser header while the session is active: file/rows modified within the last 60 s, or OpenCode `time_idle` is null.

### 7.3 Selector behaviour

- Project and session lists update live: new sessions appear, and titles and stats refresh. The current selection is kept (matched by id, not by index).
- Active sessions show the `●` marker in their row.

## 8. Configuration & theming

### 8.1 Theme model

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

### 8.2 Emphasis rules

Selection is shown with **text brightness and markers**. The one background band is the session selector's focused selection:

- Unselected list rows and transcript sections: `muted_foreground`.
- Selected transcript section and theme-selector row: `foreground`, bold, with a marker (`>` in the theme list, `▒` over the left border in the transcript) in `primary`.
- Session selector (Projects and Sessions lists):
  - Selected row in the **focused** list: the `>` marker in `primary`, never inverted. Everything after the marker is one **inverted** band (`foreground` with the reverse-video modifier, not bold), running up to the row's right margin (two cells before the pane border). Inside the band, the timestamp, live dot, title, gaps and stats all take the band's style.
  - Selected row in the **unfocused** list (e.g. Projects while Sessions has focus): text **bold** in `secondary_foreground`, not inverted; the `>` marker in the same `secondary_foreground` (not bold), so the context stays visible.
- Focused pane border: `ring`; unfocused borders: `border`.
- Selected harness tab: `primary`, underlined; other tabs: `muted_foreground`.
- In the transcript, dimming of unselected sections is controlled by `dim_unselected` (default `true`). When `false`, all sections use their normal token colours and only the marker + bold indicate selection.

### 8.3 Built-in themes

Compiled into the binary:

| Id | Display name | Source |
|---|---|---|
| `default` | Default | ratcn `default_dark` (shadcn palette) |
| `terminal` | Terminal | Terminal's own ANSI palette, with `reset` for background and foreground |
| `catppuccin` | Catppuccin | ratcn `catppuccin` (Mocha) |
| `gruvbox` | Gruvbox | ratcn `gruvbox` (dark) |
| `nord` | Nord | ratcn `nord` |
| `tokyo-night` | Tokyo Night | ratcn `tokyo_night` (night) |
| `solarized` | Solarized | ratcn `solarized` (dark) |

- Palette values are ported from ratcn (MIT, with attribution).
- The id is used in `config.toml` and `--theme`. The display name comes from the theme's `name` field.
- The default theme is `default`.

User themes: `$XDG_CONFIG_HOME/chronoscope/themes/<name>.toml`. A user theme with the same name as a built-in one replaces it.

### 8.4 Theme selector overlay

Layout per [theme-selector.md](theme-selector.md):

```
╭─ Themes ──────────────────────────────╮
│                                       │
│    Default                            │
│  > Terminal                           │
│    Catppuccin                         │
│    Gruvbox                            │
│    Nord                               │
│    Tokyo Night                        │
│    Solarized                          │
│                                       │
│ ENTER to select | ESC to cancel       │
╰───────────────────────────────────────╯
```

- Opened with `t` from the browser or the selector. It is a small centred overlay drawn on top of the current screen, so the preview is visible on real content around it.
- Lists the built-in themes in the order of §8.3, followed by user themes alphabetically. If there are more themes than fit, the list scrolls.
- On open, the cursor is on the active theme.
- `↑` / `↓` or `j` / `k` move the cursor. The highlighted theme is **applied immediately as a preview**, to the overlay and to everything behind it.
- `ENTER` selects the highlighted theme, closes the overlay and saves the choice to `config.toml`.
- `ESC` closes the overlay and restores the theme that was active when it opened.
- The selected row follows §8.2 (`>` marker in `primary`, bright bold text, not inverted); other rows use `muted_foreground`. The help line uses `help_key` / `help_text`.
- A user theme that fails to parse is listed muted with `(invalid)` and can't be selected. The error goes to the log.
- The config file is updated with `toml_edit`, so the user's comments and formatting survive. If no config file exists, one is created containing just `theme = "…"`.

### 8.5 Config file

- Location: `$XDG_CONFIG_HOME/chronoscope/config.toml` (`~/.config/chronoscope/config.toml`), via `etcetera`. On macOS the XDG-style path is used as well, so dotfiles stay portable. On Windows: `%APPDATA%\chronoscope\config.toml`.
- All keys are optional. A missing file means defaults.
- Unknown keys produce a warning (shown once as a toast), not an error.
- Invalid values fall back to the default with a warning.
- Changes are picked up on restart; hot reload is future work.

```toml
theme = "tokyo-night"
transparent_background = false   # overrides the theme's setting when present
mouse = true
show_system = true
dim_unselected = true
timestamp_format = "%Y-%m-%d %H:%M:%S"   # strftime; displayed in local time
utc = false
truncate_output_lines = 200

[harness.claude]
enabled = true
data_dir = "~/.claude/projects"            # override auto-detection
resume_command = "claude --resume {id}"

[harness.opencode]
enabled = true
db_path = "~/.local/share/opencode/opencode.db"
resume_command = "opencode --session {id}"

[projects.aliases]
"/home/jacob/Development/projects/brio" = "BRIO"
"/home/jacob/Development/projects/camera-settings" = "CameraSettings"

[keys.browser]                 # action = key or [keys]; replaces the default for that action
fold_toggle = ["enter", "space"]
copy = "y"
copy_full = "Y"

[keys.selector]
search = "/"
```

Key syntax: `a`, `A`, `ctrl-d`, `shift-tab`, `enter`, `esc`, `pgdn`, `f5`, and two-key chords as space-separated keys (`"g g"`; only in user config, since no default binding is a chord). The help overlay is generated from the active keymap, so it always reflects the user's remappings.

### 8.6 Timestamps

- Displayed in local time using `timestamp_format`. With `utc = true`, UTC is shown with a `Z` suffix.
- The selector's session rows use the same format without seconds.

## 9. Non-functional requirements

### 9.1 Platform support

| Tier | Platform | Commitment |
|---|---|---|
| 1 | Linux | Primary development and manual testing. |
| 2 | macOS | CI build + full test suite; platform bugs fixed; manual smoke test when hardware is available. |
| 3 | Windows | Best effort: CI build + test suite; features needing substantial platform-specific work may be degraded. |

Rules that keep tiers 2–3 cheap:
- No direct OS APIs; only cross-platform crates (crossterm, notify, arboard, rusqlite bundled, etcetera).
- All filesystem paths are `PathBuf`; no hard-coded `/` separators or `~` expansion by string concatenation.
- Harness data locations are resolved from a per-platform candidate list and can be overridden in config. The Claude location is `~/.claude/projects` on all platforms. The OpenCode location on macOS and Windows is **unverified** and must be confirmed before tier-2/3 release.
- Every harness adapter is tested against recorded fixture sessions (trimmed copies of real sessions), so parsing is verified on every OS in CI.

### 9.2 Scale & performance

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

### 9.3 CI & releases

- Hosted on GitHub.
- CI: a GitHub Actions matrix of `ubuntu-latest`, `macos-latest` and `windows-latest` runs `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test`.
- Releases: tagged builds produce binaries for x86_64/aarch64 Linux, aarch64/x86_64 macOS and x86_64 Windows (e.g. via `cargo-dist`).

### 9.4 Terminal environment

Primary environment: inside terminal multiplexers (**tmux**, **herdr**).

- Clipboard: try `arboard` first. When no display server is reachable or `$TMUX` is set, also emit OSC52.
  - Under tmux, wrap OSC52 in the DCS passthrough sequence.
  - Document `set -g set-clipboard on` and `set -g allow-passthrough on`.
  - herdr's OSC52 behaviour must be verified.
- `Esc` responsiveness: document tmux `set -sg escape-time 10` (the default 500 ms makes `Esc` feel laggy).
- No feature depends on the kitty keyboard protocol or on modifier+Enter. Every default binding works with legacy key encoding.
- Colours: truecolor is assumed when `$COLORTERM` is `truecolor`/`24bit`. Otherwise hex colours are mapped to the nearest xterm-256 colour. The `terminal` theme uses only ANSI colours.
- Respects `NO_COLOR` by falling back to the `terminal` theme with styling (bold, underline, reverse) only.

### 9.5 Robustness

- Never writes to harness data. The SQLite database is opened read-only.
- Unknown entry types, unknown content blocks and malformed lines are skipped and counted, not fatal.
  - A muted `N entries could not be parsed` note appears in the browser header.
  - Session metadata entries (known ones such as `agent-name`, `queue-operation`, `continued-in`, `bridge-session`, `tools-used`, and unknown types without a `message`) carry no conversation content: they are skipped and logged, but not counted in the note.
  - Details are written to a log file at `$XDG_STATE_HOME/chronoscope/chronoscope.log`.
- The schema version is checked on open (OpenCode `migration` table / Claude `version` field). An unknown newer version logs a warning, and Chronoscope still tries best-effort parsing.
- Terminal state is always restored on exit, panic or signal (panic hook + raw-mode guard).

### 9.6 Testing

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

## 10. Out of scope / future

Future work (not v1):
- Codex and Pi adapters. Their tabs are hidden until implemented.
- Hot reload of config and theme files.
- Full-text search across all sessions (needs a background index).
- `adaptive` theme (derive the palette from the terminal's OSC 10/11 colours) and light theme variants.
- Fold levels (`1`/`2`/`3`). Considered and rejected for now.
- Using `tui-realm-stdlib` components (e.g. dialogs) if hand-rolled ones prove costly and the stdlib look can match §8.

Explicit non-goals:
- Editing, deleting or archiving sessions, or writing to any harness store.
- Running or controlling agents, other than handing off via "resume in harness".
- Remote/cloud session sources.

## 11. Open items to verify

- OpenCode data directory on macOS and Windows (§9.1).
- OpenCode resume flag (`opencode --session <id>`) (§6.7).
- herdr OSC52 / clipboard passthrough behaviour (§9.4).
- Claude compaction format (no compacted session available locally to sample) (`system` `compact_boundary` + summary entry) and OpenCode compaction (`session_pending` type `compaction`) (§5.2).
- Reasoning-effort field in OpenCode (`model.variant`?) (§5.1).
