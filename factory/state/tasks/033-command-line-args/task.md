# Command-line interface

## Goal
Implement the full command line of §6.8 with `clap`:
- **Default.** With no arguments, the selector opens with `$PWD` preselection (as today).
- **Positional argument.** `chronoscope <session-id | path>` opens a session directly. Ids are looked up across harnesses by exact id or unique prefix; a path is a Claude `.jsonl` file. An ambiguous prefix opens the selector filtered to the matches.
- **`--latest`.** Opens the newest session for `$PWD`, or for `--project`.
- **Options.** `--harness` restricts the lookup and preselects the tab. `--project` preselects a project. `--theme` overrides the theme for this run. `--config` uses an alternative config file.

## Spec excerpts
From SPEC.md §6.8:

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

From SPEC.md §4.1:

Startup (no arguments):
- Opens the selector.
- Preselects the harness and project whose cwd matches `$PWD` (or the nearest ancestor), falling back to the most recently active project (the first in the list).
- Focus is on the Projects pane and **no session is selected**: the Sessions pane lists the project's sessions without a `>` marker.

From SPEC.md §6.6:

- Harness numbers are stable: `1` Claude, `2` OpenCode, `3` Codex, `4` Pi.

From SPEC.md §8.3:

- The id is used in `config.toml` and `--theme`. The display name comes from the theme's `name` field.

From SPEC.md §8.5:

- Location: `$XDG_CONFIG_HOME/chronoscope/config.toml` (`~/.config/chronoscope/config.toml`), via `etcetera`. On macOS the XDG-style path is used as well, so dotfiles stay portable. On Windows: `%APPDATA%\chronoscope\config.toml`.

From SPEC.md §3.1:

- Location: `~/.claude/projects/<encoded-cwd>/<session-uuid>.jsonl` (append-only JSONL).
- Directory name is a lossy encoding of the cwd (`/` and `.` → `-`); the real cwd is in each entry's `cwd` field.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Parsing.** Use a `clap` derive struct. `--harness` takes the values `claude` and `opencode` (case-insensitive). `--latest` conflicts with the positional argument.
- **Resolution before the TUI starts** (a pure function over the loaded index plus the parsed args, unit-tested):
  1. `--config <path>`: use that file instead of the default (task 003's `Paths` override). A missing file at that path means defaults, as usual.
  2. `--theme <id>`: overrides the config theme for this run only. An unknown id gives a startup warning toast and the default resolution, as in task 004. It is never saved.
  3. Positional argument:
     - **A path.** The argument ends with `.jsonl`, or is an existing file. A path to an existing `.jsonl` file opens directly as a Claude session, with project and cwd taken from its entries. It need not be under the data dir; outside the data dir, subagent lookup still uses `<stem>/subagents/` next to the file. A non-existing path is an error.
     - **An id.** Otherwise the argument is an id. Search every registered harness, or only `--harness`. An exact id match, or a unique prefix match, opens that session. Several prefix matches start the selector in "filtered" mode. No match is an error.
  4. `--latest`: the newest session (by `started_at`) of the project matching `--project` (if given) or `$PWD` (deepest ancestor rule from task 014), within `--harness` if given. Open it directly. No project or no session is an error.
  5. `--project <path|name>`: a path (absolute, or relative to `$PWD`; compared by components) or a display name/alias (case-insensitive). It preselects that project at startup instead of the `$PWD` rule. No match gives a warning toast, and the normal preselection is used.
  6. `--harness` without other arguments preselects that tab (if available).
- **Errors** print to stderr (`chronoscope: no session matches "abc"`) and exit with code 1, without entering the TUI. Errors are detected before the terminal is set up.
- **Filtered mode.** The selector opens with Sessions focused, showing exactly the matching sessions as a results list (project prefix, like the task-015 search results), titled ` Sessions matching "<prefix>" `. The first one is selected. `Esc` leaves filtered mode and returns to the normal startup state (Projects focused, no session selected). `Enter` opens the selection.
- **Opening directly** starts the app in the browser with the session loaded (task 021). `Ctrl-o` then opens the selector over it as usual. Index loading still happens in the background.
- Lookups need the index. At startup, build each harness's project and session lists synchronously once (it's cached and parallel, task 008) when the positional argument, `--latest` or `--project` is used. Otherwise keep the current asynchronous startup.

## Acceptance criteria
- [ ] `chronoscope --help` lists the positional argument and `--latest`, `--harness`, `--project`, `--theme` and `--config`. `--latest` together with a positional argument is rejected by clap (test).
- [ ] Each resolution is unit-tested on fixture indexes:
  - exact id → open;
  - unique prefix → open;
  - ambiguous prefix → filtered selector with exactly the matches;
  - no match → error;
  - `--harness opencode` restricts the lookup.
- [ ] A `.jsonl` path opens that file as a Claude session, including a file outside the data dir. A missing path is an error (tests).
- [ ] `--latest` opens the newest session of the `$PWD` project, or of `--project`. With no match it is an error (tests).
- [ ] `--project` by path and by name/alias (case-insensitive) preselects that project. An unknown project warns and falls back (tests).
- [ ] `--theme gruvbox` applies Gruvbox for the run without writing config. An unknown theme warns (tests).
- [ ] `--config <path>` loads that file, and theme saving (task 031) writes to it (test).
- [ ] An error exits with code 1 and a `chronoscope: …` message on stderr without touching the terminal (integration test running the binary with a temp `HOME`/data dir).
- [ ] The filtered selector looks and behaves as specified, and `Esc` leaves filtered mode (update tests plus snapshot).
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
- **Filtered selector** (ambiguous id prefix):
  - Sessions pane focused, titled ` Sessions matching "85a8" `;
  - rows are `<timestamp>  <dot slot><project> › <title>` plus stats (task 015 result-row format), with the first row selected (focused band);
  - the Projects pane keeps its normal list, with no focus.
- **Direct open.** The app starts in the browser screen (task 021 layout) with the session loading.

Snapshots (screen through `App::view`, fixture indexes, `utc = true`):
1. Filtered selector with three matches at 120x40 (text + styled).
2. After `Esc` from the filtered selector: the normal startup state, at 120x40 (text).
3. Direct open of a session id: the browser at 80x24 after loading (text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 015-selector-search
- 021-browser-screen
- 028-opencode-adapter
- 031-theme-selector
