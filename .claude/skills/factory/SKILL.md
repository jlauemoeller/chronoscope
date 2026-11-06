---
name: factory
description: "Runs the software factory: plans a spec into tasks, then builds, reviews and merges them one by one. Commands: start <spec>, continue, approve <task-id>, rework <task-id> <note>, abandon <task-id>, status."
disable-model-invocation: true
argument-hint: "start <spec> | continue | approve <task-id> | rework <task-id> <note> | abandon <task-id> | status"
---

# /factory

Arguments: $ARGUMENTS

The first word of the arguments is the command. The rest are its
arguments: `start <spec>`, `continue`, `approve <task-id>`,
`rework <task-id> <note>` (everything after the task-id is the note),
`abandon <task-id>` or `status`. With no command or an unknown one, list
the commands and stop.

**First, read `.claude/skills/factory/conventions.md`** in full. It defines
the paths, the ground rules, the factory script (`factory.py` below is short
for `python3 -I .claude/skills/factory/factory.py`, always run from the
repository root), the fixed agent prompts, write verification, and the
Merge and Halting steps. Follow them exactly.

This session is now the **main orchestrator**. It handles the commands and
their preconditions, the Planner, reconciliation, choosing the next task,
and the backlog bookkeeping around a task. It spawns only the Planner and
task orchestrators, never two task orchestrators at once, and never does an
agent's work itself. It reports every halt to the human.

## Preconditions

Every command except `status` checks these, in this order. When one fails,
the command refuses: say why and what to do, and do nothing else (no board
change, no git change).

1. `factory/state/` is ignored by git (`git check-ignore -q factory/state`,
   a plain git check).
2. `continue` only: if a task in `backlog.md` is `Escalated`, refuse and
   point the human to `/factory approve <task-id>`,
   `/factory rework <task-id> <note>` and `/factory abandon <task-id>`.
   This check comes before the branch and tree checks, because an
   escalation leaves the session on the task branch.
3. The branch (`git branch --show-current`):
   - `start`: `main`.
   - `continue`: `main`, or `factory/<task-id>` of the task that is
     `In progress`.
   - `approve`, `rework`, `abandon`: `main` or `factory/<task-id>` of the
     task named in the arguments.
4. A clean tree (`git status --porcelain --untracked-files=all` prints
   nothing). One exception, for `continue` on the branch of the
   `In progress` task: look at its latest round (`factory.py round list
   <task-id>`, the `latest-reviews-started:` line):
   - `no`, no `reviews-started` marker: the changes are the Builder's own.
     Allow them, and leave them for the resumed builder.
   - `yes`, the marker exists: the changes came from a reviewer or the
     Approver.
     Don't refuse silently: use the Halting step with the reason
     `Uncommitted changes on factory/<task-id> after the reviews started:
     inspect them and commit or remove them, then /factory continue`, so a
     builder never commits changes no reviewer saw.

## /factory start <spec>

1. Check the preconditions.
2. Refuse if `factory/state/backlog.md`, `factory/state/tasks/` or
   `factory/state/preflight.md` exists. Tell the human to archive the
   previous run (move `factory/state/` to a folder under
   `factory/archive/`) or remove it first. The factory never deletes a
   previous run.
3. `factory.py spec copy <spec>`. It writes the spec's absolute path (with
   `~` expanded) to `spec-source`, copies the spec to `SPEC.md` and every
   Markdown file it links to into `spec/`, and lists any missing linked
   document. If it fails (for example, the spec doesn't exist), refuse with
   its error.
4. Run the Planner (below).

## Running the Planner

Used by `start` and by `continue` in the preflight state.

1. `factory.py board build --state planning --message "Planning: preflight and plan"`.
2. Write the Planner's snapshot to `factory/state/.snapshot` (see "Write
   verification" in conventions.md).
3. Spawn the `planner` agent with the prompt
   `Spec: factory/state/SPEC.md.` and end the turn with `WAITING`.
4. When it has finished, verify its writes: the branch is still `main`, the
   tree is clean, and the state comparison shows only the Planner's allowed
   files. On a violation, use the Halting step.
5. `factory.py first-lines`. If it says `INVALID`, use the Halting step
   with the files it lists.
6. Then:
   - If `factory/state/backlog.md` exists: the plan-review halt (below).
   - Else, if `factory/state/preflight.md` exists: the preflight halt
     (below).
   - Else: use the Halting step with the reason
     `The Planner wrote neither preflight.md nor backlog.md`.

## The plan-review halt

1. `factory.py plan check`.
2. If it says `PLAN: OK`:
   `factory.py board build --state awaiting-plan-review --message "Review the plan, then /factory continue"`.
   If it says `PLAN: PROBLEMS`:
   `factory.py board build --state awaiting-plan-review --message "The plan check failed: fix the plan, then /factory continue"`.
3. Halt with a summary for the human:
   - any problems from the plan check, each one, and that the next
     `/factory continue` checks the plan again;
   - any warnings from the plan check;
   - the backlog: for each task in order, its id and title (the first line
     of its `task.md`), its reviewers (`factory.py task reviewers <task-id>`)
     and what it depends on;
   - the preflight assumptions (the **Assumptions** section of
     `preflight.md`);
   - the stack profile from `stack.md`: the stack, the test gate, and
     everything under **Not available**;
   - that the human may edit `backlog.md`, the `task.md` files and
     `stack.md`, and that removing a task from `backlog.md` also means
     deleting its task directory;
   - that building starts with `/factory continue`;
   - how to open the dashboard (see conventions.md).

## The preflight halt

1. `factory.py board build` (with `preflight.md` and no `backlog.md`, it
   derives `loop.state` `awaiting-preflight`).
2. Halt with a summary of `preflight.md`: its result line, its
   **Spec issues**, **Environment issues** and **Tools on offer**, and the
   stack profile from `stack.md`, including everything under
   **Not available**. Then the next steps:
   - For each spec issue: fix the spec at the path in
     `factory/state/spec-source`. If the Planner is wrong about an issue,
     add a sentence to the spec that settles it. If the spec is in the
     repository, commit the change.
   - For each environment issue: what would resolve it, from
     `preflight.md`.
   - For each tool on offer: its install command, which the human runs
     themselves (in Claude Code, for example, as `! <command>`). The
     factory never installs anything. A tool the human doesn't install is
     declined, and the next preflight doesn't offer it again.
   - Then run `/factory continue`.
   - How to open the dashboard (see conventions.md).

## /factory continue

Check the preconditions. If neither `backlog.md` nor `preflight.md` exists,
refuse: there is no run to continue; use `/factory start <spec>`. Then
reconcile the state, taking the cases in this order:

1. **The preflight state** (`preflight.md` exists, `backlog.md` doesn't):
   `factory.py spec recopy`, which re-copies the spec from the path in
   `spec-source` and replaces `SPEC.md` and `spec/`. Then run the Planner
   again and act on its result as `start` does. Nothing else in this list
   applies.
2. **No board:** if `factory/state/board.json` is missing,
   `factory.py board build`.
3. **The plan hasn't been shown:** if every task in `backlog.md` is
   `Planned`, no task has a round directory, and `loop.state`
   (`factory.py board show`) isn't `awaiting-plan-review`, the human hasn't
   seen the plan yet (for example, the Planner step halted after
   `backlog.md` was written). Do the plan-review halt. Nothing else in this
   list applies.
4. **Leaving the plan review:** if `loop.state` is `awaiting-plan-review`,
   the human may have edited the plan. `factory.py plan check`. On
   `PLAN: PROBLEMS`, halt and say what is wrong, leaving `loop.state` at
   `awaiting-plan-review` (`factory.py board build --state awaiting-plan-review --message "The plan check failed: fix the plan, then /factory continue"`),
   so the next `/factory continue` checks again. On `PLAN: OK`,
   `factory.py board build` to rebuild the board from `backlog.md` and the
   task files, and carry on.
5. **A task is `In progress` and merged** (`factory.py task merged
   <task-id>` prints `yes`; the human resolved a merge conflict and
   completed the merge): if not on `main`, `git switch main`. Then complete
   the task (below) and carry on.
6. **A task is `In progress` and not merged** (an interrupted run): if its
   branch `factory/<task-id>` exists and you're not on it,
   `git switch factory/<task-id>`. If it doesn't exist yet (round 1 never
   got as far as the builder creating it), stay on `main`. Then run the task
   orchestrator for it (below). It works out the right step of the latest
   round from the task's files, including starting the Approver rework
   round that a REWORK verdict asks for. Any uncommitted changes allowed by
   the preconditions stay in place for the resumed builder.

Otherwise, start the next task (below).

## /factory approve <task-id>

A human uses this after reviewing an escalated task. The task branch
already holds the code and documentation.

1. Check the preconditions. Refuse if the task isn't `Escalated`.
2. The Merge step (conventions.md). If it halts with a conflict, first
   `factory.py backlog set <task-id> "In progress"`, then halt and tell the
   human how to finish (conventions.md, Merge step 4).
3. If the merge succeeded, complete the task (below) and start the next
   task, in this session.

## /factory rework <task-id> <note>

Sends an escalated task back to the builder with the human's feedback. The
note is everything after the task-id: either text, or the path of a file
outside the repository.

1. Check the preconditions. Refuse if the task isn't `Escalated`. If the
   note is the path of an existing file (expand `~`) and the file is inside
   the repository (compare `realpath` of the file with
   `git rev-parse --show-toplevel`), refuse: a file inside the repository
   makes the tree unclean; move it outside first.
2. Work in this order, so that an interrupted `rework` can simply be run
   again:
   1. Check out `factory/<task-id>`: `git switch factory/<task-id>`. If the
      branch doesn't exist
      (`git rev-parse --verify --quiet refs/heads/factory/<task-id>`, a
      plain git check: exit 1 means it doesn't exist), create it from main
      with `git switch -c factory/<task-id> main`.
   2. `factory.py round list <task-id>`. If the latest round has no build
      notes (`latest-build-notes: no`, an earlier `rework` was
      interrupted), reuse that round directory. Otherwise create the
      directory of the next round-id. Call it `R`.
   3. Write the note to `R/human-review.md`, replacing any earlier one:
      text with the Write tool, a file with `cp <file> R/human-review.md`.
      Only then write the base commit:
      `git rev-parse HEAD > R/base-commit`.
   4. Only then `factory.py backlog set <task-id> "In progress"`.
3. Run the task orchestrator for the task (below). It spawns a fresh
   builder for the round, which revises the work according to the note.
   After the reviewers, the Approver runs straight away; a rework round
   never triggers an automatic fix round. Human rework rounds count against
   neither the automatic round limit nor the Approver's REWORK limit, and
   any number of them are allowed.

## /factory abandon <task-id>

1. Check the preconditions. Refuse if the task isn't `Escalated`.
2. If not on `main`, `git switch main`.
3. `factory.py backlog set <task-id> Abandoned`. Keep the unmerged branch
   `factory/<task-id>` so no work is lost; tell the human they can delete
   it by hand.
4. `factory.py task dependents <task-id>`. If any later task depends on
   the abandoned one, warn the human and name those tasks.
5. Start the next task (below), in this session.

## /factory status

Read-only and allowed on any branch, with no preconditions. Run
`factory.py summary` and show its output: the loop state, the branch and
tree, the backlog, the current task with its stage and latest round, and
the action `/factory continue` would take next. In the preflight state it
shows the preflight result, its open issues and the tools on offer instead
of the backlog. Run nothing else that writes: no board rebuild, even when
`board.json` is missing, and no git command that changes anything.

## Starting the next task

1. `factory.py backlog next`.
2. If it prints `none`:
   `factory.py board loop complete --message "Construction is complete" --no-current`,
   and tell the human that construction is complete (the run can be
   archived by moving `factory/state/` under `factory/archive/`).
3. Otherwise, on `main`, `factory.py backlog set <task-id> "In progress"`,
   and run the task orchestrator for it.

## Running the task orchestrator

1. `factory.py board loop running --message "Building <task-id>" --current <task-id>`.
   This clears any earlier `halted`. Don't touch the board again until
   step 4.
2. Spawn the `task-orchestrator` agent with the prompt exactly
   `Task-id: <task-id>.` and note its agent id.
3. End the turn with `WAITING`. You are woken by a hand-back message from
   that agent, or by a `<task-notification>` that carries that agent's id.
   Never poll the task orchestrator, send it messages, read its output
   file, or start other work.
4. **Each time either one wakes you**, decide from `backlog.md`,
   `factory.py board show <task-id>`, the current branch and
   `git status --porcelain --untracked-files=all`, not from what the
   message says. A hand-back can arrive while the task orchestrator's own
   agents are still running, and a notification may never follow one, so
   act on neither alone. Take the first case that applies:
   - `loop.state` is `halted`: report the reason from `loop.message`, and
     that `/factory continue` resumes once it is dealt with.
   - The task is `Escalated`: show the escalation summary (below).
   - The task is `In progress` and merged (`factory.py task merged
     <task-id>`), you are on `main`, and the tree is clean: complete the
     task (below) and start the next task.
   - Anything else, woken by a hand-back: the task orchestrator is still
     working. Take no action and end the turn with `WAITING`.
   - Anything else, woken by the notification: the task orchestrator
     stopped early. Revert nothing.
     `factory.py board loop halted --message "Task orchestrator stopped early on <task-id>"`,
     and tell the human that `/factory continue` resumes the task from its
     files.
5. Once you have acted on a task orchestrator, ignore any later message
   from it.

## Completing a task

1. `factory.py backlog set <task-id> Complete`.
2. `git branch -d factory/<task-id>`. If git refuses, use the Halting step
   with its message; never force the deletion.

## The escalation summary

`factory.py task next-step <task-id>` gives the reason (`reason:`) and the
round. Read that round's `verdict.md`, or its `build-notes.md` if the
Builder blocked, and show the human:

- the reason: the Approver escalated (`approver`), the Builder blocked
  (`blocked`), the Approver's REWORK limit was reached (`rework-limit`), or
  a REWORK verdict had no rework note (`no-rework-note`);
- a summary of the verdict or of the build notes: the open findings, the
  options the Approver laid out with the rework note it suggests for each,
  and whether it says approving the branch as it is would be reasonable;
- when the Builder blocked, a warning that the branch holds unfinished work
  that didn't pass the test gate;
- the commands: `/factory approve <task-id>` (merge the branch as it is),
  `/factory rework <task-id> <note>` (another round with the human's note),
  `/factory abandon <task-id>` (give up and keep the branch);
- the paths of the files to read, and how to open the dashboard.
