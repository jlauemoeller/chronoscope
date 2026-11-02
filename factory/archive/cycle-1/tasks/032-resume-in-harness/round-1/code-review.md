VERDICT: PASS

## Summary
Resume in harness is implemented cleanly. `R` in the browser and on a selected session row in the selector produces `Msg::RequestResume`. The `App` checks for a resume command and that the cwd exists, asks the `y/n` question in the help line, and on `y` stores a pure `ResumePlan` and quits. After the loop, `run_app` hands off through `hand_off(plan, &mut dyn Steps)`, which restores the terminal, then changes directory, then runs the command. The order is tested through a recording `Steps` fake, and `exec` is behind `#[cfg(unix)]`. No shell is involved. Every acceptance criterion has an effective test, the four required snapshots are committed as accepted `.snap` files, and the test gate is green. I found no must-fix issues, only nits.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites ok, 0 failed; no `.snap.new` files written or committed)

## Findings
1. [nit] `src/app/mod.rs` (the new guarded `PromptSubmit` / `PromptCancel` / `PromptChanged` / `PromptNav` arms) and `src/app/browser.rs` (`unmount_selector`, `open_session`):
   - **Problem.** The routing trusts `self.resume.pending`, which only `resume_answered` and `resume_declined` clear. `unmount_selector()` also unmounts `Id::Prompt`. It runs from `open_session`, `close_selector` and the `open_selector` failure path, and it clears `selector.search` but not `resume.pending`.
   - **Why it isn't must-fix.** None of those paths can run while the resume prompt has focus today: they are reached only by keys that the focused Prompt takes first, and there is no mouse handling yet.
   - **Why it matters later.** Once §6.9's "click outside the overlay closes it" exists, the state can be left behind. The next prompt (e.g. `/` search, then `y`, then Enter) would be routed to `resume_answered` and would exec the old plan.
   - **Change.** Clear `self.resume.pending = None` wherever the Prompt can be unmounted outside `resume_*`, at least in `unmount_selector` (or `open_session` and `close_selector`). Alternatively, tie the routing to which prompt is mounted. Add an update test for this when mouse support lands.
2. [nit] `src/app/mod.rs`, `Msg::RequestResume(Option<(HarnessId, SessionId)>)`: `None` meaning "the open session" is implicit. A small enum, e.g. `ResumeTarget::{Open, Session(HarnessId, SessionId)}`, would make the browser and selector cases self-describing at the call sites (`transcript/mod.rs`, `session_list.rs`, `resume.rs::resume_target`).
3. [nit] `src/components/prompt.rs`, `Question { between, second }`: "one subject" is encoded as two empty strings, and `confirm_two` takes five positional `&str`s. An `Option<(String, String)>` for the second part, or a slice of `(text, is_subject)` parts, would remove the empty-string sentinel and the long positional list.
4. [nit] `src/app/resume.rs`, `SystemSteps::run` (unix): `log::logger().flush()` runs before `exec`, but stdout is not flushed. Nothing is known to be left buffered, since the terminal guard has already been dropped, but a `let _ = std::io::stdout().flush();` before `exec` would make sure no clipboard or OSC output is lost when the process image is replaced.
