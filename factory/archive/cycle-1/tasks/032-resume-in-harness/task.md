# Resume a session in its harness

## Goal
Implement "resume in harness" (§6.7). `R`, in the browser or on a selected session row in the selector, asks for confirmation in the help line (`y/n`). After confirmation Chronoscope:
1. restores the terminal;
2. changes to the session's cwd;
3. replaces its own process with the harness's resume command (`exec` on Unix; spawn-and-wait on Windows, then exit with the child's status).

The command comes from the configurable template (task 009 / 028 `resume_command`). If the cwd no longer exists, an error is shown and nothing happens.

## Spec excerpts
From SPEC.md §6.7:

`R` (in the browser, or on a selected row in the selector) asks for confirmation, then:
1. restores the terminal;
2. changes to the session's cwd;
3. replaces the Chronoscope process with the harness's resume command (`exec` on Unix; spawn-and-wait on Windows).

Commands are configurable templates (§8.5). Defaults:
- Claude: `claude --resume {id}`
- OpenCode: `opencode --session {id}`. **Verify this flag** before release.

If the cwd no longer exists, show an error and don't resume.

From SPEC.md §6.5:

| Key | Action |
|---|---|
| `R` | Resume the session in its harness (§6.7) |

From SPEC.md §6.6:

| Key | Action |
|---|---|
| `R` | Resume the selected session in its harness (§6.7) |

From SPEC.md §4.3:

- **Confirmation prompts** (overwrite file, resume in harness) appear in the help line as `y/n` questions, not as dialogs.

From SPEC.md §10:

- Running or controlling agents, other than handing off via "resume in harness".

From SPEC.md §11:

- OpenCode resume flag (`opencode --session <id>`) (§6.7).

From SPEC.md §9.5:

- Terminal state is always restored on exit, panic or signal (panic hook + raw-mode guard).

From SPEC.md §8.5:

```toml
[harness.claude]
enabled = true
data_dir = "~/.claude/projects"            # override auto-detection
resume_command = "claude --resume {id}"

[harness.opencode]
enabled = true
db_path = "~/.local/share/opencode/opencode.db"
resume_command = "opencode --session {id}"
```

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **Trigger.**
  - In the browser, `browser.resume` returns `Msg::RequestResume`, which targets the open session. In a subagent view it targets the top-level session.
  - In the selector, `selector.resume` on a selected session row returns `Msg::RequestResume` for that session. With no session selected (e.g. Projects focused with none selected) it does nothing.
  - The `App` checks first:
    - if `resume_command()` returns `None`, toast `No resume command configured for <harness>`;
    - if the cwd doesn't exist, toast `Cannot resume: <cwd> no longer exists`.

    Otherwise it mounts the `Prompt` in `YesNo` mode with `Resume <short id> in <Harness>? (y/n)`, where the short id is the first 8 characters.
- **Plan, then act.**
  - On `y`, the `App` stores a pure `ResumePlan { cwd, program, args }` and sets the quit state. The main loop, after leaving the loop and restoring the terminal through the normal guard, executes the plan.
  - Separate the decision (pure, testable) from the execution (`exec`), which is a thin untested function.
  - Execution:
    1. `std::env::set_current_dir(cwd)`;
    2. on Unix, `std::os::unix::process::CommandExt::exec` (it only returns on error; then print the error to stderr and exit 1);
    3. on Windows, `Command::status()` and exit with its code.
  - Any other answer cancels and returns focus.
- The command is the `std::process::Command` from `resume_command()`. Build the plan from it (program, args, current dir), or reuse it directly. Never pass through a shell.
- **Not verified yet (§11).** Keep `opencode --session {id}` as the default. If `opencode` is installed on this machine, run `opencode --help` and record whether `--session` exists in the build notes.

Planner note — keys (binding): components resolve keys only through the keymap from task
005 (`Keymap`, contexts `global` / `browser` / `selector` / `overlay`, action names as defined
there). The only literal keys are `Ctrl-c`, which always quits, and the printable characters,
`Enter`, `Esc`, `Backspace` and arrow keys that `Prompt` takes in input mode.

## Acceptance criteria
- [ ] `R` in the browser shows `Resume <short id> in Claude? (y/n)` in the help line (update test plus snapshot).
- [ ] `R` on a selected session in the selector shows the same prompt for that session. `R` with no session selected does nothing (update tests).
- [ ] `y` produces a `ResumePlan` with cwd = the session cwd, program `claude` and args `["--resume", "<id>"]` (default template), and sets quit. `n`/`Esc` produce no plan and restore focus (update tests).
- [ ] A custom template from config is honoured in the plan (test).
- [ ] A missing cwd shows the `Cannot resume: … no longer exists` toast and no prompt (update test with a temp dir removed).
- [ ] A harness without a resume command shows the `No resume command configured` toast (update test).
- [ ] The executor restores the terminal before changing directory and exec'ing. The call order is verified through a seam (e.g. an injected executor that records calls), and the Unix `exec` path is behind `#[cfg(unix)]`.
- [ ] The `opencode --help` verification result (or "opencode not installed") is in the build notes.
- [ ] Every UI state below has a snapshot.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
Changes to the help-line row (browser) and the selector's help-line row:
- **Confirmation.** `Resume 85a86657 in Claude? (y/n)` replaces the help line, in `help_text`, with the short id and harness name in `foreground`.
- **Error toasts** (task 012 style): `Cannot resume: /path no longer exists`, `No resume command configured for OpenCode`.

Snapshots (screen through `App::view`, `utc = true`):
1. Browser with the resume confirmation at 80x24 (text + styled).
2. Selector with the resume confirmation, a session selected, at 120x40 (text).
3. The missing-cwd error toast over the browser at 80x24 (text).

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 021-browser-screen
- 028-opencode-adapter
