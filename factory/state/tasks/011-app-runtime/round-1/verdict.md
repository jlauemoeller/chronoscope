VERDICT: APPROVE

## Assessment
All three reviews are PASS and report only nits. I re-ran the gate on `factory/011-app-runtime`. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean. `INSTA_UPDATE=no cargo test` passes in every test binary: 272 lib tests, 11 pty tests and 1 panic-hook test. The working tree is clean afterwards, with no `.snap.new` files.

**Code review: PASS**
1. `Ctrl-c`, `WindowResize` and `Tick` sit behind `subscription_clause()`. **Ignored for this round, but carry it forward.** It is not a defect today. The clause is `SubClause::Always`, and the pty and loop tests show `Ctrl-c` quitting. The risk is in the future: the doc comment invites later tasks to narrow this one clause. Doing that would silence `Ctrl-c`, which SPEC §1.4 and §6.5 say must always quit, and would also block resizes and ticks. The first task that excludes prompts or overlays from the global keys (help overlay, theme selector, or prompt) should give `Ctrl-c`, resize and tick their own `SubClause::Always` subs. It should narrow the clause only for the keys that come from the keymap.
2. A resize bypasses the 50 ms interval. **Ignored.** The AC contradicts itself here: no draw within 50 ms of the last, yet a resize must be drawn at once. The builder exempts only resize requests, documents the exception and tests both behaviours. Code review and feature review both accept this reading. Any alternative would change resize latency by at most one 50 ms frame, so it is not a meaningful A/B choice for the human. Coalescing resizes during a window drag can wait until it costs something.
3. Two loop tests would hang instead of failing if quitting broke. **Ignored.** Only the test makes this fragile. The behaviour is correct, and the pty tests cover the same path with timeouts.
4. `every_default_quit_key_quits` hardcodes the default keys instead of iterating the keymap. **Ignored.** It matches today's defaults exactly. The subscription tests already build their keys from the keymap.
5. Logging starts before config is loaded, the reverse of planner steps 1–2. **Ignored.** Logging depends only on `Paths`, and this order lets config warnings reach the log. The result is equivalent or better. The deviation is now on record here.
6. There are no per-theme `App::view` snapshots. **Ignored.** This task's binding snapshot convention limits snapshots to `default` plus the transparent state. Task 013 owns the per-theme screen snapshots. The UI reviewer checked all seven themes cell by cell in a scratch crate.
7. Redundant `adapter.restore()` in `Drop`, and a panic in any thread restores the terminal. **Ignored.** The extra restore is idempotent and harmless. No worker threads exist yet, so the worker-panic case belongs to the task that adds workers.

**Feature review: PASS**
1. Undocumented change to the startup order. **Ignored**, as code review #5.
2. tuirealm's adapter installs its own panic hook on top of ours, and our doc comment implies ours is outermost. **Ignored.** The AC asks that restore runs before the previous hook, with restore injectable. `tests/panic_hook.rs` proves exactly that. In production, both hooks restore before the default panic message prints, so the behaviour is correct. Only the doc comment is incomplete.

**UI review: PASS**
1. Per-theme screen snapshots are only a smoke test. **Ignored**, as code review #6 (owned by task 013).
2. Interior cells of `SelectorFrame` keep fg `Reset`. **Ignored.** Nothing is drawn inside the frame in this task, so nothing visible changes. Later child components set their own fg from theme tokens.

## Conflicts
None. Some findings overlap, but they agree with each other:
- Startup logging order: code #5 and feature #1.
- Per-theme snapshots deferred to task 013: code #6 and UI #1.
- The resize exception: code #2 and the feature review's AC notes.

The priority order was not needed.

## Rework budget
Earlier REWORK verdicts: 0 of 2.
