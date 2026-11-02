# Release builds and user documentation

## Goal
Make Chronoscope releasable and documented.
- **Releases.** A tag-triggered GitHub Actions release workflow builds binaries for x86_64/aarch64 Linux, aarch64/x86_64 macOS and x86_64 Windows.
- **README.** It covers installation, usage and the command line, the keybindings (pointing to `?`), the config file with every key, themes and user themes, mouse and native text selection, and the terminal-multiplexer notes the spec asks to document (tmux clipboard and passthrough settings, `escape-time`).
- **Open items.** The README lists the open items still to verify and credits ratcn.

## Spec excerpts
From SPEC.md §9.3:

- Hosted on GitHub.
- CI: a GitHub Actions matrix of `ubuntu-latest`, `macos-latest` and `windows-latest` runs `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test`.
- Releases: tagged builds produce binaries for x86_64/aarch64 Linux, aarch64/x86_64 macOS and x86_64 Windows (e.g. via `cargo-dist`).

From SPEC.md §9.4:

Primary environment: inside terminal multiplexers (**tmux**, **herdr**).

- Clipboard: try `arboard` first. When no display server is reachable or `$TMUX` is set, also emit OSC52.
  - Under tmux, wrap OSC52 in the DCS passthrough sequence.
  - Document `set -g set-clipboard on` and `set -g allow-passthrough on`.
  - herdr's OSC52 behaviour must be verified.
- `Esc` responsiveness: document tmux `set -sg escape-time 10` (the default 500 ms makes `Esc` feel laggy).
- No feature depends on the kitty keyboard protocol or on modifier+Enter. Every default binding works with legacy key encoding.
- Colours: truecolor is assumed when `$COLORTERM` is `truecolor`/`24bit`. Otherwise hex colours are mapped to the nearest xterm-256 colour. The `terminal` theme uses only ANSI colours.
- Respects `NO_COLOR` by falling back to the `terminal` theme with styling (bold, underline, reverse) only.

From SPEC.md §9.1:

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

From SPEC.md §8.5:

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

From SPEC.md §8.3:

User themes: `$XDG_CONFIG_HOME/chronoscope/themes/<name>.toml`. A user theme with the same name as a built-in one replaces it.

- Palette values are ported from ratcn (MIT, with attribution).

From SPEC.md §6.9:

- Mouse capture can be disabled in config (`mouse = false`). Most terminals also bypass mouse capture while `Shift` is held, which allows native text selection; the help overlay mentions this.

From SPEC.md §11:

#### 11. Open items to verify

- OpenCode data directory on macOS and Windows (§9.1).
- OpenCode resume flag (`opencode --session <id>`) (§6.7).
- herdr OSC52 / clipboard passthrough behaviour (§9.4).
- Claude compaction format (no compacted session available locally to sample) (`system` `compact_boundary` + summary entry) and OpenCode compaction (`session_pending` type `compaction`) (§5.2).
- Reasoning-effort field in OpenCode (`model.variant`?) (§5.1).

Planner notes (decisions not in the spec; binding for this task):
- **Release workflow.** `.github/workflows/release.yml` runs on tags `v*`. Use `cargo-dist` if its generated workflow can be produced and committed here (`dist init` / `dist generate`). Otherwise, hand-write a matrix workflow:
  - `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` (an `ubuntu-24.04-arm` runner, or `cross`);
  - `x86_64-apple-darwin` and `aarch64-apple-darwin` (`macos-latest` with both targets);
  - `x86_64-pc-windows-msvc`.

  Each job runs `cargo build --release --locked --target <t>`, packages the binary (tar.gz; zip on Windows) with README and licence files, and uploads it to the GitHub release for the tag.
  - Validate the YAML (e.g. `actionlint` if available, or a YAML parse) and record how in the build notes.
  - Do not push tags or trigger releases.
- **Licence.** If the repo has no licence file, do not invent one. Package `README.md` and `THIRD-PARTY-NOTICES.md` only, and note in the build notes that the project's own licence is still to be chosen.
- **README.md** at the repo root, with sections:
  - what Chronoscope is (harnesses supported: Claude Code, OpenCode);
  - install (from releases; `cargo install --path .`);
  - usage and the command line (§6.8);
  - keybindings overview (the main keys, and that `?` shows all, remapped ones included);
  - configuration (location per platform, the full example from §8.5, unknown keys produce warnings, restart to apply);
  - themes (the built-in list, user themes in `themes/<name>.toml` with the task-002 TOML format and an example, `NO_COLOR` and `COLORTERM` behaviour);
  - mouse (`mouse = false`; hold `Shift` for native selection);
  - **Terminal multiplexers:** the tmux settings `set -g set-clipboard on`, `set -g allow-passthrough on` and `set -sg escape-time 10` with a one-line explanation each; herdr's OSC52 behaviour is unverified;
  - data sources and that Chronoscope never writes to them;
  - open items to verify (§11, updated: Claude compaction samples now exist locally);
  - acknowledgements (ratcn palettes, MIT, with a pointer to `THIRD-PARTY-NOTICES.md`).
- Keep the README's key names consistent with the keymap defaults (task 005) and the config keys with task 003. A test can assert that the README's config example parses without warnings with `Config` loading.
- CI (`ci.yml`) stays as it is.

## Acceptance criteria
- [ ] `.github/workflows/release.yml` exists, triggers on `v*` tags, and builds and uploads artifacts for all five targets from §9.3. The YAML validates (as recorded in the build notes).
- [ ] `README.md` contains every section listed in the planner notes, including the three tmux settings verbatim, and mentions that herdr's OSC52 behaviour is unverified.
- [ ] The config example in the README matches §8.5. A test extracts the README's TOML block and loads it with `Config`, getting zero warnings.
- [ ] The README's keybinding overview matches the default keymap: spot-checked by a test or by review against task 005's table, with no `Ctrl-c` as copy and no modifier+Enter bindings.
- [ ] The open items from §11 are listed in the README.
- [ ] The ratcn acknowledgement is present, and `THIRD-PARTY-NOTICES.md` is referenced.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 036-large-session-perf
