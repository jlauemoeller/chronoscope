# Shared duration format

## Goal
Give the session selector's rows and the stats footer one duration format and one formatter. Today each has its own `format_duration` and they disagree (`12m` against `12m 05s`). Later tasks use the same formatter for the turns sidebar, the details overlay and long tool calls.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §2 (the footer this cycle ends up with; tokens and `cached` come in a later task):

The footer of an Assistant block becomes:

```
5m 2s · 12 messages · 45.2k tokens · 359.4M cached
```

Unknown parts are left out as today. Cost, when known, stays last (`· $0.42`).

From SPEC.md §2.4:

Durations use **one format everywhere**: the session selector's rows, the stats footer, the turns sidebar (§6.5), the details overlay (§5.3), and tool calls of a minute or longer (§3). Today the selector and the footer each have their own `format_duration` and they disagree: the selector shows `12m` and the footer `12m 05s`.

| Duration | Resolution | Examples |
|---|---|---|
| Under 1 minute | 1 second | `0s`, `45s` |
| 1 minute up to, but not including, 1 hour | 1 second | `1m 0s`, `4m 3s`, `59m 59s` |
| 1 hour or longer | 1 minute | `1h 0m`, `3h 4m`, `2h 45m`, `26h 30m` |

- Numbers are **not zero-padded**.
- Values are truncated, not rounded, so `59m 59.9s` shows `59m 59s`.
- There is one shared formatter. Remove `format_duration` from both `src/components/session_list.rs` and `src/components/turn_block/renderers/stats.rs`, and put the shared one where neither component depends on the other.
- Only display changes: cached session stats store milliseconds, so the stats cache keeps working as is.

The selector's rows gain seconds under an hour, so `59m` can become `59m 59s`, up to 4 cells wider. No layout change is needed: `row_parts` already measures the stats, gives the title the remaining room, and drops the stats below `MIN_TITLE_WIDTH`. On narrow panes, titles lose those cells and stats can drop out slightly sooner.

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

From spec/SPEC.md §1.3:

**Every custom UI element is its own component.** The code is organised around components, not around long render functions or a central key handler.

- A component implements tui-realm's `Component` trait (`view`, `query`/`attr`, `state`, `perform(Cmd) -> CmdResult`), plus `AppComponent` (`on(&Event) -> Option<Msg>`) if it handles events. An element that appears on several screens (e.g. the help line) is one component type, mounted with different props.
- A component draws only itself. Its `view` draws its own border and content, and leaves sub-elements to child types: a child component it owns, or a small `Widget`/`StatefulWidget` type.
- **No long render functions.** A render function (`view`, or a helper it calls) that draws more than one distinct element, or grows past about 50 lines, is split into smaller components or widgets. Rendering has no side effects.
- Components are `pub` types that can be built from plain data (props and the theme) and rendered into a `TestBackend` frame on their own, without an `Application` or a terminal (§9.6).
- All colours and modifiers come from theme tokens (§8.1), passed in as a prop. Components never hard-code colours.
- `tui-realm-stdlib` is not used. The look in §4 and §8 is specific enough that wrapping stdlib components would mean overriding their rendering. Prompt text editing uses `tui-input` inside the `Prompt` component.

From spec/SPEC.md §4.1 (session rows):

Session row format: `<local start timestamp>  <title>` on the left, plus muted stats on the right (turn count · duration). The model is not shown: the browser shows it on every Assistant turn.
- Title: Claude `ai-title` → OpenCode `session_v2.title` → first user prompt (truncated) → `(untitled)`.
- Stats are computed in the background and cached (§9.2). Until they are ready, the row shows just timestamp + title.

From spec/SPEC.md §5.2 (blocks and sections):

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

From spec/SPEC.md §9.2 (scale and performance):

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

From spec/SPEC.md §9.6 (testing):

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):

- The shared formatter is `pub fn format_duration(duration: Duration) -> String` in a new crate-level module `src/duration.rs`, next to `src/timestamp.rs`, so neither component depends on the other.
- Remove `format_duration` from `src/components/session_list.rs` and `src/components/turn_block/renderers/stats.rs`, and update the re-exports in `src/components/mod.rs` and `src/components/turn_block/renderers/mod.rs`. The two `format_stats` functions stay and call the shared formatter.
- The tests that assert the old format are expected to change: `src/components/turn_block/tests.rs` (`1m 00s`, `5m 02s`, `1h 05m`) and `src/components/session_list.rs` (`12m`, `1h 05m`).
- Doc comments that describe the old format are updated with the code.

## Acceptance criteria
- [ ] Format tests cover 0s, 59.9s, 60s, 5m 2s, 59m 59.9s, exactly 1h, 1h 5m and more than 100h. The expected strings are `0s`, `59s`, `1m 0s`, `5m 2s`, `59m 59s`, `1h 0m`, `1h 5m` and, for example, `100h 30m`.
- [ ] Values are truncated, not rounded, and numbers are not zero-padded: a test shows 1h 5m 59s gives `1h 5m`.
- [ ] Selector and footer tests assert the new format, including a selector row whose stats no longer fit beside a short title. That row's stats fit in the old format (`59m`) at the chosen width and drop out in the new one (`59m 59s`), through the existing `row_parts` and `MIN_TITLE_WIDTH` logic.
- [ ] Only one duration formatter remains for these uses (tool calls under a minute keep their own, §3). A search of `src/` finds one `fn format_duration`, in `src/duration.rs`.
- [ ] `SessionStats` and the stats cache are unchanged: durations are still stored as milliseconds, and existing cache tests pass without edits.
- [ ] No layout code changes for the selector's rows.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] `cargo test --release -- --ignored budgets --nocapture` stays within budget.
- [ ] Snapshot changes are limited to rows showing a footer or selector-row duration of a minute or longer. Each changed snapshot differs only in that duration, and in a title or stats that re-fit because of it.

## UI
Text changes only; no layout, border, colour or focus changes.

- Selector session rows: the stats on the right read `<n> turns · <duration>` in the new format, for example `12 turns · 59m 59s`. They are up to 4 cells wider. On narrow panes the title gives up those cells, and the stats drop out when the title would have fewer than `MIN_TITLE_WIDTH` cells.
- Stats footer of an Assistant block: the duration part uses the new format, for example `5m 2s · 12 messages · 45.2k tokens`.

Snapshot sizes: the existing snapshots at their existing sizes, and the row that no longer fits at whichever pane width shows it (a selector at 80x24 is 47 columns wide for Sessions).

## Depends on
none
