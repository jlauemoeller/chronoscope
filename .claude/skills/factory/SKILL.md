---
name: factory
description: "Run the software factory: plan a spec into tasks, then build, review and approve each task. Commands: start <spec>, continue, approve <task-id>, rework <task-id> <note>, abandon <task-id>, status."
disable-model-invocation: true
argument-hint: "start <spec> | continue | approve <task-id> | rework <task-id> <note> | abandon <task-id> | status"
---

# /factory

The command and its arguments: `$ARGUMENTS`

First of all, read `.claude/skills/factory/conventions.md` in full. It
defines `S`, `T`, `R`, `FP` (the factory script), the ground rules, the
agent prompts, write verification, rounds, the board, and the shared Merge,
Complete a task and Halting steps. This file uses them without repeating
them. Shell snippets that use the dollar sign are in `conventions.md`, not
here.

The session running this skill is the **main orchestrator**. It handles the
commands and their preconditions, the Planner, reconciliation, choosing the
next task, the backlog bookkeeping around a task, and reporting to the human.
It never does an agent's work itself. It spawns only the `planner` and
`task-orchestrator` agents, and never two task orchestrators at once. All
mechanics go through `FP`; the main orchestrator keeps the judgement: what to
do next and what to tell the human. If an `FP` command exits non-zero, halt
with its stderr as the reason.

## Preconditions

Parse the first word of the arguments as the command. With no or an unknown
command, print the command list from the `argument-hint` above and stop.

Every command except `status` needs, checked with `FP preconditions`:
1. `state-ignored: yes`. Otherwise refuse: `factory/state/` must be ignored by
   git (the `/factory/state` entry in `.gitignore`).
2. The right branch:
   - `start`: `main`.
   - `continue`: `main`, or `factory/<task-id>` of the `In progress` task.
   - `approve`, `rework`, `abandon`: `main` or `factory/<task-id>` of the
     task named.
3. `tree: clean` (no uncommitted changes and no untracked files that git
   doesn't ignore). `continue` has one exception, described below.

`continue` checks for an `Escalated` task (`FP run-info`, `active:` line)
before it checks the branch or the tree: an escalation leaves the session on
the task branch, so a branch refusal would hide the real reason. If one is
`Escalated`, refuse and point the user to `/factory approve <task-id>`,
`/factory rework <task-id> <note>` and `/factory abandon <task-id>`.

If a precondition fails, refuse with a message saying which one and what to
do, and change nothing.

## /factory start <spec>

1. Check the preconditions.
2. Refuse if `S/backlog.md`, `S/tasks/` or `S/preflight.md` exists: tell the
   user to archive `factory/state/` (move it to a folder under
   `factory/archive/`) or remove it first. The factory never deletes a
   previous run.
3. `FP spec-copy <spec>`. It writes the absolute path of the spec (with `~`
   expanded) to `S/spec-source` and copies the spec, the project documents and
   the linked documents. Note any `missing:` lines for the summary.
4. `FP board-rebuild --loop-state planning --message "Planning"`.
5. Run the Planner (see "The Planner step").

Tell the user how to open the dashboard in the first halt of a run: run
`python3 -m http.server -d factory` from the repository root and open
`http://localhost:8000/dashboard/`.

## The Planner step

1. `FP snapshot --planner`.
2. Spawn the Planner (`subagent_type: planner`) with exactly the prompt
   `Spec: factory/state/SPEC.md.` and wait for it to finish (it may run in
   the foreground).
3. Write verification (see `conventions.md`): `FP preconditions` must show a
   clean tree on `main`; then
   `FP check-writes --planner --allow preflight.md --allow stack.md --allow backlog.md --allow 'tasks/*/task.md'`.
   On a violation, halt as `conventions.md` says.
4. `FP first-lines`. If `preflight.md` is missing or invalid, halt.
5. If `S/backlog.md` exists, go to "The plan review halt".
6. Otherwise the preflight stopped. Run "The preflight halt".

### The preflight halt

1. `FP board-rebuild --loop-state awaiting-preflight --message "Preflight: <READY|TOOLS|BLOCKED>; fix the spec or install tools, then /factory continue"`.
2. Read `S/preflight.md` and `S/stack.md` and halt with a summary:
   - the result line;
   - each **Spec issue**: where it is, what is wrong, what would resolve it.
     Tell the user to fix the spec at the path in `S/spec-source`; if the
     Planner is wrong about an issue, to add a sentence to the spec that
     settles it; and if the spec is in the repository, to commit the change;
   - each **Environment issue**, with the failing command and its fix;
   - each **Tool on offer**: what it checks, which reviewer or gate section
     it serves, and its install command, which the user runs themselves (in
     Claude Code, for example, with `! <command>`). The orchestrator never
     installs anything. A tool the user doesn't install counts as declined,
     and the next preflight doesn't offer it again;
   - the **Assumptions** and the **Stack profile changes**, briefly, and
     anything under **Not available** in `stack.md`;
   - any `missing:` linked documents from the spec copy;
   - then: run `/factory continue`.

### The plan review halt

1. Check the plan (see "Checking the plan").
2. `FP board-rebuild --loop-state awaiting-plan-review --message "<Plan ready for review | Plan problems: …>; /factory continue starts building"`.
3. Halt with:
   - the backlog: each task-id, its title (first line of `task.md`), its
     reviewers (`FP reviewers`) and its dependencies;
   - the preflight assumptions, including the specialist baselines;
   - the stack profile in brief: the test gate commands, and everything under
     **Not available**;
   - any plan problems, each saying what is wrong;
   - that the human may edit the plan (`backlog.md`, the `task.md` files and
     `stack.md`), that removing a task from `backlog.md` also means deleting
     its task directory, and that `/factory continue` checks the plan again
     and starts building;
   - on the first halt of a run, how to open the dashboard.

## Checking the plan

The plan is sound when every task in `backlog.md` has a `task.md` with all
its sections and every task directory appears in `backlog.md` exactly once;
every status is `Planned`; every `Reviews` section lists `code` and
`feature`; every dependency names an earlier task in this backlog or a task
from an earlier run that is already merged; and every task-id follows the
identifier rules, with serials continuing above those used by earlier runs.

Run `FP plan-check`. `plan: sound` means the mechanical checks pass;
`warning:` lines are worth showing but don't make the plan unsound. On
`plan: problems`, the plan is not sound: report each `problem:` line, and
leave `loop.state` at `awaiting-plan-review` so the next `/factory continue`
checks again. Never fix the plan yourself.

## /factory continue

Check the `Escalated` task first, then the other preconditions. The branch
may be `main` or the `In progress` task's branch.

The tree exception: on the `In progress` task's branch, uncommitted changes
are allowed while the latest round has no `reviews-started` marker
(`FP round-info <task-id>` prints `reviews-started: no`). They are then the
Builder's own, and are left for the resumed builder. Once the marker exists,
the changes came from a reviewer or the Approver: halt
(`FP board-update --loop-state halted --message "Uncommitted changes on <branch> after the reviews started"`)
and ask the human to inspect them and commit or remove them, because
otherwise a builder would commit changes no reviewer saw. Everywhere else a
dirty tree is refused.

Then reconcile, using `FP run-info`, taking the first case that applies:

1. **The preflight state** (`preflight-state: yes`): re-copy the spec from
   `spec-source` with the command in `conventions.md` ("The preflight
   state"), run `FP board-rebuild --loop-state planning --message "Planning"`,
   and run "The Planner step" again. Nothing else in this list applies.
2. If `board: absent`, run `FP board-rebuild`.
3. **The plan hasn't been shown** (`all-planned-no-rounds: yes` and
   `loop-state` isn't `awaiting-plan-review`; for example the Planner step
   halted after `backlog.md` was written): run "The plan review halt".
   Nothing else in this list applies.
4. **Leaving the plan review** (`loop-state: awaiting-plan-review`): the human
   may have edited the plan. Run `FP plan-check`. On problems, run
   `FP board-rebuild --loop-state awaiting-plan-review --message "Plan problems: …"`
   and halt with the problems. Otherwise run `FP board-rebuild`, and go on.
5. **A task is `In progress`** (`active: <task-id> In progress`):
   - If `FP merged <task-id>` says `yes` (the user resolved a merge
     conflict): switch to `main` if you aren't on it, run "Complete a task"
     (`conventions.md`), and carry on with "Running tasks".
   - Otherwise it is an interrupted run. If `factory/<task-id>` exists,
     check it out (or stay on it); if it doesn't, stay on `main` (round 1's
     builder creates it). Then run the task ("Running a task", step 1
     onwards). The task orchestrator works out the step from the task's
     files, including an Approver rework round that a REWORK verdict asks
     for.
6. Otherwise, carry on with "Running tasks".

## Running tasks

On `main`, take `FP next-task`.
- If it prints `none`, run
  `FP board-update --loop-state complete --message "Construction is complete" --current none`
  and halt with a message that construction is complete (mention any
  `Abandoned` tasks and their branches).
- Otherwise start it: `FP set-status <task-id> "In progress"`, then run it
  ("Running a task").

### Running a task

1. `FP board-update --loop-state running --message "Working on <task-id>" --current <task-id>`.
   This clears any earlier `halted`. Don't touch the board again until
   step 4.
2. Spawn the task orchestrator: `subagent_type: task-orchestrator`, prompt
   exactly `Task-id: <task-id>.`, with `run_in_background: true`. Note its
   agent id.
3. End the turn with `WAITING`. You are woken by a hand-back message from
   that agent or by a `<task-notification>` that carries its id. Never poll
   it, send it messages, read its output file or start other work.
4. **Each time either one wakes you**, decide from `FP backlog`,
   `FP board-show`, the current branch and `FP preconditions`, not from what
   the message says. A hand-back can arrive while the task orchestrator's own
   agents are still running, and a notification may never follow one. Take
   the first case that applies:
   - `loop-state: halted`: report the reason from `loop-message` (and, for
     a write violation or merge conflict, what the user must inspect or
     resolve). `/factory continue` resumes.
   - The task is `Escalated`: run "The escalation summary".
   - The task is `In progress`, `FP merged <task-id>` says `yes`, you are on
     `main` and the tree is clean: run "Complete a task" and carry on with
     "Running tasks", in this same session.
   - Anything else, woken by a hand-back: the task orchestrator is still
     working. Take no action and end the turn with `WAITING`.
   - Anything else, woken by the notification: the task orchestrator stopped
     early. Revert nothing, run
     `FP board-update --loop-state halted --message "Task orchestrator stopped early on <task-id>"`,
     and tell the user that `/factory continue` resumes the task from its
     files.
5. Once you have acted on a task orchestrator, ignore any later message from
   it.

### The escalation summary

Read the latest round's `verdict.md`, or its `build-notes.md` if the builder
blocked (`FP round-info <task-id>`), and show the user:
- the reason: the Approver escalated, the Builder blocked, the REWORK limit
  was reached, or a REWORK verdict had no rework note. When the Builder
  blocked, warn that the branch `factory/<task-id>` holds unfinished work
  that didn't pass the gate;
- a summary of the remaining issues, and the Approver's options with their
  suggested rework notes, and whether it says approving as is would be
  reasonable;
- the commands: `/factory approve <task-id>`, `/factory rework <task-id> <note>`
  (text, or the path of a file outside the repository), and
  `/factory abandon <task-id>`.

The session stays on the task branch.

## /factory approve <task-id>

Merge an `Escalated` task after human review.
1. Check the preconditions. Refuse if the task isn't `Escalated`.
2. Run the Merge step (`conventions.md`). On a conflict it marks the task
   `In progress` and halts: tell the user to resolve the conflicts, complete
   the merge, and run `/factory continue`, which finds the task merged and
   marks it `Complete`.
3. If the merge succeeds, run "Complete a task" and carry on with "Running
   tasks" in this session.

## /factory rework <task-id> <note>

Send an `Escalated` task back to the builder with the human's feedback. The
note is everything after the task-id: either text, or the path of an
existing file outside the repository (a file inside it would make the tree
unclean; refuse such a path).
1. Check the preconditions. Refuse if the task isn't `Escalated`.
2. Check out `factory/<task-id>`, creating it from main
   (`git switch -c factory/<task-id> main`) if it doesn't exist.
3. Work out the round with `FP round-info <task-id>`: if the latest round has
   no build notes (`build: pending`; an earlier `rework` was interrupted),
   reuse it. Otherwise create the `next-round` directory.
4. Write the note to `R/human-review.md`, replacing any earlier one: copy the
   file, or write the text with a heading line `# Human review` followed by
   the note as given.
5. `FP write-base-commit <task-id> <round-id>`.
6. Only then `FP set-status <task-id> "In progress"`.
7. Run the task ("Running a task"). The task orchestrator spawns a fresh
   builder, then the reviewers, then the Approver straight away: a rework
   round never triggers an automatic fix round. Human rework rounds count
   against neither the automatic round limit nor the Approver's REWORK limit.

Because of this order, an interrupted `rework` can simply be run again.

## /factory abandon <task-id>

Give up on an `Escalated` task.
1. Check the preconditions. Refuse if the task isn't `Escalated`.
2. `git switch main`.
3. `FP set-status <task-id> Abandoned`, then
   `FP board-update --loop-state running --message "Abandoned <task-id>" --current none`.
4. Keep the unmerged branch `factory/<task-id>` so no work is lost; tell the
   user they can delete it by hand.
5. `FP dependents <task-id>`: warn the user about every later task that
   depends on the abandoned one.
6. Carry on with "Running tasks" in this session.

## /factory status

Read-only, allowed on any branch, and changes nothing (not even the board).
Use `FP run-info`, `FP backlog`, `FP board-show` and, for the current task,
`FP round-info <task-id>`. Print:
- in the preflight state: the preflight result, its open spec and
  environment issues, and the tools on offer, instead of the backlog;
- otherwise the backlog (task-id and status), the current task with its
  stage and latest round (kind, build, reviews, verdict);
- the loop state and its message;
- the action `/factory continue` would take next, worked out from the cases
  under "/factory continue" (for example "refuses: 004-x is Escalated",
  "resumes 003-y at the reviews of round-2", "starts 005-z", "shows the plan
  for review").
