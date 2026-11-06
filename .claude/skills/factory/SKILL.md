---
name: factory
description: Run the software factory - plan a spec into tasks, then build, review, approve and merge them one at a time.
disable-model-invocation: true
argument-hint: start <spec> | continue | approve <task-id> | rework <task-id> <note> | abandon <task-id> | status
---

# /factory: the main orchestrator

Command and arguments: `$ARGUMENTS`

**First, read `.claude/skills/factory/conventions.md` in full.** It defines
the paths (`T`, `R`), the ground rules, the backlog, write verification,
rounds, the preflight state, the reviewer roster, how to update
`board.json`, the merged check, and the Merge and Halting steps. This file
uses them without repeating them.

The session running this skill is the **main orchestrator**. It handles the
commands and their preconditions, the Planner, reconciliation, choosing the
next task, and the backlog bookkeeping around a task. It spawns only the
`planner` and `task-orchestrator` agents, never two task orchestrators at
once, and never does an agent's work itself. It reports every halt to the
user.

## Preconditions

Every command except `status` checks these first. If one fails, refuse with
a message saying which one and why, and do nothing else.

1. `git check-ignore -q factory/state` succeeds (the state is ignored by
   git).
2. The working tree is clean: `git status --porcelain` prints nothing (no
   uncommitted changes, no untracked files that git doesn't ignore).
3. The current branch (`git branch --show-current`) is:
   - `start`: `main`;
   - `continue`: `main`, or `factory/<task-id>` of the task that is
     `In progress`. On that task branch, uncommitted changes are allowed
     (skip check 2) and left for the resumed builder;
   - `approve`, `rework`, `abandon`: `main` or `factory/<task-id>` for the
     task named in the command.

`continue`, `approve`, `rework` and `abandon` also refuse when there is no
run (neither `backlog.md` nor `preflight.md` exists), pointing to
`/factory start <spec>`. `approve`, `rework` and `abandon` refuse unless the
named task is in `backlog.md` with status `Escalated`.

## /factory start <spec>

1. Check the preconditions. Refuse if `<spec>` is not an existing file.
2. Refuse if `factory/state/backlog.md`, `factory/state/tasks/` or
   `factory/state/preflight.md` exists: tell the user to archive the previous
   run (move `factory/state/` to a folder under `factory/archive/`) or remove
   it first. The factory never deletes a previous run.
3. `mkdir -p factory/state`, write `<spec>` (the path as given) as the only
   line of `factory/state/spec-source`, and run **Copying the spec** from the
   conventions. Note any `MISSING` lines for the summary.
4. Create `board.json` with the Write tool (see "Creating or rebuilding"):
   `loop.state` `planning`, message `Planning <spec>`, no tasks.
5. Run **The Planner** below.

## /factory continue

Check the preconditions, then reconcile in this order:

1. **Preflight state** (`preflight.md` exists, `backlog.md` doesn't): re-copy
   the spec with **Copying the spec** (it reads `spec-source` and replaces
   `SPEC.md` and `spec/`). Set `loop.state` to `planning` with message
   `Re-running preflight` (create `board.json` first if it is missing). Run
   **The Planner**. Nothing else in this list applies.
2. **No board:** if `board.json` is missing, rebuild it (see "Creating or
   rebuilding" in the conventions).
3. **A task is `In progress` and merged** (the merged check; the user
   resolved a merge conflict): if you are on its branch, `git switch main`
   (if the tree isn't clean, refuse and ask the user to commit or move the
   changes). Mark it `Complete`, delete its branch with
   `git branch -d factory/<task-id>` (if the branch still exists), refresh its
   board entry, and carry on with step 6.
4. **A task is `In progress` and not merged** (an interrupted run): **Run the
   task** (below). The task orchestrator checks out the branch (or stays on
   it) and resumes at the right step of the latest round from the files.
5. **A task is `Escalated`:** refuse, and point the user to
   `/factory approve <task-id>`, `/factory rework <task-id> <note>` or
   `/factory abandon <task-id>`.
6. Otherwise **run the loop** (below).

## /factory approve <task-id>

Merge an `Escalated` task into main after a human has reviewed it. The task
branch already holds the code and documentation.

1. Check the preconditions (the task must be `Escalated`).
2. Run the **Merge** step from the conventions (it switches to `main` and
   merges with `git merge --no-ff -m "factory: merge <task-id>"`). Before
   halting on a merge conflict, mark the task `In progress` in `backlog.md`,
   so that once the user has resolved the conflicts and committed the merge,
   `/factory continue` finds it merged and marks it `Complete`. Then halt and
   tell the user what to do.
3. If the merge succeeds: mark the task `Complete`, delete the branch with
   `git branch -d factory/<task-id>`, refresh the task's board entry with
   `loop.state` `running` and `current` `none`, and **run the loop** in this
   session.

## /factory rework <task-id> <note>

Send an `Escalated` task back to the builder with the human's feedback.

1. Check the preconditions (the task must be `Escalated`).
2. The note is everything after the task-id. If it is a single argument that
   names an existing file, it must be outside the repository (compare
   `realpath` with `git rev-parse --show-toplevel`): refuse a file inside the
   repo, because it would make the tree unclean, and ask for the text or a
   path outside the repo. Otherwise the note is the text itself. Refuse an
   empty note.
3. Mark the task `In progress` in `backlog.md`.
4. Check out `factory/<task-id>` if you are not on it. If the branch doesn't
   exist (the builder blocked before creating it), create it from `main`
   with `git switch -c factory/<task-id> main`.
5. Create the next round directory `R` (see "Rounds"), write
   `git rev-parse HEAD` to `R/base-commit`, and write the note (the text, or
   a copy of the file) to `R/human-review.md`.
6. **Run the task** (below). The task orchestrator finds a round with a
   `human-review.md` and no build notes, spawns a fresh builder to revise
   the work according to the note, has the reviewers review the round, and
   then runs the Approver straight away. Human rework rounds count against
   neither the automatic round limit nor the Approver's rework limit.

## /factory abandon <task-id>

Give up on an `Escalated` task.

1. Check the preconditions (the task must be `Escalated`).
2. `git switch main` (if you are not on it). Keep the branch
   `factory/<task-id>` so no work is lost; the user can delete it by hand.
3. Mark the task `Abandoned` in `backlog.md` and refresh its board entry with
   `loop.state` `running` and `current` `none`.
4. Find later tasks that depend on it (their `task.md` names the task-id in
   its dependencies section: `grep -l '<task-id>' factory/state/tasks/*/task.md`,
   then check each hit). Warn the user about each one.
5. **Run the loop** in this session.

## /factory status

Read-only and allowed on any branch, with no preconditions. Write nothing,
not even `board.json`, and spawn nothing. Print:

- If there is no run: that there is none, and `/factory start <spec>`.
- In the preflight state: the preflight result (the first line of
  `preflight.md`), its open **Spec issues** and **Environment issues**, and
  its **Tools on offer** with their install commands, instead of the
  backlog.
- Otherwise: the backlog (task-id, title, status); the current task, its
  stage and latest round (with the build, review and verdict results of that
  round); `loop.state` and `loop.message` from `board.json` (or that the
  board is missing and `continue` will rebuild it).
- The action `/factory continue` would take next, worked out with the same
  reconciliation as `continue` (including any precondition that would
  refuse it), without doing it.

## The Planner

Used by `start`, and by `continue` in the preflight state.

1. Write the Planner's snapshot (`factory/state/.snapshot`, see "Write
   verification"), and note the branch and `HEAD`.
2. Spawn the `planner` agent with exactly this prompt:
   `Read factory/state/SPEC.md and the documents in factory/state/spec/.`
3. End your turn with `WAITING` until it reports.
4. Verify its writes: clean tree, still on `main`, same `HEAD`, and only the
   Planner's allowed state files changed. On a violation, halt (see
   "Halting").
5. Check the first line of `preflight.md`. A missing file or a bad first
   line halts.
6. **If it wrote `backlog.md`:** every task in it must have a `T/task.md`
   (otherwise halt: `Planner output incomplete: <missing files>`). Build
   `board.json` (see "Creating or rebuilding"; the state is
   `awaiting-plan-review`). Halt with a summary so the human can review, and
   optionally edit, the plan:
   - the backlog, in order: task-id, title, and what each depends on;
   - the preflight **Assumptions**, including each reviewer baseline;
   - the stack profile from `stack.md`: stack, test gate, no-update mode, UI
     tests, rendering, reviewer checks, and everything under
     **Not available**;
   - next step: review or edit the plan (`backlog.md`, the `task.md` files,
     `stack.md`), then run `/factory continue` to start building.
7. **Otherwise the preflight stopped.** Build `board.json` (the state is
   `awaiting-preflight`). Halt with a summary of `preflight.md` (its result,
   issues and tools) and the next steps:
   - For each spec issue: fix the spec at the path in
     `factory/state/spec-source`. If the Planner is wrong about an issue, add
     a sentence to the spec that settles it. If the spec is in the repo,
     commit the change.
   - For each tool on offer: its install command, which the user runs
     themselves (in Claude Code, for example, with `! <command>`). The
     factory never installs anything. A tool the user doesn't install is
     declined, and the next preflight doesn't offer it again.
   - Then run `/factory continue`.

## Running tasks

### Run the loop

1. Find the next available task: the first task in `backlog.md` with status
   `Planned`.
2. If there is none: set `loop.state` to `complete`, the message to
   `Construction complete`, and `current` to `none`. Tell the user that
   construction is complete, and stop.
3. Otherwise start it: you must be on `main`. Mark it `In progress` in
   `backlog.md`, then **run the task**.

### Run the task

1. Set `loop.state` to `running` (which clears any earlier `halted`), the
   message to `Running <task-id>`, and `current` to the task, refreshing the
   task's entry. Don't touch `board.json` again until step 4.
2. Spawn the `task-orchestrator` agent with exactly this prompt:
   `Task-id: <task-id>.`
3. End your turn with `WAITING`. You are woken by a hand-back message from
   that agent, or by a completion notification carrying that agent's id.
   Never poll it, send it messages, read its output file or start other work.
4. **Each time either one wakes you**, decide from `backlog.md`,
   `board.json`, the current branch and `git status`, not from what the
   message says (a hand-back can arrive while the task orchestrator's own
   agents are still running, and a notification may never follow one).
   Take the first case that applies:
   - **`loop.state` is `halted`:** report the reason from `loop.message`,
     what to fix (for a merge conflict: resolve the conflicts and commit the
     merge), and that `/factory continue` resumes. Stop.
   - **The task is `Escalated`:** show the escalation summary (below) and
     stop.
   - **The task is `In progress` and merged, you are on `main`, and the tree
     is clean:** mark it `Complete`, delete its branch with
     `git branch -d factory/<task-id>`, refresh its board entry (`current`
     `none`), and **run the loop** again for the next task.
   - **Anything else, woken by a hand-back:** the task orchestrator is still
     working. Take no action and end your turn with `WAITING`.
   - **Anything else, woken by the notification:** the task orchestrator
     stopped early. Revert nothing. Set `loop.state` to `halted` with the
     message `Task orchestrator stopped early on <task-id>`, and tell the user
     that `/factory continue` resumes the task from its files. Stop.
5. Once you have acted on a task orchestrator (any case above except the
   `WAITING` one), ignore every later message from it.

### Escalation summary

Read the latest round's `verdict.md`, or its `build-notes.md` if the builder
reported `STATUS: BLOCKED`. Show the user:
- the task-id, title and round, and whether the Approver escalated or the
  builder blocked;
- the verdict's summary and remaining findings (or the blocked reason), and
  the options the Approver laid out with the rework note it suggests for
  each;
- the commands they can run next, from `main` or the task branch:
  - `/factory approve <task-id>`: merge the branch as it is;
  - `/factory rework <task-id> <note>`: send it back to the builder with a
    note (text, or the path of a file outside the repo);
  - `/factory abandon <task-id>`: give up on it and keep the branch.
